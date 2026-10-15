//! ICS-Export eines Plans (SPEC 10.2, F11).
//!
//! Eine Veranstaltung je Einheit, mit Titel, Beginn und Dauer. Es gibt bewusst keine Orte
//! (`LOCATION`, `GEO`), keine Schwellenwerte, keine Zonenwerte und keine Belastungszahlen, weil
//! eine ICS-Datei oft geteilt wird. Ruhetage haben keine Veranstaltung.
//!
//! Konventionen (Konvention, keine Quelle):
//! - Beginn: Slot `am` um 07:00, Slot `pm` um 17:00, jeweils als lokale Zeit ohne Zeitzone
//!   (floating time, kein `TZID`). Der Kalender des Nutzers zeigt sie in seiner Zeitzone. Der
//!   Plan kennt keine Uhrzeiten.
//! - Titel: Name der Vorlage aus `catalog/locales`, dazu die Sportart in Klammern, in der
//!   Sprache des Plans (entfällt, wenn der Name sie schon nennt).
//! - Beschreibung: Beschreibung der Vorlage und die Texte der Begründungsschlüssel der
//!   Einheit, je Zeile ein Text. Beides sind feste Texte aus den Sprachdateien.
//! - `UID`: `<plan_id>-<Datum>-<slot>`, damit ein erneuter Import die Einträge ersetzt statt
//!   zu verdoppeln. `DTSTAMP` ist `created_at` des Plans, damit die Datei bei gleichem Plan
//!   byteweise gleich bleibt.

use chrono::{NaiveDateTime, NaiveTime, TimeDelta};
use icalendar::{Calendar, Component, Event, EventLike, Property};
use lastwerk_core::plan::{Plan, PlanSession, PlanSport, Slot};

use std::io::Write;
use std::path::Path;

use crate::error::CliError;
use crate::planning::read_plan;
use crate::texts::Texts;

/// Beginn des Slots `am` (Konvention).
const AM_START: (u32, u32) = (7, 0);
/// Beginn des Slots `pm` (Konvention).
const PM_START: (u32, u32) = (17, 0);

fn start_time(slot: Slot) -> NaiveTime {
    let (h, m) = match slot {
        Slot::Am => AM_START,
        Slot::Pm => PM_START,
    };
    NaiveTime::from_hms_opt(h, m, 0).expect("feste Uhrzeit")
}

const fn slot_str(slot: Slot) -> &'static str {
    match slot {
        Slot::Am => "am",
        Slot::Pm => "pm",
    }
}

const fn sport_key(sport: PlanSport) -> &'static str {
    match sport {
        PlanSport::Cycling => "sport.cycling",
        PlanSport::Running => "sport.running",
        PlanSport::Swimming => "sport.swimming",
        PlanSport::Strength => "sport.strength",
        PlanSport::Rest => "sport.rest",
    }
}

/// Titel der Veranstaltung: `<Vorlagenname> (<Sportart>)`. Ohne Text zur Vorlage steht die
/// Kennung der Vorlage da.
fn title(s: &PlanSession, texts: &Texts) -> String {
    let name = texts
        .get(&format!("template.{}.name", s.template_id))
        .unwrap_or(&s.template_id);
    match texts.get(sport_key(s.sport)) {
        // Steht die Sportart schon im Namen ("Kraft, Programm A"), wäre sie doppelt.
        Some(sport) if !name.to_lowercase().contains(&sport.to_lowercase()) => {
            format!("{name} ({sport})")
        }
        _ => name.to_string(),
    }
}

fn description(s: &PlanSession, texts: &Texts) -> String {
    let mut lines: Vec<&str> = Vec::new();
    if let Some(d) = texts.get(&format!("template.{}.description", s.template_id)) {
        lines.push(d);
    }
    for key in &s.reason_keys {
        if let Some(t) = texts.get(key) {
            lines.push(t);
        }
    }
    lines.join("\n")
}

