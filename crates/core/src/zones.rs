//! Zonenschemata und Zeit in Zonen (SPEC 8.5).
//!
//! Zonentabellen sind daten- und konfigurationsgetrieben (`catalog/zones.json`) und nicht
//! fest verdrahtet. Dieses Modul kennt nur die Struktur ([`ZoneScheme`], [`ZoneCatalog`])
//! und die Zuordnung der Samples zu Zonen ([`zone_seconds`]). Es liest keine Datei: Der
//! Katalog wird als Daten übergeben, das Laden übernimmt `lastwerk-store`.
//!
//! Alle Grenzwerte im Katalog sind Parameter mit Herkunft (`source`, `status`). Die Werte
//! stammen aus Sekundärliteratur oder sind Faustregeln und vor einer Freigabe am Original
//! zu prüfen (SPEC 14, O8). Ein Fehler in einer Quelle wird im Katalog korrigiert, ohne
//! diese Logik zu ändern (SPEC 11.1).
//!
//! Zonenzuordnung (Konvention): Eine Zone umfasst die Werte bis einschließlich ihrer oberen
//! Grenze. Bei Grenzen `[0,55, 0,75]` liegt 0,75 in Zone 2, alles über 0,75 in Zone 3. Die
//! letzte Zone ist nach oben offen, die erste nach unten.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::load::{
    ActivityInput, LoadConfig, PreparedSamples, Sample, Sport, Thresholds, check_plausibility,
};

/// Bezugswert, auf den sich die Grenzen eines Schemas beziehen (SPEC 8.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneBasis {
    /// Leistung als Anteil der FTP (Q16).
    Ftp,
    /// Herzfrequenz als Anteil der Schwellenherzfrequenz.
    Lthr,
    /// Herzfrequenz als Anteil der Herzfrequenzreserve `(HR - HR_rest) / (HR_max - HR_rest)`
    /// (Q14, Konzept der Reserve).
    HrReserve,
    /// Geschwindigkeit als Anteil der Laufschwellengeschwindigkeit.
    RunThresholdSpeed,
    /// Geschwindigkeit als Anteil der kritischen Schwimmgeschwindigkeit (Q15).
    Css,
}

/// Messkanal, aus dem die Zone eines Samples folgt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneChannel {
    Power,
    HeartRate,
    Speed,
}

impl ZoneChannel {
    /// Name des Kanals wie im Datenmodell (`power_w`, `hr_bpm`, `speed_mps`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Power => "power_w",
            Self::HeartRate => "hr_bpm",
            Self::Speed => "speed_mps",
        }
    }
}

impl ZoneBasis {
    /// Kanal, auf dem das Schema rechnet.
    pub const fn channel(self) -> ZoneChannel {
        match self {
            Self::Ftp => ZoneChannel::Power,
            Self::Lthr | Self::HrReserve => ZoneChannel::HeartRate,
            Self::RunThresholdSpeed | Self::Css => ZoneChannel::Speed,
        }
    }
}

/// Ein Zonenschema aus `catalog/zones.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneScheme {
    /// Kennung, zum Beispiel `power7`. Steht in `activity_zone_time.scheme`.
    pub id: String,
    pub basis: ZoneBasis,
    /// Sportarten, für die das Schema gilt (Konvention: der Speicher füllt nur dafür).
    pub sports: Vec<Sport>,
    /// Obere Grenzen aller Zonen außer der letzten als Faktor des Bezugswerts, streng
    /// steigend. Die letzte Zone ist offen, ein Schema mit `n` Grenzen hat `n + 1` Zonen.
    pub bounds: Vec<f32>,
    /// Herkunft der Grenzen: Quellennummer aus SPEC Anhang A oder `Faustregel`.
    pub source: String,
    /// Prüfstatus der Grenzen: `Sekundärquelle` oder `Faustregel`.
    pub status: String,
    /// Freier Hinweis, zum Beispiel "vor Freigabe am Original prüfen (O8)".
    #[serde(default)]
    pub note: String,
}

