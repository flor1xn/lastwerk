//! Belastung je Aktivität (SPEC 8.1).
//!
//! Dieses Modul ist rein: keine Ein- und Ausgabe, keine Systemzeit, kein Zufall.
//! Es berechnet aus den aufbereiteten 1-Hz-Samples einer Aktivität genau eine
//! Belastungszahl auf der TSS-Skala (100 je Stunde an der Schwelle) und wählt die
//! Methode nach Sportart und vorhandenen Messkanälen.
//!
//! Methodenwahl (SPEC 8.1, Reihenfolge bei mehreren Kanälen ist Konvention):
//!
//! | Sportart  | Kanal                      | Methode     |
//! |-----------|----------------------------|-------------|
//! | Rad       | Leistung                   | TSS         |
//! | Rad       | nur Herzfrequenz           | hrTSS       |
//! | Lauf      | Geschwindigkeit            | rTSS        |
//! | Lauf      | nur Herzfrequenz           | hrTSS       |
//! | Schwimmen | Geschwindigkeit            | sTSS        |
//! | Kraft     | RPE (Nutzereingabe)        | Session-RPE |
//! | Sonstige  | Herzfrequenz               | hrTSS       |
//!
//! Ist der Kanal vorhanden, aber die zugehörige Schwelle nicht, gibt es keinen
//! stillen Wechsel auf eine andere Methode, sondern `missing_threshold`
//! (Konvention: die Oberfläche soll den fehlenden Schwellenwert benennen, nicht
//! eine Last aus einer anderen Methode zeigen). Fehlende Daten erzeugen nie
//! eine Null, sondern einen [`LoadError`] (SPEC 5.4).
//!
//! Vor der Berechnung laufen die Plausibilitätsregeln aus SPEC 6.5
//! ([`check_plausibility`]). Verworfene Samples und Lücken werden gezählt und
//! im Ergebnis als [`Plausibility`] ausgewiesen.

use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Länge des Gleitmittelfensters für die normalisierte Leistung in Sekunden.
/// 30 s sind Teil der Definition von NP (Q16, Sekundärquelle) und deshalb keine
/// Einstellung in [`LoadConfig`].
pub const NP_WINDOW_S: usize = 30;

/// Sportart der Aktivität. Bestimmt zusammen mit den Messkanälen die Methode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sport {
    Cycling,
    Running,
    Swimming,
    Strength,
    Other,
}

/// Ein Messpunkt im 1-Hz-Raster nach Aufbereitung (SPEC 5.4).
///
/// `t_s` zählt Sekunden seit Start. Fehlende Werte sind `None`, nie Null.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub t_s: u32,
    pub power_w: Option<f32>,
    pub hr_bpm: Option<f32>,
    pub speed_mps: Option<f32>,
}

impl Sample {
    /// Sample ohne Messkanäle, nur Zeitpunkt.
    pub const fn at(t_s: u32) -> Self {
        Self {
            t_s,
            power_w: None,
            hr_bpm: None,
            speed_mps: None,
        }
    }
}

/// Eingang für die Belastungsberechnung (SPEC 5.4).
///
/// `pool_length_m` wird hier nur mitgeführt. sTSS rechnet mit dem
/// Geschwindigkeitskanal, die Beckenlänge braucht erst die Auswertung von
/// Bahnen (nicht Teil der Belastung).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityInput {
    pub sport: Sport,
    pub start_utc: DateTime<Utc>,
    /// Versatz der Ortszeit zu UTC in Sekunden, aus der FIT-Datei. Bestimmt das
    /// Tagesdatum, dem die Belastung zugerechnet wird.
    pub local_offset_s: i32,
    pub samples: Vec<Sample>,
    /// Anstrengung nach Foster (Q4), 1 bis 10. Nur für Kraft.
    pub rpe: Option<u8>,
    pub pool_length_m: Option<f32>,
}

impl ActivityInput {
    /// Tagesdatum der Aktivität in Ortszeit: Start in UTC plus `local_offset_s`.
    pub fn local_date(&self) -> NaiveDate {
        (self.start_utc + TimeDelta::seconds(i64::from(self.local_offset_s))).date_naive()
    }
}

/// Geschlecht, nur für die Wahl des TRIMP-Parametersatzes (Q17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sex {
    Male,
    Female,
}

/// Schwellenwerte, gültig ab `valid_from` (SPEC 5.4). Welche Schwellen gesetzt
/// sind, entscheidet der Nutzer. Fehlt die für die Methode nötige Schwelle,
/// meldet [`compute_load`] `missing_threshold`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Thresholds {
    pub valid_from: NaiveDate,
    pub ftp_w: Option<f32>,
    pub lthr_bpm: Option<f32>,
    pub hr_max_bpm: Option<f32>,
    pub hr_rest_bpm: Option<f32>,
    pub run_threshold_speed_mps: Option<f32>,
    pub css_mps: Option<f32>,
    pub sex: Option<Sex>,
}

impl Thresholds {
    /// Schwellenwerte ohne einen einzigen gesetzten Wert.
    pub const fn empty(valid_from: NaiveDate) -> Self {
        Self {
            valid_from,
            ftp_w: None,
            lthr_bpm: None,
            hr_max_bpm: None,
            hr_rest_bpm: None,
            run_threshold_speed_mps: None,
            css_mps: None,
            sex: None,
        }
    }
}

/// Berechnungsmethode der Belastung (SPEC 8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadMethod {
    Tss,
    HrTss,
    RTss,
    STss,
    SessionRpe,
}

/// Parameter der TRIMP-Gewichtung `x * a * exp(b * x)` nach Banister (Q17).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrimpParams {
    pub a: f32,
    pub b: f32,
}

/// Einstellbare Zahlen der Belastungsberechnung. Alle Werte sind Parameter,
/// damit eine Korrektur an einer Quelle den Rechenweg nicht ändert (SPEC 11.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadConfig {
    /// Leistung über dieser Grenze wird verworfen und die Aktivität markiert
    /// (SPEC 6.5, Faustregel).
    pub power_max_w: f32,
    /// Herzfrequenz unter dieser Grenze wird verworfen (SPEC 6.5, Faustregel).
    pub hr_min_bpm: f32,
    /// Herzfrequenz über dieser Grenze wird verworfen (SPEC 6.5, Faustregel).
    pub hr_max_bpm: f32,
    /// Längste Zeitlücke in Sekunden, die noch keine Pause ist (SPEC 6.5,
    /// Konvention). Längere Lücken trennen die Aktivität in Abschnitte.
    pub max_gap_s: u32,
    /// TRIMP-Parameter für Männer (Q17, nur Sekundärquelle, siehe SPEC O8).
    pub trimp_male: TrimpParams,
    /// TRIMP-Parameter für Frauen (Q17, nur Sekundärquelle, siehe SPEC O8).
    pub trimp_female: TrimpParams,
    /// Last je Stunde bei RPE 10 für die Umrechnung der Session-RPE auf die
    /// Lastskala (SPEC 8.1, Faustregel, einstellbar, offene Frage O14).
    pub session_rpe_load_per_hour: f32,
}

impl Default for LoadConfig {
    fn default() -> Self {
        Self {
            // SPEC 6.5: Standard 2500 W, Faustregel.
            power_max_w: 2500.0,
            // SPEC 6.5: 30 bis 230 bpm, Faustregel.
            hr_min_bpm: 30.0,
            hr_max_bpm: 230.0,
            // SPEC 6.5: Lücken bis 10 s sind Aussetzer, darüber Pausen. Konvention.
            max_gap_s: 10,
            // SPEC 8.1 nach Q17 (Sekundärquelle): Männer a = 0,64, b = 1,92.
            trimp_male: TrimpParams { a: 0.64, b: 1.92 },
            // SPEC 8.1 nach Q17 (Sekundärquelle): Frauen a = 0,86, b = 1,67.
            trimp_female: TrimpParams { a: 0.86, b: 1.67 },
            // SPEC 8.1: L = t_h * (RPE/10)^2 * 100. Faustregel.
            session_rpe_load_per_hour: 100.0,
        }
    }
}

