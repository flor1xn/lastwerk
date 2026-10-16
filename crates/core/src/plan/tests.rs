//! Tests des Planers: Katalog, Regeln, Sprachdateien, Einheiten je Regel, Eigenschaften und
//! Golden-Pläne (SPEC 11.4). Die Kataloge lädt nur der Test (`include_str!`), der Produktivcode
//! bekommt sie übergeben.

use std::collections::BTreeSet;

use chrono::{DateTime, Datelike, NaiveDate, TimeDelta, TimeZone, Utc};
use proptest::prelude::*;
use serde_json::Value;

use super::*;
use crate::load::Sport;
use crate::pmc::{DailyLoad, PmcParams, pmc};

const TEMPLATES_JSON: &str = include_str!("../../../../catalog/templates.json");
const RULES_JSON: &str = include_str!("../../../../catalog/rules.default.json");
const DE_JSON: &str = include_str!("../../../../catalog/locales/de.json");
const EN_JSON: &str = include_str!("../../../../catalog/locales/en.json");

pub(super) fn catalog() -> TemplateCatalog {
    serde_json::from_str(TEMPLATES_JSON).expect("templates.json lässt sich lesen")
}

pub(super) fn rules() -> Rules {
    serde_json::from_str(RULES_JSON).expect("rules.default.json lässt sich lesen")
}

pub(super) fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn created_at() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 13, 8, 0, 0).unwrap()
}

/// Montag, an dem die meisten Tests beginnen (4. Januar 2027).
pub(super) fn monday() -> NaiveDate {
    date(2027, 1, 4)
}

pub(super) fn avail(max_minutes: u16, allow_hard: bool, fixed_rest: bool) -> DayAvailability {
    DayAvailability {
        max_minutes,
        allow_hard,
        fixed_rest,
    }
}

/// Üblicher Kontext: Montag, Mittwoch und Sonntag ohne harte Einheiten, Freitag frei, Samstag
/// lang.
fn typical_availability() -> [DayAvailability; 7] {
    [
        avail(60, false, false),
        avail(90, true, false),
        avail(75, false, false),
        avail(90, true, false),
        avail(0, false, true),
        avail(240, true, false),
        avail(180, false, false),
    ]
}

pub(super) fn ctx() -> PlanContext {
    PlanContext {
        plan_id: "00000000-0000-4000-8000-000000000001".to_string(),
        created_at: created_at(),
        locale: Locale::De,
        week_start: monday(),
        goal: GoalSpec::Continuous,
        horizon_weeks: 8,
        ctl0: 40.0,
        atl0: 45.0,
        history: Vec::new(),
        availability: typical_availability(),
        sports: vec![
            SportWeight {
                sport: PlanSport::Cycling,
                weight: 3.0,
            },
            SportWeight {
                sport: PlanSport::Running,
                weight: 2.0,
            },
            SportWeight {
                sport: PlanSport::Swimming,
                weight: 1.0,
            },
        ],
        strength_per_week: 1,
        ctl_rise_per_week: None,
        pmc_params: PmcParams::default(),
        catalog: catalog(),
        recovery: Vec::new(),
    }
}

/// Kontext, in dem Verfügbarkeit nichts begrenzt, damit ein Test genau eine Regel trifft.
pub(super) fn open_ctx() -> PlanContext {
    PlanContext {
        availability: [avail(360, true, false); 7],
        strength_per_week: 0,
        ..ctx()
    }
}

// ---------------------------------------------------------------------------
// Hilfen zum Bauen von Plänen von Hand
// ---------------------------------------------------------------------------

pub(super) fn blank_week(start: NaiveDate, phase: Phase) -> PlanWeek {
    PlanWeek {
        week_start: start,
        phase,
        target_load: 0.0,
        planned_load: None,
        reason_keys: Vec::new(),
        days: (0..7)
            .map(|i| PlanDay {
                date: start + TimeDelta::days(i),
                sessions: Vec::new(),
            })
            .collect(),
    }
}

/// Legt eine Einheit nach Vorlage auf Tag `day` der Woche. Sportart, Härte und Last folgen
/// aus Katalog und Regeln.
pub(super) fn put(week: &mut PlanWeek, day: usize, template: &str, minutes: u32) {
    let cat = catalog();
    let t = cat.get(template).expect("Vorlage im Katalog");
    let slot = if week.days[day].sessions.is_empty() {
        Slot::Am
    } else {
        Slot::Pm
    };
    week.days[day].sessions.push(PlanSession {
        slot,
        sport: t.sport,
        template_id: t.id.clone(),
        duration_min: minutes,
        planned_load: rules().load_model.session_load(t, minutes),
        hard: t.intensity_class == IntensityClass::Hard,
        rpe: t.rpe,
        reason_keys: Vec::new(),
    });
}

fn put_load(week: &mut PlanWeek, day: usize, template: &str, minutes: u32, load: f32) {
    put(week, day, template, minutes);
    week.days[day].sessions.last_mut().unwrap().planned_load = load;
}

pub(super) fn plan_of(weeks: Vec<PlanWeek>, goal: Goal) -> Plan {
    Plan {
        schema_version: PLAN_SCHEMA_VERSION,
        plan_id: "00000000-0000-4000-8000-000000000002".to_string(),
        created_at: created_at(),
        locale: Locale::De,
        source: PlanSource::Rules,
        rules_version: "1.0.0".to_string(),
        goal,
        weeks,
        warnings: Vec::new(),
        explanation: None,
    }
}

pub(super) fn continuous_goal() -> Goal {
    Goal {
        kind: GoalKind::Continuous,
        event_date: None,
        sport: None,
        priority: None,
    }
}

fn codes(v: &[Violation], hardness: Hardness) -> Vec<String> {
    v.iter()
        .filter(|x| x.hardness == hardness)
        .map(|x| x.code.clone())
        .collect()
}

fn has(v: &[Violation], code: &str, hardness: Hardness) -> bool {
    v.iter().any(|x| x.code == code && x.hardness == hardness)
}

/// Ein ruhiger Plan, den keine Regel beanstandet: drei lockere Einheiten und viel Ruhe.
fn calm_week(start: NaiveDate) -> PlanWeek {
    let mut w = blank_week(start, Phase::Base);
    put(&mut w, 1, "bike.endurance.steady", 60);
    put(&mut w, 3, "run.easy.steady", 40);
    put(&mut w, 5, "bike.endurance.steady", 90);
    w
}

// ---------------------------------------------------------------------------
// Katalog, Regeln, Sprachdateien
// ---------------------------------------------------------------------------

#[test]
fn catalog_has_the_14_templates_of_spec_7_4_in_order() {
    let cat = catalog();
    let ids: Vec<&str> = cat.templates.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "bike.endurance.steady",
            "bike.recovery.spin",
            "bike.tempo.blocks",
            "bike.threshold.2x20",
            "bike.vo2.5x4",
            "run.easy.steady",
            "run.long.steady",
            "run.tempo.continuous",
            "run.intervals.6x3",
            "swim.technique.easy",
            "swim.threshold.sets",
            "strength.general.a",
            "strength.general.b",
            "rest.day",
        ]
    );
    cat.validate().expect("Katalog ist stimmig");
}

#[test]
fn template_ids_match_the_schema_pattern() {
    for t in &catalog().templates {
        assert!(is_valid_template_id(&t.id), "{}", t.id);
        assert!(is_valid_reason_key(&t.text_key), "{}", t.text_key);
    }
    for bad in [
        "", "bike", ".x", "Bike.x", "bike.", "bike..x", "bike.X", "1bike.x", "bike.a-b",
    ] {
        assert!(!is_valid_template_id(bad), "{bad}");
    }
    assert!(is_valid_template_id("bike.vo2.5x4"));
    assert!(is_valid_template_id("rest.day"));
}

#[test]
fn catalog_classes_follow_spec_7_4() {
    let cat = catalog();
    let class = |id: &str| cat.get(id).unwrap().intensity_class;
    for id in [
        "bike.endurance.steady",
        "bike.recovery.spin",
        "run.easy.steady",
        "swim.technique.easy",
        "rest.day",
    ] {
        assert_eq!(class(id), IntensityClass::Easy, "{id}");
    }
    for id in [
        "bike.tempo.blocks",
        "run.long.steady",
        "strength.general.a",
        "strength.general.b",
    ] {
        assert_eq!(class(id), IntensityClass::Moderate, "{id}");
    }
    for id in [
        "bike.threshold.2x20",
        "bike.vo2.5x4",
        "run.tempo.continuous",
        "run.intervals.6x3",
        "swim.threshold.sets",
    ] {
        assert_eq!(class(id), IntensityClass::Hard, "{id}");
    }
}

