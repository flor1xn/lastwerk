//! Effizienzfaktor und aerobe Entkopplung (SPEC 4.2 F36, Anhang A Q16).
//!
//! Beide Kennzahlen setzen Leistung ins Verhältnis zur Herzfrequenz:
//!
//! ```text
//! EF            = NP / mittlere HF                      (je Radeinheit)
//! Entkopplung % = (EF_1 - EF_2) / EF_1 * 100            (EF je Hälfte, Pw:HR)
//! ```
//!
//! Quelle ist Q16 (Allen, Coggan), und dort nur als Sekundärquelle: Das Buch wurde nicht
//! eingesehen, die Definitionen stammen aus mehreren übereinstimmenden Darstellungen Dritter.
//! Die Zuschreibung der Entkopplung in genau dieser Form ist nicht geprüft. Vor einer Freigabe
//! ist das Original zu beschaffen (SPEC 14, O8). Die Schwellen dieses Moduls (Mindestdauer,
//! Mindestzahl Einheiten, 5 Prozent) sind Faustregeln und als Konstanten benannt.
//!
//! Beide Größen hängen von Wetter, Hitze, Koffein und Tagesform ab. Sie beschreiben Trainingsdaten,
//! sie sagen nichts über Gesundheit (SPEC 2.3).

use chrono::{NaiveDate, TimeDelta};
use serde::{Deserialize, Serialize};

use super::{TREND_BAND_PERCENT, TrendState, change_percent};
use crate::load::Sport;

/// Mindestdauer einer Einheit für den Effizienzfaktor in Sekunden (Faustregel, 45 Minuten).
/// Bei kürzeren Einheiten bestimmen Anstieg der Herzfrequenz und Aufwärmen das Mittel.
pub const EF_MIN_DURATION_S: u32 = 45 * 60;

/// Länge des gleitenden Mittels des Effizienzfaktors in Tagen (Faustregel, vier Wochen).
pub const EF_WINDOW_DAYS: u32 = 28;

/// So viele Einheiten braucht ein Fenster mindestens, sonst gibt es keinen Mittelwert
/// (Faustregel). Eine einzelne Einheit ist kein Trend.
pub const EF_MIN_SESSIONS: usize = 2;

/// Mindestlänge der Reihe für die Entkopplung in Sekunden mit Leistung und Herzfrequenz
/// (Faustregel, 45 Minuten wie beim Effizienzfaktor). Jede Hälfte hat dann mindestens 22 Minuten.
pub const DECOUPLING_MIN_SAMPLES: usize = 45 * 60;

/// Entkopplung bis zu diesem Wert in Prozent gilt als "gekoppelt" (Faustregel der Trainerliteratur,
/// Sekundärquelle, nicht geprüft). Darüber ließ die Effizienz in der zweiten Hälfte merklich nach.
pub const DECOUPLING_COUPLED_MAX_PERCENT: f64 = 5.0;

/// Eine Aktivität als Eingang für den Effizienzfaktor.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EfSession {
    pub date: NaiveDate,
    pub sport: Sport,
    pub duration_s: u32,
    /// Normalisierte Leistung (`activities.np_w`).
    pub np_w: Option<f32>,
    /// Mittlere Herzfrequenz (`activities.avg_hr`).
    pub avg_hr_bpm: Option<f32>,
}

/// Effizienzfaktor einer Einheit an einem Tag.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EfPoint {
    pub date: NaiveDate,
    /// Watt je Herzschlag pro Minute (NP / mittlere HF).
    pub ef: f32,
}

/// Effizienzfaktor einer Einheit: NP geteilt durch die mittlere Herzfrequenz. `None`, wenn es
/// keine Radeinheit ist, sie kürzer als [`EF_MIN_DURATION_S`] ist oder NP oder Herzfrequenz
/// fehlen, nicht endlich oder nicht positiv sind. Nie eine Null als Ersatz (SPEC 6.5).
pub fn efficiency_factor(s: &EfSession) -> Option<f32> {
    if s.sport != Sport::Cycling || s.duration_s < EF_MIN_DURATION_S {
        return None;
    }
    let np = s.np_w.filter(|v| v.is_finite() && *v > 0.0)?;
    let hr = s.avg_hr_bpm.filter(|v| v.is_finite() && *v > 0.0)?;
    Some(np / hr)
}