/// Ergebnis der Plausibilitätsregeln aus SPEC 6.5. Wird je Aktivität gezählt
/// und mit der Belastung ausgewiesen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plausibility {
    /// Leistungswerte über `power_max_w`, negativ oder nicht endlich.
    pub discarded_power_samples: u32,
    /// Herzfrequenzwerte außerhalb `hr_min_bpm..=hr_max_bpm` oder nicht endlich.
    pub discarded_hr_samples: u32,
    /// Geschwindigkeitswerte negativ oder nicht endlich (Konvention, SPEC 6.5
    /// nennt für Geschwindigkeit keine Grenze).
    pub discarded_speed_samples: u32,
    /// Anzahl der Zeitlücken bis `max_gap_s`.
    pub short_gaps: u32,
    /// Fehlende Sekunden in kurzen Lücken zusammen.
    pub short_gap_s: u32,
    /// Anzahl der Zeitlücken über `max_gap_s` (Pausen).
    pub long_gaps: u32,
    /// Fehlende Sekunden in langen Lücken zusammen. Zählen nicht zur Dauer.
    pub long_gap_s: u32,
}

impl Plausibility {
    /// SPEC 6.5: Leistung über der Obergrenze markiert die Aktivität.
    pub const fn power_flagged(&self) -> bool {
        self.discarded_power_samples > 0
    }
}

/// Samples nach den Plausibilitätsregeln: in Abschnitte getrennt, unplausible
/// Kanalwerte auf `None` gesetzt.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedSamples {
    /// Zusammenhängende Abschnitte. Eine lange Lücke (über `max_gap_s`) trennt
    /// zwei Abschnitte, kurze Lücken bleiben innerhalb eines Abschnitts.
    pub segments: Vec<Vec<Sample>>,
    pub plausibility: Plausibility,
}

impl PreparedSamples {
    /// Dauer in Sekunden: je Abschnitt von erstem bis letztem Sample, kurze
    /// Lücken eingeschlossen, lange Lücken (Pausen) ausgeschlossen. Konvention.
    pub fn duration_s(&self) -> u32 {
        self.segments
            .iter()
            .filter_map(|seg| Some(seg.last()?.t_s - seg.first()?.t_s + 1))
            .sum()
    }
}

/// Belastung einer Aktivität (SPEC 5.4, um Kennzeichnung und Plausibilität
/// erweitert).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Load {
    /// Belastung auf der TSS-Skala: 100 je Stunde an der Schwelle.
    pub value: f32,
    pub method: LoadMethod,
    /// Intensitätsfaktor, nur bei Methoden mit Schwellenbezug über Leistung
    /// oder Geschwindigkeit.
    pub intensity_factor: Option<f32>,
    /// `true` bei Methoden, die nur als Faustregel belegt sind (rTSS, sTSS,
    /// Session-RPE). Die Oberfläche zeigt die Last dann als "geschätzt" (SPEC 8.1).
    pub estimated: bool,
    /// Dauer, die in die Berechnung eingeht, in Sekunden.
    pub duration_s: u32,
    pub plausibility: Plausibility,
}

/// Fehler der Belastungsberechnung (SPEC 5.4). Der Fehlercode ist über
/// [`LoadError::code`] erreichbar, der Inhalt nennt das fehlende Element.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "code", content = "detail", rename_all = "snake_case")]
pub enum LoadError {
    /// Die Methode ist durch die Messkanäle bestimmt, aber ihr Schwellenwert fehlt.
    #[error("missing_threshold: {0}")]
    MissingThreshold(String),
    /// Kein Messkanal, mit dem sich für diese Sportart rechnen lässt.
    #[error("missing_channel: {0}")]
    MissingChannel(String),
    /// Daten oder Schwellenwerte lassen keine sinnvolle Berechnung zu.
    #[error("implausible: {0}")]
    Implausible(String),
}

impl LoadError {
    /// Fehlercode als stabiler String für Speicher und Oberfläche.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::MissingThreshold(_) => "missing_threshold",
            Self::MissingChannel(_) => "missing_channel",
            Self::Implausible(_) => "implausible",
        }
    }
}

/// Schätzung der maximalen Herzfrequenz aus dem Alter: `208 - 0,7 * Alter` (Q7).
///
/// Das ist eine Schätzung mit großer Streuung zwischen Personen und ersetzt
/// keinen gemessenen Wert. Der Aufrufer kennzeichnet einen so gewonnenen Wert
/// als Schätzung (SPEC 8.6, T05).
pub fn estimate_hr_max(age_years: f32) -> f32 {
    208.0 - 0.7 * age_years
}

/// Normalisierte Leistung (Q16, Sekundärquelle): 30-Sekunden-Gleitmittel auf
/// 1-Hz-Daten, vierte Potenz, Mittelwert, vierte Wurzel.
///
/// Konvention zu Anfangsfenstern: Es zählen nur volle Fenster von 30 Werten.
/// Die ersten 29 Sekunden tragen also nur über die Fenster bei, in denen sie
/// liegen, und es gibt kein verkürztes Anfangsfenster. Bei weniger als 30
/// Werten gibt es `None`, keine Null.
///
/// Erwartet eine zusammenhängende Reihe. Lücken über `max_gap_s` trennt
/// [`check_plausibility`] vorher in Abschnitte (SPEC 6.5), die Fenster laufen
/// nicht über eine Pause hinweg.
pub fn normalized_power(p: &[f32]) -> Option<f32> {
    let (sum4, count) = np_fourth_power_sum(p)?;
    Some((sum4 / count as f64).powf(0.25) as f32)
}

/// Summe der vierten Potenzen der Fenstermittel und Anzahl der Fenster, damit
/// sich mehrere Abschnitte zu einer NP zusammenfassen lassen.
fn np_fourth_power_sum(p: &[f32]) -> Option<(f64, usize)> {
    if p.len() < NP_WINDOW_S {
        return None;
    }
    let mut window_sum: f64 = p[..NP_WINDOW_S].iter().map(|&v| f64::from(v)).sum();
    let mut sum4 = 0.0_f64;
    let mut count = 0_usize;
    let mut push = |window_sum: f64| {
        let mean = window_sum / NP_WINDOW_S as f64;
        sum4 += mean.powi(4);
        count += 1;
    };
    push(window_sum);
    for i in NP_WINDOW_S..p.len() {
        window_sum += f64::from(p[i]) - f64::from(p[i - NP_WINDOW_S]);
        push(window_sum);
    }
    Some((sum4, count))
}

/// Anstrengungseinheiten der Session-RPE nach Foster (Q4): `AU = RPE * t_min`.
pub fn session_rpe_au(rpe: u8, duration_s: u32) -> f32 {
    f32::from(rpe) * duration_s as f32 / 60.0
}

/// Plausibilitätsregeln aus SPEC 6.5, vor der Berechnung angewendet.
///
/// - Leistung über `power_max_w`, negativ oder nicht endlich: Wert verwerfen,
///   Sample behalten (Zeit und andere Kanäle bleiben), Aktivität markieren.
/// - Herzfrequenz außerhalb `hr_min_bpm..=hr_max_bpm`: Wert verwerfen.
/// - Zeitlücke bis `max_gap_s`: kein Abschnittswechsel. Leistung wird nicht mit
///   Null aufgefüllt, Herzfrequenz wird vom Rechenweg gehalten (siehe TRIMP).
/// - Zeitlücke über `max_gap_s`: Pause, neuer Abschnitt. NP-Fenster laufen nicht
///   darüber hinweg, die Pause zählt nicht zur Dauer.
///
/// Die Zeitstempel müssen streng steigen, sonst `implausible`. Eine Lücke ist
/// die Zahl der fehlenden Sekunden zwischen zwei Samples (`dt - 1`).
pub fn check_plausibility(
    samples: &[Sample],
    cfg: &LoadConfig,
) -> Result<PreparedSamples, LoadError> {
    let mut plausibility = Plausibility::default();

    // Erster Durchlauf nur über die Zeitstempel: Reihenfolge prüfen, Lücken zählen und die
    // Abschnittsgrenzen finden. Danach bekommt jeder Abschnitt einen Speicherblock passender
    // Größe. Das schrittweise Wachsen eines Vektors kostete beim Import großer Dateien den
    // Großteil der Laufzeit dieser Funktion (T38, Anhang Messung). Ergebnis und Zählung
    // sind dieselben wie beim Kopieren in einem Durchlauf.
    let mut starts: Vec<usize> = vec![0];
    for (i, pair) in samples.windows(2).enumerate() {
        let (prev_t, t) = (pair[0].t_s, pair[1].t_s);
        if t <= prev_t {
            return Err(LoadError::Implausible(format!(
                "t_s not strictly increasing at index {}",
                i + 1
            )));
        }
        let gap_s = t - prev_t - 1;
        if gap_s > cfg.max_gap_s {
            plausibility.long_gaps += 1;
            plausibility.long_gap_s += gap_s;
            starts.push(i + 1);
        } else if gap_s > 0 {
            plausibility.short_gaps += 1;
            plausibility.short_gap_s += gap_s;
        }
    }

    let mut segments: Vec<Vec<Sample>> = Vec::with_capacity(starts.len());
    if !samples.is_empty() {
        let ends = starts.iter().skip(1).copied().chain([samples.len()]);
        for (start, end) in starts.iter().copied().zip(ends) {
            segments.push(
                samples[start..end]
                    .iter()
                    .map(|raw| sanitize(*raw, cfg, &mut plausibility))
                    .collect(),
            );
        }
    }

    Ok(PreparedSamples {
        segments,
        plausibility,
    })
}

