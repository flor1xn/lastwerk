//! Duplikatregel für dieselbe Fahrt auf Radcomputer und Uhr (SPEC 6.4).
//!
//! Zwei Aktivitäten sind dieselbe, wenn alle drei Bedingungen gelten: gleiche Sportart,
//! Startzeiten höchstens 120 s auseinander, Dauern höchstens 10 Prozent auseinander
//! (Konvention, beides einstellbar). Primär ist die Aktivität mit Leistungsdaten, bei
//! Gleichstand die mit der längeren Dauer, danach die mit der kleineren `source_file.id`.
//!
//! Der Lauf geht immer über alle Aktivitäten und ist deterministisch: Das Ergebnis hängt
//! nicht von der Reihenfolge des Imports ab. Die Aktivitäten werden nach Rang geordnet, die
//! Besten werden nacheinander primär, sofern sie keiner bereits primären Aktivität
//! gleichen. Das vermeidet Ketten (A gleicht B, B gleicht C, A gleicht C nicht), die mit
//! einer Zusammenfassung nach Paaren zwei verschiedene Fahrten verschmelzen würden.

use std::collections::BTreeMap;

use rusqlite::Connection;

use crate::error::Result;
use crate::rows::parse_utc;

/// Einstellbare Schwellen der Duplikatregel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DuplicateRule {
    /// Größter Unterschied der Startzeiten in Sekunden (Konvention: 120).
    pub max_start_diff_s: i64,
    /// Größter Unterschied der Dauern, bezogen auf die längere Dauer (Konvention: 0,10).
    pub max_duration_diff_ratio: f64,
}

impl Default for DuplicateRule {
    fn default() -> Self {
        Self {
            max_start_diff_s: 120,
            max_duration_diff_ratio: 0.10,
        }
    }
}

impl DuplicateRule {
    /// Gilt die Regel für zwei Aktivitäten gleicher Sportart mit diesen Daten?
    pub fn matches(&self, a: &Candidate, b: &Candidate) -> bool {
        if a.sport != b.sport {
            return false;
        }
        if (a.start_ts - b.start_ts).abs() > self.max_start_diff_s {
            return false;
        }
        let longer = a.duration_s.max(b.duration_s);
        let diff = (a.duration_s - b.duration_s).abs();
        // Gleiche Dauer (auch 0) passt immer, sonst Verhältnis zur längeren Dauer.
        diff == 0 || (diff as f64) <= self.max_duration_diff_ratio * (longer as f64)
    }
}

/// Eine Aktivität, soweit die Regel sie braucht.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub id: i64,
    pub source_file_id: i64,
    pub sport: String,
    /// Startzeit in Unix-Sekunden.
    pub start_ts: i64,
    pub duration_s: i64,
    pub has_power: bool,
}

/// Ergebnis für eine Aktivität: `duplicate_of` ist die `id` der primären Aktivität oder
/// `None`, wenn sie selbst primär ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub id: i64,
    pub duplicate_of: Option<i64>,
}

/// Wendet die Regel auf alle Aktivitäten an. Rein, ohne Datenbank.
///
/// Die Ausgabe ist nach `id` sortiert.
pub fn resolve(candidates: &[Candidate], rule: &DuplicateRule) -> Vec<Resolution> {
    // Rang: mit Leistung zuerst, dann längere Dauer, dann kleinere source_file.id, zuletzt
    // kleinere id, damit die Ordnung total ist.
    let mut order: Vec<&Candidate> = candidates.iter().collect();
    order.sort_by(|a, b| {
        b.has_power
            .cmp(&a.has_power)
            .then(b.duration_s.cmp(&a.duration_s))
            .then(a.source_file_id.cmp(&b.source_file_id))
            .then(a.id.cmp(&b.id))
    });

    // Primäre Aktivitäten nach (Sportart, Start), damit nur Kandidaten im Zeitfenster
    // geprüft werden. Der Wert ist der Rang.
    let mut primaries: BTreeMap<(&str, i64), Vec<(usize, &Candidate)>> = BTreeMap::new();
    let mut out: Vec<Resolution> = Vec::with_capacity(candidates.len());

    for (rank, c) in order.iter().enumerate() {
        let lo = (c.sport.as_str(), c.start_ts - rule.max_start_diff_s);
        let hi = (c.sport.as_str(), c.start_ts + rule.max_start_diff_s);
        let found = primaries
            .range(lo..=hi)
            .flat_map(|(_, v)| v.iter())
            .filter(|(_, p)| rule.matches(c, p))
            .min_by_key(|(r, _)| *r)
            .map(|(_, p)| p.id);
        match found {
            Some(primary) => out.push(Resolution {
                id: c.id,
                duplicate_of: Some(primary),
            }),
            None => {
                primaries
                    .entry((c.sport.as_str(), c.start_ts))
                    .or_default()
                    .push((rank, c));
                out.push(Resolution {
                    id: c.id,
                    duplicate_of: None,
                });
            }
        }
    }
    out.sort_by_key(|r| r.id);
    out
}