/// Effizienzfaktoren aller geeigneten Einheiten, aufsteigend nach Datum (stabil bei gleichem Tag).
pub fn ef_points(sessions: &[EfSession]) -> Vec<EfPoint> {
    let mut out: Vec<EfPoint> = sessions
        .iter()
        .filter_map(|s| efficiency_factor(s).map(|ef| EfPoint { date: s.date, ef }))
        .collect();
    out.sort_by_key(|p| p.date);
    out
}

/// Mittel der Effizienzfaktoren der Einheiten in `(end - window_days, end]`, also die letzten
/// `window_days` Tage einschließlich `end`. Jede Einheit zählt gleich. `None` bei weniger als
/// [`EF_MIN_SESSIONS`] Einheiten.
pub fn ef_rolling_mean(points: &[EfPoint], end: NaiveDate, window_days: u32) -> Option<f32> {
    let start = end.checked_sub_signed(TimeDelta::days(i64::from(window_days) - 1))?;
    mean_between(points, start, end)
}

fn mean_between(points: &[EfPoint], from: NaiveDate, to: NaiveDate) -> Option<f32> {
    let values: Vec<f64> = points
        .iter()
        .filter(|p| p.date >= from && p.date <= to)
        .map(|p| f64::from(p.ef))
        .collect();
    if values.len() < EF_MIN_SESSIONS {
        return None;
    }
    Some((values.iter().sum::<f64>() / values.len() as f64) as f32)
}

/// Gleitendes Mittel für jeden Tag von `from` bis `to` einschließlich, an dem es berechenbar
/// ist (Reihe für das Diagramm).
pub fn ef_rolling_series(
    points: &[EfPoint],
    from: NaiveDate,
    to: NaiveDate,
    window_days: u32,
) -> Vec<EfPoint> {
    let mut out = Vec::new();
    let mut date = from;
    while date <= to {
        if let Some(ef) = ef_rolling_mean(points, date, window_days) {
            out.push(EfPoint { date, ef });
        }
        let Some(next) = date.checked_add_signed(TimeDelta::days(1)) else {
            break;
        };
        date = next;
    }
    out
}

/// Änderung des Mittels der letzten `window_days` Tage gegenüber den `window_days` Tagen davor,
/// in Prozent. `None`, wenn eines der beiden Fenster zu wenig Einheiten hat.
pub fn ef_change_percent(points: &[EfPoint], end: NaiveDate, window_days: u32) -> Option<f64> {
    let w = i64::from(window_days);
    let prev_end = end.checked_sub_signed(TimeDelta::days(w))?;
    let current = ef_rolling_mean(points, end, window_days).map(f64::from);
    let previous = ef_rolling_mean(points, prev_end, window_days).map(f64::from);
    change_percent(current, previous)
}

/// Deutung des Effizienztrends (Faustregel: Band [`TREND_BAND_PERCENT`]).
pub fn ef_trend(points: &[EfPoint], end: NaiveDate, window_days: u32) -> TrendState {
    TrendState::from_change(
        ef_change_percent(points, end, window_days),
        TREND_BAND_PERCENT,
    )
}

/// Stabiler Code des Effizienztrends, Schlüssel für Texte in `catalog/locales`.
pub const fn ef_trend_code(state: TrendState) -> &'static str {
    match state {
        TrendState::NoData => "efficiency.no_data",
        TrendState::Falling => "efficiency.falling",
        TrendState::Flat => "efficiency.flat",
        TrendState::Rising => "efficiency.rising",
    }
}