/// Baut den Kalender zum Plan. Gibt den ICS-Text zurück (Zeilenenden CRLF nach RFC 5545).
pub fn plan_to_ics(plan: &Plan, texts: &Texts) -> String {
    // `Calendar::new` setzt eine eigene PRODID der Bibliothek, deshalb ohne Vorgaben starten.
    let mut cal = Calendar::empty();
    cal.append_property(Property::new("VERSION", "2.0"));
    cal.append_property(Property::new("PRODID", "-//Lastwerk//Plan//EN"));
    cal.append_property(Property::new("CALSCALE", "GREGORIAN"));
    for week in &plan.weeks {
        for day in &week.days {
            let mut sessions: Vec<&PlanSession> = day.sessions.iter().collect();
            sessions.sort_by_key(|s| matches!(s.slot, Slot::Pm));
            for s in sessions {
                let start = NaiveDateTime::new(day.date, start_time(s.slot));
                let end = start + TimeDelta::minutes(i64::from(s.duration_min));
                let uid = format!("{}-{}-{}", plan.plan_id, day.date, slot_str(s.slot));
                let mut event = Event::new();
                event
                    .uid(&uid)
                    .timestamp(plan.created_at)
                    .summary(&title(s, texts))
                    .starts(start)
                    .ends(end);
                let desc = description(s, texts);
                if !desc.is_empty() {
                    event.description(&desc);
                }
                cal.push(event.done());
            }
        }
    }
    cal.to_string()
}

