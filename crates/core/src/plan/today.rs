//! Tagesentscheidung: Was trainiere ich heute und warum (SPEC 8.7, seit T44).
//!
//! Die Entscheidung liest nur, was schon berechnet ist, und führt keine neue Zahl ein:
//!
//! - Bereitschaft aus [`ReadinessState`] (R12, Q6 nur für die HRV-Überwachung, das
//!   7-Tage-Mittel aus einem Sekundärbericht; Schwellen Faustregel in `rules.default.json`).
//! - Form aus [`FormState`] (SPEC 5.5; Grenze "überlastet" ist die von R10, Faustregel; Grenze
//!   "frisch" ab +5 ist Faustregel).
//! - Varianten nur aus dem Vorlagenkatalog mit der Wahlregel von R12 (engstes Dauerfenster,
//!   Gleichstand nach Katalogreihenfolge), Lasten über `rules.load_model.session_load`.
//!
//! Die Entscheidungstabelle selbst ist eine Faustregel, abgeleitet aus R12: Was R12 vor dem
//! Planbeginn mit der ersten harten Einheit tut, schlägt die Tagesentscheidung für den
//! heutigen Tag vor, ohne den Plan zu ändern. Übernahme läuft weiter über den Coach (SPEC 9.7).
//!
//! | Lage | Zustand | Empfohlen | Alternativen |
//! |---|---|---|---|
//! | kein Plan oder Tag außerhalb | `no_plan` | nichts | keine |
//! | keine Einheit | `rest` | Ruhe | keine |
//! | Einheit schon durch R12 getauscht | `already_adjusted` | geplant | keine |
//! | hart, Bereitschaft niedrig oder Form überlastet | `lighten` | leichter, gleiche Dauer | geplant |
//! | hart, sonst | `as_planned` | geplant | leichter |
//! | locker, Bereitschaft niedrig oder Form überlastet | `keep_easy` | geplant | kürzer |
//! | locker, Bereitschaft normal, Form frisch, keine Taper-, Wettkampf- oder Erholungswoche | `as_planned` | geplant | härter |
//! | locker, sonst | `as_planned` | geplant | keine |
//!
//! "Locker" heißt hier `hard = false` (auch mittlere Einheiten und Kraft). Fehlende Bereitschaft
//! oder Form gilt nie als "normal": Sie steht in `missing` und schließt die härtere Variante aus.

use std::collections::BTreeSet;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::catalog::{Template, TemplateCatalog};
use super::context::PlanContext;
use super::generate::keys;
use super::recovery::{easy_replacement, hard_replacement};
use super::rules::{Hardness, Rules, round1};
use super::types::{Phase, Plan, PlanSession, PlanSport};
use super::validate::validate_plan;
use crate::metrics::form::FormState;
use crate::metrics::readiness::ReadinessState;

/// Eingänge der Tagesentscheidung. Alles ist schon berechnet; `core` liest keine Uhr, `date` ist
/// "heute" des Aufrufers.
#[derive(Debug, Clone, Copy)]
pub struct TodayInput<'a> {
    pub date: NaiveDate,
    /// Aktiver Plan, falls vorhanden.
    pub plan: Option<&'a Plan>,
    /// Kontext des Plans für `validate_plan`. Ohne ihn gibt es keine härtere Variante.
    pub ctx: Option<&'a PlanContext>,
    /// Bereitschaft des Tages (`metrics::readiness::readiness_state`).
    pub readiness: ReadinessState,
    /// Formzustand aus dem TSB des Tages (`metrics::form::form_state`), `None` ohne Verlauf.
    pub form: Option<FormState>,
    pub catalog: &'a TemplateCatalog,
}

/// Zustand der Entscheidung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionState {
    NoPlan,
    Rest,
    AlreadyAdjusted,
    Lighten,
    KeepEasy,
    AsPlanned,
}

impl DecisionState {
    /// Stabiler Code, Schlüssel für Texte in `catalog/locales`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::NoPlan => "decision.no_plan",
            Self::Rest => "decision.rest",
            Self::AlreadyAdjusted => "decision.already_adjusted",
            Self::Lighten => "decision.lighten",
            Self::KeepEasy => "decision.keep_easy",
            Self::AsPlanned => "decision.as_planned",
        }
    }
}

