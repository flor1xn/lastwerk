//! Beste mittlere Leistung über feste Dauern (SPEC 6.2, `activity_best_power`).
//!
//! Gesucht ist je Dauer das Zeitfenster mit der höchsten mittleren Leistung. Das Ergebnis
//! dient der FTP-Schätzung aus der besten 20-Minuten-Leistung (SPEC 8.6) und der
//! Leistungskurve.
//!
//! Konventionen (SPEC 6.5):
//!
//! - Ein Fenster liegt vollständig in einem zusammenhängenden Abschnitt. Es läuft nie über
//!   eine Pause (Lücke über 10 s) hinweg.
//! - Innerhalb eines Abschnitts zählt eine Sekunde ohne Leistungswert (kurze Lücke, fehlender
//!   oder verworfener Wert) als 0 W. Das senkt den Mittelwert und überschätzt nie. Für die
//!   normalisierte Leistung gilt die andere Regel (Werte überspringen, `load`), weil dort
//!   30-Werte-Fenster gebildet werden und ein Überspringen die Fenster nur dehnt. Hier würde
//!   Überspringen ein Fenster länger als die Dauer machen und den Bestwert aufblähen.
//! - Ist kein einziger Leistungswert vorhanden, gibt es keine Bestwerte (kein Kanal), nie Nullen.
//! - Ist kein Abschnitt so lang wie die Dauer, fehlt der Wert dieser Dauer (`None` im Sinn
//!   von "nicht in der Liste"), es wird nicht gekürzt.

use serde::{Deserialize, Serialize};

use crate::load::{ActivityInput, LoadConfig, LoadError, PreparedSamples, check_plausibility};

/// Eckdauern in Sekunden (SPEC 6.2): 5 s, 1 min, 5 min, 20 min, 1 h. Sie tragen Dashboard,
/// Detailansicht, FTP-Vorschlag (8.6) und MCP-Server.
pub const BEST_POWER_DURATIONS_S: [u32; 5] = [5, 60, 300, 1200, 3600];

/// Dauern der gespeicherten Bestwerte in Sekunden seit T33: ein Raster von 1 s bis 2 h für die
/// Leistungskurve. Das Raster ist Konvention (dichter im kurzen Bereich, wo sich die Kurve
/// stark krümmt, wie in üblichen Leistungsprofilen), keine Quelle nötig. Es enthält alle
/// [`BEST_POWER_DURATIONS_S`], damit deren Werte unverändert bleiben (Test unten).
pub const POWER_CURVE_DURATIONS_S: [u32; 22] = [
    1, 5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240, 300, 420, 600, 900, 1200, 1800, 2700, 3600,
    5400, 7200,
];

/// Beste mittlere Leistung über `duration_s`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BestPower {
    pub duration_s: u32,
    pub watts: f32,
}

/// Beste mittlere Leistung der Aktivität für jede Dauer in `durations_s`, in der Reihenfolge
/// von `durations_s`. Dauern, für die kein Abschnitt lang genug ist, fehlen in der Antwort.
/// Eine Aktivität ohne Leistungswerte liefert eine leere Liste.
pub fn best_power(
    a: &ActivityInput,
    cfg: &LoadConfig,
    durations_s: &[u32],
) -> Result<Vec<BestPower>, LoadError> {
    let prepared = check_plausibility(&a.samples, cfg)?;
    Ok(best_power_prepared(&prepared, durations_s))
}