/// Aerobe Entkopplung (Pw:HR) in Prozent aus einer Leistungs- und einer Herzfrequenzreihe im
/// Sekundentakt.
///
/// - Es zählen nur Sekunden, in denen beide Werte vorliegen, endlich sind, die Leistung nicht
///   negativ und die Herzfrequenz positiv ist. Lücken werden übersprungen, nie aufgefüllt
///   (SPEC 6.5). Bei ungleich langen Reihen zählt der gemeinsame Anfang.
/// - Die Paare werden in der Reihenfolge geteilt: erste Hälfte gegen zweite Hälfte. Bei
///   ungerader Zahl bleibt das mittlere Paar außen vor, damit beide Hälften gleich lang sind.
/// - Je Hälfte ist `EF = mittlere Leistung / mittlere Herzfrequenz`. Die mittlere Leistung
///   (nicht NP) ist eine Vereinfachung dieses Moduls: Die Quellen (Q16 nur sekundär) nennen
///   beide Varianten uneinheitlich.
/// - Positiv heißt: Die Effizienz fiel in der zweiten Hälfte (Herzfrequenz stieg bei gleicher
///   Leistung oder Leistung fiel bei gleicher Herzfrequenz).
///
/// `None` bei weniger als [`DECOUPLING_MIN_SAMPLES`] Paaren oder wenn die erste Hälfte keine
/// Leistung hat.
pub fn aerobic_decoupling(power_w: &[Option<f32>], hr_bpm: &[Option<f32>]) -> Option<f64> {
    let pairs: Vec<(f64, f64)> = power_w
        .iter()
        .zip(hr_bpm)
        .filter_map(|(p, h)| {
            let p = f64::from((*p)?);
            let h = f64::from((*h)?);
            (p.is_finite() && p >= 0.0 && h.is_finite() && h > 0.0).then_some((p, h))
        })
        .collect();
    if pairs.len() < DECOUPLING_MIN_SAMPLES {
        return None;
    }
    let half = pairs.len() / 2;
    let ef = |part: &[(f64, f64)]| {
        let n = part.len() as f64;
        let p = part.iter().map(|x| x.0).sum::<f64>() / n;
        let h = part.iter().map(|x| x.1).sum::<f64>() / n;
        p / h
    };
    let first = ef(&pairs[..half]);
    let second = ef(&pairs[pairs.len() - half..]);
    (first > 0.0).then(|| (first - second) / first * 100.0)
}

/// Deutung der Entkopplung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecouplingState {
    /// Keine Entkopplung berechenbar (Reihe zu kurz oder ohne Leistung).
    NoData,
    /// Bis [`DECOUPLING_COUPLED_MAX_PERCENT`] einschließlich, auch bei negativen Werten.
    Coupled,
    /// Darüber: die Effizienz ließ in der zweiten Hälfte nach.
    Decoupled,
}

impl DecouplingState {
    /// Stabiler Code, Schlüssel für Texte in `catalog/locales`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::NoData => "decoupling.no_data",
            Self::Coupled => "decoupling.coupled",
            Self::Decoupled => "decoupling.decoupled",
        }
    }
}