#[test]
fn spec_example_template_is_reproduced() {
    // SPEC 7.4: 15 Minuten warm, 2 mal 20 Minuten Zone 4 mit 5 Minuten Pause, 10 Minuten aus.
    let cat = catalog();
    let t = cat.get("bike.threshold.2x20").unwrap();
    assert_eq!(t.duration_min, DurationWindow { min: 60, max: 120 });
    assert_eq!(t.steps.len(), 3);
    assert_eq!(t.steps[1].kind, StepKind::Interval);
    assert_eq!(t.steps[1].reps, Some(2));
    assert_eq!(t.steps[1].minutes, 20);
    assert_eq!(t.steps[1].rest_minutes, Some(5));
    // Nebenrechnung: Intervallblock 2 * 20 + 1 * 5 = 45, dehnbar 15 + 10 = 25, Nenndauer 70.
    assert_eq!(t.fixed_minutes(), 45);
    assert_eq!(t.flexible_minutes(), 25);
    assert_eq!(t.nominal_minutes(), 70);
}

#[test]
fn easy_zone_minutes_follow_the_hand_calculation() {
    let cat = catalog();
    // bike.threshold.2x20 bei 70 Minuten (Nenndauer): leicht sind Aufwärmen (Zone 2) 15,
    // Pause 5 und Abwärmen 10 = 30 Minuten.
    let t = cat.get("bike.threshold.2x20").unwrap();
    assert!((t.easy_zone_minutes(70).unwrap() - 30.0).abs() < 1e-9);
    // Bei 60 Minuten bleiben 45 fest, die dehnbaren 15 Minuten teilen sich 15:10. Leicht:
    // 15 * (15 / 25) + 10 * (15 / 25) = 15, dazu die Pause 5 = 20.
    assert!((t.easy_zone_minutes(60).unwrap() - 20.0).abs() < 1e-9);
    // Gleichmäßige Zone-2-Fahrt ist ganz leicht.
    let e = cat.get("bike.endurance.steady").unwrap();
    assert!((e.easy_zone_minutes(120).unwrap() - 120.0).abs() < 1e-9);
    // Kraft hat keine Zonen.
    assert!(
        cat.get("strength.general.a")
            .unwrap()
            .easy_zone_minutes(45)
            .is_none()
    );
}

#[test]
fn catalog_validation_rejects_broken_templates() {
    let mut cat = catalog();
    cat.templates[0].id = "Bike.Endurance".to_string();
    assert_eq!(cat.validate().unwrap_err().code(), "catalog_invalid");

    let mut cat = catalog();
    cat.templates[1].id = cat.templates[0].id.clone();
    assert!(cat.validate().is_err());

    let mut cat = catalog();
    cat.templates[0].steps[0].zone_scheme = Some("run_speed5".to_string());
    assert!(cat.validate().is_err());

    let mut cat = catalog();
    // Mindestdauer unter den Intervallblöcken (45 Minuten) ist unmöglich.
    cat.templates[3].duration_min.min = 40;
    assert!(cat.validate().is_err());

    let mut cat = catalog();
    cat.templates[11].rpe = None;
    assert!(cat.validate().is_err());
}

#[test]
fn rules_default_loads_validates_and_names_sources() {
    let r = rules();
    r.validate().expect("Standardregeln sind stimmig");
    assert_eq!(r.version, "1.0.0");
    assert!(
        r.reserved.is_empty(),
        "R12 ist umgesetzt und nicht mehr reserviert"
    );
    // SPEC 8.4: Härte und Quelle je Regel.
    let expected = [
        ("R01", Hardness::Hard, "Faustregel"),
        ("R02", Hardness::Soft, "Faustregel"),
        ("R03", Hardness::Hard, "Faustregel"),
        ("R04", Hardness::Hard, "Faustregel"),
        ("R05", Hardness::Hard, "Faustregel"),
        ("R06", Hardness::Soft, "Faustregel"),
        ("R07", Hardness::Soft, "Faustregel"),
        ("R08", Hardness::Soft, "Q5"),
        ("R09", Hardness::Hard, "Q10"),
        ("R10", Hardness::Soft, "Faustregel"),
        ("R11", Hardness::Hard, "Nutzereingabe"),
        ("R12", Hardness::Soft, "Q6"),
    ];
    for ((code, meta), (ecode, hardness, source)) in r.metas().iter().zip(expected) {
        assert_eq!(*code, ecode);
        assert_eq!(meta.hardness, hardness, "{code}");
        assert_eq!(meta.source, source, "{code}");
        assert!(is_valid_source(&meta.source));
    }
    // SPEC 8.4: Standardwerte.
    assert_eq!(r.r01.max_ctl_rise_per_week, 5.0);
    assert_eq!(r.r01.max_setting, 8.0);
    assert_eq!((r.r02.warn_at, r.r02.reject_at), (0.10, 0.30));
    assert_eq!(r.r03.max_hard_per_week, 2);
    assert_eq!(r.r05.min_rest_days_per_week, 1);
    assert_eq!(r.r06.load_weeks, 3);
    assert_eq!((r.r06.recovery_min, r.r06.recovery_max), (0.6, 0.7));
    assert_eq!(
        (
            r.r07.min_per_week,
            r.r07.max_per_week,
            r.r07.max_per_week_taper
        ),
        (1, 2, 1)
    );
    assert_eq!(r.r08.min_easy_share, 0.7);
    assert_eq!((r.r09.taper_days_min, r.r09.taper_days_max), (7, 14));
    assert_eq!(r.r10.tsb_warn_below, -30.0);
    // R12: Schwellen sind Faustregeln, die Basislinie ein 7-Tage-Mittel (Q6, sekundär).
    assert_eq!(
        (r.r12.hrv_drop_fraction, r.r12.sleep_deficit_fraction),
        (0.15, 0.20)
    );
    assert_eq!(
        (
            r.r12.baseline_days,
            r.r12.min_baseline_days,
            r.r12.max_age_days
        ),
        (7, 4, 3)
    );
}

#[test]
fn rules_validation_rejects_contradictions() {
    let mut r = rules();
    r.version = "1.0".to_string();
    assert_eq!(r.validate().unwrap_err().code(), "invalid_rules");
    let mut r = rules();
    r.r01.max_ctl_rise_per_week = 9.0;
    assert!(r.validate().is_err());
    let mut r = rules();
    r.phases.event.taper_weeks = 2;
    assert!(
        r.validate().is_err(),
        "2 Wochen Taper reichen bis 20 Tage, R9 erlaubt 14"
    );
    let mut r = rules();
    r.r02.warn_at = 0.4;
    assert!(r.validate().is_err());
    let mut r = rules();
    r.r05.meta.source = "Erfindung".to_string();
    assert!(r.validate().is_err());
}

fn lookup<'a>(v: &'a Value, path: &str) -> Option<&'a str> {
    let mut node = v;
    for p in path.split('.') {
        node = node.get(p)?;
    }
    node.as_str()
}

#[test]
fn all_texts_exist_in_both_languages() {
    let de: Value = serde_json::from_str(DE_JSON).unwrap();
    let en: Value = serde_json::from_str(EN_JSON).unwrap();
    let mut paths: Vec<String> = Vec::new();
    for t in &catalog().templates {
        paths.push(format!("{}.name", t.text_key));
        paths.push(format!("{}.description", t.text_key));
    }
    for k in reason_keys::ALL {
        paths.push(k.to_string());
    }
    for n in 1..=12 {
        paths.push(format!("rule.R{n:02}.name"));
        paths.push(format!("rule.R{n:02}.explanation"));
    }
    for p in &paths {
        for (name, tree) in [("de", &de), ("en", &en)] {
            let text = lookup(tree, p).unwrap_or_else(|| panic!("{name}: Schlüssel {p} fehlt"));
            assert!(!text.is_empty(), "{name}: {p} ist leer");
            assert!(
                !text.contains('–') && !text.contains('—'),
                "{name}: {p} hat Gedankenstrich"
            );
        }
    }
}

#[test]
fn reason_keys_match_the_schema_pattern_and_are_unique() {
    let mut seen = BTreeSet::new();
    for k in reason_keys::ALL {
        assert!(is_valid_reason_key(k), "{k}");
        assert!(seen.insert(k), "doppelt: {k}");
    }
}

// ---------------------------------------------------------------------------
// Kontextprüfung
// ---------------------------------------------------------------------------

#[test]
fn context_errors_are_reported_with_codes() {
    let r = rules();
    let mut c = ctx();
    c.week_start = date(2027, 1, 5);
    assert_eq!(
        generate_plan(&c, &r).unwrap_err().code(),
        "week_start_not_monday"
    );

    let mut c = ctx();
    c.horizon_weeks = 27;
    assert_eq!(generate_plan(&c, &r).unwrap_err().code(), "invalid_horizon");
    c.horizon_weeks = 0;
    assert_eq!(generate_plan(&c, &r).unwrap_err().code(), "invalid_horizon");

    let mut c = ctx();
    c.goal = GoalSpec::Event {
        event_date: date(2027, 1, 2),
        sport: PlanSport::Cycling,
        priority: Priority::A,
    };
    assert_eq!(
        generate_plan(&c, &r).unwrap_err().code(),
        "event_before_start"
    );

    let mut c = ctx();
    c.sports.clear();
    assert_eq!(generate_plan(&c, &r).unwrap_err().code(), "no_sports");

    let mut c = ctx();
    c.ctl_rise_per_week = Some(9.0);
    assert_eq!(
        generate_plan(&c, &r).unwrap_err().code(),
        "invalid_ctl_rise"
    );

    let mut c = ctx();
    c.catalog
        .templates
        .retain(|t| t.sport != PlanSport::Swimming);
    assert_eq!(
        generate_plan(&c, &r).unwrap_err().code(),
        "missing_template"
    );
}