impl ZoneScheme {
    /// Zahl der Zonen: Grenzen plus die offene letzte Zone.
    pub fn zone_count(&self) -> usize {
        self.bounds.len() + 1
    }

    /// Gilt das Schema für die Sportart?
    pub fn applies_to(&self, sport: Sport) -> bool {
        self.sports.contains(&sport)
    }

    /// Prüft die Struktur: mindestens eine Grenze, alle endlich und positiv, streng
    /// steigend, mindestens eine Sportart, Herkunft angegeben.
    pub fn validate(&self) -> Result<(), ZoneError> {
        let bad = |why: &str| ZoneError::InvalidScheme(format!("{}: {why}", self.id));
        if self.id.is_empty() {
            return Err(ZoneError::InvalidScheme("empty id".into()));
        }
        if self.bounds.is_empty() {
            return Err(bad("no bounds"));
        }
        if self.sports.is_empty() {
            return Err(bad("no sports"));
        }
        if self.source.is_empty() || self.status.is_empty() {
            return Err(bad("source and status are required"));
        }
        if self.bounds.iter().any(|b| !(b.is_finite() && *b > 0.0)) {
            return Err(bad("bounds must be finite and positive"));
        }
        if self.bounds.windows(2).any(|w| w[0] >= w[1]) {
            return Err(bad("bounds must be strictly increasing"));
        }
        Ok(())
    }

    /// Zone (0-basiert) zu einem Anteil des Bezugswerts. Obere Grenze einschließlich.
    pub fn zone_index(&self, ratio: f32) -> usize {
        self.bounds.iter().filter(|&&b| ratio > b).count()
    }
}

/// Alle Schemata aus `catalog/zones.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneCatalog {
    pub format_version: u32,
    pub schemes: Vec<ZoneScheme>,
}

impl ZoneCatalog {
    /// Prüft jedes Schema und dass keine Kennung doppelt vorkommt.
    pub fn validate(&self) -> Result<(), ZoneError> {
        for (i, s) in self.schemes.iter().enumerate() {
            s.validate()?;
            if self.schemes[..i].iter().any(|o| o.id == s.id) {
                return Err(ZoneError::InvalidScheme(format!("duplicate id {}", s.id)));
            }
        }
        Ok(())
    }

    /// Das Schema mit dieser Kennung.
    pub fn scheme(&self, id: &str) -> Option<&ZoneScheme> {
        self.schemes.iter().find(|s| s.id == id)
    }

    /// Schemata, die für die Sportart gelten, in Katalogreihenfolge.
    pub fn for_sport(&self, sport: Sport) -> impl Iterator<Item = &ZoneScheme> {
        self.schemes.iter().filter(move |s| s.applies_to(sport))
    }
}

/// Fehler der Zonenberechnung. Wie bei der Belastung erzeugt Fehlendes nie Nullen, sondern
/// einen Fehler, den die Aufrufer als "nicht berechenbar" ausweisen.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "code", content = "detail", rename_all = "snake_case")]
pub enum ZoneError {
    /// Der Bezugswert des Schemas fehlt.
    #[error("missing_threshold: {0}")]
    MissingThreshold(String),
    /// Kein Sample der Aktivität hat den Kanal des Schemas.
    #[error("missing_channel: {0}")]
    MissingChannel(String),
    /// Bezugswerte oder Zeitstempel lassen keine Berechnung zu.
    #[error("implausible: {0}")]
    Implausible(String),
    /// Das Schema selbst ist ungültig (Katalogfehler).
    #[error("invalid_scheme: {0}")]
    InvalidScheme(String),
}

impl ZoneError {
    /// Fehlercode als stabiler String.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::MissingThreshold(_) => "missing_threshold",
            Self::MissingChannel(_) => "missing_channel",
            Self::Implausible(_) => "implausible",
            Self::InvalidScheme(_) => "invalid_scheme",
        }
    }
}

/// Positive, endliche Schwelle oder Fehler.
fn positive(value: Option<f32>, name: &str) -> Result<f32, ZoneError> {
    let v = value.ok_or_else(|| ZoneError::MissingThreshold(name.into()))?;
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(ZoneError::Implausible(format!("{name} must be positive")))
    }
}