/// Lädt alle Aktivitäten, wendet die Regel an und schreibt `is_primary` und `duplicate_of`.
/// Gibt die Zahl der Aktivitäten zurück, die als Duplikat gelten.
pub fn apply(conn: &Connection, rule: &DuplicateRule) -> Result<usize> {
    let mut stmt = conn.prepare(
        "SELECT id, source_file_id, sport, start_utc, duration_s, has_power FROM activity",
    )?;
    let candidates = stmt
        .query_map([], |r| {
            let start: String = r.get(3)?;
            Ok(Candidate {
                id: r.get(0)?,
                source_file_id: r.get(1)?,
                sport: r.get(2)?,
                start_ts: crate::error::conversion(3, parse_utc(&start))?.timestamp(),
                duration_s: r.get(4)?,
                has_power: r.get::<_, i64>(5)? != 0,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);

    let resolved = resolve(&candidates, rule);
    let mut update =
        conn.prepare("UPDATE activity SET is_primary = ?2, duplicate_of = ?3 WHERE id = ?1")?;
    let mut duplicates = 0;
    for r in &resolved {
        update.execute(rusqlite::params![
            r.id,
            i64::from(r.duplicate_of.is_none()),
            r.duplicate_of
        ])?;
        if r.duplicate_of.is_some() {
            duplicates += 1;
        }
    }
    Ok(duplicates)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: i64, start_ts: i64, duration_s: i64, has_power: bool) -> Candidate {
        Candidate {
            id,
            source_file_id: id,
            sport: "cycling".into(),
            start_ts,
            duration_s,
            has_power,
        }
    }

    fn dup_of(r: &[Resolution], id: i64) -> Option<i64> {
        r.iter().find(|x| x.id == id).unwrap().duplicate_of
    }

    /// Edge/Uhr wie im Testkit: 20 s Versatz, 3600 s gegen 3570 s. Differenz 30 s, erlaubt
    /// sind 10 Prozent von 3600 s = 360 s.
    #[test]
    fn edge_and_watch_are_one_ride_and_power_wins() {
        // Die Uhr hat die kleinere id, damit die Primärwahl nicht an der Reihenfolge hängt.
        let watch = c(1, 1_000_020, 3570, false);
        let edge = c(2, 1_000_000, 3600, true);
        let r = resolve(&[watch, edge], &DuplicateRule::default());
        assert_eq!(dup_of(&r, 2), None);
        assert_eq!(dup_of(&r, 1), Some(2));
    }

    /// Grenze Start: genau 120 s zählt noch, 121 s nicht. Rechnung: 1 000 120 - 1 000 000.
    #[test]
    fn start_difference_boundary_is_120_seconds() {
        let rule = DuplicateRule::default();
        assert!(rule.matches(&c(1, 1_000_000, 3600, true), &c(2, 1_000_120, 3600, true)));
        assert!(!rule.matches(&c(1, 1_000_000, 3600, true), &c(2, 1_000_121, 3600, true)));
    }

    /// Negativtest aus der Aufgabe: 3 Minuten = 180 s Abstand sind zwei Fahrten.
    #[test]
    fn three_minutes_apart_are_not_duplicates() {
        let r = resolve(
            &[c(1, 1_000_000, 3600, true), c(2, 1_000_180, 3600, true)],
            &DuplicateRule::default(),
        );
        assert!(r.iter().all(|x| x.duplicate_of.is_none()));
    }

    /// Dauer: 3240 s gegen 3600 s sind 360 s, also genau 10 Prozent der längeren. 3239 s
    /// sind 361 s und damit zu viel.
    #[test]
    fn duration_difference_boundary_is_ten_percent_of_the_longer() {
        let rule = DuplicateRule::default();
        assert!(rule.matches(&c(1, 0, 3600, true), &c(2, 0, 3240, true)));
        assert!(!rule.matches(&c(1, 0, 3600, true), &c(2, 0, 3239, true)));
    }

    #[test]
    fn different_sports_never_match() {
        let mut run = c(2, 1_000_000, 3600, true);
        run.sport = "running".into();
        let r = resolve(
            &[c(1, 1_000_000, 3600, true), run],
            &DuplicateRule::default(),
        );
        assert!(r.iter().all(|x| x.duplicate_of.is_none()));
    }

    #[test]
    fn tie_on_power_goes_to_the_longer_then_to_the_smaller_file_id() {
        let rule = DuplicateRule::default();
        // Beide ohne Leistung: die längere gewinnt.
        let r = resolve(&[c(1, 0, 3500, false), c(2, 10, 3600, false)], &rule);
        assert_eq!(dup_of(&r, 1), Some(2));
        // Gleiche Dauer: die mit der kleineren source_file.id gewinnt.
        let r = resolve(&[c(1, 10, 3600, false), c(2, 0, 3600, false)], &rule);
        assert_eq!(dup_of(&r, 2), Some(1));
    }

    #[test]
    fn result_does_not_depend_on_input_order() {
        let rule = DuplicateRule::default();
        let a = [
            c(1, 0, 3600, true),
            c(2, 20, 3570, false),
            c(3, 100, 3590, false),
            c(4, 5000, 1800, false),
        ];
        let mut b = a.clone();
        b.reverse();
        assert_eq!(resolve(&a, &rule), resolve(&b, &rule));
    }

    /// Kette: B gleicht A und C, A und C liegen 200 s auseinander. Mit Primärwahl nach Rang
    /// wird B primär (längste Dauer), A und C sind Duplikate von B. Wäre A primär, bliebe C
    /// eine eigene Fahrt.
    #[test]
    fn chain_is_resolved_by_rank_not_by_pairs() {
        let a = c(1, 0, 3500, false);
        let b = c(2, 100, 3600, false);
        let cc = c(3, 200, 3500, false);
        let r = resolve(&[a, b, cc], &DuplicateRule::default());
        assert_eq!(dup_of(&r, 2), None);
        assert_eq!(dup_of(&r, 1), Some(2));
        assert_eq!(dup_of(&r, 3), Some(2));
    }

    #[test]
    fn rule_is_adjustable() {
        let rule = DuplicateRule {
            max_start_diff_s: 200,
            max_duration_diff_ratio: 0.5,
        };
        assert!(rule.matches(&c(1, 0, 3600, true), &c(2, 180, 2000, true)));
        assert!(!DuplicateRule::default().matches(&c(1, 0, 3600, true), &c(2, 180, 2000, true)));
    }
}