/// Art einer Variante.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariantKind {
    /// Die Einheit wie im Plan.
    Planned,
    /// Lockere Vorlage derselben Sportart mit gleicher Dauer.
    Lighter,
    /// Dieselbe Vorlage mit der Mindestdauer ihres Katalogfensters.
    Shorter,
    /// Harte Vorlage derselben Sportart mit gleicher Dauer.
    Harder,
}

/// Eine Einheit, die die Entscheidung empfiehlt oder anbietet. `template_id` stammt aus dem
/// Plan (`planned`) oder aus dem Katalog (alle anderen).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Variant {
    pub kind: VariantKind,
    pub sport: PlanSport,
    pub template_id: String,
    pub duration_min: u32,
    pub hard: bool,
    /// Geplante Last; bei `planned` aus dem Plan, sonst `load_model.session_load`.
    pub planned_load: f32,
}

/// Grund der Entscheidung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionReason {
    /// Bereitschaft niedrig (R12).
    ReadinessLow,
    /// Form überlastet (R10).
    FormOverloaded,
    /// Form frisch und Bereitschaft normal, deshalb die härtere Variante (Faustregel, SPEC 5.5).
    FormFresh,
    /// Die Einheit wurde schon vom Planer nach R12 getauscht.
    RecoverySwap,
}

impl DecisionReason {
    /// Stabiler Code, Schlüssel für Texte in `catalog/locales`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::ReadinessLow => "decision.reason.readiness_low",
            Self::FormOverloaded => "decision.reason.form_overloaded",
            Self::FormFresh => "decision.reason.form_fresh",
            Self::RecoverySwap => "decision.reason.recovery_swap",
        }
    }
}

/// Fehlende Eingänge in `missing`.
pub mod missing {
    /// Bereitschaft ohne Daten (`readiness.no_data`).
    pub const READINESS: &str = "readiness";
    /// Kein Formzustand.
    pub const FORM: &str = "form";
    /// Plan ohne Kontext, deshalb keine Prüfung einer härteren Variante.
    pub const PLAN_CONTEXT: &str = "plan_context";
}

/// Ergebnis der Tagesentscheidung.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TodayDecision {
    pub state: DecisionState,
    /// Empfohlene Einheit. Leer bei `no_plan` und `rest`, und bei `lighten`, wenn der Katalog
    /// keine lockere Vorlage mit passendem Fenster hat (dann bleibt nur die Alternative).
    pub main: Option<Variant>,
    pub alternatives: Vec<Variant>,
    pub reasons: Vec<DecisionReason>,
    /// Regelcodes wie in `validate_plan` (`R05`, `R10`, `R12`).
    pub rules: Vec<&'static str>,
    pub missing: Vec<&'static str>,
}