#[test]
fn ctl_rise_setting_up_to_8_is_accepted() {
    let r = rules();
    let mut c = ctx();
    c.ctl_rise_per_week = Some(8.0);
    assert!(generate_plan(&c, &r).is_ok());
}

// ---------------------------------------------------------------------------
// Einheiten je Regel: Pläne mit absichtlicher Verletzung
// ---------------------------------------------------------------------------

#[test]
fn calm_plan_passes_all_rules() {
    let c = open_ctx();
    let plan = plan_of(vec![calm_week(monday())], continuous_goal());
    let v = validate_plan(&plan, &c, &rules());
    assert!(v.is_empty(), "{v:?}");
    assert!(plan.is_acceptable(&v));
}

#[test]
fn r04_two_hard_days_in_a_row_are_rejected() {
    let c = open_ctx();
    let mut w = calm_week(monday());
    put(&mut w, 2, "bike.threshold.2x20", 70);
    put(&mut w, 3, "run.tempo.continuous", 40);
    let plan = plan_of(vec![w], continuous_goal());
    let v = validate_plan(&plan, &c, &rules());
    assert!(has(&v, "R04", Hardness::Hard), "{v:?}");
    assert!(!plan.is_acceptable(&v));
}

#[test]
fn r04_applies_across_the_week_boundary() {
    let c = open_ctx();
    let mut w1 = blank_week(monday(), Phase::Base);
    put(&mut w1, 2, "bike.endurance.steady", 60);
    put(&mut w1, 6, "bike.threshold.2x20", 70);
    let mut w2 = blank_week(monday() + TimeDelta::days(7), Phase::Base);
    put(&mut w2, 0, "run.tempo.continuous", 40);
    put(&mut w2, 2, "bike.endurance.steady", 60);
    let plan = plan_of(vec![w1, w2], continuous_goal());
    let v = validate_plan(&plan, &c, &rules());
    assert!(has(&v, "R04", Hardness::Hard), "{v:?}");
    assert_eq!(
        v.iter().find(|x| x.code == "R04").unwrap().date,
        Some(monday() + TimeDelta::days(7))
    );
}

#[test]
fn r03_three_hard_sessions_in_a_week_are_rejected() {
    let c = open_ctx();
    let mut w = blank_week(monday(), Phase::Build);
    put(&mut w, 0, "bike.threshold.2x20", 70);
    put(&mut w, 2, "run.tempo.continuous", 40);
    put(&mut w, 4, "bike.vo2.5x4", 66);
    put(&mut w, 5, "bike.endurance.steady", 60);
    let plan = plan_of(vec![w], continuous_goal());
    let v = validate_plan(&plan, &c, &rules());
    assert!(has(&v, "R03", Hardness::Hard), "{v:?}");
    assert!(!has(&v, "R04", Hardness::Hard));
    assert!(!plan.is_acceptable(&v));
}

#[test]
fn r05_week_without_rest_day_is_rejected() {
    let c = open_ctx();
    let mut w = blank_week(monday(), Phase::Base);
    for d in 0..7 {
        put(&mut w, d, "bike.endurance.steady", 60);
    }
    let plan = plan_of(vec![w], continuous_goal());
    let v = validate_plan(&plan, &c, &rules());
    assert!(has(&v, "R05", Hardness::Hard), "{v:?}");
    assert!(!plan.is_acceptable(&v));
}

#[test]
fn r11_availability_is_never_exceeded() {
    // Dienstag hat 90 Minuten, geplant sind 120.
    let c = ctx();
    let mut w = blank_week(monday(), Phase::Base);
    put(&mut w, 1, "bike.endurance.steady", 120);
    let plan = plan_of(vec![w], continuous_goal());
    let v = validate_plan(&plan, &c, &rules());
    assert!(has(&v, "R11", Hardness::Hard), "{v:?}");
    assert!(!plan.is_acceptable(&v));
}

#[test]
fn r11_fixed_rest_day_and_forbidden_hard_day_are_rejected() {
    let c = ctx();
    // Freitag ist fester Ruhetag, Montag erlaubt keine harte Einheit.
    let mut w = blank_week(monday(), Phase::Base);
    put(&mut w, 4, "run.easy.steady", 30);
    put(&mut w, 0, "bike.threshold.2x20", 60);
    let plan = plan_of(vec![w], continuous_goal());
    let v = validate_plan(&plan, &c, &rules());
    let r11: Vec<&Violation> = v.iter().filter(|x| x.code == "R11").collect();
    assert_eq!(r11.len(), 2, "{v:?}");
    assert!(r11.iter().all(|x| x.hardness == Hardness::Hard));
}

#[test]
fn r01_ctl_rise_above_5_is_rejected() {
    let c = open_ctx();
    // Ein einziger Tag mit Last 700 bei CTL0 = 40: CTL fällt bis Sonntag nur wenig zurück,
    // Tag 1: 40 + (700 - 40) * 0,023528 = 55,53, der Anstieg liegt weit über 5.
    let mut w = blank_week(monday(), Phase::Base);
    put_load(&mut w, 0, "bike.endurance.steady", 300, 700.0);
    let plan = plan_of(vec![w], continuous_goal());
    let v = validate_plan(&plan, &c, &rules());
    assert!(has(&v, "R01", Hardness::Hard), "{v:?}");
    assert!(!plan.is_acceptable(&v));
}

#[test]
fn r01_limit_is_adjustable_up_to_8() {
    // Wochenlast 7 * 80 = 560 bei CTL0 = 40: gleichmäßig 80 je Tag. Anstieg nach 7 Tagen
    // = (80 - 40) * (1 - exp(-7/42)) = 40 * 0,153518 = 6,14.
    let mut c = open_ctx();
    let mut w = blank_week(monday(), Phase::Base);
    for d in 0..7 {
        put_load(&mut w, d, "bike.endurance.steady", 60, 80.0);
    }
    // R5 verletzt das absichtlich, hier zählt nur R1.
    let plan = plan_of(vec![w], continuous_goal());
    let strict = validate_plan(&plan, &c, &rules());
    assert!(has(&strict, "R01", Hardness::Hard), "{strict:?}");
    c.ctl_rise_per_week = Some(8.0);
    let relaxed = validate_plan(&plan, &c, &rules());
    assert!(!relaxed.iter().any(|x| x.code == "R01"), "{relaxed:?}");
}

fn history_week(minutes: f32, load: f32) -> Vec<WeekHistory> {
    vec![WeekHistory {
        week_start: monday() - TimeDelta::days(7),
        sport: Sport::Cycling,
        load,
        minutes,
    }]
}

#[test]
fn r02_volume_plus_35_percent_is_rejected() {
    let mut c = open_ctx();
    c.history = history_week(200.0, 150.0);
    // 270 Minuten gegen 200: +35 Prozent, ab 30 Prozent hart.
    let mut w = blank_week(monday(), Phase::Base);
    put(&mut w, 1, "bike.endurance.steady", 90);
    put(&mut w, 3, "bike.endurance.steady", 90);
    put(&mut w, 5, "bike.endurance.steady", 90);
    let plan = plan_of(vec![w], continuous_goal());
    let v = validate_plan(&plan, &c, &rules());
    assert!(has(&v, "R02", Hardness::Hard), "{v:?}");
    assert!(!plan.is_acceptable(&v));
}

#[test]
fn r02_volume_plus_15_percent_warns_and_exactly_30_percent_is_rejected() {
    let mut c = open_ctx();
    c.history = history_week(200.0, 150.0);
    // 230 Minuten gegen 200: +15 Prozent, weich.
    let mut w = blank_week(monday(), Phase::Base);
    put(&mut w, 1, "bike.endurance.steady", 100);
    put(&mut w, 3, "bike.endurance.steady", 130);
    let plan = plan_of(vec![w], continuous_goal());
    let v = validate_plan(&plan, &c, &rules());
    assert!(has(&v, "R02", Hardness::Soft), "{v:?}");
    assert!(plan.is_acceptable(&v));
    // 260 gegen 200: genau 30 Prozent, "ab 30 Prozent" lehnt ab.
    let mut w = blank_week(monday(), Phase::Base);
    put(&mut w, 1, "bike.endurance.steady", 130);
    put(&mut w, 3, "bike.endurance.steady", 130);
    let plan = plan_of(vec![w], continuous_goal());
    let v = validate_plan(&plan, &c, &rules());
    assert!(has(&v, "R02", Hardness::Hard), "{v:?}");
    // 220 gegen 200: genau 10 Prozent warnt, 219 nicht.
    let mut w = blank_week(monday(), Phase::Base);
    put(&mut w, 1, "bike.endurance.steady", 110);
    put(&mut w, 3, "bike.endurance.steady", 110);
    let v = validate_plan(&plan_of(vec![w], continuous_goal()), &c, &rules());
    assert!(has(&v, "R02", Hardness::Soft), "{v:?}");
    let mut w = blank_week(monday(), Phase::Base);
    put(&mut w, 1, "bike.endurance.steady", 110);
    put(&mut w, 3, "bike.endurance.steady", 109);
    let v = validate_plan(&plan_of(vec![w], continuous_goal()), &c, &rules());
    assert!(!v.iter().any(|x| x.code == "R02"), "{v:?}");
}