/// Deutet eine Entkopplung in Prozent (Faustregel: Grenze [`DECOUPLING_COUPLED_MAX_PERCENT`]).
pub fn decoupling_state(percent: Option<f64>) -> DecouplingState {
    match percent {
        Some(p) if p.is_finite() => {
            if p > DECOUPLING_COUPLED_MAX_PERCENT {
                DecouplingState::Decoupled
            } else {
                DecouplingState::Coupled
            }
        }
        _ => DecouplingState::NoData,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(n: i64) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 1, 1).unwrap() + TimeDelta::days(n)
    }

    fn ride(minutes: u32, np: Option<f32>, hr: Option<f32>) -> EfSession {
        EfSession {
            date: day(0),
            sport: Sport::Cycling,
            duration_s: minutes * 60,
            np_w: np,
            avg_hr_bpm: hr,
        }
    }

    fn pt(n: i64, ef: f32) -> EfPoint {
        EfPoint { date: day(n), ef }
    }

    /// NP 200 W bei mittlerer HF 140 bpm: `200 / 140 = 1,428571`.
    const EF_200_W_140_BPM: f32 = 1.428_571;

    #[test]
    fn ef_is_np_over_mean_hr() {
        let ef = efficiency_factor(&ride(60, Some(200.0), Some(140.0))).unwrap();
        assert!((ef - EF_200_W_140_BPM).abs() < 1e-6);
    }

    #[test]
    fn ef_needs_a_long_enough_cycling_session_with_both_values() {
        // Grenze 45 min gilt "ab".
        assert!(efficiency_factor(&ride(45, Some(200.0), Some(140.0))).is_some());
        assert!(efficiency_factor(&ride(44, Some(200.0), Some(140.0))).is_none());
        assert!(efficiency_factor(&ride(60, None, Some(140.0))).is_none());
        assert!(efficiency_factor(&ride(60, Some(200.0), None)).is_none());
        assert!(efficiency_factor(&ride(60, Some(200.0), Some(0.0))).is_none());
        assert!(efficiency_factor(&ride(60, Some(0.0), Some(140.0))).is_none());
        assert!(efficiency_factor(&ride(60, Some(f32::NAN), Some(140.0))).is_none());
        let mut run = ride(60, Some(200.0), Some(140.0));
        run.sport = Sport::Running;
        assert!(efficiency_factor(&run).is_none());
    }

    #[test]
    fn points_are_sorted_by_date_and_skip_unfit_sessions() {
        let mut a = ride(60, Some(200.0), Some(140.0));
        a.date = day(5);
        let mut b = ride(60, Some(210.0), Some(140.0));
        b.date = day(2);
        let short = ride(20, Some(210.0), Some(140.0));
        let p = ef_points(&[a, short, b]);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].date, day(2));
        assert_eq!(p[1].date, day(5));
    }

    /// Fenster 28 Tage bis Tag 100 sind Tag 73 bis 100. Einheiten dort: Tag 90 (1,50) und Tag 95
    /// (1,54), Mittel 1,52. Das Fenster davor endet an Tag 72 (Tag 45 bis 72): Tag 60 (1,40) und
    /// Tag 70 (1,44), Mittel 1,42. Änderung `(1,52 - 1,42) / 1,42 * 100 = 7,0423`.
    const EF_CHANGE_PERCENT: f64 = 7.0423;

    fn two_windows() -> Vec<EfPoint> {
        vec![pt(60, 1.40), pt(70, 1.44), pt(90, 1.50), pt(95, 1.54)]
    }

    #[test]
    fn rolling_mean_covers_the_window_including_the_end_day() {
        let p = two_windows();
        let m = ef_rolling_mean(&p, day(100), EF_WINDOW_DAYS).unwrap();
        assert!((m - 1.52).abs() < 1e-6);
        // Der Tag 73 liegt noch im Fenster, Tag 72 nicht mehr: Fenster bis Tag 100 mit
        // 28 Tagen beginnt an Tag 73.
        let edge = vec![pt(72, 9.0), pt(73, 1.0), pt(100, 3.0)];
        assert_eq!(ef_rolling_mean(&edge, day(100), 28), Some(2.0));
        // Eine einzelne Einheit ergibt kein Mittel.
        assert_eq!(ef_rolling_mean(&[pt(90, 1.5)], day(100), 28), None);
    }

    #[test]
    fn change_and_trend_compare_with_the_window_before() {
        let p = two_windows();
        let c = ef_change_percent(&p, day(100), EF_WINDOW_DAYS).unwrap();
        assert!((c - EF_CHANGE_PERCENT).abs() < 1e-3, "{c}");
        assert_eq!(ef_trend(&p, day(100), EF_WINDOW_DAYS), TrendState::Rising);
        // Ohne Vorfenster keine Aussage.
        assert_eq!(
            ef_trend(&p[2..], day(100), EF_WINDOW_DAYS),
            TrendState::NoData
        );
        // Sinkende Werte: Mittel 1,40 gegen 1,50 = -6,67 Prozent.
        let falling = vec![pt(60, 1.50), pt(70, 1.50), pt(90, 1.40), pt(95, 1.40)];
        assert_eq!(
            ef_trend(&falling, day(100), EF_WINDOW_DAYS),
            TrendState::Falling
        );
        // Innerhalb des Bands: 1,51 gegen 1,50 = 0,67 Prozent.
        let flat = vec![pt(60, 1.50), pt(70, 1.50), pt(90, 1.51), pt(95, 1.51)];
        assert_eq!(ef_trend(&flat, day(100), EF_WINDOW_DAYS), TrendState::Flat);
    }

    #[test]
    fn rolling_series_lists_only_computable_days() {
        let p = two_windows();
        let s = ef_rolling_series(&p, day(58), day(72), EF_WINDOW_DAYS);
        // Ab Tag 70 liegen zwei Einheiten im Fenster (Tag 60 und 70).
        assert_eq!(s.first().map(|x| x.date), Some(day(70)));
        assert!((s[0].ef - 1.42).abs() < 1e-6);
        assert!(ef_rolling_series(&p, day(10), day(5), 28).is_empty());
    }

    #[test]
    fn trend_codes_are_stable() {
        assert_eq!(ef_trend_code(TrendState::Rising), "efficiency.rising");
        assert_eq!(ef_trend_code(TrendState::Flat), "efficiency.flat");
        assert_eq!(ef_trend_code(TrendState::Falling), "efficiency.falling");
        assert_eq!(ef_trend_code(TrendState::NoData), "efficiency.no_data");
    }

    fn series(
        first: (f32, f32),
        second: (f32, f32),
        n: usize,
    ) -> (Vec<Option<f32>>, Vec<Option<f32>>) {
        let mut p = vec![Some(first.0); n / 2];
        let mut h = vec![Some(first.1); n / 2];
        p.extend(vec![Some(second.0); n - n / 2]);
        h.extend(vec![Some(second.1); n - n / 2]);
        (p, h)
    }

    /// 3000 s, erste Hälfte 200 W bei 140 bpm, zweite 200 W bei 147 bpm.
    /// `EF_1 = 200 / 140 = 1,428571`, `EF_2 = 200 / 147 = 1,360544`,
    /// `(EF_1 - EF_2) / EF_1 = 1 - 140 / 147 = 7 / 147 = 0,047619`, also 4,7619 Prozent.
    const DECOUPLING_HR_DRIFT: f64 = 4.7619;

    #[test]
    fn decoupling_matches_the_hand_calculation() {
        let (p, h) = series((200.0, 140.0), (200.0, 147.0), 3000);
        let d = aerobic_decoupling(&p, &h).unwrap();
        assert!((d - DECOUPLING_HR_DRIFT).abs() < 1e-3, "{d}");
        assert_eq!(decoupling_state(Some(d)), DecouplingState::Coupled);
    }

    /// Gleiche HF 140 bpm, Leistung 200 W dann 190 W: `1 - 190 / 200 = 0,05`, also 5 Prozent,
    /// genau auf der Grenze und damit noch gekoppelt. Bei 188 W: 6 Prozent, entkoppelt.
    #[test]
    fn decoupling_from_falling_power_and_the_five_percent_limit() {
        let (p, h) = series((200.0, 140.0), (190.0, 140.0), 3000);
        let d = aerobic_decoupling(&p, &h).unwrap();
        assert!((d - 5.0).abs() < 1e-9, "{d}");
        assert_eq!(decoupling_state(Some(5.0)), DecouplingState::Coupled);
        let (p, h) = series((200.0, 140.0), (188.0, 140.0), 3000);
        let d = aerobic_decoupling(&p, &h).unwrap();
        assert!((d - 6.0).abs() < 1e-9, "{d}");
        assert_eq!(decoupling_state(Some(d)), DecouplingState::Decoupled);
    }

    #[test]
    fn decoupling_skips_gaps_and_needs_enough_data() {
        // Negative Entkopplung: HF fällt, bleibt gekoppelt.
        let (p, h) = series((200.0, 150.0), (200.0, 140.0), 3000);
        assert!(aerobic_decoupling(&p, &h).unwrap() < 0.0);
        // Lücken (None) werden übersprungen und verschieben die Hälften nicht: 3200 Sekunden,
        // je 100 Sekunden Lücke in jeder Hälfte (erst ohne Leistung, dann ohne HF), 3000 Paare
        // bleiben, 1500 je Hälfte.
        let (mut p, mut h) = series((200.0, 140.0), (200.0, 147.0), 3200);
        for x in &mut p[..100] {
            *x = None;
        }
        for x in &mut h[1600..1700] {
            *x = None;
        }
        let d = aerobic_decoupling(&p, &h).unwrap();
        assert!((d - DECOUPLING_HR_DRIFT).abs() < 1e-3, "{d}");
        // Zu kurz: 44 Minuten.
        let (p, h) = series((200.0, 140.0), (200.0, 147.0), 44 * 60);
        assert_eq!(aerobic_decoupling(&p, &h), None);
        // Ohne Leistung in der ersten Hälfte gibt es keinen Bezug.
        let (p, h) = series((0.0, 140.0), (200.0, 147.0), 3000);
        assert_eq!(aerobic_decoupling(&p, &h), None);
        assert_eq!(aerobic_decoupling(&[], &[]), None);
        assert_eq!(decoupling_state(None), DecouplingState::NoData);
    }

    #[test]
    fn decoupling_codes_are_stable() {
        assert_eq!(DecouplingState::Coupled.code(), "decoupling.coupled");
        assert_eq!(DecouplingState::Decoupled.code(), "decoupling.decoupled");
        assert_eq!(DecouplingState::NoData.code(), "decoupling.no_data");
    }
}