/// Entscheidet, was heute ansteht. Siehe Tabelle im Modulkopf.
pub fn today_decision(input: &TodayInput<'_>, rules: &Rules) -> TodayDecision {
    let mut missing = Vec::new();
    if input.readiness == ReadinessState::NoData {
        missing.push(missing::READINESS);
    }
    if input.form.is_none() {
        missing.push(missing::FORM);
    }
    if input.plan.is_some() && input.ctx.is_none() {
        missing.push(missing::PLAN_CONTEXT);
    }
    let mut out = TodayDecision {
        state: DecisionState::NoPlan,
        main: None,
        alternatives: Vec::new(),
        reasons: Vec::new(),
        rules: Vec::new(),
        missing,
    };

    let Some(plan) = input.plan else {
        return out;
    };
    let Some((wi, di)) = locate(plan, input.date) else {
        return out;
    };
    let week = &plan.weeks[wi];
    let day = &week.days[di];
    if day.sessions.is_empty() {
        out.state = DecisionState::Rest;
        out.rules.push("R05");
        return out;
    }

    // Eine von R12 getauschte Einheit ist schon angepasst, sie wird nicht weiter verändert.
    if let Some(s) = day.sessions.iter().find(|s| is_swapped(s)) {
        out.state = DecisionState::AlreadyAdjusted;
        out.main = Some(planned(s));
        out.reasons.push(DecisionReason::RecoverySwap);
        out.rules.push("R12");
        return out;
    }

    // Maßgeblich ist die erste harte Einheit des Tages, sonst die erste.
    let si = day.sessions.iter().position(|s| s.hard).unwrap_or(0);
    let session = &day.sessions[si];
    let readiness_low = input.readiness == ReadinessState::Low;
    let overloaded = input.form == Some(FormState::Overloaded);
    let strained = readiness_low || overloaded;
    let variant = |kind, t: &Template, minutes| Variant {
        kind,
        sport: t.sport,
        template_id: t.id.clone(),
        duration_min: minutes,
        hard: kind == VariantKind::Harder,
        planned_load: rules.load_model.session_load(t, minutes),
    };
    let lighter = || {
        easy_replacement(input.catalog, session)
            .map(|t| variant(VariantKind::Lighter, t, session.duration_min))
    };

    if strained {
        if readiness_low {
            out.reasons.push(DecisionReason::ReadinessLow);
            out.rules.push("R12");
        }
        if overloaded {
            out.reasons.push(DecisionReason::FormOverloaded);
            out.rules.push("R10");
        }
    }

    if session.hard {
        if strained {
            out.state = DecisionState::Lighten;
            out.main = lighter();
            out.alternatives.push(planned(session));
        } else {
            out.state = DecisionState::AsPlanned;
            out.main = Some(planned(session));
            out.alternatives.extend(lighter());
        }
        return out;
    }

    out.main = Some(planned(session));
    if strained {
        out.state = DecisionState::KeepEasy;
        let shorter = input
            .catalog
            .get(&session.template_id)
            .filter(|t| t.duration_min.min < session.duration_min)
            .map(|t| variant(VariantKind::Shorter, t, t.duration_min.min));
        out.alternatives.extend(shorter);
        return out;
    }

    out.state = DecisionState::AsPlanned;
    let fresh = input.readiness == ReadinessState::Normal && input.form == Some(FormState::Fresh);
    let quiet_week = matches!(week.phase, Phase::Taper | Phase::Race | Phase::Recovery);
    if fresh && !quiet_week {
        let harder = input.ctx.and_then(|ctx| {
            let t = hard_replacement(input.catalog, session)?;
            let v = variant(VariantKind::Harder, t, session.duration_min);
            harder_is_safe(plan, ctx, rules, (wi, di, si), &v).then_some(v)
        });
        if let Some(v) = harder {
            out.alternatives.push(v);
            out.reasons.push(DecisionReason::FormFresh);
        }
    }
    out
}

/// Woche und Tag des Datums im Plan.
fn locate(plan: &Plan, date: NaiveDate) -> Option<(usize, usize)> {
    plan.weeks.iter().enumerate().find_map(|(wi, w)| {
        w.days
            .iter()
            .position(|d| d.date == date)
            .map(|di| (wi, di))
    })
}

fn is_swapped(s: &PlanSession) -> bool {
    !s.hard
        && s.reason_keys
            .iter()
            .any(|k| k == keys::SESSION_RECOVERY_SWAP)
}

fn planned(s: &PlanSession) -> Variant {
    Variant {
        kind: VariantKind::Planned,
        sport: s.sport,
        template_id: s.template_id.clone(),
        duration_min: s.duration_min,
        hard: s.hard,
        planned_load: s.planned_load,
    }
}

/// Harte Verletzungen als Menge aus Regelcode und Datum.
fn hard_violations(
    plan: &Plan,
    ctx: &PlanContext,
    rules: &Rules,
) -> BTreeSet<(String, Option<NaiveDate>)> {
    validate_plan(plan, ctx, rules)
        .into_iter()
        .filter(|v| v.hardness == Hardness::Hard)
        .map(|v| (v.code, v.date))
        .collect()
}

