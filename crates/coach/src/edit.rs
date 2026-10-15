//! Änderungsvorschlag des Modells (SPEC 7.2, 9.4): Prüfung gegen `plan-edit.schema.json` und
//! Anwendung auf den Basisplan.
//!
//! Das Modell gibt keinen Plan zurück, sondern höchstens 14 Änderungen (`replace`, `move`,
//! `remove`) und eine Erklärung. Dieses Modul prüft die Antwort in den Schritten 1 bis 3 aus
//! 9.4 und rechnet danach `planned_load` je Einheit und je Woche mit dem Lastmodell aus `core`
//! neu. Das Modell liefert nie eine Last.
//!
//! Fehler sind [`Issue`]s mit festem Code und höchstens Datum, Slot, JSON-Pfad und Stichwort.
//! Sie enthalten keinen Text aus der Antwort des Modells, weil sie als Codes in den
//! Korrekturaufruf gehen (9.4).

use std::collections::BTreeMap;

use chrono::NaiveDate;
use lastwerk_core::plan::{
    Hardness, Plan, PlanSession, PlanWeek, Rules, Slot, TemplateCatalog, Violation, round1,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Inhalt von `schema/plan-edit.schema.json`.
pub const EDIT_SCHEMA: &str = include_str!("../../../schema/plan-edit.schema.json");

/// Längste Erklärung in Zeichen (Schema `maxLength`, SPEC 7.2).
pub const MAX_EXPLANATION_CHARS: usize = 1200;

/// Höchstzahl der Änderungen je Antwort (Schema `maxItems`, SPEC 7.2).
pub const MAX_EDITS: usize = 14;

/// Höchstzahl der Fehler, die in einen Korrekturaufruf gehen (Konvention). Bei mehr bleibt der
/// Korrekturtext kurz, die ersten Fehler nach Datum und Code genügen als Hinweis.
pub const MAX_ISSUES: usize = 12;

/// Feste Fehlercodes der Prüfung. Regelverletzungen tragen den Regelcode (`R03`).
pub mod code {
    pub const NOT_JSON: &str = "NOT_JSON";
    pub const SCHEMA: &str = "SCHEMA";
    pub const UNKNOWN_TEMPLATE: &str = "UNKNOWN_TEMPLATE";
    pub const SPORT_MISMATCH: &str = "SPORT_MISMATCH";
    pub const DURATION_OUT_OF_WINDOW: &str = "DURATION_OUT_OF_WINDOW";
    pub const UNKNOWN_DATE: &str = "UNKNOWN_DATE";
    pub const NO_SESSION: &str = "NO_SESSION";
    pub const SLOT_OCCUPIED: &str = "SLOT_OCCUPIED";
    pub const BASE_PLAN_INVALID: &str = "BASE_PLAN_INVALID";
}

/// Ein Fehler der Antwort als Code mit Ort. Kein Freitext.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Issue {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<Slot>,
    /// JSON-Pfad in der Antwort des Modells (nur bei `SCHEMA`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Schlüsselwort des Schemas, das verletzt wurde (nur bei `SCHEMA`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword: Option<String>,
    /// Parameter der Regelverletzung, von `core` berechnet (nur bei Regelcodes).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub detail: BTreeMap<String, String>,
}

impl Issue {
    pub(crate) fn new(code: &str) -> Self {
        Self {
            code: code.to_string(),
            date: None,
            slot: None,
            path: None,
            keyword: None,
            detail: BTreeMap::new(),
        }
    }

    fn at(code: &str, date: NaiveDate, slot: Slot) -> Self {
        Self {
            date: Some(date),
            slot: Some(slot),
            ..Self::new(code)
        }
    }

    /// Schemafehler an `path` mit dem verletzten Schlüsselwort.
    pub(crate) fn schema(path: &str, keyword: &str) -> Self {
        Self {
            path: Some(path.to_string()),
            keyword: Some(keyword.to_string()),
            ..Self::new(code::SCHEMA)
        }
    }

    pub fn base_plan_invalid() -> Self {
        Self::new(code::BASE_PLAN_INVALID)
    }

    /// Reihenfolge für stabile Ausgaben: Datum, Code, Slot, Pfad, Stichwort.
    fn sort_key(
        &self,
    ) -> (
        Option<NaiveDate>,
        &str,
        Option<bool>,
        &Option<String>,
        &Option<String>,
    ) {
        (
            self.date,
            &self.code,
            self.slot.map(is_pm),
            &self.path,
            &self.keyword,
        )
    }

    /// Verletzung einer Regel als Fehler. `detail` kommt aus `core`.
    pub fn from_violation(v: &Violation) -> Self {
        Self {
            date: v.date,
            detail: v.detail.clone(),
            ..Self::new(&v.code)
        }
    }
}