/// Setzt unplausible Kanalwerte eines Samples auf `None` und zählt sie (SPEC 6.5).
fn sanitize(mut s: Sample, cfg: &LoadConfig, plausibility: &mut Plausibility) -> Sample {
    if let Some(p) = s.power_w
        && !(p.is_finite() && p >= 0.0 && p <= cfg.power_max_w)
    {
        plausibility.discarded_power_samples += 1;
        s.power_w = None;
    }
    if let Some(hr) = s.hr_bpm
        && !(hr.is_finite() && hr >= cfg.hr_min_bpm && hr <= cfg.hr_max_bpm)
    {
        plausibility.discarded_hr_samples += 1;
        s.hr_bpm = None;
    }
    if let Some(v) = s.speed_mps
        && !(v.is_finite() && v >= 0.0)
    {
        plausibility.discarded_speed_samples += 1;
        s.speed_mps = None;
    }
    s
}

/// Belastung einer Aktivität nach SPEC 8.1. Methodenwahl siehe Modulkommentar.
pub fn compute_load(
    a: &ActivityInput,
    th: &Thresholds,
    cfg: &LoadConfig,
) -> Result<Load, LoadError> {
    if a.samples.is_empty() {
        return Err(LoadError::MissingChannel("samples".into()));
    }
    let prepared = check_plausibility(&a.samples, cfg)?;
    compute_load_prepared(a, &prepared, th, cfg)
}

/// Wie [`compute_load`], aber mit den schon geprüften Samples. `prepared` muss das Ergebnis
/// von [`check_plausibility`] für `a.samples` mit demselben `cfg` sein.
///
/// Der Import rechnet aus einer Aktivität Belastung, Zeit in Zonen und Bestleistungen. Jede
/// dieser Funktionen prüfte die Samples bisher selbst; bei großen Dateien war das der
/// größte Rechenposten (T38, Anhang Messung). Mit dieser Fassung prüft der Aufrufer einmal.
pub fn compute_load_prepared(
    a: &ActivityInput,
    prepared: &PreparedSamples,
    th: &Thresholds,
    cfg: &LoadConfig,
) -> Result<Load, LoadError> {
    if a.samples.is_empty() {
        return Err(LoadError::MissingChannel("samples".into()));
    }
    let has = |pick: fn(&Sample) -> bool| prepared.segments.iter().flatten().any(pick);
    let has_power = has(|s| s.power_w.is_some());
    let has_hr = has(|s| s.hr_bpm.is_some());
    let has_speed = has(|s| s.speed_mps.is_some());

    match a.sport {
        // Konvention: Leistung vor Herzfrequenz, weil TSS die belegtere Methode ist.
        Sport::Cycling if has_power => tss(prepared, th),
        Sport::Cycling if has_hr => hr_tss(prepared, th, cfg),
        Sport::Cycling => Err(LoadError::MissingChannel("power_w or hr_bpm".into())),
        // Konvention: Geschwindigkeit vor Herzfrequenz, weil rTSS den
        // Schwellenbezug der Sportart trägt und die Herzfrequenz nachläuft.
        Sport::Running if has_speed => r_tss(prepared, th),
        Sport::Running if has_hr => hr_tss(prepared, th, cfg),
        Sport::Running => Err(LoadError::MissingChannel("speed_mps or hr_bpm".into())),
        Sport::Swimming if has_speed => s_tss(prepared, th),
        Sport::Swimming => Err(LoadError::MissingChannel("speed_mps".into())),
        // RPE ist Nutzereingabe, kein Messkanal. Ihr Fehlen wird trotzdem als
        // fehlender Kanal gemeldet, weil kein Schwellenwert fehlt (Konvention).
        Sport::Strength => match a.rpe {
            Some(rpe) => session_rpe(prepared, rpe, cfg),
            None => Err(LoadError::MissingChannel("rpe".into())),
        },
        // Konvention: SPEC 8.1 nennt für Sonstige keine Methode. Mit
        // Herzfrequenz ist hrTSS die einzige sportartunabhängige Methode.
        Sport::Other if has_hr => hr_tss(prepared, th, cfg),
        Sport::Other => Err(LoadError::MissingChannel("hr_bpm".into())),
    }
}

/// Positive, endliche Schwelle oder Fehler. Null oder negativ wäre eine
/// Division durch Null beziehungsweise ein negativer Intensitätsfaktor.
fn positive_threshold(value: Option<f32>, name: &str) -> Result<f64, LoadError> {
    let v = value.ok_or_else(|| LoadError::MissingThreshold(name.into()))?;
    if v.is_finite() && v > 0.0 {
        Ok(f64::from(v))
    } else {
        Err(LoadError::Implausible(format!("{name} must be positive")))
    }
}

/// TSS (Q16, Sekundärquelle): `IF = NP / FTP`, `TSS = t_s * NP * IF / (FTP * 3600) * 100`.
///
/// NP wird über alle Abschnitte gemeinsam gebildet: Summe der vierten Potenzen
/// aller vollen Fenster geteilt durch ihre Anzahl. Fehlende Leistungswerte
/// innerhalb eines Abschnitts werden übersprungen, ein Fenster umfasst dann 30
/// gültige Werte statt 30 Sekunden (Konvention, SPEC 6.5: nicht mit Null
/// auffüllen). Die Dauer `t_s` umfasst den ganzen Abschnitt, auch Sekunden ohne
/// Leistungswert.
fn tss(prepared: &PreparedSamples, th: &Thresholds) -> Result<Load, LoadError> {
    let ftp = positive_threshold(th.ftp_w, "ftp_w")?;
    let mut sum4 = 0.0_f64;
    let mut count = 0_usize;
    for seg in &prepared.segments {
        let series: Vec<f32> = seg.iter().filter_map(|s| s.power_w).collect();
        if let Some((s4, c)) = np_fourth_power_sum(&series) {
            sum4 += s4;
            count += c;
        }
    }
    if count == 0 {
        return Err(LoadError::Implausible(format!(
            "no full {NP_WINDOW_S} s window of power samples"
        )));
    }
    let np = (sum4 / count as f64).powf(0.25);
    let intensity = np / ftp;
    let duration_s = prepared.duration_s();
    let value = f64::from(duration_s) * np * intensity / (ftp * 3600.0) * 100.0;
    Ok(Load {
        value: value as f32,
        method: LoadMethod::Tss,
        intensity_factor: Some(intensity as f32),
        estimated: false,
        duration_s,
        plausibility: prepared.plausibility,
    })
}

/// TRIMP-Gewicht je Minute bei relativer Herzfrequenzreserve `x` (Q17,
/// Sekundärquelle): `x * a * exp(b * x)`. `x` unter Null (Herzfrequenz unter
/// dem Ruhepuls) wird auf Null gesetzt, damit kein negativer Beitrag entsteht
/// (Konvention). Über Eins bleibt es erlaubt, ein gemessener Wert über
/// `hr_max_bpm` zählt dann stärker.
fn trimp_per_minute(x: f64, params: TrimpParams) -> f64 {
    let x = x.max(0.0);
    x * f64::from(params.a) * (f64::from(params.b) * x).exp()
}

