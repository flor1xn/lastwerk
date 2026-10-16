//! Befehle `thresholds set|list|suggest` und `athlete set` (SPEC 5.4, 8.5, 8.6).
//!
//! Die CLI übernimmt nie selbst einen Vorschlag (SPEC 8.6). Ein Vorschlag wird nur ausgegeben,
//! samt Warnhinweis und dem Befehl, mit dem der Nutzer ihn ausdrücklich übernimmt
//! (`--from-suggestion`, Herkunft `suggested_accepted`).

use std::io::Write;
use std::path::PathBuf;

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use lastwerk_core::{
    FTP_FRACTION_OF_BEST_20_MIN, LoadConfig, Sex, Sport, estimate_hr_max, suggest_css, suggest_ftp,
};
use lastwerk_store::{
    ImportOptions, NewThreshold, RecomputeReport, SETTING_ATHLETE_BIRTH_YEAR,
    SETTING_ATHLETE_BIRTH_YEAR_ORIGIN, SETTING_ATHLETE_SEX, SETTING_ATHLETE_SEX_ORIGIN, Store,
    ThresholdKind, ThresholdOrigin, ThresholdRecord,
};

use crate::error::CliError;
use crate::report::{num, sport_label};

/// Dauer, deren beste Leistung die FTP-Schätzung nutzt (SPEC 8.6): 20 Minuten.
const FTP_TEST_S: u32 = 1200;

/// Strecken der beiden Zeitläufe für die CSS-Schätzung (SPEC 8.6: zum Beispiel 400 m und
/// 200 m, Q15).
const CSS_LONG_M: f32 = 400.0;
const CSS_SHORT_M: f32 = 200.0;

/// Auftrag von `thresholds set`.
#[derive(Debug)]
pub struct SetRequest {
    /// `None` gilt für alle Sportarten.
    pub sport: Option<Sport>,
    pub entries: Vec<(ThresholdKind, f64)>,
    pub valid_from: NaiveDate,
    pub note: Option<String>,
    /// Der Nutzer übernimmt ausdrücklich einen Vorschlag (Herkunft `suggested_accepted`).
    pub from_suggestion: bool,
    /// Ordner mit den FIT-Dateien für die Neuberechnung der betroffenen Aktivitäten.
    pub dir: Option<PathBuf>,
}

/// Auftrag von `thresholds suggest`.
#[derive(Debug)]
pub struct SuggestRequest {
    /// `None` heißt alle Sportarten.
    pub sport: Option<Sport>,
    /// Zeiten in Sekunden für 400 m und 200 m Schwimmen.
    pub css_trial_s: Option<(f32, f32)>,
}

/// Auftrag von `athlete set`.
#[derive(Debug)]
pub struct AthleteRequest {
    pub sex: Option<Sex>,
    pub birth_year: Option<i32>,
    pub dir: Option<PathBuf>,
}

/// Name der Art in Ausgaben und Fehlermeldungen (Deutsch).
fn kind_label(kind: ThresholdKind) -> &'static str {
    match kind {
        ThresholdKind::FtpW => "FTP (W)",
        ThresholdKind::LthrBpm => "Schwellenherzfrequenz (bpm)",
        ThresholdKind::HrMaxBpm => "Maximale Herzfrequenz (bpm)",
        ThresholdKind::HrRestBpm => "Ruhepuls (bpm)",
        ThresholdKind::RunSpeedMps => "Laufschwelle (m/s)",
        ThresholdKind::CssMps => "CSS (m/s)",
    }
}

fn origin_label(origin: ThresholdOrigin) -> &'static str {
    match origin {
        ThresholdOrigin::Manual => "manuell",
        ThresholdOrigin::SuggestedAccepted => "Vorschlag übernommen",
        ThresholdOrigin::Garmin => "aus Garmin",
    }
}