/// Zeit in Zonen einer Aktivität in Sekunden, Index 0 ist Zone 1.
///
/// Jedes Sample steht für eine Sekunde und zählt in genau eine Zone. Samples ohne den
/// Kanal des Schemas zählen nicht, Zeitlücken ebenfalls nicht (SPEC 6.5: nichts auffüllen).
/// Die Summe über alle Zonen ist deshalb die Zahl der Samples mit Kanal. Unplausible Werte
/// nach SPEC 6.5 (zum Beispiel Leistung über der Obergrenze) werden wie in der Belastung
/// verworfen und zählen nicht ([`check_plausibility`] mit `cfg`).
///
/// Abweichung von der Skizze in SPEC 5.4: zusätzlich `th` (Bezugswert) und `cfg`
/// (Plausibilitätsgrenzen), und ein Ergebnistyp mit Fehler statt einer nackten Liste.
///
/// Fehler: `MissingThreshold`, wenn der Bezugswert fehlt, `MissingChannel`, wenn kein Sample
/// den Kanal hat. Die maximale Herzfrequenz ersetzt dieses Modul nie selbst. Fehlt sie,
/// setzt der Aufrufer eine Schätzung ein (`estimate_hr_max`) und kennzeichnet sie.
pub fn zone_seconds(
    a: &ActivityInput,
    scheme: &ZoneScheme,
    th: &Thresholds,
    cfg: &LoadConfig,
) -> Result<Vec<u32>, ZoneError> {
    scheme.validate()?;
    let reference = reference(scheme.basis, th)?;
    let prepared =
        check_plausibility(&a.samples, cfg).map_err(|e| ZoneError::Implausible(e.to_string()))?;
    count_zones(scheme, &reference, &prepared)
}

/// Wie [`zone_seconds`], aber mit den schon geprüften Samples. `prepared` muss das Ergebnis
/// von [`check_plausibility`] für die Samples der Aktivität sein, mit demselben
/// [`LoadConfig`], das sonst `zone_seconds` bekäme. Grund wie bei
/// [`crate::load::compute_load_prepared`]: Der Import prüft die Samples einmal statt je
/// Schema (T38, Anhang Messung). Prüfung des Schemas und des Bezugswerts wie bei
/// `zone_seconds`, in derselben Reihenfolge.
pub fn zone_seconds_prepared(
    prepared: &PreparedSamples,
    scheme: &ZoneScheme,
    th: &Thresholds,
) -> Result<Vec<u32>, ZoneError> {
    scheme.validate()?;
    let reference = reference(scheme.basis, th)?;
    count_zones(scheme, &reference, prepared)
}

fn count_zones(
    scheme: &ZoneScheme,
    reference: &Reference,
    prepared: &PreparedSamples,
) -> Result<Vec<u32>, ZoneError> {
    let mut seconds = vec![0_u32; scheme.zone_count()];
    let mut counted = 0_u32;
    for s in prepared.segments.iter().flatten() {
        if let Some(r) = reference.ratio(s) {
            seconds[scheme.zone_index(r)] += 1;
            counted += 1;
        }
    }
    if counted == 0 {
        return Err(ZoneError::MissingChannel(
            scheme.basis.channel().as_str().into(),
        ));
    }
    Ok(seconds)
}

/// Bezug eines Schemas: Kanal, Nullpunkt und Maß. Der Anteil eines Werts ist
/// `(Wert - offset) / scale`. Nur die Herzfrequenzreserve hat einen Nullpunkt (den
/// Ruhepuls), bei allen anderen ist er 0.
struct Reference {
    channel: ZoneChannel,
    offset: f32,
    scale: f32,
}

impl Reference {
    /// Anteil des Bezugswerts, den ein Sample erreicht, oder `None` ohne Kanalwert. Gerechnet
    /// in `f32`, damit ein Wert genau auf einer Grenze (zum Beispiel 225 W bei FTP 250 und
    /// Grenze 0,9) so gerundet wird wie die Grenze selbst und in der unteren Zone landet.
    fn ratio(&self, s: &Sample) -> Option<f32> {
        let value = match self.channel {
            ZoneChannel::Power => s.power_w,
            ZoneChannel::HeartRate => s.hr_bpm,
            ZoneChannel::Speed => s.speed_mps,
        }?;
        Some((value - self.offset) / self.scale)
    }
}