/// hrTSS (SPEC 8.1): `TRIMP(Aktivität) / TRIMP(1 h bei LTHR) * 100`.
///
/// TRIMP summiert über Minuten `dt_min * x * a * exp(b * x)` mit
/// `x = (HR - HR_rest) / (HR_max - HR_rest)` (Q17, Sekundärquelle). Bei
/// 1-Hz-Samples ist `dt_min = 1/60` je Sample. Der Normierer ist
/// `60 * x_LTHR * a * exp(b * x_LTHR)`, die Normierung auf 100 je Stunde an der
/// LTHR ist Konvention. Fehlt das Geschlecht, gibt es `missing_threshold`,
/// es wird kein Mittel der Parametersätze gebildet (SPEC 8.1).
///
/// Kurze Lücken (SPEC 6.5): Der letzte gültige Herzfrequenzwert vor der Lücke
/// wird für die fehlenden Sekunden gehalten. Verworfene oder fehlende Werte
/// innerhalb des Rasters werden nicht gehalten, sondern übersprungen (Konvention).
fn hr_tss(
    prepared: &PreparedSamples,
    th: &Thresholds,
    cfg: &LoadConfig,
) -> Result<Load, LoadError> {
    let lthr = positive_threshold(th.lthr_bpm, "lthr_bpm")?;
    let hr_max = positive_threshold(th.hr_max_bpm, "hr_max_bpm")?;
    let hr_rest = positive_threshold(th.hr_rest_bpm, "hr_rest_bpm")?;
    let params = match th.sex {
        Some(Sex::Male) => cfg.trimp_male,
        Some(Sex::Female) => cfg.trimp_female,
        None => return Err(LoadError::MissingThreshold("sex".into())),
    };
    if hr_max <= hr_rest {
        return Err(LoadError::Implausible(
            "hr_max_bpm must exceed hr_rest_bpm".into(),
        ));
    }
    if lthr <= hr_rest {
        return Err(LoadError::Implausible(
            "lthr_bpm must exceed hr_rest_bpm".into(),
        ));
    }
    let reserve = hr_max - hr_rest;
    let x_of = |hr: f32| (f64::from(hr) - hr_rest) / reserve;
    let normalizer = 60.0 * trimp_per_minute((lthr - hr_rest) / reserve, params);

    let mut trimp = 0.0_f64;
    let mut hr_seconds = 0_u32;
    for seg in &prepared.segments {
        let mut prev: Option<&Sample> = None;
        for s in seg {
            if let Some(p) = prev
                && let Some(hr) = p.hr_bpm
            {
                let held_s = s.t_s - p.t_s - 1;
                trimp += f64::from(held_s) * trimp_per_minute(x_of(hr), params) / 60.0;
                hr_seconds += held_s;
            }
            if let Some(hr) = s.hr_bpm {
                trimp += trimp_per_minute(x_of(hr), params) / 60.0;
                hr_seconds += 1;
            }
            prev = Some(s);
        }
    }
    if hr_seconds == 0 {
        return Err(LoadError::MissingChannel("hr_bpm".into()));
    }
    Ok(Load {
        value: (trimp / normalizer * 100.0) as f32,
        method: LoadMethod::HrTss,
        intensity_factor: None,
        estimated: false,
        duration_s: prepared.duration_s(),
        plausibility: prepared.plausibility,
    })
}

/// Mittlere Geschwindigkeit über alle gültigen Werte aller Abschnitte.
fn mean_speed(prepared: &PreparedSamples) -> Option<f64> {
    let mut sum = 0.0_f64;
    let mut n = 0_u32;
    for v in prepared
        .segments
        .iter()
        .flatten()
        .filter_map(|s| s.speed_mps)
    {
        sum += f64::from(v);
        n += 1;
    }
    (n > 0).then(|| sum / f64::from(n))
}

/// Last aus Intensitätsfaktor und Dauer: `t_h * IF^exponent * 100`.
/// Gemeinsamer Rechenweg für rTSS (Exponent 2) und sTSS (Exponent 3).
fn speed_based_load(
    prepared: &PreparedSamples,
    threshold_mps: f64,
    exponent: i32,
    method: LoadMethod,
) -> Result<Load, LoadError> {
    let v = mean_speed(prepared).ok_or_else(|| LoadError::MissingChannel("speed_mps".into()))?;
    let intensity = v / threshold_mps;
    let duration_s = prepared.duration_s();
    let t_h = f64::from(duration_s) / 3600.0;
    Ok(Load {
        value: (t_h * intensity.powi(exponent) * 100.0) as f32,
        method,
        intensity_factor: Some(intensity as f32),
        // Faustregel ohne begutachtete Quelle, SPEC 8.1.
        estimated: true,
        duration_s,
        plausibility: prepared.plausibility,
    })
}

/// rTSS (SPEC 8.1, Faustregel, ohne Steigungskorrektur):
/// `IF = v / v_Schwelle`, `rTSS = t_h * IF^2 * 100` mit `v` als mittlerer
/// Geschwindigkeit der Aktivität.
fn r_tss(prepared: &PreparedSamples, th: &Thresholds) -> Result<Load, LoadError> {
    let v_th = positive_threshold(th.run_threshold_speed_mps, "run_threshold_speed_mps")?;
    speed_based_load(prepared, v_th, 2, LoadMethod::RTss)
}

/// sTSS (SPEC 8.1, Faustregel): `IF = v / CSS`, `sTSS = t_h * IF^3 * 100`.
/// CSS nach Q15.
fn s_tss(prepared: &PreparedSamples, th: &Thresholds) -> Result<Load, LoadError> {
    let css = positive_threshold(th.css_mps, "css_mps")?;
    speed_based_load(prepared, css, 3, LoadMethod::STss)
}