/// Eine Änderung nach 7.2.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    Replace {
        date: NaiveDate,
        slot: Slot,
        template_id: String,
        duration_min: u32,
    },
    Move {
        from_date: NaiveDate,
        from_slot: Slot,
        to_date: NaiveDate,
        to_slot: Slot,
    },
    Remove {
        date: NaiveDate,
        slot: Slot,
    },
}

/// Antwort des Modells nach `plan-edit.schema.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanEdit {
    pub schema_version: u32,
    pub edits: Vec<Edit>,
    pub explanation: String,
}

/// Das Schema als JSON-Wert.
pub fn edit_schema() -> Value {
    serde_json::from_str(EDIT_SCHEMA).expect("plan-edit.schema.json ist gültiges JSON")
}

/// Kürzt `text` auf höchstens `max` Zeichen (Unicode-Skalarwerte, wie `maxLength` zählt).
pub fn truncate_chars(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((i, _)) => text[..i].to_string(),
        None => text.to_string(),
    }
}

/// Prüft die Antwort des Modells: gültiges JSON, Schema, Typen. Eine zu lange Erklärung wird
/// vor der Schemaprüfung auf [`MAX_EXPLANATION_CHARS`] gekürzt und gilt nicht als Fehler
/// (Konvention): Die Länge der Erklärung ist reine Anzeige, ein Korrekturaufruf dafür wäre
/// verschwendet. Alles andere am Schema ist ein Fehler.
pub fn parse_reply(content: &str) -> Result<PlanEdit, Vec<Issue>> {
    let mut value: Value =
        serde_json::from_str(content.trim()).map_err(|_| vec![Issue::new(code::NOT_JSON)])?;
    if let Some(Value::String(text)) = value.get_mut("explanation")
        && text.chars().nth(MAX_EXPLANATION_CHARS).is_some()
    {
        *text = truncate_chars(text, MAX_EXPLANATION_CHARS);
    }
    let validator = jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&edit_schema())
        .expect("plan-edit.schema.json ist ein gültiges Schema");
    let issues = schema_issues(&validator, &value, "");
    if !issues.is_empty() {
        return Err(issues);
    }
    serde_json::from_value(value).map_err(|_| {
        // Das Schema ließ die Antwort durch, die Typen nicht (zum Beispiel ein Datum, das es
        // nicht gibt). Der Pfad ist unbekannt, der Code genügt.
        vec![Issue {
            path: Some("/".into()),
            keyword: Some("type".into()),
            ..Issue::new(code::SCHEMA)
        }]
    })
}

/// Fehler der Schemaprüfung als [`Issue`]s mit JSON-Pfad (mit `prefix` davor) und
/// Schlüsselwort, sortiert und begrenzt. Kein Text aus der geprüften Antwort.
pub(crate) fn schema_issues(
    validator: &jsonschema::Validator,
    value: &Value,
    prefix: &str,
) -> Vec<Issue> {
    let mut issues: Vec<Issue> = validator
        .iter_errors(value)
        .map(|e| {
            let path = format!("{prefix}{}", e.instance_path());
            Issue::schema(
                if path.is_empty() { "/" } else { &path },
                e.kind().keyword(),
            )
        })
        .collect();
    issues.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    issues.dedup();
    issues.truncate(MAX_ISSUES);
    issues
}

fn is_pm(slot: Slot) -> bool {
    matches!(slot, Slot::Pm)
}

fn find_day(plan: &Plan, date: NaiveDate) -> Option<(usize, usize)> {
    for (wi, w) in plan.weeks.iter().enumerate() {
        if let Some(di) = w.days.iter().position(|d| d.date == date) {
            return Some((wi, di));
        }
    }
    None
}

fn session_index(plan: &Plan, wi: usize, di: usize, slot: Slot) -> Option<usize> {
    plan.weeks[wi].days[di]
        .sessions
        .iter()
        .position(|s| s.slot == slot)
}

fn sort_sessions(plan: &mut Plan, wi: usize, di: usize) {
    plan.weeks[wi].days[di]
        .sessions
        .sort_by_key(|s| is_pm(s.slot));
}

/// Summe der Einheitenlasten, auf eine Nachkommastelle gerundet wie im Planer.
fn week_load(week: &PlanWeek) -> f32 {
    let sum: f64 = week
        .days
        .iter()
        .flat_map(|d| d.sessions.iter())
        .map(|s| f64::from(s.planned_load))
        .sum();
    round1(sum)
}