#[test]
fn r02_after_a_recovery_week_the_reference_is_the_last_load_week() {
    let c = open_ctx();
    let mut w1 = blank_week(monday(), Phase::Base);
    put(&mut w1, 1, "bike.endurance.steady", 100);
    put(&mut w1, 3, "bike.endurance.steady", 100);
    let mut w2 = blank_week(monday() + TimeDelta::days(7), Phase::Recovery);
    put(&mut w2, 1, "bike.endurance.steady", 65);
    put(&mut w2, 3, "bike.endurance.steady", 65);
    // 210 Minuten gegen 130 der Erholungswoche wären +61 Prozent, gegen 200 der letzten
    // Belastungswoche sind es +5 Prozent.
    let mut w3 = blank_week(monday() + TimeDelta::days(14), Phase::Base);
    put(&mut w3, 1, "bike.endurance.steady", 105);
    put(&mut w3, 3, "bike.endurance.steady", 105);
    let plan = plan_of(vec![w1, w2, w3], continuous_goal());
    let v = validate_plan(&plan, &c, &rules());
    assert!(!v.iter().any(|x| x.code == "R02"), "{v:?}");
}

#[test]
fn r09_taper_of_3_days_is_rejected() {
    // Termin Donnerstag 2027-02-04. Davor zwei Aufbauwochen, die Rennwoche beginnt am
    // Montag 2027-02-01: Taper = 3 Tage, erlaubt sind 7 bis 14.
    let c = open_ctx();
    let w1 = calm_week(date(2027, 1, 18));
    let w2 = calm_week(date(2027, 1, 25));
    let mut w3 = blank_week(date(2027, 2, 1), Phase::Race);
    put(&mut w3, 0, "run.easy.steady", 30);
    let goal = Goal {
        kind: GoalKind::Event,
        event_date: Some(date(2027, 2, 4)),
        sport: Some(PlanSport::Running),
        priority: Some(Priority::A),
    };
    let mut w1 = w1;
    w1.phase = Phase::Build;
    let mut w2 = w2;
    w2.phase = Phase::Build;
    let plan = plan_of(vec![w1, w2, w3], goal);
    let mut c = c;
    c.week_start = date(2027, 1, 18);
    let v = validate_plan(&plan, &c, &rules());
    let r09 = v
        .iter()
        .find(|x| x.code == "R09" && x.hardness == Hardness::Hard);
    assert!(r09.is_some(), "{v:?}");
    assert_eq!(r09.unwrap().detail["taper_days"], "3");
    assert!(!plan.is_acceptable(&v));
}

#[test]
fn r09_taper_of_one_week_before_race_week_is_accepted() {
    // Termin Samstag 2027-02-06, Taperwoche ab Montag 2027-01-25: 12 Tage.
    let mut c = open_ctx();
    c.week_start = date(2027, 1, 18);
    let mut w1 = calm_week(date(2027, 1, 18));
    w1.phase = Phase::Build;
    put(&mut w1, 6, "bike.endurance.steady", 90);
    let mut w2 = blank_week(date(2027, 1, 25), Phase::Taper);
    put(&mut w2, 1, "bike.endurance.steady", 60);
    put(&mut w2, 5, "bike.endurance.steady", 60);
    let mut w3 = blank_week(date(2027, 2, 1), Phase::Race);
    put(&mut w3, 0, "run.easy.steady", 30);
    let goal = Goal {
        kind: GoalKind::Event,
        event_date: Some(date(2027, 2, 6)),
        sport: Some(PlanSport::Running),
        priority: Some(Priority::A),
    };
    let plan = plan_of(vec![w1, w2, w3], goal);
    let v = validate_plan(&plan, &c, &rules());
    assert!(!v.iter().any(|x| x.code == "R09"), "{v:?}");
}

#[test]
fn r09_taper_longer_than_14_days_is_rejected() {
    let mut c = open_ctx();
    c.week_start = date(2027, 1, 18);
    let mut w1 = calm_week(date(2027, 1, 18));
    w1.phase = Phase::Build;
    let w2 = blank_week(date(2027, 1, 25), Phase::Taper);
    let w3 = blank_week(date(2027, 2, 1), Phase::Taper);
    let w4 = blank_week(date(2027, 2, 8), Phase::Race);
    let goal = Goal {
        kind: GoalKind::Event,
        event_date: Some(date(2027, 2, 14)),
        sport: Some(PlanSport::Cycling),
        priority: Some(Priority::A),
    };
    let plan = plan_of(vec![w1, w2, w3, w4], goal);
    let v = validate_plan(&plan, &c, &rules());
    assert!(has(&v, "R09", Hardness::Hard), "{v:?}");
}

#[test]
fn r09_taper_volume_must_drop_softly() {
    let mut c = open_ctx();
    c.week_start = date(2027, 1, 18);
    let mut w1 = calm_week(date(2027, 1, 18));
    w1.phase = Phase::Build;
    // Taperwoche mit mehr Last als die Woche davor: weiche Verletzung.
    let mut w2 = blank_week(date(2027, 1, 25), Phase::Taper);
    put(&mut w2, 1, "bike.endurance.steady", 120);
    put(&mut w2, 3, "bike.endurance.steady", 120);
    put(&mut w2, 5, "bike.endurance.steady", 120);
    let w3 = blank_week(date(2027, 2, 1), Phase::Race);
    let goal = Goal {
        kind: GoalKind::Event,
        event_date: Some(date(2027, 2, 6)),
        sport: Some(PlanSport::Cycling),
        priority: Some(Priority::A),
    };
    let plan = plan_of(vec![w1, w2, w3], goal);
    let v = validate_plan(&plan, &c, &rules());
    assert!(has(&v, "R09", Hardness::Soft), "{v:?}");
    assert!(!has(&v, "R09", Hardness::Hard), "{v:?}");
}

#[test]
fn r06_fifth_load_week_without_recovery_warns() {
    let c = open_ctx();
    let weeks: Vec<PlanWeek> = (0..5)
        .map(|i| calm_week(monday() + TimeDelta::days(7 * i)))
        .collect();
    let plan = plan_of(weeks, continuous_goal());
    let v = validate_plan(&plan, &c, &rules());
    let r06: Vec<_> = v.iter().filter(|x| x.code == "R06").collect();
    // Die vierte Woche hat noch eine Nachfolgerin ohne Erholung, die fünfte ist die letzte.
    assert_eq!(r06.len(), 1, "{v:?}");
    assert_eq!(r06[0].hardness, Hardness::Soft);
    assert_eq!(r06[0].date, Some(monday() + TimeDelta::days(21)));
    assert!(plan.is_acceptable(&v));
}

#[test]
fn r06_recovery_week_outside_60_to_70_percent_warns() {
    let c = open_ctx();
    let w1 = calm_week(monday());
    // Erholungswoche mit gleicher Last wie die Woche davor.
    let mut w2 = calm_week(monday() + TimeDelta::days(7));
    w2.phase = Phase::Recovery;
    let v = validate_plan(
        &plan_of(vec![w1.clone(), w2], continuous_goal()),
        &c,
        &rules(),
    );
    assert!(has(&v, "R06", Hardness::Soft), "{v:?}");
    // Gleiche Erholungswoche mit 65 Prozent: keine Meldung.
    let mut w3 = blank_week(monday() + TimeDelta::days(7), Phase::Recovery);
    for (d, t, m) in [
        (1usize, "bike.endurance.steady", 60u32),
        (3, "run.easy.steady", 40),
        (5, "bike.endurance.steady", 90),
    ] {
        put(&mut w3, d, t, (f64::from(m) * 0.65).round() as u32);
    }
    let v = validate_plan(&plan_of(vec![w1, w3], continuous_goal()), &c, &rules());
    assert!(!v.iter().any(|x| x.code == "R06"), "{v:?}");
}

#[test]
fn r07_strength_before_hard_day_and_too_many_warn() {
    let mut c = open_ctx();
    c.strength_per_week = 1;
    let mut w = blank_week(monday(), Phase::Build);
    put(&mut w, 1, "strength.general.a", 45);
    put(&mut w, 2, "bike.threshold.2x20", 70);
    put(&mut w, 4, "strength.general.b", 45);
    put(&mut w, 5, "strength.general.a", 45);
    let v = validate_plan(&plan_of(vec![w], continuous_goal()), &c, &rules());
    let r07: Vec<_> = v.iter().filter(|x| x.code == "R07").collect();
    // Drei Krafteinheiten (mehr als 2) und Kraft am Tag vor der harten Einheit.
    assert_eq!(r07.len(), 2, "{v:?}");
    assert!(r07.iter().all(|x| x.hardness == Hardness::Soft));
}