/// Session-RPE (Q4) auf der Lastskala: `L = t_h * (RPE/10)^2 * Last_je_Stunde`
/// mit `Last_je_Stunde` aus [`LoadConfig`] (SPEC 8.1, Faustregel, O14). Die
/// Anstrengungseinheiten `AU` liefert [`session_rpe_au`].
fn session_rpe(prepared: &PreparedSamples, rpe: u8, cfg: &LoadConfig) -> Result<Load, LoadError> {
    if !(1..=10).contains(&rpe) {
        return Err(LoadError::Implausible(format!("rpe {rpe} outside 1..=10")));
    }
    let duration_s = prepared.duration_s();
    let t_h = f64::from(duration_s) / 3600.0;
    let rel = f64::from(rpe) / 10.0;
    Ok(Load {
        value: (t_h * rel * rel * f64::from(cfg.session_rpe_load_per_hour)) as f32,
        method: LoadMethod::SessionRpe,
        intensity_factor: None,
        estimated: true,
        duration_s,
        plausibility: prepared.plausibility,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use proptest::prelude::*;

    /// Toleranz für handgerechnete Werte in f32.
    const EPS: f32 = 1e-3;

    fn assert_close(actual: f32, expected: f32, tol: f32) {
        assert!(
            (actual - expected).abs() <= tol,
            "expected {expected}, got {actual} (tolerance {tol})"
        );
    }

    fn activity(sport: Sport, samples: Vec<Sample>) -> ActivityInput {
        ActivityInput {
            sport,
            start_utc: Utc.with_ymd_and_hms(2026, 3, 1, 6, 30, 0).unwrap(),
            local_offset_s: 3600,
            samples,
            rpe: None,
            pool_length_m: None,
        }
    }

    fn thresholds() -> Thresholds {
        Thresholds::empty(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
    }

    fn power_samples(n: u32, power_w: f32) -> Vec<Sample> {
        (0..n)
            .map(|t| Sample {
                power_w: Some(power_w),
                ..Sample::at(t)
            })
            .collect()
    }

    fn hr_samples(range: std::ops::Range<u32>, hr_bpm: f32) -> Vec<Sample> {
        range
            .map(|t| Sample {
                hr_bpm: Some(hr_bpm),
                ..Sample::at(t)
            })
            .collect()
    }

    fn speed_samples(n: u32, speed_mps: f32) -> Vec<Sample> {
        (0..n)
            .map(|t| Sample {
                speed_mps: Some(speed_mps),
                ..Sample::at(t)
            })
            .collect()
    }

    /// Schwellen für hrTSS: Ruhepuls 50, Maximum 190, LTHR 160, Mann.
    fn hr_thresholds(sex: Option<Sex>) -> Thresholds {
        Thresholds {
            lthr_bpm: Some(160.0),
            hr_max_bpm: Some(190.0),
            hr_rest_bpm: Some(50.0),
            sex,
            ..thresholds()
        }
    }

    // ---- Referenzwerte aus SPEC 11.2 ----------------------------------------

    /// T-NP-1: 3600 s konstant 200 W. Gleitmittel konstant 200, vierte Wurzel
    /// des Mittels von 200^4 ergibt 200.
    const T_NP_1_EXPECTED_W: f32 = 200.0;

    #[test]
    fn t_np_1_constant_power() {
        let p = vec![200.0_f32; 3600];
        assert_close(normalized_power(&p).unwrap(), T_NP_1_EXPECTED_W, EPS);
    }

    /// T-NP-2: Wechsel 30 s mit 100 W und 30 s mit 300 W, lange Dauer. Das
    /// Gleitmittel läuft linear zwischen 100 und 300 W. Im stetigen Grenzfall
    /// ist das Mittel der vierten Potenz `(300^5 - 100^5) / (5 * 200) = 2,42e9`,
    /// die vierte Wurzel 221,8 W. Toleranz 2 Prozent bei 1 Hz (diskret ergibt
    /// sich 221,85 W).
    const T_NP_2_EXPECTED_W: f32 = 221.8;

    #[test]
    fn t_np_2_alternating_power() {
        let p: Vec<f32> = (0..3600)
            .map(|t| if (t / 30) % 2 == 0 { 100.0 } else { 300.0 })
            .collect();
        let np = normalized_power(&p).unwrap();
        assert_close(np, T_NP_2_EXPECTED_W, T_NP_2_EXPECTED_W * 0.02);
    }

    proptest! {
        /// T-NP-3: Eigenschaft der Potenzmittelung, `Mittelwert <= NP <= Maximum`.
        ///
        /// Die untere Schranke ist der Mittelwert der Fenstermittel, also der
        /// Werte, über die die Potenzmittelung läuft. Der rohe Mittelwert aller
        /// Samples ist keine gültige Schranke, weil Randsamples in weniger
        /// Fenstern liegen: 1000 W gefolgt von 30 mal 0 W hat den rohen
        /// Mittelwert 32,3 W, aber NP 28,0 W.
        #[test]
        fn t_np_3_np_between_mean_and_max(
            p in prop::collection::vec(0.0_f32..=2500.0, NP_WINDOW_S..400)
        ) {
            let np = normalized_power(&p).unwrap();
            let window_means: Vec<f64> = p
                .windows(NP_WINDOW_S)
                .map(|w| w.iter().map(|&v| f64::from(v)).sum::<f64>() / NP_WINDOW_S as f64)
                .collect();
            let mean = window_means.iter().sum::<f64>() / window_means.len() as f64;
            let max = p.iter().copied().fold(0.0_f32, f32::max);
            let tol = 1e-3;
            prop_assert!(f64::from(np) >= mean - tol, "mean {mean} > np {np}");
            prop_assert!(np <= max + tol as f32, "np {np} > max {max}");
        }
    }

    /// T-TSS-1: wie T-NP-1, FTP 250 W. `IF = 200 / 250 = 0,8`,
    /// `TSS = 3600 * 200 * 0,8 / (250 * 3600) * 100 = 64`.
    const T_TSS_1_EXPECTED_IF: f32 = 0.8;
    const T_TSS_1_EXPECTED_TSS: f32 = 64.0;

    #[test]
    fn t_tss_1_one_hour_below_ftp() {
        let a = activity(Sport::Cycling, power_samples(3600, 200.0));
        let th = Thresholds {
            ftp_w: Some(250.0),
            ..thresholds()
        };
        let load = compute_load(&a, &th, &LoadConfig::default()).unwrap();
        assert_eq!(load.method, LoadMethod::Tss);
        assert!(!load.estimated);
        assert_eq!(load.duration_s, 3600);
        assert_close(load.intensity_factor.unwrap(), T_TSS_1_EXPECTED_IF, EPS);
        assert_close(load.value, T_TSS_1_EXPECTED_TSS, EPS);
        assert_eq!(load.plausibility, Plausibility::default());
    }

    /// T-TSS-2: 1800 s konstant 250 W, FTP 250 W. `IF = 1`,
    /// `TSS = 1800 * 250 * 1 / (250 * 3600) * 100 = 50`.
    const T_TSS_2_EXPECTED_IF: f32 = 1.0;
    const T_TSS_2_EXPECTED_TSS: f32 = 50.0;

    #[test]
    fn t_tss_2_half_hour_at_ftp() {
        let a = activity(Sport::Cycling, power_samples(1800, 250.0));
        let th = Thresholds {
            ftp_w: Some(250.0),
            ..thresholds()
        };
        let load = compute_load(&a, &th, &LoadConfig::default()).unwrap();
        assert_close(load.intensity_factor.unwrap(), T_TSS_2_EXPECTED_IF, EPS);
        assert_close(load.value, T_TSS_2_EXPECTED_TSS, EPS);
    }

    /// T-RTSS-1: 3600 s bei Schwellengeschwindigkeit. `IF = 1`,
    /// `rTSS = 1 * 1^2 * 100 = 100`.
    const T_RTSS_1_EXPECTED: f32 = 100.0;

    #[test]
    fn t_rtss_1_one_hour_at_threshold_speed() {
        let a = activity(Sport::Running, speed_samples(3600, 4.0));
        let th = Thresholds {
            run_threshold_speed_mps: Some(4.0),
            ..thresholds()
        };
        let load = compute_load(&a, &th, &LoadConfig::default()).unwrap();
        assert_eq!(load.method, LoadMethod::RTss);
        assert!(load.estimated, "rTSS ist Faustregel und damit geschätzt");
        assert_close(load.intensity_factor.unwrap(), 1.0, EPS);
        assert_close(load.value, T_RTSS_1_EXPECTED, EPS);
    }

    /// T-STSS-1: 1800 s mit IF = 0,9. `sTSS = 0,5 * 0,9^3 * 100 = 36,45`.
    /// Umsetzung: CSS 1,0 m/s, Geschwindigkeit 0,9 m/s.
    const T_STSS_1_EXPECTED: f32 = 36.45;

    #[test]
    fn t_stss_1_half_hour_at_if_0_9() {
        let a = activity(Sport::Swimming, speed_samples(1800, 0.9));
        let th = Thresholds {
            css_mps: Some(1.0),
            ..thresholds()
        };
        let load = compute_load(&a, &th, &LoadConfig::default()).unwrap();
        assert_eq!(load.method, LoadMethod::STss);
        assert!(load.estimated, "sTSS ist Faustregel und damit geschätzt");
        assert_close(load.intensity_factor.unwrap(), 0.9, EPS);
        assert_close(load.value, T_STSS_1_EXPECTED, EPS);
    }

    /// T-SRPE-1: 60 min, RPE 6. `AU = 6 * 60 = 360`,
    /// Verlaufslast `L = 1 * (6/10)^2 * 100 = 1 * 0,36 * 100 = 36`.
    const T_SRPE_1_EXPECTED_AU: f32 = 360.0;
    const T_SRPE_1_EXPECTED_LOAD: f32 = 36.0;

    #[test]
    fn t_srpe_1_one_hour_rpe_6() {
        let mut a = activity(Sport::Strength, (0..3600).map(Sample::at).collect());
        a.rpe = Some(6);
        let load = compute_load(&a, &thresholds(), &LoadConfig::default()).unwrap();
        assert_eq!(load.method, LoadMethod::SessionRpe);
        assert!(
            load.estimated,
            "Umrechnung auf die Lastskala ist Faustregel"
        );
        assert_eq!(load.intensity_factor, None);
        assert_close(load.value, T_SRPE_1_EXPECTED_LOAD, EPS);
        assert_close(
            session_rpe_au(6, load.duration_s),
            T_SRPE_1_EXPECTED_AU,
            EPS,
        );
    }

    /// T-HRMAX-1: Alter 40. `208 - 0,7 * 40 = 208 - 28 = 180`.
    const T_HRMAX_1_EXPECTED_BPM: f32 = 180.0;

    #[test]
    fn t_hrmax_1_age_40() {
        assert_close(estimate_hr_max(40.0), T_HRMAX_1_EXPECTED_BPM, EPS);
    }

    // ---- hrTSS: Normierung (Konvention) und Handrechnung -------------------

    /// Konvention aus SPEC 8.1: Eine Stunde konstant an der LTHR ergibt
    /// hrTSS = 100, unabhängig von den TRIMP-Konstanten. Rechenweg: 3600
    /// Samples je `1/60 * x_L * a * exp(b * x_L)` summieren sich zu
    /// `60 * x_L * a * exp(b * x_L)`, das ist genau der Normierer.
    const HRTSS_ONE_HOUR_AT_LTHR: f32 = 100.0;

    #[test]
    fn hrtss_one_hour_at_lthr_is_100_for_any_constants() {
        let a = activity(Sport::Running, hr_samples(0..3600, 160.0));
        for (sex, cfg) in [
            (Sex::Male, LoadConfig::default()),
            (Sex::Female, LoadConfig::default()),
            (
                Sex::Male,
                LoadConfig {
                    trimp_male: TrimpParams { a: 1.5, b: 0.3 },
                    ..LoadConfig::default()
                },
            ),
        ] {
            let load = compute_load(&a, &hr_thresholds(Some(sex)), &cfg).unwrap();
            assert_eq!(load.method, LoadMethod::HrTss);
            assert!(!load.estimated);
            assert_eq!(load.intensity_factor, None);
            assert_close(load.value, HRTSS_ONE_HOUR_AT_LTHR, EPS);
        }
    }

    /// Handrechnung mit den Konstanten aus Q17 (Sekundärquelle): Mann, Ruhepuls
    /// 50, Maximum 190, LTHR 160, 30 min bei 120 bpm.
    /// `x = (120 - 50) / 140 = 0,5`, je Minute `0,5 * 0,64 * exp(0,96) =
    /// 0,32 * 2,611696 = 0,835743`, TRIMP = 30 * 0,835743 = 25,07229.
    /// `x_L = 110 / 140 = 0,785714`, Normierer `60 * 0,785714 * 0,64 * exp(1,508571)
    /// = 30,171429 * 4,520269 = 136,38296`. hrTSS = 25,07229 / 136,38296 * 100
    /// = 18,3837.
    const HRTSS_MALE_30MIN_AT_120: f32 = 18.3837;

    /// Dieselben Daten mit den Konstanten für Frauen: je Minute
    /// `0,5 * 0,86 * exp(0,835) = 0,991070`, TRIMP = 29,73210, Normierer
    /// `60 * 0,785714 * 0,86 * exp(1,312143) = 150,58120`, hrTSS = 19,7449.
    const HRTSS_FEMALE_30MIN_AT_120: f32 = 19.7449;

    #[test]
    fn hrtss_hand_calculated_male_and_female() {
        let a = activity(Sport::Cycling, hr_samples(0..1800, 120.0));
        let cfg = LoadConfig::default();
        let male = compute_load(&a, &hr_thresholds(Some(Sex::Male)), &cfg).unwrap();
        assert_close(male.value, HRTSS_MALE_30MIN_AT_120, EPS);
        let female = compute_load(&a, &hr_thresholds(Some(Sex::Female)), &cfg).unwrap();
        assert_close(female.value, HRTSS_FEMALE_30MIN_AT_120, EPS);
    }

    #[test]
    fn hrtss_holds_heart_rate_over_short_gap() {
        // 1795 Samples an der LTHR plus 5 gehaltene Sekunden ergeben 3600 s
        // Herzfrequenz: hrTSS bleibt 100 (Konvention aus SPEC 6.5).
        let mut samples = hr_samples(0..1800, 160.0);
        samples.extend(hr_samples(1805..3600, 160.0));
        let a = activity(Sport::Running, samples);
        let load =
            compute_load(&a, &hr_thresholds(Some(Sex::Male)), &LoadConfig::default()).unwrap();
        assert_close(load.value, HRTSS_ONE_HOUR_AT_LTHR, EPS);
        assert_eq!(load.duration_s, 3600);
        assert_eq!(load.plausibility.short_gaps, 1);
        assert_eq!(load.plausibility.short_gap_s, 5);
    }

    #[test]
    fn hrtss_clamps_heart_rate_below_rest_to_zero() {
        // 40 bpm liegt unter dem Ruhepuls 50: Beitrag Null statt negativ.
        let a = activity(Sport::Other, hr_samples(0..600, 40.0));
        let load =
            compute_load(&a, &hr_thresholds(Some(Sex::Male)), &LoadConfig::default()).unwrap();
        assert_close(load.value, 0.0, EPS);
    }

    // ---- Negativtests --------------------------------------------------------

    #[test]
    fn missing_ftp_is_missing_threshold_without_fallback_to_hr() {
        // Leistung und Herzfrequenz vorhanden, FTP fehlt: kein stiller Wechsel
        // auf hrTSS (Konvention), sondern missing_threshold.
        let samples: Vec<Sample> = (0..3600)
            .map(|t| Sample {
                power_w: Some(200.0),
                hr_bpm: Some(150.0),
                ..Sample::at(t)
            })
            .collect();
        let a = activity(Sport::Cycling, samples);
        let err =
            compute_load(&a, &hr_thresholds(Some(Sex::Male)), &LoadConfig::default()).unwrap_err();
        assert_eq!(err.code(), "missing_threshold");
        assert_eq!(err, LoadError::MissingThreshold("ftp_w".into()));
    }

    #[test]
    fn missing_run_threshold_is_missing_threshold() {
        let a = activity(Sport::Running, speed_samples(600, 3.5));
        let err = compute_load(&a, &thresholds(), &LoadConfig::default()).unwrap_err();
        assert_eq!(err.code(), "missing_threshold");
    }

    #[test]
    fn missing_css_is_missing_threshold() {
        let a = activity(Sport::Swimming, speed_samples(600, 1.0));
        let err = compute_load(&a, &thresholds(), &LoadConfig::default()).unwrap_err();
        assert_eq!(err.code(), "missing_threshold");
    }

    #[test]
    fn missing_sex_for_hrtss_is_missing_threshold() {
        let a = activity(Sport::Running, hr_samples(0..600, 150.0));
        let err = compute_load(&a, &hr_thresholds(None), &LoadConfig::default()).unwrap_err();
        assert_eq!(err, LoadError::MissingThreshold("sex".into()));
    }

    #[test]
    fn missing_hr_max_for_hrtss_is_missing_threshold() {
        let a = activity(Sport::Cycling, hr_samples(0..600, 150.0));
        let th = Thresholds {
            hr_max_bpm: None,
            ..hr_thresholds(Some(Sex::Female))
        };
        let err = compute_load(&a, &th, &LoadConfig::default()).unwrap_err();
        assert_eq!(err, LoadError::MissingThreshold("hr_max_bpm".into()));
    }

    #[test]
    fn no_channel_is_missing_channel() {
        let th = Thresholds {
            ftp_w: Some(250.0),
            ..hr_thresholds(Some(Sex::Male))
        };
        let cfg = LoadConfig::default();
        let blank: Vec<Sample> = (0..600).map(Sample::at).collect();
        for sport in [
            Sport::Cycling,
            Sport::Running,
            Sport::Swimming,
            Sport::Other,
        ] {
            let err = compute_load(&activity(sport, blank.clone()), &th, &cfg).unwrap_err();
            assert_eq!(err.code(), "missing_channel", "{sport:?}");
        }
        // Schwimmen rechnet nur mit Geschwindigkeit, Herzfrequenz reicht nicht.
        let swim = activity(Sport::Swimming, hr_samples(0..600, 150.0));
        assert_eq!(
            compute_load(&swim, &th, &cfg).unwrap_err().code(),
            "missing_channel"
        );
        // Kraft ohne RPE.
        let strength = activity(Sport::Strength, blank);
        assert_eq!(
            compute_load(&strength, &th, &cfg).unwrap_err(),
            LoadError::MissingChannel("rpe".into())
        );
        // Ohne Samples.
        let empty = activity(Sport::Cycling, Vec::new());
        assert_eq!(
            compute_load(&empty, &th, &cfg).unwrap_err().code(),
            "missing_channel"
        );
    }

    #[test]
    fn non_increasing_timestamps_are_implausible() {
        let mut samples = power_samples(60, 200.0);
        samples[10].t_s = 9;
        let a = activity(Sport::Cycling, samples);
        let th = Thresholds {
            ftp_w: Some(250.0),
            ..thresholds()
        };
        let err = compute_load(&a, &th, &LoadConfig::default()).unwrap_err();
        assert_eq!(err.code(), "implausible");
    }

    #[test]
    fn too_short_for_np_window_is_implausible() {
        let a = activity(Sport::Cycling, power_samples(29, 200.0));
        let th = Thresholds {
            ftp_w: Some(250.0),
            ..thresholds()
        };
        let err = compute_load(&a, &th, &LoadConfig::default()).unwrap_err();
        assert_eq!(err.code(), "implausible");
        assert_eq!(normalized_power(&[200.0; 29]), None);
    }

    #[test]
    fn rpe_outside_range_is_implausible() {
        let mut a = activity(Sport::Strength, (0..600).map(Sample::at).collect());
        a.rpe = Some(11);
        let err = compute_load(&a, &thresholds(), &LoadConfig::default()).unwrap_err();
        assert_eq!(err.code(), "implausible");
    }

    #[test]
    fn hr_max_not_above_rest_is_implausible() {
        let a = activity(Sport::Running, hr_samples(0..600, 150.0));
        let th = Thresholds {
            hr_max_bpm: Some(50.0),
            ..hr_thresholds(Some(Sex::Male))
        };
        let err = compute_load(&a, &th, &LoadConfig::default()).unwrap_err();
        assert_eq!(err.code(), "implausible");
    }

    #[test]
    fn zero_ftp_is_implausible() {
        let a = activity(Sport::Cycling, power_samples(60, 200.0));
        let th = Thresholds {
            ftp_w: Some(0.0),
            ..thresholds()
        };
        let err = compute_load(&a, &th, &LoadConfig::default()).unwrap_err();
        assert_eq!(err.code(), "implausible");
    }

    // ---- Plausibilitätsregeln SPEC 6.5 --------------------------------------

    #[test]
    fn discards_power_above_limit_and_flags_activity() {
        let mut samples = power_samples(3600, 200.0);
        samples[100].power_w = Some(3000.0);
        samples[200].power_w = Some(f32::NAN);
        samples[300].power_w = Some(-5.0);
        let a = activity(Sport::Cycling, samples);
        let th = Thresholds {
            ftp_w: Some(250.0),
            ..thresholds()
        };
        let load = compute_load(&a, &th, &LoadConfig::default()).unwrap();
        assert_eq!(load.plausibility.discarded_power_samples, 3);
        assert!(load.plausibility.power_flagged());
        // Verworfene Werte werden übersprungen, nicht mit Null gefüllt: NP bleibt 200.
        assert_close(load.intensity_factor.unwrap(), 0.8, EPS);
        assert_eq!(load.duration_s, 3600);
    }

    #[test]
    fn discards_heart_rate_outside_30_to_230() {
        let mut samples = hr_samples(0..3600, 160.0);
        samples[0].hr_bpm = Some(29.0);
        samples[1].hr_bpm = Some(231.0);
        samples[2].hr_bpm = Some(30.0);
        samples[3].hr_bpm = Some(230.0);
        let prepared = check_plausibility(&samples, &LoadConfig::default()).unwrap();
        assert_eq!(prepared.plausibility.discarded_hr_samples, 2);
        assert_eq!(prepared.segments.len(), 1);
        assert_eq!(prepared.segments[0][0].hr_bpm, None);
        assert_eq!(prepared.segments[0][2].hr_bpm, Some(30.0));
    }

    #[test]
    fn discards_negative_speed() {
        let mut samples = speed_samples(600, 3.0);
        samples[5].speed_mps = Some(-1.0);
        let prepared = check_plausibility(&samples, &LoadConfig::default()).unwrap();
        assert_eq!(prepared.plausibility.discarded_speed_samples, 1);
    }

    #[test]
    fn long_gap_splits_np_window_short_gap_does_not() {
        let th = Thresholds {
            ftp_w: Some(250.0),
            ..thresholds()
        };
        let cfg = LoadConfig::default();
        // 29 Werte, Pause von 20 s, 29 Werte: kein volles Fenster.
        let mut long = power_samples(29, 200.0);
        long.extend((49..78).map(|t| Sample {
            power_w: Some(200.0),
            ..Sample::at(t)
        }));
        let err = compute_load(&activity(Sport::Cycling, long), &th, &cfg).unwrap_err();
        assert_eq!(err.code(), "implausible");
        // 29 Werte, Lücke von 5 s, 29 Werte: ein Abschnitt, Fenster über 58 Werte.
        let mut short = power_samples(29, 200.0);
        short.extend((34..63).map(|t| Sample {
            power_w: Some(200.0),
            ..Sample::at(t)
        }));
        let load = compute_load(&activity(Sport::Cycling, short), &th, &cfg).unwrap();
        assert_close(load.intensity_factor.unwrap(), 0.8, EPS);
        assert_eq!(load.duration_s, 63);
        assert_eq!(load.plausibility.short_gaps, 1);
        assert_eq!(load.plausibility.short_gap_s, 5);
        assert_eq!(load.plausibility.long_gaps, 0);
    }

    #[test]
    fn long_gap_is_a_pause_and_not_part_of_duration() {
        // Zwei mal 1800 s bei FTP mit 100 s Pause: Dauer 3600 s, TSS 100.
        let mut samples = power_samples(1800, 250.0);
        samples.extend((1900..3700).map(|t| Sample {
            power_w: Some(250.0),
            ..Sample::at(t)
        }));
        let a = activity(Sport::Cycling, samples);
        let th = Thresholds {
            ftp_w: Some(250.0),
            ..thresholds()
        };
        let load = compute_load(&a, &th, &LoadConfig::default()).unwrap();
        assert_eq!(load.duration_s, 3600);
        assert_close(load.value, 100.0, EPS);
        assert_eq!(load.plausibility.long_gaps, 1);
        assert_eq!(load.plausibility.long_gap_s, 100);
        assert_eq!(load.plausibility.short_gaps, 0);
    }

    #[test]
    fn gap_of_exactly_max_gap_is_short() {
        let cfg = LoadConfig::default();
        let samples = [Sample::at(0), Sample::at(11), Sample::at(23)];
        let prepared = check_plausibility(&samples, &cfg).unwrap();
        assert_eq!(prepared.plausibility.short_gaps, 1);
        assert_eq!(prepared.plausibility.short_gap_s, 10);
        assert_eq!(prepared.plausibility.long_gaps, 1);
        assert_eq!(prepared.plausibility.long_gap_s, 11);
        assert_eq!(prepared.segments.len(), 2);
        assert_eq!(prepared.duration_s(), 12 + 1);
    }

    // ---- Methodenwahl ---------------------------------------------------------

    #[test]
    fn running_prefers_speed_over_heart_rate() {
        let samples: Vec<Sample> = (0..3600)
            .map(|t| Sample {
                speed_mps: Some(4.0),
                hr_bpm: Some(160.0),
                ..Sample::at(t)
            })
            .collect();
        let a = activity(Sport::Running, samples);
        let th = Thresholds {
            run_threshold_speed_mps: Some(4.0),
            ..hr_thresholds(Some(Sex::Male))
        };
        let load = compute_load(&a, &th, &LoadConfig::default()).unwrap();
        assert_eq!(load.method, LoadMethod::RTss);
    }

    #[test]
    fn cycling_without_power_uses_heart_rate() {
        let a = activity(Sport::Cycling, hr_samples(0..3600, 160.0));
        let load =
            compute_load(&a, &hr_thresholds(Some(Sex::Male)), &LoadConfig::default()).unwrap();
        assert_eq!(load.method, LoadMethod::HrTss);
        assert_close(load.value, HRTSS_ONE_HOUR_AT_LTHR, EPS);
    }

    // ---- Typen ------------------------------------------------------------------

    #[test]
    fn local_date_uses_offset() {
        // 23:30 UTC mit Versatz plus zwei Stunden liegt am Folgetag.
        let mut a = activity(Sport::Other, Vec::new());
        a.start_utc = Utc.with_ymd_and_hms(2026, 3, 1, 23, 30, 0).unwrap();
        a.local_offset_s = 7200;
        assert_eq!(a.local_date(), NaiveDate::from_ymd_opt(2026, 3, 2).unwrap());
        a.local_offset_s = -3600;
        assert_eq!(a.local_date(), NaiveDate::from_ymd_opt(2026, 3, 1).unwrap());
    }

    #[test]
    fn load_error_codes_are_stable_strings() {
        let cases = [
            (
                LoadError::MissingThreshold("ftp_w".into()),
                "missing_threshold",
            ),
            (
                LoadError::MissingChannel("hr_bpm".into()),
                "missing_channel",
            ),
            (LoadError::Implausible("x".into()), "implausible"),
        ];
        for (err, code) in cases {
            assert_eq!(err.code(), code);
            assert!(err.to_string().starts_with(code), "{err}");
        }
    }

    /// Bisherige Fassung von [`check_plausibility`] in einem Durchlauf (vor T38), nur als
    /// Referenz für den Gleichheitstest.
    fn check_plausibility_reference(
        samples: &[Sample],
        cfg: &LoadConfig,
    ) -> Result<PreparedSamples, LoadError> {
        let mut plausibility = Plausibility::default();
        let mut segments: Vec<Vec<Sample>> = Vec::new();
        let mut current: Vec<Sample> = Vec::new();
        for (i, raw) in samples.iter().enumerate() {
            if i > 0 {
                let prev_t = samples[i - 1].t_s;
                if raw.t_s <= prev_t {
                    return Err(LoadError::Implausible(format!(
                        "t_s not strictly increasing at index {i}"
                    )));
                }
                let gap_s = raw.t_s - prev_t - 1;
                if gap_s > cfg.max_gap_s {
                    plausibility.long_gaps += 1;
                    plausibility.long_gap_s += gap_s;
                    segments.push(std::mem::take(&mut current));
                } else if gap_s > 0 {
                    plausibility.short_gaps += 1;
                    plausibility.short_gap_s += gap_s;
                }
            }
            let mut s = *raw;
            if let Some(p) = s.power_w
                && !(p.is_finite() && p >= 0.0 && p <= cfg.power_max_w)
            {
                plausibility.discarded_power_samples += 1;
                s.power_w = None;
            }
            if let Some(hr) = s.hr_bpm
                && !(hr.is_finite() && hr >= cfg.hr_min_bpm && hr <= cfg.hr_max_bpm)
            {
                plausibility.discarded_hr_samples += 1;
                s.hr_bpm = None;
            }
            if let Some(v) = s.speed_mps
                && !(v.is_finite() && v >= 0.0)
            {
                plausibility.discarded_speed_samples += 1;
                s.speed_mps = None;
            }
            current.push(s);
        }
        if !current.is_empty() {
            segments.push(current);
        }
        Ok(PreparedSamples {
            segments,
            plausibility,
        })
    }

    /// Linearer Kongruenzgenerator (Konstanten nach Knuth, MMIX) für wiederholbare
    /// Testreihen ohne Zufallsquelle.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> u32 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 32) as u32
        }
    }

    /// Synthetische Reihe mit kurzen und langen Lücken, fehlenden, unplausiblen und nicht
    /// endlichen Werten. Mit `disorder` steht irgendwo ein Zeitstempel nicht steigend.
    fn noisy_series(rng: &mut Lcg, n: usize, disorder: bool) -> Vec<Sample> {
        let pick = |rng: &mut Lcg, normal: f32, bad: [f32; 3]| -> Option<f32> {
            match rng.next() % 20 {
                0 => None,
                1 => Some(bad[(rng.next() % 3) as usize]),
                _ => Some(normal + (rng.next() % 100) as f32),
            }
        };
        let mut t = rng.next() % 5;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            out.push(Sample {
                t_s: t,
                power_w: pick(rng, 150.0, [3000.0, -5.0, f32::NAN]),
                hr_bpm: pick(rng, 100.0, [20.0, 250.0, f32::INFINITY]),
                speed_mps: pick(rng, 2.0, [-1.0, f32::NEG_INFINITY, f32::NAN]),
            });
            t += match rng.next() % 50 {
                0 => 11 + rng.next() % 300,
                1..=3 => 2 + rng.next() % 10,
                _ => 1,
            };
            if disorder && i == n / 2 {
                t = t.saturating_sub(5 + rng.next() % 20);
            }
        }
        out
    }

    /// Die Fassung mit zwei Durchläufen liefert dieselben Abschnitte, Werte (bitgleich, über
    /// die Debug-Ausgabe verglichen, die auch -0,0 unterscheidet), Zählungen und Fehler wie
    /// die bisherige Fassung.
    #[test]
    fn check_plausibility_matches_single_pass_reference() {
        let cfg = LoadConfig::default();
        let mut rng = Lcg(0x5EED_0038);
        let mut cases: Vec<Vec<Sample>> = vec![
            Vec::new(),
            vec![Sample::at(0)],
            vec![Sample::at(0), Sample::at(20)],
            vec![Sample::at(3), Sample::at(3)],
        ];
        for n in [2, 5, 30, 31, 500, 5000] {
            for disorder in [false, true] {
                cases.push(noisy_series(&mut rng, n, disorder));
            }
        }
        for samples in &cases {
            let new = check_plausibility(samples, &cfg);
            let old = check_plausibility_reference(samples, &cfg);
            assert_eq!(
                format!("{new:?}"),
                format!("{old:?}"),
                "n = {}",
                samples.len()
            );
        }
    }

    /// Die Fassungen mit vorab geprüften Samples (`*_prepared`, für den Import) liefern
    /// dieselben Ergebnisse wie die Fassungen, die selbst prüfen: Belastung je Sportart,
    /// Zeit in Zonen für jedes Schema des Typs und Bestleistungen, auf synthetischen Reihen
    /// mit Lücken und unplausiblen Werten.
    #[test]
    fn prepared_variants_match_self_checking_functions() {
        use crate::best_power::{BEST_POWER_DURATIONS_S, best_power, best_power_prepared};
        use crate::zones::{ZoneBasis, ZoneScheme, zone_seconds, zone_seconds_prepared};

        let cfg = LoadConfig::default();
        let th = Thresholds {
            ftp_w: Some(250.0),
            lthr_bpm: Some(165.0),
            hr_max_bpm: Some(190.0),
            hr_rest_bpm: Some(50.0),
            run_threshold_speed_mps: Some(3.6),
            css_mps: Some(1.2),
            sex: Some(Sex::Female),
            ..thresholds()
        };
        let schemes: Vec<ZoneScheme> = [
            ZoneBasis::Ftp,
            ZoneBasis::Lthr,
            ZoneBasis::HrReserve,
            ZoneBasis::RunThresholdSpeed,
            ZoneBasis::Css,
        ]
        .into_iter()
        .map(|basis| ZoneScheme {
            id: "test".into(),
            basis,
            sports: vec![Sport::Cycling],
            bounds: vec![0.6, 0.8, 1.0, 1.2],
            source: "Faustregel".into(),
            status: "Faustregel".into(),
            note: String::new(),
        })
        .collect();
        let mut rng = Lcg(0x5EED_0039);
        for n in [1, 29, 30, 400, 4000] {
            let samples = noisy_series(&mut rng, n, false);
            let prepared = check_plausibility(&samples, &cfg).unwrap();
            for sport in [
                Sport::Cycling,
                Sport::Running,
                Sport::Swimming,
                Sport::Strength,
                Sport::Other,
            ] {
                let mut a = activity(sport, samples.clone());
                a.rpe = Some(6);
                assert_eq!(
                    format!("{:?}", compute_load(&a, &th, &cfg)),
                    format!("{:?}", compute_load_prepared(&a, &prepared, &th, &cfg)),
                    "{sport:?} n = {n}"
                );
            }
            let a = activity(Sport::Cycling, samples.clone());
            for scheme in &schemes {
                assert_eq!(
                    zone_seconds(&a, scheme, &th, &cfg),
                    zone_seconds_prepared(&prepared, scheme, &th)
                );
                assert_eq!(
                    zone_seconds(&a, scheme, &thresholds(), &cfg),
                    zone_seconds_prepared(&prepared, scheme, &thresholds())
                );
            }
            assert_eq!(
                format!(
                    "{:?}",
                    best_power(&a, &cfg, &BEST_POWER_DURATIONS_S).unwrap()
                ),
                format!(
                    "{:?}",
                    best_power_prepared(&prepared, &BEST_POWER_DURATIONS_S)
                )
            );
        }
    }
}
