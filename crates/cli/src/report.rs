//! Textausgabe der Berichte (Deutsch, Nutzertexte der CLI).
//!
//! Die Zahlenformate nutzen das Dezimalkomma. Der Plausibilitätsbericht enthält nur
//! Kennzahlen: keine Dateinamen, Pfade, Zeitpunkte oder Seriennummern (SPEC 11.5).

use std::io::{self, Write};

use chrono::NaiveDate;
use lastwerk_core::Sport;
use lastwerk_store::{ImportReport, NP_RATIO_FLAG, PlausibilitySummary, Range};

/// Zahl mit `decimals` Nachkommastellen und Dezimalkomma.
pub fn num(v: f64, decimals: usize) -> String {
    format!("{v:.decimals$}").replace('.', ",")
}

/// Sportart für Ausgaben, `None` heißt alle Sportarten.
pub fn sport_label(sport: Option<Sport>) -> &'static str {
    match sport {
        None => "alle Sportarten",
        Some(Sport::Cycling) => "Rad",
        Some(Sport::Running) => "Lauf",
        Some(Sport::Swimming) => "Schwimmen",
        Some(Sport::Strength) => "Kraft",
        Some(Sport::Other) => "Sonstige",
    }
}

/// Eine Zeile des Verlaufs von Fitness, Ermüdung und Form (SPEC 8.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PmcRow {
    pub date: NaiveDate,
    /// Tageslast, 0 an Tagen ohne zählende Belastung.
    pub load: f32,
    pub ctl: f32,
    pub atl: f32,
    pub tsb: f32,
}

/// Zahl mit drei Nachkommastellen und Dezimalpunkt. Ein Wert, der auf null rundet, wird ohne
/// Vorzeichen geschrieben (`-0.000` wäre irreführend und für den Golden-Test instabil).
fn fixed3(v: f32) -> String {
    let text = format!("{v:.3}");
    if text == "-0.000" {
        "0.000".to_owned()
    } else {
        text
    }
}

/// Verlauf als CSV mit Kopfzeile `date,load,ctl,atl,tsb`, Dezimalpunkt, drei Nachkommastellen
/// (SPEC 12, M2: `pmc.csv`).
pub fn write_pmc_csv(out: &mut dyn Write, rows: &[PmcRow]) -> io::Result<()> {
    writeln!(out, "date,load,ctl,atl,tsb")?;
    for r in rows {
        writeln!(
            out,
            "{},{},{},{},{}",
            r.date,
            fixed3(r.load),
            fixed3(r.ctl),
            fixed3(r.atl),
            fixed3(r.tsb)
        )?;
    }
    Ok(())
}

/// Verlauf als JSON-Liste, eine Zeile je Tag, gleiche Zahlenformate wie das CSV. Das Datum
/// enthält nur Ziffern und Bindestriche, es braucht kein Maskieren.
pub fn write_pmc_json(out: &mut dyn Write, rows: &[PmcRow]) -> io::Result<()> {
    writeln!(out, "[")?;
    for (i, r) in rows.iter().enumerate() {
        let comma = if i + 1 < rows.len() { "," } else { "" };
        writeln!(
            out,
            "  {{\"date\":\"{}\",\"load\":{},\"ctl\":{},\"atl\":{},\"tsb\":{}}}{comma}",
            r.date,
            fixed3(r.load),
            fixed3(r.ctl),
            fixed3(r.atl),
            fixed3(r.tsb)
        )?;
    }
    writeln!(out, "]")
}

fn range(r: Option<Range>, scale: f64, decimals: usize, unit: &str) -> String {
    match r {
        None => "keine Werte".to_owned(),
        Some(r) => {
            let text = format!(
                "{} bis {}",
                num(r.min * scale, decimals),
                num(r.max * scale, decimals)
            );
            if unit.is_empty() {
                text
            } else {
                format!("{text} {unit}")
            }
        }
    }
}