#[test]
fn r07_strength_requested_but_missing_warns() {
    let mut c = open_ctx();
    c.strength_per_week = 2;
    let v = validate_plan(
        &plan_of(vec![calm_week(monday())], continuous_goal()),
        &c,
        &rules(),
    );
    assert!(has(&v, "R07", Hardness::Soft), "{v:?}");
    c.strength_per_week = 0;
    let v = validate_plan(
        &plan_of(vec![calm_week(monday())], continuous_goal()),
        &c,
        &rules(),
    );
    assert!(!v.iter().any(|x| x.code == "R07"), "{v:?}");
}

#[test]
fn r08_too_little_easy_time_warns() {
    let c = open_ctx();
    // Zwei harte Einheiten (je 60 Minuten) und eine lockere von 60: leicht = 20 + 20 + 60
    // = 100 von 180 Minuten, 56 Prozent.
    let mut w = blank_week(monday(), Phase::Build);
    put(&mut w, 1, "bike.threshold.2x20", 60);
    put(&mut w, 3, "bike.threshold.2x20", 60);
    put(&mut w, 5, "bike.endurance.steady", 60);
    let v = validate_plan(&plan_of(vec![w], continuous_goal()), &c, &rules());
    let r08 = v.iter().find(|x| x.code == "R08").expect("R08");
    assert_eq!(r08.hardness, Hardness::Soft);
    assert_eq!(r08.detail["easy_share"], "0.556");
}

#[test]
fn r10_form_below_minus_30_warns() {
    let c = open_ctx();
    // Zwei Tage Last 250 bei CTL0 = 40, ATL0 = 45: ATL steigt schnell, am dritten Tag liegt der
    // TSB (CTL minus ATL des Vortags) weit unter -30.
    let mut w = blank_week(monday(), Phase::Build);
    put_load(&mut w, 0, "bike.endurance.steady", 240, 250.0);
    put_load(&mut w, 1, "bike.endurance.steady", 240, 250.0);
    put_load(&mut w, 2, "bike.endurance.steady", 240, 250.0);
    let v = validate_plan(&plan_of(vec![w], continuous_goal()), &c, &rules());
    let r10 = v.iter().find(|x| x.code == "R10").expect("R10");
    assert_eq!(r10.hardness, Hardness::Soft);
}

#[test]
fn violations_serialize_with_keys_for_texts() {
    let c = open_ctx();
    let mut w = calm_week(monday());
    put(&mut w, 2, "bike.threshold.2x20", 70);
    put(&mut w, 3, "run.tempo.continuous", 40);
    let v = validate_plan(&plan_of(vec![w], continuous_goal()), &c, &rules());
    let r04 = v.iter().find(|x| x.code == "R04").unwrap();
    assert_eq!(r04.message_key, "rule.R04.explanation");
    let json = serde_json::to_value(r04).unwrap();
    assert_eq!(json["hardness"], "hard");
    assert_eq!(json["code"], "R04");
    let warnings = violations_to_warnings(&v);
    assert!(warnings.iter().all(|w| w.severity == Severity::Warn));
}

// ---------------------------------------------------------------------------
// Erzeugung
// ---------------------------------------------------------------------------

fn event_ctx() -> PlanContext {
    // 12 Wochen ab Montag 2027-01-04, Termin Samstag 2027-03-27 (Woche 12).
    PlanContext {
        goal: GoalSpec::Event {
            event_date: date(2027, 3, 27),
            sport: PlanSport::Cycling,
            priority: Priority::A,
        },
        horizon_weeks: 26,
        history: history_week(300.0, 230.0),
        ..ctx()
    }
}

fn continuous_ctx() -> PlanContext {
    PlanContext {
        horizon_weeks: 8,
        ..ctx()
    }
}

fn assert_good(plan: &Plan, c: &PlanContext, r: &Rules) {
    let v = validate_plan(plan, c, r);
    assert!(plan.is_acceptable(&v), "harte Verletzungen: {v:?}");
}

#[test]
fn generated_plan_has_header_fields_from_the_context() {
    let c = continuous_ctx();
    let plan = generate_plan(&c, &rules()).unwrap();
    assert_eq!(plan.schema_version, 1);
    assert_eq!(plan.plan_id, c.plan_id);
    assert_eq!(plan.created_at, c.created_at);
    assert_eq!(plan.source, PlanSource::Rules);
    assert_eq!(plan.rules_version, "1.0.0");
    assert_eq!(plan.weeks.len(), 8);
    assert_eq!(plan.weeks[0].week_start, monday());
    let json = serde_json::to_value(&plan).unwrap();
    assert_eq!(json["created_at"], "2026-10-13T08:00:00Z");
    assert_eq!(json["goal"], serde_json::json!({"kind": "continuous"}));
    assert_eq!(json["locale"], "de");
    assert_eq!(json["source"], "rules");
}

#[test]
fn continuous_plan_has_blocks_of_three_plus_one() {
    let c = continuous_ctx();
    let plan = generate_plan(&c, &rules()).unwrap();
    let phases: Vec<Phase> = plan.weeks.iter().map(|w| w.phase).collect();
    use Phase::{Base as B, Recovery as R};
    assert_eq!(phases, [B, B, B, R, B, B, B, R]);
    assert_good(&plan, &c, &rules());
}

#[test]
fn recovery_week_is_60_to_70_percent_of_the_last_load_week() {
    let c = continuous_ctx();
    let plan = generate_plan(&c, &rules()).unwrap();
    let load = |i: usize| f64::from(plan.weeks[i].planned_load.unwrap());
    let ratio = load(3) / load(2);
    assert!((0.55..=0.75).contains(&ratio), "Verhältnis {ratio}");
    assert!(
        !plan.warnings.iter().any(|w| w.code == "R06"),
        "{:?}",
        plan.warnings
    );
}

#[test]
fn event_plan_runs_backwards_from_the_date() {
    let c = event_ctx();
    let r = rules();
    let plan = generate_plan(&c, &r).unwrap();
    assert_eq!(plan.weeks.len(), 12);
    let last = plan.weeks.last().unwrap();
    assert_eq!(last.phase, Phase::Race);
    assert_eq!(last.week_start, date(2027, 3, 22));
    assert_eq!(plan.weeks[10].phase, Phase::Taper);
    assert_eq!(plan.weeks[9].phase, Phase::Peak);
    assert_eq!(plan.weeks[8].phase, Phase::Peak);
    // Der Tag vor dem Termin (Freitag), der Termin und der Tag danach bleiben ohne Einheit.
    assert!(last.days[4].sessions.is_empty());
    assert!(last.days[5].sessions.is_empty());
    assert!(last.days[6].sessions.is_empty());
    // Taper: vom Montag 2027-03-15 bis zum Termin 12 Tage, im Rahmen von R9.
    assert_good(&plan, &c, &r);
    // Das Volumen sinkt im Taper.
    let load = |i: usize| plan.weeks[i].planned_load.unwrap();
    assert!(load(10) < load(9), "{} {}", load(10), load(9));
    assert!(load(11) < load(10), "{} {}", load(11), load(10));
    assert!(plan.warnings.iter().all(|w| w.code != "R09"));
}

#[test]
fn event_beyond_the_horizon_cuts_the_plan() {
    let mut c = event_ctx();
    c.horizon_weeks = 6;
    let plan = generate_plan(&c, &rules()).unwrap();
    assert_eq!(plan.weeks.len(), 6);
    assert!(plan.weeks.iter().all(|w| w.phase != Phase::Race));
    assert!(
        plan.weeks[5]
            .reason_keys
            .contains(&reason_keys::EVENT_AFTER_HORIZON.to_string())
    );
    assert_good(&plan, &c, &rules());
}

#[test]
fn event_in_the_first_week_gives_only_a_race_week() {
    let mut c = event_ctx();
    c.goal = GoalSpec::Event {
        event_date: date(2027, 1, 9),
        sport: PlanSport::Running,
        priority: Priority::B,
    };
    let plan = generate_plan(&c, &rules()).unwrap();
    assert_eq!(plan.weeks.len(), 1);
    assert_eq!(plan.weeks[0].phase, Phase::Race);
    assert_good(&plan, &c, &rules());
}

#[test]
fn event_on_a_monday_leaves_the_whole_race_week_free() {
    let mut c = event_ctx();
    c.goal = GoalSpec::Event {
        event_date: date(2027, 1, 18),
        sport: PlanSport::Cycling,
        priority: Priority::A,
    };
    let plan = generate_plan(&c, &rules()).unwrap();
    assert_eq!(plan.weeks.len(), 3);
    assert!(plan.weeks[2].days.iter().all(|d| d.sessions.is_empty()));
    assert_good(&plan, &c, &rules());
}