fn reference(basis: ZoneBasis, th: &Thresholds) -> Result<Reference, ZoneError> {
    let channel = basis.channel();
    Ok(match basis {
        ZoneBasis::Ftp => Reference {
            channel,
            offset: 0.0,
            scale: positive(th.ftp_w, "ftp_w")?,
        },
        ZoneBasis::Lthr => Reference {
            channel,
            offset: 0.0,
            scale: positive(th.lthr_bpm, "lthr_bpm")?,
        },
        ZoneBasis::HrReserve => {
            let hr_max = positive(th.hr_max_bpm, "hr_max_bpm")?;
            let hr_rest = positive(th.hr_rest_bpm, "hr_rest_bpm")?;
            if hr_max <= hr_rest {
                return Err(ZoneError::Implausible(
                    "hr_max_bpm must exceed hr_rest_bpm".into(),
                ));
            }
            Reference {
                channel,
                offset: hr_rest,
                scale: hr_max - hr_rest,
            }
        }
        ZoneBasis::RunThresholdSpeed => Reference {
            channel,
            offset: 0.0,
            scale: positive(th.run_threshold_speed_mps, "run_threshold_speed_mps")?,
        },
        ZoneBasis::Css => Reference {
            channel,
            offset: 0.0,
            scale: positive(th.css_mps, "css_mps")?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone, Utc};
    use proptest::prelude::*;

    fn scheme(basis: ZoneBasis, bounds: &[f32]) -> ZoneScheme {
        ZoneScheme {
            id: "test".into(),
            basis,
            sports: vec![Sport::Cycling],
            bounds: bounds.to_vec(),
            source: "Faustregel".into(),
            status: "Faustregel".into(),
            note: String::new(),
        }
    }

    /// Coggan-artige Grenzen als Testdaten (Sekundärquelle, SPEC 8.5).
    fn power7() -> ZoneScheme {
        scheme(ZoneBasis::Ftp, &[0.55, 0.75, 0.90, 1.05, 1.20, 1.50])
    }

    fn thresholds() -> Thresholds {
        Thresholds::empty(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
    }

    fn with_ftp(ftp: f32) -> Thresholds {
        Thresholds {
            ftp_w: Some(ftp),
            ..thresholds()
        }
    }

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

    fn hr(t: u32, bpm: f32) -> Sample {
        Sample {
            hr_bpm: Some(bpm),
            ..Sample::at(t)
        }
    }

    fn run(a: &ActivityInput, s: &ZoneScheme, th: &Thresholds) -> Result<Vec<u32>, ZoneError> {
        zone_seconds(a, s, th, &LoadConfig::default())
    }

    /// 3600 s mit 200 W bei FTP 250: Anteil 200 / 250 = 0,8. Das liegt über 0,75 und unter
    /// 0,90, also in Zone 3. Alle 3600 s dort, sonst nichts.
    #[test]
    fn constant_200_w_at_ftp_250_is_zone_3() {
        let a = activity((0..3600).map(|t| power(t, 200.0)).collect());
        let z = run(&a, &power7(), &with_ftp(250.0)).unwrap();
        assert_eq!(z, vec![0, 0, 3600, 0, 0, 0, 0]);
    }

    /// Je Zone ein Wert, FTP 250, handgerechnete Anteile:
    /// 100 W = 0,40 (Z1), 150 W = 0,60 (Z2), 200 W = 0,80 (Z3), 240 W = 0,96 (Z4),
    /// 275 W = 1,10 (Z5), 350 W = 1,40 (Z6), 500 W = 2,00 (Z7).
    #[test]
    fn each_zone_gets_its_seconds() {
        let plan = [
            (100.0, 10),
            (150.0, 20),
            (200.0, 30),
            (240.0, 40),
            (275.0, 5),
            (350.0, 6),
            (500.0, 7),
        ];
        let mut samples = Vec::new();
        for (w, n) in plan {
            for _ in 0..n {
                samples.push(power(samples.len() as u32, w));
            }
        }
        let z = run(&activity(samples), &power7(), &with_ftp(250.0)).unwrap();
        assert_eq!(z, vec![10, 20, 30, 40, 5, 6, 7]);
    }

    /// Eine Leistung genau auf der Grenze gehört zur unteren Zone (Konvention: obere Grenze
    /// einschließlich): 187,5 W / 250 = 0,75 liegt in Zone 2, 225 W / 250 = 0,9 in Zone 3.
    #[test]
    fn value_on_the_bound_belongs_to_the_lower_zone() {
        let a = activity(vec![power(0, 187.5), power(1, 225.0), power(2, 225.5)]);
        let z = run(&a, &power7(), &with_ftp(250.0)).unwrap();
        assert_eq!(z, vec![0, 1, 1, 1, 0, 0, 0]);
    }

    /// Lücken zählen nicht: 10 Samples, dann eine lange Pause, dann 10 Samples. Die Summe ist
    /// 20, nicht die verstrichene Zeit. Samples ohne Leistung zählen auch nicht.
    #[test]
    fn gaps_and_samples_without_the_channel_do_not_count() {
        let mut samples: Vec<Sample> = (0..10).map(|t| power(t, 200.0)).collect();
        samples.extend((100..110).map(|t| power(t, 200.0)));
        samples.extend((110..115).map(|t| hr(t, 140.0)));
        let z = run(&activity(samples), &power7(), &with_ftp(250.0)).unwrap();
        assert_eq!(z.iter().sum::<u32>(), 20);
    }

    #[test]
    fn implausible_values_are_not_counted() {
        // 3000 W liegt über der Obergrenze 2500 W (SPEC 6.5) und wird verworfen.
        let a = activity(vec![power(0, 200.0), power(1, 3000.0), power(2, 200.0)]);
        let z = run(&a, &power7(), &with_ftp(250.0)).unwrap();
        assert_eq!(z.iter().sum::<u32>(), 2);
    }

    #[test]
    fn missing_threshold_and_missing_channel_are_errors() {
        let a = activity(vec![power(0, 200.0)]);
        let err = run(&a, &power7(), &thresholds()).unwrap_err();
        assert_eq!(err, ZoneError::MissingThreshold("ftp_w".into()));
        assert_eq!(err.code(), "missing_threshold");

        let hr_only = activity(vec![hr(0, 140.0)]);
        let err = run(&hr_only, &power7(), &with_ftp(250.0)).unwrap_err();
        assert_eq!(err, ZoneError::MissingChannel("power_w".into()));
    }

    #[test]
    fn non_positive_threshold_is_implausible() {
        let a = activity(vec![power(0, 200.0)]);
        let err = run(&a, &power7(), &with_ftp(0.0)).unwrap_err();
        assert_eq!(err.code(), "implausible");
    }

    /// Karvonen-artig: HR_max 190, HR_rest 50, Reserve 140. Anteile der Reserve:
    /// 100 bpm = 50/140 = 0,36 (Z1), 130 bpm = 80/140 = 0,57 (Z1, unter 0,60),
    /// 145 bpm = 95/140 = 0,68 (Z2), 160 bpm = 110/140 = 0,79 (Z3), 170 bpm = 120/140 = 0,86
    /// (Z4), 185 bpm = 135/140 = 0,96 (Z5).
    #[test]
    fn heart_rate_reserve_zones() {
        let s = scheme(ZoneBasis::HrReserve, &[0.60, 0.70, 0.80, 0.90]);
        let th = Thresholds {
            hr_max_bpm: Some(190.0),
            hr_rest_bpm: Some(50.0),
            ..thresholds()
        };
        let samples: Vec<Sample> = [100.0, 130.0, 145.0, 160.0, 170.0, 185.0]
            .iter()
            .enumerate()
            .map(|(i, &b)| hr(i as u32, b))
            .collect();
        let z = run(&activity(samples), &s, &th).unwrap();
        assert_eq!(z, vec![2, 1, 1, 1, 1]);
    }

    #[test]
    fn heart_rate_reserve_needs_max_above_rest() {
        let s = scheme(ZoneBasis::HrReserve, &[0.60, 0.70]);
        let th = Thresholds {
            hr_max_bpm: Some(50.0),
            hr_rest_bpm: Some(60.0),
            ..thresholds()
        };
        let err = run(&activity(vec![hr(0, 100.0)]), &s, &th).unwrap_err();
        assert_eq!(err.code(), "implausible");
    }

    /// Geschwindigkeitsschemata: Laufschwelle 4,0 m/s, 3,0 m/s = 0,75 (Zone 1 bei Grenze 0,75),
    /// 3,6 m/s = 0,90 (Zone 2 bei Grenze 0,95), 5,0 m/s = 1,25 (letzte Zone).
    #[test]
    fn speed_zones_use_the_speed_channel() {
        let s = scheme(ZoneBasis::RunThresholdSpeed, &[0.75, 0.95]);
        let th = Thresholds {
            run_threshold_speed_mps: Some(4.0),
            ..thresholds()
        };
        let samples = [3.0, 3.6, 5.0]
            .iter()
            .enumerate()
            .map(|(i, &v)| Sample {
                speed_mps: Some(v),
                ..Sample::at(i as u32)
            })
            .collect();
        let z = run(&activity(samples), &s, &th).unwrap();
        assert_eq!(z, vec![1, 1, 1]);
    }

    #[test]
    fn invalid_schemes_are_rejected() {
        assert!(scheme(ZoneBasis::Ftp, &[]).validate().is_err());
        assert!(scheme(ZoneBasis::Ftp, &[0.8, 0.8]).validate().is_err());
        assert!(scheme(ZoneBasis::Ftp, &[0.9, 0.8]).validate().is_err());
        assert!(scheme(ZoneBasis::Ftp, &[f32::NAN]).validate().is_err());
        assert!(scheme(ZoneBasis::Ftp, &[-0.5, 0.5]).validate().is_err());
        assert!(power7().validate().is_ok());
        let dup = ZoneCatalog {
            format_version: 1,
            schemes: vec![power7(), power7()],
        };
        assert!(dup.validate().is_err());
    }

    #[test]
    fn catalog_selects_schemes_by_sport() {
        let mut run_scheme = scheme(ZoneBasis::RunThresholdSpeed, &[0.8]);
        run_scheme.id = "run".into();
        run_scheme.sports = vec![Sport::Running];
        let cat = ZoneCatalog {
            format_version: 1,
            schemes: vec![power7(), run_scheme],
        };
        let ids: Vec<&str> = cat
            .for_sport(Sport::Running)
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(ids, ["run"]);
        assert!(cat.scheme("test").is_some());
        assert!(cat.scheme("nope").is_none());
    }

    proptest! {
        /// Eigenschaft: Die Summe über die Zonen ist die Zahl der Samples mit plausibler
        /// Leistung, unabhängig von Lücken und fehlenden Kanalwerten.
        #[test]
        fn sum_equals_samples_with_channel(
            watts in proptest::collection::vec(proptest::option::of(0.0_f32..3000.0), 1..200),
            gap_every in 3_usize..40,
        ) {
            let mut t = 0_u32;
            let mut samples = Vec::new();
            let mut expected = 0_u32;
            for (i, w) in watts.iter().enumerate() {
                if i % gap_every == 0 {
                    t += 20; // lange Pause, kein Sample
                }
                samples.push(Sample { power_w: *w, ..Sample::at(t) });
                if matches!(w, Some(v) if *v <= 2500.0) {
                    expected += 1;
                }
                t += 1;
            }
            let result = run(&activity(samples), &power7(), &with_ftp(250.0));
            if expected == 0 {
                prop_assert!(matches!(result, Err(ZoneError::MissingChannel(_))));
            } else {
                prop_assert_eq!(result.unwrap().iter().sum::<u32>(), expected);
            }
        }
    }
}