fn counted(items: &[(String, usize)]) -> String {
    items
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Bericht eines Imports oder Wiederaufbaus. `title` ist die Überschrift.
pub fn write_import_report(out: &mut dyn Write, title: &str, r: &ImportReport) -> io::Result<()> {
    writeln!(out, "{title}")?;
    writeln!(out, "  Dateien gefunden:        {}", r.files_found)?;
    writeln!(out, "  Neu:                     {}", r.new)?;
    writeln!(out, "  Aktualisiert:            {}", r.updated)?;
    writeln!(out, "  Unverändert:             {}", r.unchanged)?;
    writeln!(out, "  Ohne Aktivität:          {}", r.not_activity)?;
    writeln!(out, "  Fehler:                  {}", r.error_count())?;
    writeln!(
        out,
        "  Aktivitäten zählend:     {} (Duplikate: {})",
        r.primary, r.duplicates
    )?;
    writeln!(
        out,
        "  Belastung in diesem Lauf: {} berechnet, {} ohne Schwelle, {} ohne Messkanal, {} unplausibel",
        r.loads_ok, r.loads_missing_threshold, r.loads_missing_channel, r.loads_implausible
    )?;
    if !r.errors.is_empty() {
        writeln!(out, "Fehlerhafte Dateien:")?;
        for e in &r.errors {
            let path = if e.rel_path.is_empty() {
                "(Ordner)"
            } else {
                &e.rel_path
            };
            writeln!(out, "  {path}: {}", e.code)?;
        }
    }
    Ok(())
}

/// Kennzahlen des Plausibilitätslaufs.
pub fn write_plausibility(out: &mut dyn Write, s: &PlausibilitySummary) -> io::Result<()> {
    writeln!(
        out,
        "Plausibilitätslauf (nur Kennzahlen, nichts wird gespeichert)"
    )?;
    writeln!(
        out,
        "Dateien: {} gefunden, {} Aktivitäten, {} ohne Aktivität, {} Fehler",
        s.files_total, s.files_ok, s.files_not_activity, s.files_error
    )?;
    if !s.error_codes.is_empty() {
        writeln!(out, "  Fehlercodes: {}", counted(&s.error_codes))?;
    }
    writeln!(
        out,
        "Aktivitäten: {}, davon zählend {}, Duplikatpaare {}",
        s.activities, s.primary, s.duplicate_pairs
    )?;
    if let Some(start) = s.duplicate_max_start_diff_s {
        let dur = s.duplicate_max_duration_diff_pct.unwrap_or(0.0);
        writeln!(
            out,
            "  Größter Startabstand eines Paars: {start} s, größte Dauerabweichung: {} %",
            num(dur, 1)
        )?;
    }
    if !s.by_sport.is_empty() {
        writeln!(out, "  Sportarten (zählend): {}", counted(&s.by_sport))?;
    }
    writeln!(out, "Spannweiten über zählende Aktivitäten:")?;
    writeln!(
        out,
        "  Dauer:                {}",
        range(s.duration_s, 1.0 / 60.0, 0, "min")
    )?;
    writeln!(
        out,
        "  Distanz:              {}",
        range(s.distance_m, 0.001, 1, "km")
    )?;
    writeln!(
        out,
        "  Mittlere Herzfrequenz: {}",
        range(s.avg_hr, 1.0, 0, "bpm")
    )?;
    writeln!(
        out,
        "  Mittlere Leistung:    {}",
        range(s.avg_power_w, 1.0, 0, "W")
    )?;
    writeln!(
        out,
        "  Normalisierte Leistung: {}",
        range(s.np_w, 1.0, 0, "W")
    )?;
    writeln!(
        out,
        "Belastung: berechenbar bei {} von {} zählenden Aktivitäten",
        s.load_computable, s.primary
    )?;
    match s.share_not_computable() {
        Some(share) => {
            let detail = if s.load_not_computable.is_empty() {
                String::new()
            } else {
                format!(" ({})", counted(&s.load_not_computable))
            };
            writeln!(
                out,
                "  Nicht berechenbar: {} %{detail}",
                num(share * 100.0, 1)
            )?;
        }
        None => writeln!(out, "  Nicht berechenbar: keine zählenden Aktivitäten")?,
    }
    writeln!(
        out,
        "  Last je Aktivität: {}",
        range(s.load_value, 1.0, 0, "")
    )?;
    if s.load_per_year.is_empty() {
        writeln!(out, "  Jahreslast: keine Werte")?;
    } else {
        let years: Vec<String> = s
            .load_per_year
            .iter()
            .map(|(y, v)| format!("{y}: {}", num(*v, 0)))
            .collect();
        writeln!(out, "  Jahreslast: {}", years.join(", "))?;
    }
    writeln!(out, "  Wochenlast: {}", range(s.load_per_week, 1.0, 0, ""))?;
    writeln!(
        out,
        "NP zu Durchschnittsleistung: {}, auffällig (über {}): {}",
        range(s.np_ratio, 1.0, 2, ""),
        num(NP_RATIO_FLAG, 1),
        s.np_ratio_flagged
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;
    use lastwerk_core::{DailyLoad, PmcParams, pmc};
    use lastwerk_store::FileIssue;

    /// Feste synthetische Tageslasten für den Golden-Test (SPEC 11.4): 60 Tage ab Januar 2026.
    /// Das Muster aus sieben Tagen hat die Lasten 0, 60, 80, 0, 100, 150, 40 und ist in der
    /// fünften Woche (Tag 28 bis 34) halbiert (Erholungswoche). Tage mit Last 0 fehlen in der
    /// Liste, wie sie auch `daily_loads` auslässt.
    fn golden_rows() -> Vec<PmcRow> {
        const WEEK: [f32; 7] = [0.0, 60.0, 80.0, 0.0, 100.0, 150.0, 40.0];
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let daily: Vec<DailyLoad> = (0..60_i64)
            .filter_map(|n| {
                let base = WEEK[(n % 7) as usize];
                let load = if (28..35).contains(&n) {
                    base / 2.0
                } else {
                    base
                };
                (load > 0.0).then(|| DailyLoad {
                    date: start + TimeDelta::days(n),
                    load,
                })
            })
            .collect();
        let to = start + TimeDelta::days(59);
        pmc(&daily, &PmcParams::default(), start, to)
            .into_iter()
            .map(|p| PmcRow {
                date: p.date,
                load: daily
                    .iter()
                    .find(|d| d.date == p.date)
                    .map_or(0.0, |d| d.load),
                ctl: p.ctl,
                atl: p.atl,
                tsb: p.tsb,
            })
            .collect()
    }

    /// Handrechnung der ersten drei Tage mit `1 - exp(-1/42) = 0,023528` und
    /// `1 - exp(-1/7) = 0,133122`:
    /// Tag 1 (Last 0): alles 0. Tag 2 (Last 60): CTL = 60 * 0,023528 = 1,4117,
    /// ATL = 60 * 0,133122 = 7,9873, TSB = 0. Tag 3 (Last 80):
    /// CTL = 1,4117 + (80 - 1,4117) * 0,023528 = 3,2607,
    /// ATL = 7,9873 + (80 - 7,9873) * 0,133122 = 17,5738, TSB = 1,4117 - 7,9873 = -6,5756.
    #[test]
    fn golden_input_matches_the_hand_calculation_for_the_first_days() {
        const EXPECTED: [(f32, f32, f32, f32); 3] = [
            (0.0, 0.0, 0.0, 0.0),
            (60.0, 1.4117, 7.9873, 0.0),
            (80.0, 3.2607, 17.5738, -6.5756),
        ];
        let rows = golden_rows();
        assert_eq!(rows.len(), 60);
        for (row, (load, ctl, atl, tsb)) in rows.iter().zip(EXPECTED) {
            assert!((row.load - load).abs() < 1e-3, "{row:?}");
            assert!((row.ctl - ctl).abs() < 1e-3, "{row:?}");
            assert!((row.atl - atl).abs() < 1e-3, "{row:?}");
            assert!((row.tsb - tsb).abs() < 1e-3, "{row:?}");
        }
    }

    /// Golden-Test für `pmc.csv` (SPEC 11.4, 12 M2). Der Snapshot liegt im Repository.
    #[test]
    fn pmc_csv_golden() {
        let mut buf = Vec::new();
        write_pmc_csv(&mut buf, &golden_rows()).unwrap();
        insta::assert_snapshot!("pmc_csv", String::from_utf8(buf).unwrap());
    }

    #[test]
    fn pmc_json_golden_for_three_days() {
        let mut buf = Vec::new();
        write_pmc_json(&mut buf, &golden_rows()[..3]).unwrap();
        insta::assert_snapshot!("pmc_json_3_days", String::from_utf8(buf).unwrap());
    }

    #[test]
    fn values_that_round_to_zero_have_no_sign() {
        assert_eq!(fixed3(-0.0001), "0.000");
        assert_eq!(fixed3(-0.0), "0.000");
        assert_eq!(fixed3(-0.0006), "-0.001");
        assert_eq!(fixed3(2.35283), "2.353");
    }

    #[test]
    fn numbers_use_a_decimal_comma() {
        assert_eq!(num(1.25, 1), "1,2");
        assert_eq!(num(0.5, 2), "0,50");
        assert_eq!(num(1234.0, 0), "1234");
    }

    #[test]
    fn import_report_lists_counts_and_errors() {
        let r = ImportReport {
            files_found: 3,
            new: 1,
            errors: vec![
                FileIssue {
                    rel_path: "a/b.fit".into(),
                    code: "crc_mismatch".into(),
                },
                FileIssue {
                    rel_path: String::new(),
                    code: "io".into(),
                },
            ],
            ..ImportReport::default()
        };
        let mut buf = Vec::new();
        write_import_report(&mut buf, "Import", &r).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("Neu:                     1"));
        assert!(text.contains("Fehler:                  2"));
        assert!(text.contains("a/b.fit: crc_mismatch"));
        assert!(text.contains("(Ordner): io"));
    }
}