/// Nur Rad, ohne Kraft, Verfügbarkeit wie im Abnahmelauf der CLI: Mo 60, Di bis Do 90,
/// Fr fester Ruhetag, Sa 240, So 150 Minuten, harte Einheiten überall erlaubt. Vorwoche 140 Last
/// bei 300 Minuten, damit R2 nicht greift.
fn small_taper_ctx(event_date: NaiveDate) -> PlanContext {
    let mut availability = [avail(90, true, false); 7];
    availability[0] = avail(60, true, false);
    availability[4] = avail(0, false, true);
    availability[5] = avail(240, true, false);
    availability[6] = avail(150, true, false);
    PlanContext {
        goal: GoalSpec::Event {
            event_date,
            sport: PlanSport::Cycling,
            priority: Priority::A,
        },
        horizon_weeks: 26,
        history: history_week(300.0, 140.0),
        availability,
        sports: vec![SportWeight {
            sport: PlanSport::Cycling,
            weight: 1.0,
        }],
        strength_per_week: 0,
        ..ctx()
    }
}

fn sessions_of(week: &PlanWeek) -> Vec<(usize, &str, u32)> {
    week.days
        .iter()
        .enumerate()
        .flat_map(|(i, d)| {
            d.sessions
                .iter()
                .map(move |s| (i, s.template_id.as_str(), s.duration_min))
        })
        .collect()
}

// Rennwoche mit Ziel unter der Mindestlast einer harten Einheit (Handrechnung, Last je Minute
// `100 / 60 * IF^2`, SPEC 8.3 und `load_model`):
// - Ziel: 50 Prozent der Vorwoche, `0,5 * 140 = 70`.
// - Harte Radeinheit: kürzestes Fenster 60 min, `60 / 60 * 0,95^2 * 100 = 90,25 > 70`, sie
//   kann nie bleiben und entfällt zuerst.
// - Termin Sonntag: Sa und So gesperrt, Fr fester Ruhetag, leicht bleiben Mo, Mi, Do (der
//   harte Tag Di entfällt). Mindestlast `3 * 30 / 60 * 0,65^2 * 100 = 3 * 21,125 = 63,375 <= 70`.
// - Dauer je Einheit `70 / (3 * 0,704167) = 33,1` min, gerundet 35, zusammen
//   `105 / 60 * 42,25 = 73,94`. Die Feinabstimmung kürzt die erste Einheit (Mo) um 5 min:
//   `100 / 60 * 42,25 = 70,42`, Abstand 0,42 unter einem halben Schritt (1,76).
// - Geplante Last aus den gerundeten Einheiten: `21,1 + 24,6 + 24,6 = 70,3`.
const RACE_WEEK_SMALL_TARGET: f32 = 70.0;
const RACE_WEEK_SMALL_PLANNED: f32 = 70.3;

#[test]
fn race_week_below_the_minimum_of_a_hard_session_keeps_easy_sessions() {
    // Termin Sonntag 2027-01-10, der Plan besteht nur aus der Rennwoche.
    let c = small_taper_ctx(date(2027, 1, 10));
    let r = rules();
    let plan = generate_plan(&c, &r).unwrap();
    assert_eq!(plan.weeks.len(), 1);
    let race = &plan.weeks[0];
    assert_eq!(race.phase, Phase::Race);
    assert_eq!(race.target_load, RACE_WEEK_SMALL_TARGET);
    assert_eq!(race.planned_load, Some(RACE_WEEK_SMALL_PLANNED));
    assert_eq!(
        sessions_of(race),
        [
            (0, "bike.recovery.spin", 30),
            (2, "bike.recovery.spin", 35),
            (3, "bike.recovery.spin", 35),
        ]
    );
    assert!(
        !race
            .reason_keys
            .contains(&reason_keys::FALLBACK_REST.to_string())
    );
    // Die harte Einheit ist entfallen und die Woche sagt das.
    assert!(
        race.reason_keys
            .contains(&reason_keys::HARD_REDUCED.to_string())
    );
    // Der harte Tag (Di) ist entfallen, die Einheit am Mittwoch ist keine Erholung danach.
    assert_eq!(
        race.days[2].sessions[0].reason_keys,
        [reason_keys::SESSION_EASY.to_string()]
    );
    assert_good(&plan, &c, &r);
}

// Taper mit Ziel unter harter Einheit plus Langeinheit (Handrechnung wie oben):
// - Ziel: 70 Prozent der Vorwoche, `0,7 * 140 = 98`.
// - Langeinheit (Sa) mindestens 30 min leicht, 21,125. Harte Einheit dazu
//   `90,25 + 21,125 = 111,375 > 98`, sie entfällt vor den leichten.
// - Leicht: Mo, Mi, Do, So. Mindestlast mit Langeinheit `5 * 21,125 = 105,6 > 98`, also entfällt
//   die späteste leichte Einheit (So), danach `4 * 21,125 = 84,5 <= 98`.
// - Dauer `98 / (0,704167 * (2 + 3)) = 27,8` min je Gewicht: Langeinheit 55 min, leichte 30 min
//   (Mindestdauer der Vorlage), zusammen `145 / 60 * 42,25 = 102,1`; die Feinabstimmung kürzt die
//   Langeinheit auf 50 min, `140 / 60 * 42,25 = 98,58`.
// - Geplante Last aus den gerundeten Einheiten: `35,2 + 3 * 21,1 = 98,5`.
// Ohne die Korrektur fielen erst alle leichten Einheiten und dann die harte weg, übrig bliebe
// nur die Langeinheit. Die Rennwoche danach entspricht dem Test oben (Ziel 70).
const TAPER_SMALL_TARGET: f32 = 98.0;
const TAPER_SMALL_PLANNED: f32 = 98.5;

#[test]
fn taper_below_hard_plus_long_drops_the_hard_session_before_the_easy_ones() {
    // Termin Sonntag 2027-01-17: Taper ab 2027-01-04, Rennwoche ab 2027-01-11.
    let c = small_taper_ctx(date(2027, 1, 17));
    let r = rules();
    let plan = generate_plan(&c, &r).unwrap();
    assert_eq!(plan.weeks.len(), 2);
    let taper = &plan.weeks[0];
    assert_eq!(taper.phase, Phase::Taper);
    assert_eq!(taper.target_load, TAPER_SMALL_TARGET);
    assert_eq!(taper.planned_load, Some(TAPER_SMALL_PLANNED));
    assert_eq!(
        sessions_of(taper),
        [
            (0, "bike.recovery.spin", 30),
            (2, "bike.recovery.spin", 30),
            (3, "bike.recovery.spin", 30),
            (5, "bike.endurance.steady", 50),
        ]
    );
    assert!(
        taper
            .reason_keys
            .contains(&reason_keys::HARD_REDUCED.to_string())
    );
    let race = &plan.weeks[1];
    assert_eq!(race.target_load, RACE_WEEK_SMALL_TARGET);
    assert_eq!(race.planned_load, Some(RACE_WEEK_SMALL_PLANNED));
    assert_good(&plan, &c, &r);
}

#[test]
fn week_left_empty_despite_a_positive_target_says_so() {
    // Rennwoche mit 10 Prozent der Vorwoche: `0,1 * 140 = 14` liegt unter der kürzesten
    // leichten Radeinheit (30 min, 21,125). Die Woche bleibt leer und trägt den Grund.
    let c = small_taper_ctx(date(2027, 1, 10));
    let mut r = rules();
    r.phases.race.volume_factor = Some(0.1);
    let plan = generate_plan(&c, &r).unwrap();
    let race = &plan.weeks[0];
    assert_eq!(race.target_load, 14.0);
    assert_eq!(race.planned_load, Some(0.0));
    assert!(race.days.iter().all(|d| d.sessions.is_empty()));
    assert!(
        race.reason_keys
            .contains(&reason_keys::FALLBACK_REST.to_string()),
        "{:?}",
        race.reason_keys
    );
    assert_good(&plan, &c, &r);
}

#[test]
fn hard_sessions_only_on_allowed_days_and_never_on_consecutive_days() {
    let c = continuous_ctx();
    let plan = generate_plan(&c, &rules()).unwrap();
    let mut prev_hard = false;
    let mut hard_total = 0;
    for w in &plan.weeks {
        let hard_in_week = w
            .days
            .iter()
            .flat_map(|d| &d.sessions)
            .filter(|s| s.hard)
            .count();
        assert!(hard_in_week <= 2);
        hard_total += hard_in_week;
        for d in &w.days {
            let hard = d.sessions.iter().any(|s| s.hard);
            assert!(
                !(hard && prev_hard),
                "harte Einheiten an Folgetagen am {}",
                d.date
            );
            if hard {
                assert!(
                    c.availability[d.date.weekday().num_days_from_monday() as usize].allow_hard
                );
            }
            prev_hard = hard;
        }
    }
    assert!(
        hard_total > 0,
        "ein Plan über 8 Wochen enthält harte Einheiten"
    );
}

#[test]
fn long_session_sits_on_the_day_with_the_most_time() {
    let c = continuous_ctx();
    let plan = generate_plan(&c, &rules()).unwrap();
    // Samstag hat mit 240 Minuten die meiste Zeit.
    for w in plan.weeks.iter().filter(|w| w.phase.is_load_week()) {
        let sat = &w.days[5];
        assert!(
            sat.sessions.iter().any(|s| s
                .reason_keys
                .contains(&reason_keys::SESSION_LONG.to_string())),
            "Woche {}",
            w.week_start
        );
        let longest = w
            .days
            .iter()
            .flat_map(|d| &d.sessions)
            .filter(|s| s.sport.is_endurance())
            .map(|s| s.duration_min)
            .max()
            .unwrap();
        assert_eq!(sat.sessions[0].duration_min, longest);
    }
}