/// Wendet `edits` der Reihe nach auf eine Kopie von `base` an (Schritt 3 aus 9.4). Jede
/// Änderung wird gegen den Katalog geprüft: Vorlage bekannt, Sportart wie im Slot, Dauer im
/// Fenster der Vorlage, Slot belegt oder frei wie die Operation es verlangt. Eine Änderung mit
/// Fehler wird übersprungen und als [`Issue`] gemeldet; der Aufrufer verwirft dann das
/// Ergebnis. `planned_load` der geänderten Einheiten und jeder Woche rechnet `core` neu
/// (`LoadModel::session_load`), `hard` und `rpe` kommen aus der Vorlage.
pub fn apply_edits(
    base: &Plan,
    edits: &[Edit],
    catalog: &TemplateCatalog,
    rules: &Rules,
) -> (Plan, Vec<Issue>) {
    let mut plan = base.clone();
    let mut issues = Vec::new();
    for edit in edits {
        match edit {
            Edit::Replace {
                date,
                slot,
                template_id,
                duration_min,
            } => {
                let Some((wi, di)) = find_day(&plan, *date) else {
                    issues.push(Issue {
                        date: Some(*date),
                        ..Issue::new(code::UNKNOWN_DATE)
                    });
                    continue;
                };
                let Some(si) = session_index(&plan, wi, di, *slot) else {
                    issues.push(Issue::at(code::NO_SESSION, *date, *slot));
                    continue;
                };
                let Some(template) = catalog.get(template_id) else {
                    issues.push(Issue::at(code::UNKNOWN_TEMPLATE, *date, *slot));
                    continue;
                };
                let session = &mut plan.weeks[wi].days[di].sessions[si];
                if template.sport != session.sport {
                    issues.push(Issue::at(code::SPORT_MISMATCH, *date, *slot));
                    continue;
                }
                let window = template.duration_min;
                if !(window.min..=window.max).contains(duration_min) {
                    issues.push(Issue::at(code::DURATION_OUT_OF_WINDOW, *date, *slot));
                    continue;
                }
                let hard = template.intensity_class == lastwerk_core::plan::IntensityClass::Hard;
                // Ein von R12 ersetzter lockerer Tag bleibt als ersetzt gekennzeichnet, solange
                // die neue Einheit nicht hart ist, sonst sähe R12 den nächsten harten Tag als
                // erste harte Einheit und meldete ihn.
                let keep_swap = !hard
                    && session
                        .reason_keys
                        .iter()
                        .any(|k| k == lastwerk_core::plan::reason_keys::SESSION_RECOVERY_SWAP);
                *session = PlanSession {
                    slot: *slot,
                    sport: session.sport,
                    template_id: template.id.clone(),
                    duration_min: *duration_min,
                    planned_load: rules.load_model.session_load(template, *duration_min),
                    hard,
                    rpe: template.rpe,
                    reason_keys: if keep_swap {
                        vec![lastwerk_core::plan::reason_keys::SESSION_RECOVERY_SWAP.to_string()]
                    } else {
                        Vec::new()
                    },
                };
            }
            Edit::Move {
                from_date,
                from_slot,
                to_date,
                to_slot,
            } => {
                let Some((fw, fd)) = find_day(&plan, *from_date) else {
                    issues.push(Issue {
                        date: Some(*from_date),
                        ..Issue::new(code::UNKNOWN_DATE)
                    });
                    continue;
                };
                let Some((tw, td)) = find_day(&plan, *to_date) else {
                    issues.push(Issue {
                        date: Some(*to_date),
                        ..Issue::new(code::UNKNOWN_DATE)
                    });
                    continue;
                };
                let Some(si) = session_index(&plan, fw, fd, *from_slot) else {
                    issues.push(Issue::at(code::NO_SESSION, *from_date, *from_slot));
                    continue;
                };
                if from_date == to_date && from_slot == to_slot {
                    continue;
                }
                if session_index(&plan, tw, td, *to_slot).is_some() {
                    issues.push(Issue::at(code::SLOT_OCCUPIED, *to_date, *to_slot));
                    continue;
                }
                let mut session = plan.weeks[fw].days[fd].sessions.remove(si);
                session.slot = *to_slot;
                plan.weeks[tw].days[td].sessions.push(session);
                sort_sessions(&mut plan, tw, td);
            }
            Edit::Remove { date, slot } => {
                let Some((wi, di)) = find_day(&plan, *date) else {
                    issues.push(Issue {
                        date: Some(*date),
                        ..Issue::new(code::UNKNOWN_DATE)
                    });
                    continue;
                };
                let Some(si) = session_index(&plan, wi, di, *slot) else {
                    issues.push(Issue::at(code::NO_SESSION, *date, *slot));
                    continue;
                };
                plan.weeks[wi].days[di].sessions.remove(si);
            }
        }
    }
    for week in &mut plan.weeks {
        week.planned_load = Some(week_load(week));
    }
    (plan, issues)
}