/// Zahl mit Dezimalkomma ohne überflüssige Nachkommastellen (höchstens drei).
fn value_text(v: f64) -> String {
    let text = format!("{v:.3}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    trimmed.replace('.', ",")
}

/// Prüft Wert und Art gegen die Plausibilitätsgrenzen aus SPEC 6.5 (`LoadConfig`) und gegen
/// die Sportart. Zahlen, die die SPEC nicht nennt, werden nicht erfunden: Geschwindigkeiten
/// müssen nur positiv sein.
fn check_entry(
    sport: Option<Sport>,
    kind: ThresholdKind,
    value: f64,
    cfg: &LoadConfig,
) -> Result<(), CliError> {
    let bad = |why: String| CliError::Input(format!("{}: {why}", kind_label(kind)));
    if !(value.is_finite() && value > 0.0) {
        return Err(bad("Der Wert muss eine positive Zahl sein.".into()));
    }
    let hr_range = f64::from(cfg.hr_min_bpm)..=f64::from(cfg.hr_max_bpm);
    match kind {
        ThresholdKind::FtpW if value > f64::from(cfg.power_max_w) => {
            return Err(bad(format!(
                "Der Wert liegt über der Obergrenze von {} W.",
                num(f64::from(cfg.power_max_w), 0)
            )));
        }
        ThresholdKind::LthrBpm | ThresholdKind::HrMaxBpm | ThresholdKind::HrRestBpm
            if !hr_range.contains(&value) =>
        {
            return Err(bad(format!(
                "Der Wert liegt außerhalb von {} bis {} bpm.",
                num(f64::from(cfg.hr_min_bpm), 0),
                num(f64::from(cfg.hr_max_bpm), 0)
            )));
        }
        _ => {}
    }
    let required = match kind {
        ThresholdKind::FtpW => Some((Sport::Cycling, "Rad")),
        ThresholdKind::RunSpeedMps => Some((Sport::Running, "Lauf")),
        ThresholdKind::CssMps => Some((Sport::Swimming, "Schwimmen")),
        _ => None,
    };
    if let (Some((needed, label)), Some(s)) = (required, sport)
        && s != needed
    {
        return Err(bad(format!(
            "Diese Schwelle gilt nur für {label}, nicht für {}.",
            sport_label(Some(s))
        )));
    }
    Ok(())
}

/// `thresholds set`: legt die Schwellenwerte an und rechnet, falls ein Ordner angegeben ist,
/// die betroffenen Aktivitäten neu.
pub fn set(
    store: &mut Store,
    req: &SetRequest,
    now: DateTime<Utc>,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    if req.entries.is_empty() {
        return Err(CliError::Input(
            "Bitte --kind mit --value oder eine Kurzform wie --ftp angeben.".into(),
        ));
    }
    let cfg = LoadConfig::default();
    for (kind, value) in &req.entries {
        check_entry(req.sport, *kind, *value, &cfg)?;
    }
    if let Some(dir) = &req.dir
        && !dir.is_dir()
    {
        return Err(CliError::Input(
            "Der angegebene Ordner existiert nicht oder ist kein Ordner.".into(),
        ));
    }

    let origin = if req.from_suggestion {
        ThresholdOrigin::SuggestedAccepted
    } else {
        ThresholdOrigin::Manual
    };
    let mut ids = Vec::new();
    for (kind, value) in &req.entries {
        let id = store.insert_threshold(&NewThreshold {
            sport: req.sport,
            kind: *kind,
            value: *value,
            valid_from: req.valid_from,
            origin,
            note: req.note.clone(),
        })?;
        ids.push(id);
        wr(
            out,
            format_args!(
                "Schwelle gespeichert: {} = {} ({}, ab {}, {})",
                kind_label(*kind),
                value_text(*value),
                sport_label(req.sport),
                req.valid_from,
                origin_label(origin)
            ),
        )?;
    }

    // Betroffen sind Aktivitäten ab dem Gültigkeitsdatum bis zur nächsten Schwelle gleicher
    // Art. Gibt es für eine der neuen Schwellen kein Ende, bleibt der Zeitraum offen.
    let mut until: Option<NaiveDate> = None;
    let mut open_ended = false;
    for id in &ids {
        match store.threshold_valid_until(*id)? {
            Some(d) => until = Some(until.map_or(d, |u| u.max(d))),
            None => open_ended = true,
        }
    }
    let to = if open_ended { None } else { until };

    match &req.dir {
        Some(dir) => {
            let report = store.recompute_loads(
                dir,
                req.sport,
                req.valid_from,
                to,
                now,
                &ImportOptions::default(),
            )?;
            write_recompute(out, &report)
        }
        None => wr(
            out,
            format_args!(
                "Hinweis: Bereits berechnete Belastungen und Zonen ändern sich erst, wenn sie \
                 neu berechnet werden. Mit --dir <ordner> an diesem Befehl oder mit \
                 `lastwerk rebuild --dir <ordner>`."
            ),
        ),
    }
}

/// Ausgabe der Neuberechnung.
fn write_recompute(out: &mut dyn Write, r: &RecomputeReport) -> Result<(), CliError> {
    wr(
        out,
        format_args!(
            "Neu berechnet: {} von {} betroffenen Aktivitäten ({} berechnet, {} ohne Schwelle, \
             {} ohne Messkanal, {} unplausibel)",
            r.recomputed,
            r.candidates,
            r.loads_ok,
            r.loads_missing_threshold,
            r.loads_missing_channel,
            r.loads_implausible
        ),
    )?;
    if r.file_missing > 0 {
        wr(
            out,
            format_args!(
                "  Datei nicht gefunden oder nicht lesbar: {} (Aktivität unverändert)",
                r.file_missing
            ),
        )?;
    }
    if r.file_changed > 0 {
        wr(
            out,
            format_args!(
                "  Datei seit dem Import verändert: {} (Aktivität unverändert, bitte `import` ausführen)",
                r.file_changed
            ),
        )?;
    }
    Ok(())
}

fn wr(out: &mut dyn Write, args: std::fmt::Arguments<'_>) -> Result<(), CliError> {
    writeln!(out, "{args}").map_err(|e| CliError::Internal(format!("Ausgabefehler: {}", e.kind())))
}

/// `thresholds list`: alle Schwellenwerte mit Gültigkeitsdatum und Herkunft. Fehlt jede
/// maximale Herzfrequenz, steht die Schätzung aus dem Alter dabei, gekennzeichnet als
/// Schätzung (SPEC 8.5).
pub fn list(store: &Store, now: DateTime<Utc>, out: &mut dyn Write) -> Result<(), CliError> {
    let rows = store.list_thresholds()?;
    wr(out, format_args!("Schwellenwerte"))?;
    if rows.is_empty() {
        wr(out, format_args!("  (keine gesetzt)"))?;
    }
    for t in &rows {
        wr(out, format_args!("  {}", threshold_line(t)))?;
    }
    if !rows.iter().any(|t| t.kind == ThresholdKind::HrMaxBpm)
        && let Some(hr_max) = store.effective_hr_max(Sport::Other, now.date_naive())?
        && hr_max.estimated
    {
        let birth_year = store
            .get_setting(SETTING_ATHLETE_BIRTH_YEAR)?
            .unwrap_or_default();
        wr(
            out,
            format_args!(
                "  Maximale Herzfrequenz (bpm): {} (geschätzt, 208 - 0,7 * Alter nach Q7, \
                 Geburtsjahr {birth_year}; estimated)",
                value_text(f64::from(hr_max.bpm))
            ),
        )?;
    }
    match store.get_setting(SETTING_ATHLETE_SEX)?.as_deref() {
        Some("male") => wr(out, format_args!("Geschlecht (nur TRIMP): männlich"))?,
        Some("female") => wr(out, format_args!("Geschlecht (nur TRIMP): weiblich"))?,
        _ => {}
    }
    Ok(())
}

fn threshold_line(t: &ThresholdRecord) -> String {
    let mut line = format!(
        "{}: {} | {} | ab {} | {}",
        kind_label(t.kind),
        value_text(t.value),
        sport_label(t.sport),
        t.valid_from,
        origin_label(t.origin)
    );
    if let Some(note) = &t.note {
        line.push_str(" | ");
        line.push_str(note);
    }
    line
}

/// `thresholds suggest`: Vorschläge nach SPEC 8.6. Die CLI übernimmt nie selbst.
pub fn suggest(store: &Store, req: &SuggestRequest, out: &mut dyn Write) -> Result<(), CliError> {
    let wants = |sport: Sport| req.sport.is_none_or(|s| s == sport);
    wr(
        out,
        format_args!("Vorschläge (nichts wird gespeichert, bitte prüfen und selbst übernehmen)"),
    )?;

    if wants(Sport::Cycling) {
        let proposal = store
            .best_power_peak(Sport::Cycling, FTP_TEST_S)?
            .and_then(|peak| suggest_ftp(peak.watts as f32).map(|ftp| (peak, ftp)));
        match proposal {
            Some((peak, ftp)) => {
                wr(
                    out,
                    format_args!(
                        "FTP: {} W ({} Prozent der besten 20-Minuten-Leistung von {} W am {})",
                        num(f64::from(ftp.round()), 0),
                        num(f64::from(FTP_FRACTION_OF_BEST_20_MIN) * 100.0, 0),
                        num(peak.watts, 0),
                        peak.local_date
                    ),
                )?;
                wr(
                    out,
                    format_args!(
                        "  Warnung: Faustregel (Q16 nur als Sekundärquelle). Sinnvoll nur, wenn \
                         diese 20 Minuten ein maximaler Test waren; eine Fahrt mit weniger \
                         Einsatz unterschätzt die FTP."
                    ),
                )?;
                wr(
                    out,
                    format_args!(
                        "  Übernehmen: lastwerk thresholds set --sport cycling --ftp {} --from {} \
                         --from-suggestion",
                        ftp.round(),
                        peak.local_date
                    ),
                )?;
            }
            None => wr(
                out,
                format_args!(
                    "FTP: nicht ableitbar, es gibt keine Radfahrt mit Leistung über 20 Minuten."
                ),
            )?,
        }
    }

    if req.sport.is_none() {
        wr(
            out,
            format_args!(
                "LTHR: aus dem Cache nicht ableitbar. Der Vorschlag braucht die Herzfrequenz \
                 eines 30-Minuten-Tests (Mittel der letzten 20 Minuten, Faustregel). Den Wert \
                 mit `lastwerk thresholds set --sport all --lthr <bpm> --from <datum>` eintragen."
            ),
        )?;
    }

    if wants(Sport::Swimming) {
        match req.css_trial_s {
            Some((t400, t200)) => match suggest_css(CSS_LONG_M, t400, CSS_SHORT_M, t200) {
                Some(css) => {
                    wr(
                        out,
                        format_args!(
                            "CSS: {} m/s (400 m in {} s, 200 m in {} s, Q15)",
                            num(f64::from(css), 3),
                            num(f64::from(t400), 0),
                            num(f64::from(t200), 0)
                        ),
                    )?;
                    wr(
                        out,
                        format_args!(
                            "  Übernehmen: lastwerk thresholds set --sport swimming --css {} \
                             --from <datum> --from-suggestion",
                            num(f64::from(css), 3).replace(',', ".")
                        ),
                    )?;
                }
                None => {
                    return Err(CliError::Input(
                        "CSS: Die Zeit für 400 m muss länger sein als die für 200 m, beide positiv."
                            .into(),
                    ));
                }
            },
            None => wr(
                out,
                format_args!(
                    "CSS: aus den Daten nicht ableitbar. Der Vorschlag braucht zwei Zeitläufe \
                     (Q15): `--css-t400 <sekunden> --css-t200 <sekunden>` angeben."
                ),
            )?,
        }
    }

    if req.sport == Some(Sport::Running) {
        wr(
            out,
            format_args!("Lauf: SPEC 8.6 sieht keinen Vorschlag für die Laufschwelle vor."),
        )?;
    }
    Ok(())
}

/// `athlete set`: Geschlecht (nur für die TRIMP-Konstanten) und Geburtsjahr (nur für die
/// Schätzung der maximalen Herzfrequenz, Q7) als Einstellungen.
pub fn athlete_set(
    store: &mut Store,
    req: &AthleteRequest,
    now: DateTime<Utc>,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    if req.sex.is_none() && req.birth_year.is_none() {
        return Err(CliError::Input(
            "Bitte --sex und/oder --birth-year angeben.".into(),
        ));
    }
    if let Some(year) = req.birth_year
        && !(1900..=now.year()).contains(&year)
    {
        return Err(CliError::Input(format!(
            "Das Geburtsjahr muss zwischen 1900 und {} liegen.",
            now.year()
        )));
    }
    if let Some(dir) = &req.dir
        && !dir.is_dir()
    {
        return Err(CliError::Input(
            "Der angegebene Ordner existiert nicht oder ist kein Ordner.".into(),
        ));
    }
    if let Some(sex) = req.sex {
        let text = match sex {
            Sex::Male => "male",
            Sex::Female => "female",
        };
        store.set_setting(SETTING_ATHLETE_SEX, text)?;
        // Ein eigener Wert geht vor Garmin (SPEC 8.6): der Abgleich lässt ihn danach stehen.
        store.set_setting(SETTING_ATHLETE_SEX_ORIGIN, "manual")?;
        wr(
            out,
            format_args!(
                "Geschlecht gespeichert: {} (nur für die TRIMP-Konstanten)",
                if sex == Sex::Male {
                    "männlich"
                } else {
                    "weiblich"
                }
            ),
        )?;
    }
    if let Some(year) = req.birth_year {
        store.set_setting(SETTING_ATHLETE_BIRTH_YEAR, &year.to_string())?;
        store.set_setting(SETTING_ATHLETE_BIRTH_YEAR_ORIGIN, "manual")?;
        let age = now.year() - year;
        wr(
            out,
            format_args!(
                "Geburtsjahr gespeichert: {year} (nur für die Schätzung der maximalen \
                 Herzfrequenz: {} bpm bei Alter {age}, Q7; estimated)",
                value_text(f64::from(estimate_hr_max(age as f32)))
            ),
        )?;
    }
    match &req.dir {
        Some(dir) => {
            let from = NaiveDate::from_ymd_opt(1900, 1, 1)
                .ok_or_else(|| CliError::Internal("Datum".into()))?;
            let report =
                store.recompute_loads(dir, None, from, None, now, &ImportOptions::default())?;
            write_recompute(out, &report)
        }
        None => wr(
            out,
            format_args!(
                "Hinweis: Bereits berechnete Belastungen und Zonen ändern sich erst nach \
                 `lastwerk rebuild --dir <ordner>` oder mit --dir <ordner> an diesem Befehl."
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_text_trims_zeros_and_uses_a_comma() {
        assert_eq!(value_text(250.0), "250");
        assert_eq!(value_text(3.5), "3,5");
        assert_eq!(value_text(1.1765), "1,177");
    }

    #[test]
    fn entries_are_checked_against_the_plausibility_limits() {
        let cfg = LoadConfig::default();
        let ok = |s, k, v| check_entry(s, k, v, &cfg).is_ok();
        assert!(ok(Some(Sport::Cycling), ThresholdKind::FtpW, 250.0));
        assert!(!ok(Some(Sport::Cycling), ThresholdKind::FtpW, 0.0));
        assert!(!ok(Some(Sport::Cycling), ThresholdKind::FtpW, 2600.0));
        assert!(!ok(None, ThresholdKind::FtpW, f64::NAN));
        assert!(ok(None, ThresholdKind::HrMaxBpm, 190.0));
        assert!(!ok(None, ThresholdKind::HrMaxBpm, 250.0));
        assert!(!ok(None, ThresholdKind::HrRestBpm, 20.0));
        // Sportart muss zur Schwelle passen.
        assert!(!ok(Some(Sport::Running), ThresholdKind::FtpW, 250.0));
        assert!(!ok(Some(Sport::Cycling), ThresholdKind::CssMps, 1.2));
        assert!(ok(Some(Sport::Swimming), ThresholdKind::CssMps, 1.2));
        assert!(ok(Some(Sport::Running), ThresholdKind::RunSpeedMps, 3.5));
    }
}