#[test]
fn friday_fixed_rest_stays_free_and_strength_is_planned() {
    let c = continuous_ctx();
    let plan = generate_plan(&c, &rules()).unwrap();
    let mut strength = 0;
    for w in &plan.weeks {
        assert!(w.days[4].sessions.is_empty());
        strength += w
            .days
            .iter()
            .flat_map(|d| &d.sessions)
            .filter(|s| s.sport == PlanSport::Strength)
            .count();
    }
    assert!(
        strength >= plan.weeks.len(),
        "mindestens eine Krafteinheit je Woche, hat {strength}"
    );
}

#[test]
fn every_session_carries_template_and_reason_keys() {
    let c = event_ctx();
    let plan = generate_plan(&c, &rules()).unwrap();
    for w in &plan.weeks {
        assert!(!w.reason_keys.is_empty());
        for k in &w.reason_keys {
            assert!(reason_keys::ALL.contains(&k.as_str()), "{k}");
        }
        for s in w.days.iter().flat_map(|d| &d.sessions) {
            assert!(c.catalog.get(&s.template_id).is_some());
            assert!(!s.reason_keys.is_empty());
            for k in &s.reason_keys {
                assert!(reason_keys::ALL.contains(&k.as_str()), "{k}");
            }
            if s.sport == PlanSport::Strength {
                assert_eq!(s.rpe, Some(6));
            } else {
                assert_eq!(s.rpe, None);
            }
        }
    }
}

#[test]
fn planned_load_of_the_week_matches_the_sessions_and_stays_near_the_target() {
    let c = continuous_ctx();
    let plan = generate_plan(&c, &rules()).unwrap();
    for w in &plan.weeks {
        let sum: f32 = w.sessions_load();
        assert!(
            (w.planned_load.unwrap() - sum).abs() < 0.2,
            "{} {}",
            w.planned_load.unwrap(),
            sum
        );
    }
    // Ohne Begrenzung durch Verfügbarkeit liegt die Wochenlast nahe am Ziel (Rundung auf 5
    // Minuten je Einheit): hier mit großzügiger Verfügbarkeit.
    let mut c = continuous_ctx();
    c.availability = [avail(300, true, false); 7];
    c.availability[4] = avail(0, false, true);
    let plan = generate_plan(&c, &rules()).unwrap();
    for w in plan.weeks.iter().filter(|w| w.phase.is_load_week()) {
        let t = f64::from(w.target_load);
        let p = f64::from(w.planned_load.unwrap());
        assert!(
            (p - t).abs() <= 0.08 * t + 12.0,
            "Ziel {t}, geplant {p}, Woche {}",
            w.week_start
        );
    }
}

#[test]
fn generation_is_deterministic() {
    for c in [continuous_ctx(), event_ctx()] {
        let a = serde_json::to_string(&generate_plan(&c, &rules()).unwrap()).unwrap();
        let b = serde_json::to_string(&generate_plan(&c, &rules()).unwrap()).unwrap();
        assert_eq!(a, b);
    }
}

#[test]
fn low_availability_caps_the_target_and_says_so() {
    let mut c = continuous_ctx();
    c.ctl0 = 90.0;
    c.availability = [avail(40, false, false); 7];
    c.availability[6] = avail(0, false, true);
    let plan = generate_plan(&c, &rules()).unwrap();
    assert!(plan.weeks.iter().any(|w| {
        w.reason_keys
            .contains(&reason_keys::CAPPED_AVAILABILITY.to_string())
    }));
    assert_good(&plan, &c, &rules());
}

#[test]
fn no_usable_day_gives_empty_weeks_without_violation() {
    let mut c = continuous_ctx();
    c.availability = [avail(0, false, false); 7];
    let plan = generate_plan(&c, &rules()).unwrap();
    assert!(
        plan.weeks
            .iter()
            .all(|w| w.days.iter().all(|d| d.sessions.is_empty()))
    );
    assert!(
        plan.weeks[0]
            .reason_keys
            .contains(&reason_keys::NO_TRAINING_DAYS.to_string())
    );
    assert_good(&plan, &c, &rules());
}

#[test]
fn single_sport_plans_use_only_that_sport() {
    let mut c = continuous_ctx();
    c.sports = vec![SportWeight {
        sport: PlanSport::Running,
        weight: 1.0,
    }];
    c.strength_per_week = 0;
    let plan = generate_plan(&c, &rules()).unwrap();
    for s in plan
        .weeks
        .iter()
        .flat_map(|w| &w.days)
        .flat_map(|d| &d.sessions)
    {
        assert_eq!(s.sport, PlanSport::Running);
    }
    assert_good(&plan, &c, &rules());
}

#[test]
fn sports_follow_their_weights() {
    let mut c = continuous_ctx();
    c.horizon_weeks = 12;
    c.availability = [avail(150, true, false); 7];
    c.availability[4] = avail(0, false, true);
    let plan = generate_plan(&c, &rules()).unwrap();
    let count = |sport: PlanSport| {
        plan.weeks
            .iter()
            .flat_map(|w| &w.days)
            .flat_map(|d| &d.sessions)
            .filter(|s| s.sport == sport)
            .count()
    };
    let (bike, run, swim) = (
        count(PlanSport::Cycling),
        count(PlanSport::Running),
        count(PlanSport::Swimming),
    );
    assert!(
        bike > run && run >= swim,
        "Rad {bike}, Lauf {run}, Schwimmen {swim}"
    );
    assert!(swim > 0);
}

#[test]
fn generated_plan_matches_the_schema_shape() {
    let plan = generate_plan(&event_ctx(), &rules()).unwrap();
    assert_schema_shape(&serde_json::to_value(&plan).unwrap());
}

/// Prüft Pflichtfelder, erlaubte Felder, Aufzählungen und Grenzen von SPEC 7.1.
fn assert_schema_shape(v: &Value) {
    let keys = |o: &Value| -> BTreeSet<String> { o.as_object().unwrap().keys().cloned().collect() };
    let allowed = |o: &Value, required: &[&str], optional: &[&str]| {
        let k = keys(o);
        for r in required {
            assert!(k.contains(*r), "Pflichtfeld {r} fehlt in {o}");
        }
        for key in &k {
            assert!(
                required.contains(&key.as_str()) || optional.contains(&key.as_str()),
                "unerlaubtes Feld {key}"
            );
        }
    };
    allowed(
        v,
        &[
            "schema_version",
            "plan_id",
            "created_at",
            "locale",
            "source",
            "rules_version",
            "goal",
            "weeks",
        ],
        &["warnings", "explanation"],
    );
    assert_eq!(v["schema_version"], 1);
    assert!(["de", "en"].contains(&v["locale"].as_str().unwrap()));
    assert!(["rules", "llm"].contains(&v["source"].as_str().unwrap()));
    let rv = v["rules_version"].as_str().unwrap();
    assert_eq!(rv.split('.').count(), 3);
    allowed(&v["goal"], &["kind"], &["event_date", "sport", "priority"]);
    let weeks = v["weeks"].as_array().unwrap();
    assert!((1..=26).contains(&weeks.len()));
    for w in weeks {
        allowed(
            w,
            &["week_start", "phase", "target_load", "days"],
            &["planned_load", "reason_keys"],
        );
        assert!(
            ["base", "build", "peak", "taper", "recovery", "race"]
                .contains(&w["phase"].as_str().unwrap())
        );
        assert!(w["target_load"].as_f64().unwrap() >= 0.0);
        let days = w["days"].as_array().unwrap();
        assert_eq!(days.len(), 7);
        for d in days {
            allowed(d, &["date", "sessions"], &[]);
            let sessions = d["sessions"].as_array().unwrap();
            assert!(sessions.len() <= 2);
            for s in sessions {
                allowed(
                    s,
                    &[
                        "slot",
                        "sport",
                        "template_id",
                        "duration_min",
                        "planned_load",
                        "hard",
                    ],
                    &["rpe", "reason_keys"],
                );
                assert!(["am", "pm"].contains(&s["slot"].as_str().unwrap()));
                assert!(
                    ["cycling", "running", "swimming", "strength", "rest"]
                        .contains(&s["sport"].as_str().unwrap())
                );
                assert!(is_valid_template_id(s["template_id"].as_str().unwrap()));
                let m = s["duration_min"].as_u64().unwrap();
                assert!((10..=360).contains(&m));
                assert!(s["planned_load"].as_f64().unwrap() >= 0.0);
                assert!(s["hard"].is_boolean());
                if let Some(keys) = s.get("reason_keys") {
                    for k in keys.as_array().unwrap() {
                        assert!(is_valid_reason_key(k.as_str().unwrap()));
                    }
                }
            }
        }
    }
    for w in v["warnings"].as_array().unwrap_or(&Vec::new()) {
        allowed(w, &["code", "severity"], &["date"]);
        let code = w["code"].as_str().unwrap();
        assert!(code.len() == 3 && code.starts_with('R'), "{code}");
        assert!(["info", "warn"].contains(&w["severity"].as_str().unwrap()));
    }
}