/// `lastwerk export-ics`: liest und prüft den Plan (Schema), schreibt die ICS-Datei.
pub fn run(plan_path: &Path, out_path: &Path, out: &mut dyn Write) -> Result<(), CliError> {
    let plan = read_plan(plan_path)?;
    let texts = Texts::load(plan.locale)?;
    let ics = plan_to_ics(&plan, &texts);
    std::fs::write(out_path, ics).map_err(|e| {
        CliError::Input(format!(
            "Die ICS-Datei lässt sich nicht schreiben: {}",
            e.kind()
        ))
    })?;
    let events: usize = plan
        .weeks
        .iter()
        .flat_map(|w| w.days.iter())
        .map(|d| d.sessions.len())
        .sum();
    writeln!(out, "ICS-Datei geschrieben: {events} Veranstaltungen.")
        .map_err(|e| CliError::Internal(format!("Ausgabefehler: {}", e.kind())))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::testctx::{event_ctx, rules_and_catalog};
    use chrono::NaiveDateTime;
    use lastwerk_core::plan::{Locale, generate_plan};

    fn plan(locale: Locale) -> Plan {
        let (rules, _) = rules_and_catalog();
        let ctx = lastwerk_core::plan::PlanContext {
            locale,
            ..event_ctx()
        };
        generate_plan(&ctx, &rules).unwrap()
    }

    fn sessions(plan: &Plan) -> Vec<(chrono::NaiveDate, &PlanSession)> {
        plan.weeks
            .iter()
            .flat_map(|w| w.days.iter())
            .flat_map(|d| d.sessions.iter().map(move |s| (d.date, s)))
            .collect()
    }

    #[test]
    fn slots_have_fixed_start_times() {
        assert_eq!(start_time(Slot::Am).to_string(), "07:00:00");
        assert_eq!(start_time(Slot::Pm).to_string(), "17:00:00");
    }

    #[test]
    fn the_calendar_parses_and_has_one_event_per_session() {
        let plan = plan(Locale::De);
        let ics = plan_to_ics(&plan, &Texts::load(Locale::De).unwrap());
        // RFC 5545: Zeilenenden CRLF.
        assert!(ics.contains("\r\n"));
        assert!(!ics.replace("\r\n", "").contains('\n'));

        // Mit der Bibliothek einlesen (Parser-Feature, nur Tests).
        let cal: icalendar::Calendar = ics.parse().expect("ICS lässt sich einlesen");
        assert_eq!(cal.components.len(), sessions(&plan).len());

        let unfolded = icalendar::parser::unfold(&ics);
        let parsed = icalendar::parser::read_calendar(&unfolded).expect("ICS lässt sich einlesen");
        let events: Vec<_> = parsed
            .components
            .iter()
            .filter(|c| c.name == "VEVENT")
            .collect();
        assert_eq!(events.len(), sessions(&plan).len());
        assert_eq!(events.len(), parsed.components.len());
        assert!(!events.is_empty());
    }

    #[test]
    fn events_carry_only_title_time_and_duration() {
        let plan = plan(Locale::De);
        let ics = plan_to_ics(&plan, &Texts::load(Locale::De).unwrap());
        // Keine Orte, keine Koordinaten, keine Zeitzonen.
        for forbidden in ["LOCATION", "GEO", "TZID", "ATTENDEE", "ORGANIZER", "URL"] {
            assert!(!ics.contains(forbidden), "{forbidden}");
        }
        let unfolded = icalendar::parser::unfold(&ics);
        let parsed = icalendar::parser::read_calendar(&unfolded).unwrap();
        let allowed: BTreeSet<&str> = [
            "UID",
            "DTSTAMP",
            "DTSTART",
            "DTEND",
            "SUMMARY",
            "DESCRIPTION",
        ]
        .into_iter()
        .collect();
        let mut uids = BTreeSet::new();
        let list = sessions(&plan);
        for (event, (day, session)) in parsed.components.iter().zip(&list) {
            for p in &event.properties {
                assert!(allowed.contains(p.name.as_ref()), "{}", p.name.as_ref());
            }
            let get = |name: &str| event.find_prop(name).unwrap().val.as_ref().to_string();
            // UID, Beginn nach Konvention, Dauer wie im Plan.
            let slot = if session.slot == Slot::Am { "am" } else { "pm" };
            assert_eq!(get("UID"), format!("{}-{day}-{slot}", plan.plan_id));
            assert!(uids.insert(get("UID")), "UID doppelt");
            let fmt = "%Y%m%dT%H%M%S";
            let start = NaiveDateTime::parse_from_str(&get("DTSTART"), fmt).unwrap();
            let end = NaiveDateTime::parse_from_str(&get("DTEND"), fmt).unwrap();
            assert_eq!(start.date(), *day);
            let hour = if session.slot == Slot::Am { 7 } else { 17 };
            assert_eq!(start.time().to_string(), format!("{hour:02}:00:00"));
            assert_eq!((end - start).num_minutes(), i64::from(session.duration_min));
            assert!(!get("SUMMARY").is_empty());
            // DTSTAMP ist die Erzeugungszeit des Plans, nicht die Uhr beim Export.
            assert_eq!(get("DTSTAMP"), "20261013T080000Z");
        }
        assert_eq!(uids.len(), list.len());
    }

    #[test]
    fn rest_days_have_no_event_and_titles_follow_the_plan_language() {
        for (locale, expected) in [
            (Locale::De, "Grundlagenfahrt (Rad)"),
            (Locale::En, "Steady endurance ride (Cycling)"),
        ] {
            let plan = plan(locale);
            let texts = Texts::load(locale).unwrap();
            let ics = plan_to_ics(&plan, &texts);
            let days_without_session = plan
                .weeks
                .iter()
                .flat_map(|w| w.days.iter())
                .filter(|d| d.sessions.is_empty())
                .count();
            assert!(days_without_session > 0, "der Plan hat Ruhetage");
            let events = ics.matches("BEGIN:VEVENT").count();
            assert_eq!(events, sessions(&plan).len());
            // Jede Vorlage der Sprache hat einen Titel.
            for (_, s) in sessions(&plan) {
                let name = texts
                    .get(&format!("template.{}.name", s.template_id))
                    .unwrap_or_else(|| panic!("Titel für {} fehlt", s.template_id));
                assert!(ics.contains(&format!("SUMMARY:{}", name.replace(',', "\\,"))));
            }
            assert!(
                ics.contains(&format!("SUMMARY:{expected}")),
                "{expected} fehlt"
            );
        }
    }

    #[test]
    fn the_export_is_stable() {
        let plan = plan(Locale::De);
        let texts = Texts::load(Locale::De).unwrap();
        assert_eq!(plan_to_ics(&plan, &texts), plan_to_ics(&plan, &texts));
    }
}