/// Wie [`best_power`], aber mit den schon geprüften Samples. `prepared` muss das Ergebnis
/// von [`check_plausibility`] für die Samples der Aktivität sein. Grund wie bei
/// [`crate::load::compute_load_prepared`]: Der Import prüft die Samples einmal statt je
/// Rechnung (T38, Anhang Messung).
pub fn best_power_prepared(prepared: &PreparedSamples, durations_s: &[u32]) -> Vec<BestPower> {
    let mut any_power = false;
    // Je Abschnitt eine dichte 1-Hz-Reihe mit 0 W für fehlende Sekunden, als Summe über
    // die Reihe (Präfixsummen), damit jedes Fenster in O(1) berechenbar ist.
    let mut prefix_sums: Vec<Vec<f64>> = Vec::new();
    for seg in &prepared.segments {
        let (Some(first), Some(last)) = (seg.first(), seg.last()) else {
            continue;
        };
        let len = (last.t_s - first.t_s + 1) as usize;
        let mut dense = vec![0.0_f64; len];
        for s in seg {
            if let Some(p) = s.power_w {
                dense[(s.t_s - first.t_s) as usize] = f64::from(p);
                any_power = true;
            }
        }
        let mut sums = Vec::with_capacity(len + 1);
        let mut acc = 0.0_f64;
        sums.push(acc);
        for v in dense {
            acc += v;
            sums.push(acc);
        }
        prefix_sums.push(sums);
    }
    if !any_power {
        return Vec::new();
    }

    let mut out = Vec::new();
    for &d in durations_s.iter().filter(|&&d| d > 0) {
        let d_len = d as usize;
        let best = prefix_sums
            .iter()
            .filter(|sums| sums.len() > d_len)
            .flat_map(|sums| (0..sums.len() - d_len).map(move |i| sums[i + d_len] - sums[i]))
            .reduce(f64::max);
        if let Some(sum) = best {
            out.push(BestPower {
                duration_s: d,
                watts: (sum / f64::from(d)) as f32,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curve_grid_is_ascending_and_contains_the_corner_durations() {
        assert!(POWER_CURVE_DURATIONS_S.windows(2).all(|w| w[0] < w[1]));
        for d in BEST_POWER_DURATIONS_S {
            assert!(POWER_CURVE_DURATIONS_S.contains(&d), "{d}");
        }
    }
    use crate::load::{Sample, Sport};
    use chrono::{TimeZone, Utc};

    const EPS: f32 = 1e-3;

    fn activity(samples: Vec<Sample>) -> ActivityInput {
        ActivityInput {
            sport: Sport::Cycling,
            start_utc: Utc.with_ymd_and_hms(2026, 3, 1, 6, 30, 0).unwrap(),
            local_offset_s: 0,
            samples,
            rpe: None,
            pool_length_m: None,
        }
    }

    fn power(t: u32, w: f32) -> Sample {
        Sample {
            power_w: Some(w),
            ..Sample::at(t)
        }
    }

    fn best(a: &ActivityInput) -> Vec<BestPower> {
        best_power(a, &LoadConfig::default(), &BEST_POWER_DURATIONS_S).unwrap()
    }

    fn watts(list: &[BestPower], duration_s: u32) -> Option<f32> {
        list.iter()
            .find(|b| b.duration_s == duration_s)
            .map(|b| b.watts)
    }

    /// Konstant 200 W über 3600 s: Jedes Fenster hat den Mittelwert 200, auch das einzige
    /// Fenster über 3600 s. Alle fünf Bestwerte sind 200 W.
    #[test]
    fn constant_200_w_gives_200_for_every_duration() {
        let a = activity((0..3600).map(|t| power(t, 200.0)).collect());
        let list = best(&a);
        assert_eq!(list.len(), 5);
        for b in &list {
            assert!((b.watts - 200.0).abs() < EPS, "{b:?}");
        }
    }

    /// Wechsel 30 s mit 300 W und 30 s mit 100 W, 3600 s. Bester 5-s-Wert: ein Fenster ganz in
    /// der 300-W-Phase, 300 W. Jedes 60-s-Fenster enthält je 30 s beider Phasen:
    /// `(30 * 300 + 30 * 100) / 60 = 200`. Gleiches gilt für Vielfache von 60 s (300 s,
    /// 1200 s, 3600 s).
    #[test]
    fn interval_30_30_gives_300_for_5_s_and_200_for_60_s() {
        let samples = (0..3600)
            .map(|t| power(t, if (t / 30) % 2 == 0 { 300.0 } else { 100.0 }))
            .collect();
        let list = best(&activity(samples));
        assert!((watts(&list, 5).unwrap() - 300.0).abs() < EPS);
        assert!((watts(&list, 60).unwrap() - 200.0).abs() < EPS);
        assert!((watts(&list, 300).unwrap() - 200.0).abs() < EPS);
        assert!((watts(&list, 1200).unwrap() - 200.0).abs() < EPS);
        assert!((watts(&list, 3600).unwrap() - 200.0).abs() < EPS);
    }

    /// Dauern, die länger als die Aktivität sind, fehlen. 100 s reichen für 5 s und 60 s.
    #[test]
    fn durations_longer_than_the_activity_are_missing() {
        let a = activity((0..100).map(|t| power(t, 150.0)).collect());
        let list = best(&a);
        assert_eq!(
            list.iter().map(|b| b.duration_s).collect::<Vec<_>>(),
            [5, 60]
        );
    }

    /// Ein Fenster läuft nicht über eine Pause. Zweimal 40 s mit 300 W, dazwischen 100 s
    /// Pause. Ein 60-s-Fenster über die Pause gäbe es nur mit Auffüllen, hier fehlt der Wert.
    /// Der 5-s-Wert bleibt 300 W.
    #[test]
    fn windows_do_not_cross_pauses() {
        let mut samples: Vec<Sample> = (0..40).map(|t| power(t, 300.0)).collect();
        samples.extend((140..180).map(|t| power(t, 300.0)));
        let list = best(&activity(samples));
        assert!((watts(&list, 5).unwrap() - 300.0).abs() < EPS);
        assert_eq!(watts(&list, 60), None);
    }

    /// Kurze Lücken zählen als 0 W: 60 s mit 300 W, aber 6 Sekunden ohne Eintrag in der Mitte
    /// (Zeitlücke von 6 s, unter der Pausengrenze 10 s). Fenster über 60 s: 54 Sekunden mit
    /// 300 W und 6 mit 0 W, Mittel `54 * 300 / 60 = 270`.
    #[test]
    fn short_gaps_count_as_zero_watts() {
        let mut samples: Vec<Sample> = (0..30).map(|t| power(t, 300.0)).collect();
        samples.extend((36..66).map(|t| power(t, 300.0)));
        let list = best(&activity(samples));
        assert!((watts(&list, 60).unwrap() - 270.0).abs() < EPS);
        assert!((watts(&list, 5).unwrap() - 300.0).abs() < EPS);
    }

    #[test]
    fn no_power_channel_gives_no_values() {
        let samples = (0..100)
            .map(|t| Sample {
                hr_bpm: Some(140.0),
                ..Sample::at(t)
            })
            .collect();
        assert!(best(&activity(samples)).is_empty());
    }

    /// Verworfene Spitzen (über 2500 W) erhöhen den Bestwert nicht, der Wert zählt 0 W. 20 s mit
    /// 200 W, an Sekunde 10 eine Spitze von 3000 W. Bester 5-s-Wert: ein Fenster neben der
    /// Spitze, 200 W (ein Fenster mit der Spitze hätte `(3000 + 4 * 200) / 5 = 760` ergeben).
    /// Über 20 s: 19 Sekunden mit 200 W und eine mit 0 W, `19 * 200 / 20 = 190`.
    #[test]
    fn discarded_spikes_do_not_raise_the_best_value() {
        let mut samples: Vec<Sample> = (0..20).map(|t| power(t, 200.0)).collect();
        samples[10] = power(10, 3000.0);
        let list = best_power(&activity(samples), &LoadConfig::default(), &[5, 20]).unwrap();
        assert!((watts(&list, 5).unwrap() - 200.0).abs() < EPS);
        assert!((watts(&list, 20).unwrap() - 190.0).abs() < EPS);
    }
}