#[test]
fn plan_roundtrips_through_json() {
    let plan = generate_plan(&event_ctx(), &rules()).unwrap();
    let text = serde_json::to_string(&plan).unwrap();
    let back: Plan = serde_json::from_str(&text).unwrap();
    assert_eq!(back, plan);
    let wrong = text.replacen("\"schema_version\":1", "\"schema_version\":2", 1);
    assert!(serde_json::from_str::<Plan>(&wrong).is_err());
}

// ---------------------------------------------------------------------------
// Golden-Tests (insta)
// ---------------------------------------------------------------------------

#[test]
fn golden_event_in_12_weeks() {
    let plan = generate_plan(&event_ctx(), &rules()).unwrap();
    insta::assert_json_snapshot!("event_in_12_weeks", plan);
}

#[test]
fn golden_continuous_8_weeks() {
    let plan = generate_plan(&continuous_ctx(), &rules()).unwrap();
    insta::assert_json_snapshot!("continuous_8_weeks", plan);
}

// ---------------------------------------------------------------------------
// Eigenschaften (proptest)
// ---------------------------------------------------------------------------

fn arb_availability() -> impl Strategy<Value = DayAvailability> {
    (
        prop_oneof![
            1 => Just(0u16),
            1 => 10u16..60,
            6 => 60u16..=300,
            1 => 301u16..=480,
        ],
        any::<bool>(),
        prop::bool::weighted(0.1),
    )
        .prop_map(|(max_minutes, allow_hard, fixed_rest)| DayAvailability {
            max_minutes,
            allow_hard,
            fixed_rest,
        })
}

fn arb_sports() -> impl Strategy<Value = Vec<SportWeight>> {
    (
        prop::sample::subsequence(
            vec![PlanSport::Cycling, PlanSport::Running, PlanSport::Swimming],
            1..=3,
        ),
        prop::collection::vec(1.0f32..6.0, 3),
    )
        .prop_map(|(sports, weights)| {
            sports
                .into_iter()
                .zip(weights)
                .map(|(sport, weight)| SportWeight { sport, weight })
                .collect()
        })
}

/// Zufällige, gültige Kontexte: Horizont bis 12 Wochen (Laufzeit), Termin vor und hinter dem
/// Horizont, wechselnde Verfügbarkeit, Historie passend zur Fitness oder ohne Historie.
fn arb_ctx() -> impl Strategy<Value = PlanContext> {
    let time = (
        0i64..40,
        1u32..=12,
        prop::option::of((0i64..=98, 0usize..3, 0usize..3)),
        0.0f32..120.0,
        0.5f32..1.8,
    );
    let history = (prop::option::of(0.3f32..1.8), 0.7f32..1.6);
    let setup = (
        prop::array::uniform7(arb_availability()),
        arb_sports(),
        0u8..=3,
        prop::option::of(2.0f32..8.0),
        (30.0f32..60.0, 4.0f32..12.0),
    );
    (time, history, setup).prop_map(
        |(
            (week_offset, horizon, event, ctl0, atl_factor),
            (hist_factor, minutes_factor),
            (availability, sports, strength, rise, (tau_ctl, tau_atl)),
        )| {
            let week_start = monday() + TimeDelta::days(7 * week_offset);
            let goal = match event {
                Some((days, sport, prio)) => GoalSpec::Event {
                    event_date: week_start + TimeDelta::days(days),
                    sport: [PlanSport::Cycling, PlanSport::Running, PlanSport::Swimming][sport],
                    priority: [Priority::A, Priority::B, Priority::C][prio],
                },
                None => GoalSpec::Continuous,
            };
            let history = hist_factor
                .map(|f| {
                    let load = 7.0 * ctl0 * f;
                    vec![WeekHistory {
                        week_start: week_start - TimeDelta::days(7),
                        sport: Sport::Cycling,
                        load,
                        minutes: load * minutes_factor,
                    }]
                })
                .unwrap_or_default();
            PlanContext {
                week_start,
                goal,
                horizon_weeks: horizon,
                ctl0,
                atl0: ctl0 * atl_factor,
                history,
                availability,
                sports,
                strength_per_week: strength,
                ctl_rise_per_week: rise,
                pmc_params: PmcParams {
                    tau_ctl_d: tau_ctl,
                    tau_atl_d: tau_atl,
                    ..PmcParams::default()
                },
                ..ctx()
            }
        },
    )
}

/// Unabhängige Prüfung der Eigenschaften aus SPEC 11.4, ohne `validate_plan` zu benutzen.
fn check_properties(c: &PlanContext, plan: &Plan, r: &Rules) -> Result<(), TestCaseError> {
    // Ausgabe besteht validate_plan.
    let v = validate_plan(plan, c, r);
    prop_assert!(
        plan.is_acceptable(&v),
        "harte Verletzungen: {:?}",
        codes(&v, Hardness::Hard)
    );

    // JSON: genau 7 Tage je Woche, höchstens 2 Einheiten je Tag.
    let json = serde_json::to_value(plan).unwrap();
    for w in json["weeks"].as_array().unwrap() {
        prop_assert_eq!(w["days"].as_array().unwrap().len(), 7);
        for d in w["days"].as_array().unwrap() {
            prop_assert!(d["sessions"].as_array().unwrap().len() <= 2);
        }
    }
    prop_assert!((1..=26).contains(&plan.weeks.len()));

    // Keine harten Tage an Folgetagen, auch nicht über Wochengrenzen.
    let days: Vec<&PlanDay> = plan.weeks.iter().flat_map(|w| &w.days).collect();
    for pair in days.windows(2) {
        let h = |d: &PlanDay| d.sessions.iter().any(|s| s.hard);
        prop_assert!(
            !(h(pair[0]) && h(pair[1])),
            "harte Tage an Folgetagen: {}",
            pair[1].date
        );
    }

    // Verfügbarkeit nie überschritten, feste Ruhetage frei, harte Einheiten nur an erlaubten Tagen.
    for d in &days {
        let a = c.availability[d.date.weekday().num_days_from_monday() as usize];
        let minutes: u32 = d.sessions.iter().map(|s| s.duration_min).sum();
        prop_assert!(
            minutes <= u32::from(a.max_minutes),
            "{}: {} > {}",
            d.date,
            minutes,
            a.max_minutes
        );
        if a.fixed_rest {
            prop_assert!(d.sessions.is_empty());
        }
        if !a.allow_hard {
            prop_assert!(d.sessions.iter().all(|s| !s.hard));
        }
    }

    // Je Woche höchstens R3 harte Einheiten und mindestens ein Ruhetag.
    for w in &plan.weeks {
        let hard = w
            .days
            .iter()
            .flat_map(|d| &d.sessions)
            .filter(|s| s.hard)
            .count();
        prop_assert!(hard <= r.r03.max_hard_per_week as usize);
        prop_assert!(w.days.iter().any(|d| d.sessions.is_empty()));
        for s in w.days.iter().flat_map(|d| &d.sessions) {
            prop_assert!((10..=360).contains(&s.duration_min));
            prop_assert!(s.planned_load.is_finite() && s.planned_load >= 0.0);
            prop_assert!(is_valid_template_id(&s.template_id));
        }
    }

    // CTL-Anstieg je Woche höchstens R1 (eigene Rechnung mit pmc).
    let daily: Vec<DailyLoad> = days
        .iter()
        .flat_map(|d| {
            d.sessions.iter().map(move |s| DailyLoad {
                date: d.date,
                load: s.planned_load,
            })
        })
        .collect();
    let from = plan.weeks[0].week_start;
    let to = plan.weeks.last().unwrap().week_start + TimeDelta::days(6);
    let points = pmc(&daily, &c.pmc_start(), from, to);
    let limit = c.ctl_rise_limit(r);
    let mut prev = c.ctl0;
    for (i, _) in plan.weeks.iter().enumerate() {
        let end = points[7 * i + 6].ctl;
        prop_assert!(
            end - prev <= limit + 1e-3,
            "Woche {}: Anstieg {} über {}",
            i,
            end - prev,
            limit
        );
        prev = end;
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 1000, ..ProptestConfig::default() })]

    #[test]
    fn generated_plans_satisfy_the_hard_rules(c in arb_ctx()) {
        let r = rules();
        let plan = generate_plan(&c, &r).unwrap();
        check_properties(&c, &plan, &r)?;
    }

}

proptest! {
    #![proptest_config(ProptestConfig { cases: 200, ..ProptestConfig::default() })]

    #[test]
    fn generation_is_deterministic_for_random_contexts(c in arb_ctx()) {
        let r = rules();
        let a = serde_json::to_string(&generate_plan(&c, &r).unwrap()).unwrap();
        let b = serde_json::to_string(&generate_plan(&c.clone(), &r).unwrap()).unwrap();
        prop_assert_eq!(a, b);
    }
}