/// Harte Verletzungen als Fehler für den Korrekturaufruf, sortiert und begrenzt.
pub fn hard_issues(violations: &[Violation]) -> Vec<Issue> {
    let mut issues: Vec<Issue> = violations
        .iter()
        .filter(|v| v.hardness == Hardness::Hard)
        .map(Issue::from_violation)
        .collect();
    issues.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    issues.dedup();
    issues.truncate(MAX_ISSUES);
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_file_is_valid_json_with_the_documented_limits() {
        let schema = edit_schema();
        assert_eq!(schema["properties"]["edits"]["maxItems"], MAX_EDITS);
        assert_eq!(
            schema["properties"]["explanation"]["maxLength"],
            MAX_EXPLANATION_CHARS
        );
    }

    #[test]
    fn truncation_counts_characters_not_bytes() {
        assert_eq!(truncate_chars("äöü", 2), "äö");
        assert_eq!(truncate_chars("abc", 5), "abc");
        assert_eq!(truncate_chars("abc", 3), "abc");
        assert_eq!(truncate_chars("", 0), "");
    }

    #[test]
    fn valid_reply_is_parsed() {
        let reply = r#"{"schema_version":1,"edits":[
            {"op":"replace","date":"2027-01-05","slot":"am","template_id":"bike.endurance","duration_min":60},
            {"op":"move","from_date":"2027-01-05","from_slot":"am","to_date":"2027-01-06","to_slot":"pm"},
            {"op":"remove","date":"2027-01-07","slot":"am"}],
            "explanation":"Kurz."}"#;
        let edit = parse_reply(reply).unwrap();
        assert_eq!(edit.edits.len(), 3);
        assert_eq!(edit.explanation, "Kurz.");
    }

    #[test]
    fn invalid_replies_give_codes_without_text_from_the_reply() {
        let issues = parse_reply("das ist kein JSON").unwrap_err();
        assert_eq!(issues, vec![Issue::new(code::NOT_JSON)]);

        // Unbekanntes Feld, falsche Operation, zu viele Änderungen, falsches Datum.
        for reply in [
            r#"{"schema_version":1,"edits":[],"explanation":"x","plan":{}}"#,
            r#"{"schema_version":1,"edits":[{"op":"delete","date":"2027-01-05","slot":"am"}],"explanation":"x"}"#,
            r#"{"schema_version":1,"edits":[{"op":"remove","date":"morgen","slot":"am"}],"explanation":"x"}"#,
            r#"{"schema_version":2,"edits":[],"explanation":"x"}"#,
            r#"{"schema_version":1,"edits":[]}"#,
            r#"[]"#,
        ] {
            let issues = parse_reply(reply).unwrap_err();
            assert!(!issues.is_empty(), "{reply}");
            assert!(issues.iter().all(|i| i.code == code::SCHEMA), "{reply}");
            // Der JSON-Text der Fehler enthält nur Codes, Pfade und Stichwörter.
            let text = serde_json::to_string(&issues).unwrap();
            assert!(!text.contains("morgen"), "{text}");
            assert!(!text.contains("delete"), "{text}");
        }
        let many: Vec<String> = (0..15)
            .map(|_| r#"{"op":"remove","date":"2027-01-05","slot":"am"}"#.to_string())
            .collect();
        let reply = format!(
            r#"{{"schema_version":1,"edits":[{}],"explanation":"x"}}"#,
            many.join(",")
        );
        let issues = parse_reply(&reply).unwrap_err();
        assert_eq!(issues[0].keyword.as_deref(), Some("maxItems"));
    }

    #[test]
    fn long_explanation_is_cut_not_rejected() {
        let long = "ä".repeat(MAX_EXPLANATION_CHARS + 500);
        let reply = format!(r#"{{"schema_version":1,"edits":[],"explanation":"{long}"}}"#);
        let edit = parse_reply(&reply).unwrap();
        assert_eq!(edit.explanation.chars().count(), MAX_EXPLANATION_CHARS);
    }

    #[test]
    fn explanation_text_is_kept_unchanged() {
        let nasty = r#"<script>alert(1)<\/script> [klick](http://evil.example) **fett**"#;
        let reply = format!(r#"{{"schema_version":1,"edits":[],"explanation":"{nasty}"}}"#);
        let edit = parse_reply(&reply).unwrap();
        assert_eq!(
            edit.explanation,
            "<script>alert(1)</script> [klick](http://evil.example) **fett**"
        );
    }
}