/// Die härtere Variante ist zulässig, wenn `validate_plan` auf einer Kopie mit der getauschten
/// Einheit keine harte Verletzung meldet, die der Plan ohne Tausch nicht schon hat (R1, R3, R4,
/// R11 und alle übrigen harten Regeln). Verglichen wird nach Regelcode und Datum, damit eine
/// schon bestehende Verletzung derselben Regel in einer anderen Woche eine neue nicht verdeckt.
fn harder_is_safe(
    plan: &Plan,
    ctx: &PlanContext,
    rules: &Rules,
    (wi, di, si): (usize, usize, usize),
    v: &Variant,
) -> bool {
    let mut copy = plan.clone();
    let week = &mut copy.weeks[wi];
    let s = &mut week.days[di].sessions[si];
    s.template_id = v.template_id.clone();
    s.hard = true;
    s.planned_load = v.planned_load;
    s.reason_keys.clear();
    if week.planned_load.is_some() {
        week.planned_load = Some(round1(f64::from(week.sessions_load())));
    }
    let before = hard_violations(plan, ctx, rules);
    hard_violations(&copy, ctx, rules).is_subset(&before)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{
        avail, blank_week, catalog, continuous_goal, ctx, monday, open_ctx, plan_of, put, rules,
    };
    use super::super::*;
    use super::*;
    use crate::metrics::form::form_state;
    use crate::metrics::readiness::{readiness_state, recovery_deviation};
    use chrono::TimeDelta;

    /// Tag der Testwoche ab Montag, 4. Januar 2027 (0 ist Montag).
    fn day(offset: i64) -> NaiveDate {
        monday() + TimeDelta::days(offset)
    }

    /// Woche ab Montag: Dienstag `bike.threshold.2x20` 90 min (hart), Donnerstag
    /// `run.easy.steady` 40 min, Samstag `bike.endurance.steady` 100 min, sonst Ruhe.
    fn week(phase: Phase) -> PlanWeek {
        let mut w = blank_week(monday(), phase);
        put(&mut w, 1, "bike.threshold.2x20", 90);
        put(&mut w, 3, "run.easy.steady", 40);
        put(&mut w, 5, "bike.endurance.steady", 100);
        w
    }

    fn plan_with(w: PlanWeek) -> Plan {
        plan_of(vec![w], continuous_goal())
    }

    fn decide(
        plan: Option<&Plan>,
        c: Option<&PlanContext>,
        date: NaiveDate,
        readiness: ReadinessState,
        form: Option<FormState>,
    ) -> TodayDecision {
        let cat = catalog();
        let input = TodayInput {
            date,
            plan,
            ctx: c,
            readiness,
            form,
            catalog: &cat,
        };
        today_decision(&input, &rules())
    }

    fn ids(v: &[Variant]) -> Vec<(VariantKind, &str, u32)> {
        v.iter()
            .map(|x| (x.kind, x.template_id.as_str(), x.duration_min))
            .collect()
    }

    // Handrechnung der Lasten (`load_model`, Faustregel): Last = Minuten / 60 * IF^2 * 100,
    // auf eine Nachkommastelle gerundet.
    /// 90 min locker: 1,5 * 0,65^2 * 100 = 1,5 * 42,25 = 63,375, gerundet 63,4.
    const LOAD_EASY_90: f32 = 63.4;
    /// 100 min hart: 100 / 60 * 0,95^2 * 100 = 1,6667 * 90,25 = 150,417, gerundet 150,4.
    const LOAD_HARD_100: f32 = 150.4;
    /// 25 min locker: 25 / 60 * 42,25 = 17,604, gerundet 17,6.
    const LOAD_EASY_25: f32 = 17.6;

    /// Erholungstage für den Dienstag: sieben Tage davor mit HRV 60 ms (Basislinie
    /// 7 * 60 / 7 = 60 ms) und der Dienstag selbst mit `hrv`.
    fn hrv_series(hrv: f32) -> Vec<RecoveryDay> {
        let rd = |date, hrv| RecoveryDay {
            date,
            hrv_overnight_ms: Some(hrv),
            resting_hr_bpm: None,
            sleep_duration_s: None,
            sleep_score: None,
        };
        let mut v: Vec<RecoveryDay> = (1..=7)
            .map(|n| rd(day(1) - TimeDelta::days(n), 60.0))
            .collect();
        v.push(rd(day(1), hrv));
        v
    }

    #[test]
    fn hrv_15_percent_below_the_baseline_lightens_the_hard_session() {
        let r = rules();
        let c = open_ctx();
        let plan = plan_with(week(Phase::Base));
        // 51 ms: (60 - 51) / 60 = 9 / 60 = 0,15, die Schwelle von R12 gilt "ab".
        let dev = recovery_deviation(&hrv_series(51.0), day(1), &r);
        let low = readiness_state(dev.as_ref(), &r);
        assert_eq!(low, ReadinessState::Low);
        let d = decide(
            Some(&plan),
            Some(&c),
            day(1),
            low,
            Some(FormState::Balanced),
        );
        assert_eq!(d.state, DecisionState::Lighten);
        assert_eq!(d.state.code(), "decision.lighten");
        // Lockere Rad-Vorlagen mit Fenster um 90 min: nur bike.endurance.steady (45 bis 300),
        // bike.recovery.spin endet bei 60.
        let main = d.main.as_ref().expect("leichtere Variante");
        assert_eq!(
            (
                main.kind,
                main.template_id.as_str(),
                main.duration_min,
                main.hard
            ),
            (VariantKind::Lighter, "bike.endurance.steady", 90, false)
        );
        assert_eq!(main.planned_load, LOAD_EASY_90);
        assert_eq!(
            ids(&d.alternatives),
            vec![(VariantKind::Planned, "bike.threshold.2x20", 90)]
        );
        assert_eq!(d.reasons, vec![DecisionReason::ReadinessLow]);
        assert_eq!(d.rules, vec!["R12"]);
        assert!(d.missing.is_empty());

        // 52 ms: 8 / 60 = 0,1333 liegt unter 0,15, also normal und wie geplant.
        let dev = recovery_deviation(&hrv_series(52.0), day(1), &r);
        let normal = readiness_state(dev.as_ref(), &r);
        assert_eq!(normal, ReadinessState::Normal);
        let d = decide(
            Some(&plan),
            Some(&c),
            day(1),
            normal,
            Some(FormState::Balanced),
        );
        assert_eq!(d.state, DecisionState::AsPlanned);
        assert_eq!(d.main.as_ref().unwrap().kind, VariantKind::Planned);
        assert_eq!(
            ids(&d.alternatives),
            vec![(VariantKind::Lighter, "bike.endurance.steady", 90)]
        );
        assert!(d.reasons.is_empty() && d.rules.is_empty());
    }

    #[test]
    fn tsb_limits_follow_the_form_state() {
        let r = rules();
        let c = open_ctx();
        let plan = plan_with(week(Phase::Base));
        let at = |tsb: f32, date| {
            decide(
                Some(&plan),
                Some(&c),
                date,
                ReadinessState::Normal,
                Some(form_state(tsb, &r)),
            )
        };
        // TSB -30,0 liegt genau auf der Grenze von R10 und ist noch "belastet": wie geplant.
        assert_eq!(at(-30.0, day(1)).state, DecisionState::AsPlanned);
        // -30,1 liegt darunter, "überlastet": leichter, Grund R10.
        let d = at(-30.1, day(1));
        assert_eq!(d.state, DecisionState::Lighten);
        assert_eq!(d.reasons, vec![DecisionReason::FormOverloaded]);
        assert_eq!(d.rules, vec!["R10"]);
        // Locker geplant und überlastet: bleibt locker, kürzer bis zur Fenstergrenze.
        let d = at(-30.1, day(3));
        assert_eq!(d.state, DecisionState::KeepEasy);
        assert_eq!(
            ids(&d.alternatives),
            vec![(VariantKind::Shorter, "run.easy.steady", 25)]
        );
        assert_eq!(d.alternatives[0].planned_load, LOAD_EASY_25);
        // +5,0 ist "frisch". Hart geplant ändert das nichts.
        let d = at(5.0, day(1));
        assert_eq!(d.state, DecisionState::AsPlanned);
        assert!(d.alternatives.iter().all(|v| v.kind != VariantKind::Harder));
        // Locker geplant, frisch und Bereitschaft normal: die härtere Variante. Harte
        // Rad-Vorlagen mit Fenster um 100 min: bike.threshold.2x20 (60 bis 120) und
        // bike.vo2.5x4 (60 bis 105); das engere gewinnt.
        let d = at(5.0, day(5));
        assert_eq!(d.state, DecisionState::AsPlanned);
        assert_eq!(
            ids(&d.alternatives),
            vec![(VariantKind::Harder, "bike.vo2.5x4", 100)]
        );
        assert!(d.alternatives[0].hard);
        assert_eq!(d.alternatives[0].planned_load, LOAD_HARD_100);
        assert_eq!(d.reasons, vec![DecisionReason::FormFresh]);
        // 4,9 ist nur ausgewogen: keine härtere Variante.
        assert!(at(4.9, day(5)).alternatives.is_empty());
    }

    #[test]
    fn a_shorter_variant_needs_room_below_the_planned_duration() {
        // run.easy.steady beginnt bei 25 min: 25 min geplant lässt nichts zu kürzen.
        let c = open_ctx();
        let mut w = blank_week(monday(), Phase::Base);
        put(&mut w, 3, "run.easy.steady", 25);
        let d = decide(
            Some(&plan_with(w)),
            Some(&c),
            day(3),
            ReadinessState::Low,
            None,
        );
        assert_eq!(d.state, DecisionState::KeepEasy);
        assert!(d.alternatives.is_empty());
    }

    fn session(sport: PlanSport, minutes: u32) -> PlanSession {
        PlanSession {
            slot: Slot::Am,
            sport,
            template_id: "x.y".to_string(),
            duration_min: minutes,
            planned_load: 0.0,
            hard: false,
            rpe: None,
            reason_keys: Vec::new(),
        }
    }

    #[test]
    fn replacements_choose_the_narrowest_window() {
        let cat = catalog();
        let hard = |sport, m| hard_replacement(&cat, &session(sport, m)).map(|t| t.id.clone());
        let easy = |sport, m| easy_replacement(&cat, &session(sport, m)).map(|t| t.id.clone());
        let id = |s: &str| Some(s.to_string());
        // Lauf hart: run.tempo.continuous 40 bis 75, run.intervals.6x3 40 bis 70.
        assert_eq!(hard(PlanSport::Running, 60), id("run.intervals.6x3"));
        assert_eq!(hard(PlanSport::Running, 72), id("run.tempo.continuous"));
        assert_eq!(hard(PlanSport::Running, 76), None);
        // Rad hart: 60 bis 120 gegen 60 bis 105.
        assert_eq!(hard(PlanSport::Cycling, 105), id("bike.vo2.5x4"));
        assert_eq!(hard(PlanSport::Cycling, 106), id("bike.threshold.2x20"));
        // Kraft hat keine harte Vorlage.
        assert_eq!(hard(PlanSport::Strength, 45), None);
        // Rad locker: bike.endurance.steady 45 bis 300, bike.recovery.spin 30 bis 60.
        assert_eq!(easy(PlanSport::Cycling, 50), id("bike.recovery.spin"));
        assert_eq!(easy(PlanSport::Cycling, 61), id("bike.endurance.steady"));
        // Schwimmen locker endet bei 60 min.
        assert_eq!(easy(PlanSport::Swimming, 70), None);
    }

    #[test]
    fn without_an_easy_template_lighten_keeps_only_the_planned_alternative() {
        let c = open_ctx();
        let mut w = blank_week(monday(), Phase::Base);
        put(&mut w, 1, "swim.threshold.sets", 70);
        let plan = plan_with(w);
        let d = decide(Some(&plan), Some(&c), day(1), ReadinessState::Low, None);
        assert_eq!(d.state, DecisionState::Lighten);
        assert_eq!(d.main, None);
        assert_eq!(
            ids(&d.alternatives),
            vec![(VariantKind::Planned, "swim.threshold.sets", 70)]
        );
        assert_eq!(d.missing, vec![missing::FORM]);
    }

    fn harder_offered(plan: &Plan, c: &PlanContext, date: NaiveDate) -> bool {
        let d = decide(
            Some(plan),
            Some(c),
            date,
            ReadinessState::Normal,
            Some(FormState::Fresh),
        );
        assert_eq!(d.state, DecisionState::AsPlanned);
        d.alternatives.iter().any(|v| v.kind == VariantKind::Harder)
    }

    #[test]
    fn harder_is_rejected_when_it_would_break_r3_r4_or_r11() {
        let r = rules();
        let c = open_ctx();
        let base = plan_with(week(Phase::Base));
        assert!(is_acceptable(&validate_plan(&base, &c, &r)));
        assert!(harder_offered(&base, &c, day(5)));

        // R3: Dienstag und Donnerstag schon hart, ein dritter harter Tag wäre zu viel.
        let mut w = week(Phase::Base);
        w.days[3].sessions.clear();
        put(&mut w, 3, "run.intervals.6x3", 50);
        let plan = plan_with(w);
        assert!(is_acceptable(&validate_plan(&plan, &c, &r)));
        assert!(!harder_offered(&plan, &c, day(5)));

        // R4: Mittwoch locker direkt nach dem harten Dienstag.
        let mut w = week(Phase::Base);
        put(&mut w, 2, "run.easy.steady", 40);
        let plan = plan_with(w);
        assert!(!harder_offered(&plan, &c, day(2)));
        // Der Donnerstag liegt nicht neben einem harten Tag.
        assert!(harder_offered(&base, &c, day(3)));

        // R11: Samstag erlaubt keine harte Einheit.
        let mut c11 = open_ctx();
        c11.availability[5] = avail(360, false, false);
        assert!(!harder_offered(&base, &c11, day(5)));
    }

    #[test]
    fn an_existing_hard_violation_does_not_hide_a_new_one_elsewhere() {
        let r = rules();
        // Der Dienstag verletzt R11 schon (keine harte Einheit erlaubt). Der Samstag darf
        // trotzdem hart werden, ein harter Mittwoch nicht (neue Verletzung R4).
        let mut c = open_ctx();
        c.availability[1] = avail(360, false, false);
        let mut w = week(Phase::Base);
        put(&mut w, 2, "run.easy.steady", 40);
        let plan = plan_with(w);
        assert!(!is_acceptable(&validate_plan(&plan, &c, &r)));
        assert!(harder_offered(&plan, &c, day(5)));
        assert!(!harder_offered(&plan, &c, day(2)));
    }

    #[test]
    fn harder_needs_a_load_week_complete_data_and_a_context() {
        let c = open_ctx();
        for phase in [Phase::Taper, Phase::Race, Phase::Recovery] {
            assert!(
                !harder_offered(&plan_with(week(phase)), &c, day(5)),
                "{phase:?}"
            );
        }
        for phase in [Phase::Base, Phase::Build, Phase::Peak] {
            assert!(
                harder_offered(&plan_with(week(phase)), &c, day(5)),
                "{phase:?}"
            );
        }
        let plan = plan_with(week(Phase::Base));
        // Fehlende Bereitschaft gilt nicht als normal.
        let d = decide(
            Some(&plan),
            Some(&c),
            day(5),
            ReadinessState::NoData,
            Some(FormState::Fresh),
        );
        assert_eq!(d.state, DecisionState::AsPlanned);
        assert!(d.alternatives.is_empty());
        assert_eq!(d.missing, vec![missing::READINESS]);
        // Ohne Formzustand ebenso.
        let d = decide(Some(&plan), Some(&c), day(5), ReadinessState::Normal, None);
        assert!(d.alternatives.is_empty());
        assert_eq!(d.missing, vec![missing::FORM]);
        // Ohne Kontext keine Prüfung, also keine härtere Variante.
        let d = decide(
            Some(&plan),
            None,
            day(5),
            ReadinessState::Normal,
            Some(FormState::Fresh),
        );
        assert!(d.alternatives.is_empty());
        assert_eq!(d.missing, vec![missing::PLAN_CONTEXT]);
    }

    #[test]
    fn edge_cases_no_plan_rest_and_already_adjusted() {
        let c = open_ctx();
        let plan = plan_with(week(Phase::Base));
        let fresh = Some(FormState::Fresh);
        // Ohne Plan.
        let d = decide(None, None, day(1), ReadinessState::Low, fresh);
        assert_eq!(d.state, DecisionState::NoPlan);
        assert_eq!(d.state.code(), "decision.no_plan");
        assert!(d.main.is_none() && d.alternatives.is_empty() && d.rules.is_empty());
        assert!(d.missing.is_empty());
        // Tag vor und nach dem Plan.
        for date in [day(-1), day(7)] {
            let d = decide(Some(&plan), Some(&c), date, ReadinessState::Low, fresh);
            assert_eq!(d.state, DecisionState::NoPlan, "{date}");
        }
        // Montag ohne Einheit.
        let d = decide(Some(&plan), Some(&c), day(0), ReadinessState::Low, fresh);
        assert_eq!(d.state, DecisionState::Rest);
        assert!(d.main.is_none() && d.alternatives.is_empty() && d.reasons.is_empty());
        assert_eq!(d.rules, vec!["R05"]);
        // Von R12 getauschte Einheit: bleibt, wie sie ist, auch bei niedriger Bereitschaft.
        let mut w = blank_week(monday(), Phase::Base);
        put(&mut w, 1, "bike.endurance.steady", 90);
        w.days[1].sessions[0].reason_keys = vec![keys::SESSION_RECOVERY_SWAP.to_string()];
        let swapped = plan_with(w);
        let d = decide(
            Some(&swapped),
            Some(&c),
            day(1),
            ReadinessState::Low,
            Some(FormState::Overloaded),
        );
        assert_eq!(d.state, DecisionState::AlreadyAdjusted);
        assert_eq!(
            d.main.as_ref().map(|v| (v.kind, v.template_id.as_str())),
            Some((VariantKind::Planned, "bike.endurance.steady"))
        );
        assert!(d.alternatives.is_empty());
        assert_eq!(d.reasons, vec![DecisionReason::RecoverySwap]);
        assert_eq!(d.rules, vec!["R12"]);
    }

    #[test]
    fn both_signals_name_both_rules() {
        let c = open_ctx();
        let plan = plan_with(week(Phase::Base));
        let d = decide(
            Some(&plan),
            Some(&c),
            day(1),
            ReadinessState::Low,
            Some(FormState::Overloaded),
        );
        assert_eq!(d.state, DecisionState::Lighten);
        assert_eq!(
            d.reasons,
            vec![DecisionReason::ReadinessLow, DecisionReason::FormOverloaded]
        );
        assert_eq!(d.rules, vec!["R12", "R10"]);
    }

    #[test]
    fn variants_of_a_generated_plan_come_from_the_catalog() {
        let r = rules();
        let c = ctx();
        let cat = catalog();
        let plan = generate_plan(&c, &r).unwrap();
        let mut harder = 0;
        for d in plan.weeks.iter().flat_map(|w| &w.days) {
            let planned_ids: Vec<&str> =
                d.sessions.iter().map(|s| s.template_id.as_str()).collect();
            for readiness in [
                ReadinessState::Low,
                ReadinessState::Normal,
                ReadinessState::NoData,
            ] {
                for form in [Some(FormState::Fresh), Some(FormState::Overloaded), None] {
                    let out = decide(Some(&plan), Some(&c), d.date, readiness, form);
                    assert_ne!(out.state, DecisionState::NoPlan);
                    for v in out.main.iter().chain(&out.alternatives) {
                        let t = cat.get(&v.template_id).expect("Vorlage im Katalog");
                        assert_eq!(t.sport, v.sport);
                        let w = t.duration_min;
                        assert!(w.min <= v.duration_min && v.duration_min <= w.max);
                        match v.kind {
                            VariantKind::Planned => {
                                assert!(planned_ids.contains(&v.template_id.as_str()))
                            }
                            VariantKind::Harder => {
                                harder += 1;
                                assert!(v.hard);
                            }
                            VariantKind::Lighter | VariantKind::Shorter => assert!(!v.hard),
                        }
                    }
                }
            }
        }
        assert!(
            harder > 0,
            "der typische Plan bietet mindestens einmal härter an"
        );
    }

    #[test]
    fn codes_and_serde_shapes_are_stable() {
        assert_eq!(DecisionState::KeepEasy.code(), "decision.keep_easy");
        assert_eq!(
            DecisionState::AlreadyAdjusted.code(),
            "decision.already_adjusted"
        );
        assert_eq!(
            DecisionReason::FormFresh.code(),
            "decision.reason.form_fresh"
        );
        let d = TodayDecision {
            state: DecisionState::Lighten,
            main: Some(Variant {
                kind: VariantKind::Lighter,
                sport: PlanSport::Cycling,
                template_id: "bike.endurance.steady".to_string(),
                duration_min: 90,
                hard: false,
                planned_load: 63.5,
            }),
            alternatives: Vec::new(),
            reasons: vec![DecisionReason::ReadinessLow],
            rules: vec!["R12"],
            missing: vec![missing::FORM],
        };
        assert_eq!(
            serde_json::to_value(&d).unwrap(),
            serde_json::json!({
                "state": "lighten",
                "main": {
                    "kind": "lighter",
                    "sport": "cycling",
                    "template_id": "bike.endurance.steady",
                    "duration_min": 90,
                    "hard": false,
                    "planned_load": 63.5
                },
                "alternatives": [],
                "reasons": ["readiness_low"],
                "rules": ["R12"],
                "missing": ["form"]
            })
        );
    }
}
