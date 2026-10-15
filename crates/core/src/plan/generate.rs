//! Regelbasierter Planer (SPEC 8.3). Deterministisch, ohne Zufall und ohne Systemzeit.
//!
//! Ablauf je Plan: Phasen, dann je Woche Wochenziel, Wochenstruktur, Vorlagenwahl und Dauern,
//! Vorausberechnung mit `pmc`, Validierung mit Neuwahl und Begründungsschlüssel. Gleichstände
//! entscheidet die Katalogreihenfolge bzw. die Reihenfolge der Wochentage. Alle Zahlen, die
//! hier vorkommen, stehen in `rules.default.json` (Quelle und Härte je Regel).
//!
//! Wiederholung (Schritt 6): Verletzt eine Woche eine harte Regel oder eine vermeidbare weiche
//! Regel (R2, R7, R8, R10), baut der Planer sie neu auf: zuerst mit kleinerem Wochenziel, dann mit
//! weniger harten Einheiten (die Einheit wird leicht) und ohne mittlere Einheit. Gelingt keine
//! Fassung ohne harte Verletzung, wird die Woche zur Ruhewoche mit Warnung, nie zu einem
//! Plan mit harter Verletzung.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate, TimeDelta};

use super::catalog::{IntensityClass, Template};
use super::context::{GoalSpec, PlanContext};
use super::error::PlanError;
use super::recovery::{RULE_CODE as RECOVERY_RULE, apply_swap};
use super::rules::{Hardness, Rules, round1};
use super::types::{
    Goal, MAX_SESSION_MINUTES, PLAN_SCHEMA_VERSION, Phase, Plan, PlanDay, PlanSession, PlanSource,
    PlanSport, PlanWarning, PlanWeek, Severity, Slot,
};
use super::validate::{plan_pmc, validate_plan, validate_weeks, violations_to_warnings};

/// Begründungsschlüssel (Texte unter `reason.*` in `catalog/locales`).
pub mod keys {
    pub const PHASE_BASE: &str = "reason.week.phase_base";
    pub const PHASE_BUILD: &str = "reason.week.phase_build";
    pub const PHASE_PEAK: &str = "reason.week.phase_peak";
    pub const PHASE_TAPER: &str = "reason.week.phase_taper";
    pub const PHASE_RECOVERY: &str = "reason.week.phase_recovery";
    pub const PHASE_RACE: &str = "reason.week.phase_race";
    pub const GROWTH: &str = "reason.week.growth";
    pub const START_FROM_CTL: &str = "reason.week.start_from_ctl";
    pub const RECOVERY_BLOCK: &str = "reason.week.recovery_block";
    pub const TAPER_REDUCTION: &str = "reason.week.taper_reduction";
    pub const CAPPED_CTL: &str = "reason.week.capped_ctl";
    pub const CAPPED_AVAILABILITY: &str = "reason.week.capped_availability";
    pub const TARGET_REDUCED: &str = "reason.week.target_reduced";
    pub const HARD_REDUCED: &str = "reason.week.hard_reduced";
    pub const MODERATE_REMOVED: &str = "reason.week.moderate_removed";
    pub const FALLBACK_REST: &str = "reason.week.fallback_rest";
    pub const NO_TRAINING_DAYS: &str = "reason.week.no_training_days";
    pub const EVENT_AFTER_HORIZON: &str = "reason.week.event_after_horizon";
    pub const SESSION_LONG: &str = "reason.session.long";
    pub const SESSION_HARD: &str = "reason.session.hard";
    pub const SESSION_MODERATE: &str = "reason.session.moderate";
    pub const SESSION_EASY: &str = "reason.session.easy";
    pub const SESSION_RECOVERY_AFTER_HARD: &str = "reason.session.recovery_after_hard";
    pub const SESSION_STRENGTH: &str = "reason.session.strength";
    pub const SESSION_RACE_OPENER: &str = "reason.session.race_opener";
    pub const SESSION_RECOVERY_SWAP: &str = "reason.session.recovery_swap";

    /// Alle Schlüssel, für den Test gegen die Sprachdateien.
    pub const ALL: [&str; 26] = [
        PHASE_BASE,
        PHASE_BUILD,
        PHASE_PEAK,
        PHASE_TAPER,
        PHASE_RECOVERY,
        PHASE_RACE,
        GROWTH,
        START_FROM_CTL,
        RECOVERY_BLOCK,
        TAPER_REDUCTION,
        CAPPED_CTL,
        CAPPED_AVAILABILITY,
        TARGET_REDUCED,
        HARD_REDUCED,
        MODERATE_REMOVED,
        FALLBACK_REST,
        NO_TRAINING_DAYS,
        EVENT_AFTER_HORIZON,
        SESSION_LONG,
        SESSION_HARD,
        SESSION_MODERATE,
        SESSION_EASY,
        SESSION_RECOVERY_AFTER_HARD,
        SESSION_STRENGTH,
        SESSION_RACE_OPENER,
        SESSION_RECOVERY_SWAP,
    ];
}

fn phase_key(phase: Phase) -> &'static str {
    match phase {
        Phase::Base => keys::PHASE_BASE,
        Phase::Build => keys::PHASE_BUILD,
        Phase::Peak => keys::PHASE_PEAK,
        Phase::Taper => keys::PHASE_TAPER,
        Phase::Recovery => keys::PHASE_RECOVERY,
        Phase::Race => keys::PHASE_RACE,
    }
}

/// Eine Woche des Phasenplans.
#[derive(Debug, Clone)]
struct WeekSpec {
    week_start: NaiveDate,
    phase: Phase,
    /// Tage, an denen nicht trainiert wird (Rennwoche: Tag vor dem Termin, Termin, danach).
    blocked: [bool; 7],
    /// Wochentag des Termins, nur in der Rennwoche.
    event_idx: Option<usize>,
    /// Der Termin liegt hinter dem Planungshorizont, dies ist die letzte Planwoche.
    event_after_horizon: bool,
}

fn weekday_idx(d: NaiveDate) -> usize {
    d.weekday().num_days_from_monday() as usize
}

/// Phasenplan nach SPEC 8.3, Schritt 1.
///
/// Mit Termin rückwärts: Rennwoche (1), Taper, Spitze, Aufbau, davor Basis. Ohne Termin
/// Blöcke aus `R06.load_weeks` Belastungswochen und einer Erholungswoche. Eine Erholungswoche
/// ersetzt die Belastungswoche nach dem vollen Block, außer es folgt Taper oder Rennwoche
/// (dann wirkt der Taper als Erholung, siehe R6).
fn build_schedule(ctx: &PlanContext, rules: &Rules) -> Vec<WeekSpec> {
    let ev = &rules.phases.event;
    let horizon = ctx.horizon_weeks as usize;
    // Phase der i-ten Woche vor dem Überlagern der Erholungswochen, `None` hinter dem Plan.
    type Underlying = Box<dyn Fn(usize) -> Option<Phase>>;
    let (underlying, n, event_idx, after_horizon): (Underlying, usize, Option<usize>, bool) =
        match &ctx.goal {
            GoalSpec::Event { event_date, .. } => {
                let idx = weekday_idx(*event_date);
                let monday = *event_date - TimeDelta::days(idx as i64);
                let total = ((monday - ctx.week_start).num_days() / 7 + 1) as usize;
                let (tw, pw, bw) = (
                    ev.taper_weeks as usize,
                    ev.peak_weeks as usize,
                    ev.build_weeks as usize,
                );
                let underlying = move |i: usize| -> Phase {
                    let from_end = total - 1 - i;
                    if from_end == 0 {
                        Phase::Race
                    } else if from_end <= tw {
                        Phase::Taper
                    } else if from_end <= tw + pw {
                        Phase::Peak
                    } else if from_end <= tw + pw + bw {
                        Phase::Build
                    } else {
                        Phase::Base
                    }
                };
                let n = total.min(horizon);
                let phase_of = Box::new(move |i: usize| (i < total).then(|| underlying(i)));
                (phase_of, n, Some(idx), total > n)
            }
            GoalSpec::Continuous => {
                let load_phase = rules.phases.continuous.load_phase;
                (
                    Box::new(move |i: usize| (i < horizon).then_some(load_phase)),
                    horizon,
                    None,
                    false,
                )
            }
        };

    let mut specs = Vec::with_capacity(n);
    let mut streak = 0u32;
    for i in 0..n {
        let u = underlying(i).unwrap_or(Phase::Base);
        let next = underlying(i + 1);
        let phase = if u.is_load_week() {
            if streak >= rules.r06.load_weeks && !matches!(next, Some(Phase::Taper | Phase::Race)) {
                streak = 0;
                Phase::Recovery
            } else {
                streak += 1;
                u
            }
        } else {
            streak = 0;
            u
        };
        let mut blocked = [false; 7];
        let mut event = None;
        if phase == Phase::Race
            && let Some(e) = event_idx
        {
            event = Some(e);
            for (d, b) in blocked.iter_mut().enumerate() {
                // Der Tag vor dem Termin, der Termin und die Tage danach bleiben frei.
                *b = d + 1 >= e;
            }
        }
        specs.push(WeekSpec {
            week_start: ctx.week_start + TimeDelta::days(7 * i as i64),
            phase,
            blocked,
            event_idx: event,
            event_after_horizon: after_horizon && i + 1 == n,
        });
    }
    specs
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Long,
    Hard,
    Moderate,
    Easy,
}

/// Zustand, der von Woche zu Woche weitergegeben wird.
#[derive(Debug, Clone, Default)]
struct Rotation {
    hard: BTreeMap<PlanSport, usize>,
    strength: usize,
}

/// Was am letzten Tag der Vorwoche lag (für R4 und R7 über die Wochengrenze).
#[derive(Debug, Clone, Copy, Default)]
struct Carry {
    last_day_hard: bool,
    last_day_strength: bool,
}

struct Built {
    week: PlanWeek,
    capped: bool,
    rotation: Rotation,
    training_days: usize,
}

/// Entwurf einer Ausdauereinheit.
#[derive(Debug, Clone)]
struct Draft<'c> {
    day: usize,
    role: Role,
    sport: PlanSport,
    templates: Vec<&'c Template>,
}

/// Sortierschlüssel der Tageswahl für harte Einheiten (kleiner ist besser).
type HardKey = (usize, Reverse<usize>, Reverse<u32>, u32);

/// Auswahl der Tage für harte Einheiten: höchstens `n`, nie an Folgetagen, bevorzugt nicht
/// neben der Langeinheit, mit großem Abstand und an Tagen mit viel Zeit. Der kleinste
/// Wochentag entscheidet Gleichstände.
fn choose_hard_days(
    ok: &[bool; 7],
    n: usize,
    long_day: Option<usize>,
    minutes: &[u16; 7],
) -> Vec<usize> {
    for size in (1..=n.min(7)).rev() {
        let mut best: Option<(HardKey, Vec<usize>)> = None;
        for mask in 0u32..128 {
            if mask.count_ones() as usize != size {
                continue;
            }
            let days: Vec<usize> = (0..7).filter(|i| mask >> i & 1 == 1).collect();
            if days.iter().any(|&d| !ok[d]) || days.windows(2).any(|w| w[1] - w[0] < 2) {
                continue;
            }
            let near_long = days
                .iter()
                .filter(|&&d| long_day.is_some_and(|l| d.abs_diff(l) == 1))
                .count();
            let gap = days.windows(2).map(|w| w[1] - w[0]).min().unwrap_or(7);
            let time: u32 = days.iter().map(|&d| u32::from(minutes[d])).sum();
            let key = (near_long, Reverse(gap), Reverse(time), mask);
            if best.as_ref().is_none_or(|(k, _)| key < *k) {
                best = Some((key, days));
            }
        }
        if let Some((_, days)) = best {
            return days;
        }
    }
    Vec::new()
}

/// Sportart mit dem größten Verhältnis Gewicht zu (bisherige Einheiten + 1), nach D'Hondt.
/// Gleichstand: Reihenfolge in `ctx.sports`.
fn pick_sport(
    ctx: &PlanContext,
    eligible: impl Fn(PlanSport) -> bool,
    counts: &BTreeMap<PlanSport, u32>,
) -> Option<PlanSport> {
    let mut best: Option<(PlanSport, f64)> = None;
    for s in &ctx.sports {
        if !eligible(s.sport) {
            continue;
        }
        let score = f64::from(s.weight) / f64::from(counts.get(&s.sport).copied().unwrap_or(0) + 1);
        if best.is_none_or(|(_, b)| score > b) {
            best = Some((s.sport, score));
        }
    }
    best.map(|(s, _)| s)
}

fn eff_min(t: &Template, min_session: u32) -> u32 {
    t.duration_min.min.max(min_session)
}

fn eff_max(t: &Template, budget: u32) -> u32 {
    t.duration_min.max.min(MAX_SESSION_MINUTES).min(budget)
}

/// Vorlage zur Zieldauer `t` im Rahmen des Tagesbudgets (SPEC 8.3, Schritt 4).
///
/// Unter den Vorlagen, deren Dauerfenster `t` enthält: Langeinheit mit dem größten Fenster,
/// harte Einheit reihum (Abwechslung), sonst das engste Fenster. Enthält keines `t`, gewinnt
/// das nächstliegende Fenster. Gleichstände nach Katalogreihenfolge.
fn choose_template<'c>(
    cands: &[&'c Template],
    t: f64,
    budget: u32,
    role: Role,
    sport: PlanSport,
    min_session: u32,
    rot: &mut Rotation,
) -> Option<&'c Template> {
    let feasible: Vec<&'c Template> = cands
        .iter()
        .copied()
        .filter(|tp| eff_min(tp, min_session) <= eff_max(tp, budget))
        .collect();
    if feasible.is_empty() {
        return None;
    }
    let t = t.min(f64::from(budget));
    let contains: Vec<&'c Template> = feasible
        .iter()
        .copied()
        .filter(|tp| {
            f64::from(eff_min(tp, min_session)) <= t && t <= f64::from(eff_max(tp, budget))
        })
        .collect();
    if !contains.is_empty() {
        return Some(match role {
            Role::Long => {
                let mut best = contains[0];
                for tp in &contains[1..] {
                    if tp.duration_min.max > best.duration_min.max {
                        best = tp;
                    }
                }
                best
            }
            Role::Hard => {
                let c = rot.hard.entry(sport).or_insert(0);
                let pick = contains[*c % contains.len()];
                *c += 1;
                pick
            }
            Role::Moderate | Role::Easy => {
                let mut best = contains[0];
                for tp in &contains[1..] {
                    if tp.duration_min.max < best.duration_min.max {
                        best = tp;
                    }
                }
                best
            }
        });
    }
    let dist = |tp: &Template| {
        let lo = f64::from(eff_min(tp, min_session));
        let hi = f64::from(eff_max(tp, budget));
        if t < lo { lo - t } else { t - hi }
    };
    let mut best = feasible[0];
    for tp in &feasible[1..] {
        if dist(tp) < dist(best) {
            best = tp;
        }
    }
    Some(best)
}

fn role_templates(ctx: &PlanContext, sport: PlanSport, role: Role) -> Vec<&Template> {
    let cat = &ctx.catalog;
    match role {
        Role::Long => ctx
            .catalog
            .templates
            .iter()
            .filter(|t| {
                t.sport == sport
                    && matches!(
                        t.intensity_class,
                        IntensityClass::Easy | IntensityClass::Moderate
                    )
            })
            .collect(),
        Role::Hard => cat.select(sport, IntensityClass::Hard),
        Role::Moderate => cat.select(sport, IntensityClass::Moderate),
        Role::Easy => cat.select(sport, IntensityClass::Easy),
    }
}

/// Einheit in der Planung, bevor Dauern feststehen.
enum Item<'c> {
    Endurance(Draft<'c>),
    Strength {
        day: usize,
        template: &'c Template,
        minutes: u32,
    },
}

impl Item<'_> {
    fn day(&self) -> usize {
        match self {
            Item::Endurance(d) => d.day,
            Item::Strength { day, .. } => *day,
        }
    }

    /// Je kleiner, desto früher entfällt die Einheit, wenn das Wochenziel zu klein ist.
    fn drop_rank(&self) -> u8 {
        match self {
            Item::Endurance(d) => match d.role {
                Role::Easy => 0,
                Role::Moderate => 1,
                Role::Hard => 3,
                Role::Long => 4,
            },
            Item::Strength { .. } => 2,
        }
    }
}

struct Generator<'a> {
    ctx: &'a PlanContext,
    rules: &'a Rules,
    goal: Goal,
    weeks: Vec<PlanWeek>,
    carry: Carry,
    rotation: Rotation,
    /// Geplante Last der letzten Belastungswoche, Bezug für den Zuwachs.
    last_load: f64,
    notes: Vec<PlanWarning>,
}

impl<'a> Generator<'a> {
    fn ctl_at_start(&self) -> f64 {
        plan_pmc(&self.weeks, self.ctx)
            .last()
            .map_or(f64::from(self.ctx.ctl0), |p| f64::from(p.ctl))
    }

    /// Höchste Wochenlast, bei der gleichmäßig verteilte Last die CTL um höchstens
    /// `Grenze * planning_margin` steigen lässt. Nach `CTL' = CTL + (L - CTL) * (1 - e^(-7/tau))`
    /// für sieben Tage mit gleicher Tageslast `L` (Modell SPEC 8.2).
    fn ramp_cap(&self, ctl_start: f64) -> f64 {
        let limit = f64::from(self.ctx.ctl_rise_limit(self.rules))
            * f64::from(self.rules.r01.planning_margin);
        let tau = f64::from(self.ctx.pmc_params.tau_ctl_d);
        let gain = 1.0 - (-7.0 / tau).exp();
        7.0 * (ctl_start + limit / gain)
    }

    fn add_week(&mut self, spec: &WeekSpec) {
        let rules = self.rules;
        let profile = rules.phases.profile(spec.phase);
        let ctl_start = self.ctl_at_start();
        let mut keys: Vec<String> = vec![phase_key(spec.phase).to_string()];
        if self.weeks.is_empty() && self.start_from_ctl() {
            keys.push(keys::START_FROM_CTL.to_string());
        }

        let mut target = match spec.phase {
            Phase::Base | Phase::Build | Phase::Peak => {
                keys.push(keys::GROWTH.to_string());
                self.last_load * (1.0 + f64::from(profile.weekly_growth.unwrap_or(0.0)))
            }
            Phase::Recovery => {
                keys.push(keys::RECOVERY_BLOCK.to_string());
                self.last_load * f64::from(profile.volume_factor.unwrap_or(1.0))
            }
            Phase::Taper | Phase::Race => {
                keys.push(keys::TAPER_REDUCTION.to_string());
                self.last_load * f64::from(profile.volume_factor.unwrap_or(1.0))
            }
        };
        let cap = self.ramp_cap(ctl_start);
        if target > cap {
            target = cap;
            keys.push(keys::CAPPED_CTL.to_string());
        }
        let target = target.max(0.0);

        let hard0 = profile.max_hard.min(rules.r03.max_hard_per_week) as usize;
        let mod0 = profile.moderate_sessions as usize;
        let mut variants = vec![(hard0, mod0)];
        if mod0 > 0 {
            variants.push((hard0, 0));
        }
        for k in (0..hard0).rev() {
            variants.push((k, 0));
        }

        let g = &rules.generator;
        let mut tries = 0u32;
        let mut accepted: Option<(Built, (usize, usize), f64)> = None;
        let mut fallback: Option<(usize, (usize, usize), f64, Built)> = None;
        let mut first_hard_code: Option<String> = None;
        'search: for &(h, m) in &variants {
            for k in 0..=g.scale_steps {
                if tries >= g.max_retries {
                    break 'search;
                }
                tries += 1;
                let scale = 1.0 - f64::from(g.scale_step) * f64::from(k);
                let built = self.build_week(spec, target * scale, h, m);
                let violations = self.check_candidate(&built.week);
                if let Some(v) = violations.iter().find(|v| v.hardness == Hardness::Hard) {
                    first_hard_code.get_or_insert_with(|| v.code.clone());
                    continue;
                }
                // Vermeidbar sind weiche Verletzungen, die eine andere Fassung der Woche beheben
                // kann. In Taper und Rennwoche bleibt die Intensität (R9), dort bleibt R8 eine
                // Warnung.
                let intense_phase = matches!(spec.phase, Phase::Taper | Phase::Race);
                let avoidable = violations.iter().any(|v| match v.code.as_str() {
                    "R02" | "R07" | "R10" => true,
                    "R08" => !intense_phase,
                    _ => false,
                });
                if !avoidable {
                    accepted = Some((built, (h, m), scale));
                    break 'search;
                }
                if fallback
                    .as_ref()
                    .is_none_or(|(n, ..)| violations.len() < *n)
                {
                    fallback = Some((violations.len(), (h, m), scale, built));
                }
                // Ein kleineres Wochenziel hilft nur bei R2 und R10, sonst nächste Fassung.
                if !violations
                    .iter()
                    .any(|v| matches!(v.code.as_str(), "R02" | "R10"))
                {
                    continue 'search;
                }
            }
        }
        let chosen = accepted.or(fallback.map(|(_, hm, scale, b)| (b, hm, scale)));

        let (mut built, (h, m), scale) = match chosen {
            Some(c) => c,
            None => {
                // Keine Fassung ohne harte Verletzung: Ruhewoche mit Warnung statt Verletzung.
                self.notes.push(PlanWarning {
                    code: first_hard_code.unwrap_or_else(|| "R01".to_string()),
                    severity: Severity::Warn,
                    date: Some(spec.week_start),
                });
                keys.push(keys::FALLBACK_REST.to_string());
                let mut b = self.empty_week(spec, target);
                b.week.reason_keys = keys;
                self.finish_week(b, spec);
                return;
            }
        };
        if scale < 1.0 {
            keys.push(keys::TARGET_REDUCED.to_string());
        }
        // Auch wenn das Wochenziel nicht für die harte Einheit reicht und sie deshalb entfällt
        // (Rennwoche und Taper mit kleinem Ziel), soll die Woche das sagen.
        let hard_planned = built
            .week
            .days
            .iter()
            .flat_map(|d| d.sessions.iter())
            .filter(|s| s.hard)
            .count();
        if h < hard0 || hard_planned < h {
            keys.push(keys::HARD_REDUCED.to_string());
        }
        if m < mod0 {
            keys.push(keys::MODERATE_REMOVED.to_string());
        }
        if built.capped {
            keys.push(keys::CAPPED_AVAILABILITY.to_string());
        }
        if built.training_days == 0 {
            keys.push(keys::NO_TRAINING_DAYS.to_string());
        } else if built.week.target_load > 0.0
            && built.week.days.iter().all(|d| d.sessions.is_empty())
        {
            // Positives Ziel, aber schon die kürzeste Einheit liegt darüber: Die Woche bleibt
            // leer und sagt das. Eine Warnung gibt es nicht, weil keine Regel verletzt ist
            // (Warnungen tragen den Code einer Regel, SPEC 7.1 und 8.4).
            keys.push(keys::FALLBACK_REST.to_string());
        }
        if spec.event_after_horizon {
            keys.push(keys::EVENT_AFTER_HORIZON.to_string());
        }
        built.week.reason_keys = keys;
        self.finish_week(built, spec);
    }

    /// Ohne Verlauf (erste Woche) beginnt der Planer bei der CTL-äquivalenten Wochenlast.
    fn start_from_ctl(&self) -> bool {
        !self.ctx.previous_week().is_some_and(|p| p.load > 0.0)
    }

    fn empty_week(&self, spec: &WeekSpec, target: f64) -> Built {
        Built {
            week: PlanWeek {
                week_start: spec.week_start,
                phase: spec.phase,
                target_load: round1(target),
                planned_load: Some(0.0),
                reason_keys: Vec::new(),
                days: (0..7)
                    .map(|i| PlanDay {
                        date: spec.week_start + TimeDelta::days(i),
                        sessions: Vec::new(),
                    })
                    .collect(),
            },
            capped: false,
            rotation: self.rotation.clone(),
            training_days: 0,
        }
    }

    /// Übernimmt die Woche und rechnet den Zustand für die nächste fort.
    fn finish_week(&mut self, built: Built, spec: &WeekSpec) {
        let week = built.week;
        let planned = f64::from(week.planned_load.unwrap_or(0.0));
        if spec.phase.is_load_week() && planned > 0.0 {
            self.last_load = planned;
        }
        let last = week.days.last();
        self.carry = Carry {
            last_day_hard: last.is_some_and(|d| d.sessions.iter().any(|s| s.hard)),
            last_day_strength: last
                .is_some_and(|d| d.sessions.iter().any(|s| s.sport == PlanSport::Strength)),
        };
        self.rotation = built.rotation;
        self.weeks.push(week);
    }

    /// Verletzungen, die in oder nach der neuen Woche liegen. Ältere Wochen sind schon
    /// angenommen.
    fn check_candidate(&mut self, week: &PlanWeek) -> Vec<super::validate::Violation> {
        let start = week.week_start;
        self.weeks.push(week.clone());
        let v = validate_weeks(&self.weeks, &self.goal, self.ctx, self.rules);
        self.weeks.pop();
        // R12 wertet erst `generate_plan` nach dem Aufbau aller Wochen aus (Austausch der
        // ersten harten Einheit). Als Kriterium für eine Fassung der Woche taugt sie nicht.
        v.into_iter()
            .filter(|v| v.code != RECOVERY_RULE && v.date.is_none_or(|d| d >= start))
            .collect()
    }

    /// Wochenstruktur, Vorlagenwahl und Dauern (SPEC 8.3, Schritte 3 und 4).
    fn build_week(&self, spec: &WeekSpec, target: f64, hard_n: usize, moderate_n: usize) -> Built {
        let ctx = self.ctx;
        let rules = self.rules;
        let profile = rules.phases.profile(spec.phase);
        let min_s = rules.structure.min_session_minutes;
        let step = rules.structure.duration_step_min;
        let avail = &ctx.availability;
        let minutes_of: [u16; 7] = std::array::from_fn(|i| avail[i].max_minutes);
        let mut rot = self.rotation.clone();

        // Nutzbare Tage: kein fester Ruhetag, nicht gesperrt, genug Zeit für eine Einheit.
        let mut usable: [bool; 7] = std::array::from_fn(|i| {
            !avail[i].fixed_rest && !spec.blocked[i] && u32::from(avail[i].max_minutes) >= min_s
        });
        // R5: Gibt es zu wenige Ruhetage, wird der nutzbare Tag mit der wenigsten Zeit frei.
        let min_rest = rules.r05.min_rest_days_per_week as usize;
        while usable.iter().filter(|u| !**u).count() < min_rest {
            let Some(i) = (0..7)
                .filter(|&i| usable[i])
                .min_by_key(|&i| (minutes_of[i], i))
            else {
                break;
            };
            usable[i] = false;
        }
        let training_days = usable.iter().filter(|u| **u).count();

        // Langeinheit am nutzbaren Tag mit der größten Zeit, bei Gleichstand der spätere Tag.
        let long_day = if profile.long_session {
            (0..7)
                .filter(|&i| usable[i])
                .max_by_key(|&i| (minutes_of[i], i))
        } else {
            None
        };

        // Harte Tage (R3, R4).
        let opener_days = rules.phases.event.race_opener_min_days_before as usize;
        let hard_ok: [bool; 7] = std::array::from_fn(|i| {
            usable[i]
                && avail[i].allow_hard
                && Some(i) != long_day
                && !(i == 0 && (self.carry.last_day_hard || self.carry.last_day_strength))
                && spec.event_idx.is_none_or(|e| i + opener_days <= e)
        });
        let hard_days = choose_hard_days(&hard_ok, hard_n, long_day, &minutes_of);

        let mut taken = [false; 7];
        if let Some(l) = long_day {
            taken[l] = true;
        }
        for &d in &hard_days {
            taken[d] = true;
        }
        let near_hard =
            |i: usize| -> usize { hard_days.iter().filter(|&&h| h.abs_diff(i) == 1).count() };

        // Mittlere Einheiten: Tage mit viel Zeit, nicht neben harten Tagen.
        let mut moderate_days: Vec<usize> = Vec::new();
        for _ in 0..moderate_n {
            let pick = (0..7)
                .filter(|&i| usable[i] && !taken[i])
                .min_by_key(|&i| (near_hard(i), Reverse(minutes_of[i]), i));
            let Some(i) = pick else { break };
            taken[i] = true;
            moderate_days.push(i);
        }

        // Kraft (R7): nicht am Tag vor einer harten Einheit.
        let strength_n = (ctx.strength_per_week as usize)
            .min(profile.strength_max as usize)
            .min(rules.r07.max_per_week as usize);
        let mut strength_days: Vec<(usize, &Template, u32)> = Vec::new();
        let strength_templates = ctx
            .catalog
            .select(PlanSport::Strength, IntensityClass::Moderate);
        if strength_n > 0 && !strength_templates.is_empty() {
            for _ in 0..strength_n {
                let tpl = strength_templates[rot.strength % strength_templates.len()];
                let lo = eff_min(tpl, min_s);
                let minutes = tpl
                    .nominal_minutes()
                    .max(lo)
                    .min(eff_max(tpl, u32::MAX).max(lo));
                let pick = (0..7)
                    .filter(|&i| {
                        usable[i]
                            && !strength_days.iter().any(|(d, ..)| *d == i)
                            && !(i < 6 && hard_days.contains(&(i + 1)))
                            && u32::from(minutes_of[i]) >= minutes + min_s
                    })
                    .min_by_key(|&i| {
                        let busy = u8::from(hard_days.contains(&i) || Some(i) == long_day);
                        let adjacent = strength_days
                            .iter()
                            .filter(|(d, ..)| d.abs_diff(i) == 1)
                            .count();
                        (busy, adjacent, i)
                    });
                let Some(i) = pick else { break };
                rot.strength += 1;
                strength_days.push((i, tpl, minutes));
            }
        }

        // Ausdauereinheiten mit Sportart. Reihenfolge: Langeinheit, harte, mittlere, leichte.
        let mut counts: BTreeMap<PlanSport, u32> = BTreeMap::new();
        let mut items: Vec<Item> = Vec::new();
        let mut order: Vec<(usize, Role)> = Vec::new();
        if let Some(l) = long_day {
            order.push((l, Role::Long));
        }
        order.extend(hard_days.iter().map(|&d| (d, Role::Hard)));
        order.extend(moderate_days.iter().map(|&d| (d, Role::Moderate)));
        for i in 0..7 {
            if usable[i] && !taken[i] {
                order.push((i, Role::Easy));
            }
        }
        for (day, role) in order {
            let mut role = role;
            let sport = loop {
                let picked = if role == Role::Long {
                    Some(ctx.primary_sport())
                } else {
                    pick_sport(ctx, |s| !role_templates(ctx, s, role).is_empty(), &counts)
                };
                match picked {
                    Some(s) => break Some(s),
                    None if role != Role::Easy => role = Role::Easy,
                    None => break None,
                }
            };
            let Some(sport) = sport else { continue };
            *counts.entry(sport).or_insert(0) += 1;
            items.push(Item::Endurance(Draft {
                day,
                role,
                sport,
                templates: role_templates(ctx, sport, role),
            }));
        }
        for &(day, template, minutes) in &strength_days {
            items.push(Item::Strength {
                day,
                template,
                minutes,
            });
        }

        let rate_of = |class: IntensityClass| rules.load_model.rate_per_min(class);
        let provisional_class = |d: &Draft| match d.role {
            Role::Hard => IntensityClass::Hard,
            Role::Moderate => IntensityClass::Moderate,
            Role::Long | Role::Easy => IntensityClass::Easy,
        };
        let strength_load = |items: &[Item]| -> f64 {
            items
                .iter()
                .map(|it| match it {
                    Item::Strength {
                        template, minutes, ..
                    } => {
                        crate::plan::rules::LoadModel::strength_rate_per_min(
                            template.rpe.unwrap_or(5),
                        ) * f64::from(*minutes)
                    }
                    Item::Endurance(_) => 0.0,
                })
                .sum()
        };
        let strength_minutes_on = |items: &[Item], day: usize| -> u32 {
            items
                .iter()
                .map(|it| match it {
                    Item::Strength {
                        day: d, minutes, ..
                    } if *d == day => *minutes,
                    _ => 0,
                })
                .sum()
        };

        // Untergrenze der Last einer Einheit (kürzeste passende Vorlage, Kraft mit Nenndauer);
        // zu kleines Wochenziel lässt Einheiten entfallen.
        let item_lo = |items: &[Item], it: &Item| -> f64 {
            match it {
                Item::Strength {
                    template, minutes, ..
                } => {
                    crate::plan::rules::LoadModel::strength_rate_per_min(template.rpe.unwrap_or(5))
                        * f64::from(*minutes)
                }
                Item::Endurance(d) => {
                    let budget = u32::from(minutes_of[d.day]) - strength_minutes_on(items, d.day);
                    d.templates
                        .iter()
                        .filter(|t| eff_min(t, min_s) <= eff_max(t, budget))
                        .map(|t| eff_min(t, min_s))
                        .min()
                        .map_or(0.0, |lo| f64::from(lo) * rate_of(provisional_class(d)))
                }
            }
        };
        let lo_load = |items: &[Item]| -> f64 { items.iter().map(|it| item_lo(items, it)).sum() };
        // Einheiten ohne passende Vorlage im Tagesbudget zuerst: Hart und Mittel werden
        // leicht, scheitert auch das, entfällt die Einheit.
        let mut i = 0;
        while i < items.len() {
            let fits = match &items[i] {
                Item::Endurance(d) => {
                    let budget = u32::from(minutes_of[d.day]) - strength_minutes_on(&items, d.day);
                    d.templates
                        .iter()
                        .any(|t| eff_min(t, min_s) <= eff_max(t, budget))
                }
                Item::Strength { .. } => true,
            };
            if fits {
                i += 1;
                continue;
            }
            let (day, role, sport) = match &items[i] {
                Item::Endurance(d) => (d.day, d.role, d.sport),
                Item::Strength { .. } => unreachable!("Kraft passt immer"),
            };
            if role == Role::Hard || role == Role::Moderate {
                let easy = role_templates(ctx, sport, Role::Easy);
                items[i] = Item::Endurance(Draft {
                    day,
                    role: Role::Easy,
                    sport,
                    templates: easy,
                });
            } else {
                items.remove(i);
            }
        }
        // Einheiten, deren Mindestlast schon zusammen mit der Langeinheit allein über dem
        // Wochenziel liegt, können nie bleiben. Sie entfallen vor den leichten Einheiten,
        // sonst fielen erst alle leichten und dann sie selbst weg, und die Woche bliebe leer,
        // obwohl leichte Einheiten in das Ziel passen (Abweichungsvermerk in SPEC 8.3). Beispiel
        // Rennwoche: Eine harte Radeinheit braucht mindestens 60 min bei IF 0,95, also
        // 90,25 Last, das Ziel liegt bei 50 Prozent der letzten Belastungswoche oft darunter.
        let long_lo: f64 = items
            .iter()
            .filter(|it| it.drop_rank() >= 4)
            .map(|it| item_lo(&items, it))
            .sum();
        let mut i = 0;
        while i < items.len() {
            if items[i].drop_rank() < 4 && long_lo + item_lo(&items, &items[i]) > target {
                items.remove(i);
            } else {
                i += 1;
            }
        }
        while lo_load(&items) > target {
            let victim = items
                .iter()
                .enumerate()
                .filter(|(_, it)| it.drop_rank() < 4)
                .min_by_key(|(_, it)| (it.drop_rank(), Reverse(it.day())))
                .map(|(i, _)| i);
            let Some(v) = victim else { break };
            items.remove(v);
        }

        // Dauern.
        let strength_total = strength_load(&items);
        let target_end = (target - strength_total).max(0.0);
        let mut endurance: Vec<&Draft> = Vec::new();
        for it in &items {
            if let Item::Endurance(d) = it {
                endurance.push(d);
            }
        }
        let weight_of = |d: &Draft| {
            if d.role == Role::Long {
                f64::from(rules.structure.long_weight)
            } else {
                1.0
            }
        };
        let denom: f64 = endurance
            .iter()
            .map(|d| weight_of(d) * rate_of(provisional_class(d)))
            .sum();
        let x = if denom > 0.0 { target_end / denom } else { 0.0 };

        struct Chosen<'c> {
            day: usize,
            role: Role,
            sport: PlanSport,
            template: &'c Template,
            lo: u32,
            hi: u32,
            rate: f64,
            minutes: u32,
        }
        let mut chosen: Vec<Chosen> = Vec::new();
        for d in &endurance {
            let budget = u32::from(minutes_of[d.day]) - strength_minutes_on(&items, d.day);
            let Some(tpl) = choose_template(
                &d.templates,
                weight_of(d) * x,
                budget,
                d.role,
                d.sport,
                min_s,
                &mut rot,
            ) else {
                continue;
            };
            chosen.push(Chosen {
                day: d.day,
                role: d.role,
                sport: d.sport,
                template: tpl,
                lo: eff_min(tpl, min_s),
                hi: eff_max(tpl, budget),
                rate: rate_of(tpl.intensity_class),
                minutes: 0,
            });
        }
        let denom2: f64 = chosen
            .iter()
            .map(|c| {
                let w = if c.role == Role::Long {
                    f64::from(rules.structure.long_weight)
                } else {
                    1.0
                };
                w * c.rate
            })
            .sum();
        let x2 = if denom2 > 0.0 {
            target_end / denom2
        } else {
            0.0
        };
        let round_step = |m: f64| -> u32 {
            let s = f64::from(step.max(1));
            ((m / s).round() * s).max(0.0) as u32
        };
        for c in &mut chosen {
            let w = if c.role == Role::Long {
                f64::from(rules.structure.long_weight)
            } else {
                1.0
            };
            c.minutes = round_step(w * x2).clamp(c.lo, c.hi.max(c.lo));
        }
        // Feinabstimmung in Schritten, bis das Wochenziel am nächsten liegt.
        let current = |chosen: &[Chosen]| -> f64 {
            chosen.iter().map(|c| c.rate * f64::from(c.minutes)).sum()
        };
        for _ in 0..200 {
            let mut changed = false;
            for i in 0..chosen.len() {
                let diff = target_end - current(&chosen);
                let step_load = chosen[i].rate * f64::from(step);
                if diff >= 0.5 * step_load && chosen[i].minutes + step <= chosen[i].hi {
                    chosen[i].minutes += step;
                    changed = true;
                } else if diff <= -0.5 * step_load && chosen[i].minutes >= chosen[i].lo + step {
                    chosen[i].minutes -= step;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let capped = !chosen.is_empty()
            && chosen.iter().all(|c| c.minutes >= c.hi)
            && target_end > current(&chosen) + 1e-6;

        // Einheiten je Tag zusammensetzen.
        let mut per_day: Vec<Vec<PlanSession>> = (0..7).map(|_| Vec::new()).collect();
        // Nur Tage, an denen die harte Einheit auch bleibt. Ist sie wegen des Wochenziels
        // entfallen, ist die leichte Einheit am Folgetag keine Erholung nach einer harten.
        let hard_set: Vec<usize> = chosen
            .iter()
            .filter(|c| c.template.intensity_class == IntensityClass::Hard)
            .map(|c| c.day)
            .collect();
        for c in &chosen {
            let mut reason = match c.role {
                Role::Long => vec![keys::SESSION_LONG.to_string()],
                Role::Hard if spec.event_idx.is_some() => {
                    vec![keys::SESSION_RACE_OPENER.to_string()]
                }
                Role::Hard => vec![keys::SESSION_HARD.to_string()],
                Role::Moderate => vec![keys::SESSION_MODERATE.to_string()],
                Role::Easy => vec![keys::SESSION_EASY.to_string()],
            };
            if c.role == Role::Easy && c.day > 0 && hard_set.contains(&(c.day - 1)) {
                reason = vec![keys::SESSION_RECOVERY_AFTER_HARD.to_string()];
            }
            per_day[c.day].push(PlanSession {
                slot: Slot::Am,
                sport: c.sport,
                template_id: c.template.id.clone(),
                duration_min: c.minutes,
                planned_load: rules.load_model.session_load(c.template, c.minutes),
                hard: c.template.intensity_class == IntensityClass::Hard,
                rpe: None,
                reason_keys: reason,
            });
        }
        for it in &items {
            if let Item::Strength {
                day,
                template,
                minutes,
            } = it
            {
                let slot = if per_day[*day].is_empty() {
                    Slot::Am
                } else {
                    Slot::Pm
                };
                per_day[*day].push(PlanSession {
                    slot,
                    sport: PlanSport::Strength,
                    template_id: template.id.clone(),
                    duration_min: *minutes,
                    planned_load: rules.load_model.session_load(template, *minutes),
                    hard: false,
                    rpe: template.rpe,
                    reason_keys: vec![keys::SESSION_STRENGTH.to_string()],
                });
            }
        }
        let days: Vec<PlanDay> = per_day
            .into_iter()
            .enumerate()
            .map(|(i, sessions)| PlanDay {
                date: spec.week_start + TimeDelta::days(i as i64),
                sessions,
            })
            .collect();
        let planned: f64 = days
            .iter()
            .flat_map(|d| d.sessions.iter())
            .map(|s| f64::from(s.planned_load))
            .sum();
        let target_load = if capped { planned } else { target };
        Built {
            week: PlanWeek {
                week_start: spec.week_start,
                phase: spec.phase,
                target_load: round1(target_load),
                planned_load: Some(round1(planned)),
                reason_keys: Vec::new(),
                days,
            },
            capped,
            rotation: rot,
            training_days,
        }
    }
}

/// Erzeugt einen Wochenplan (SPEC 8.3). Gleiche Eingaben ergeben gleiches JSON.
///
/// Der Plan trägt `source = rules`. Weiche Verletzungen stehen als Warnungen im Plan.
/// Kann eine Woche die harten Regeln nicht erfüllen, wird sie zur Ruhewoche mit einer Warnung
/// (Code der verletzten Regel), nie zu einer harten Verletzung im Plan.
pub fn generate_plan(ctx: &PlanContext, rules: &Rules) -> Result<Plan, PlanError> {
    rules.validate()?;
    ctx.validate(rules)?;
    let schedule = build_schedule(ctx, rules);

    let prev = ctx.previous_week().filter(|p| p.load > 0.0);
    let start = prev.map_or(7.0 * f64::from(ctx.ctl0), |p| p.load);
    let mut generator = Generator {
        ctx,
        rules,
        goal: Goal::from(&ctx.goal),
        weeks: Vec::with_capacity(schedule.len()),
        carry: Carry::default(),
        rotation: Rotation::default(),
        last_load: start.max(f64::from(rules.structure.min_week_load)),
        notes: Vec::new(),
    };
    for spec in &schedule {
        generator.add_week(spec);
    }

    // R12: Erholungsdaten ersetzen die erste harte Einheit durch eine lockere. Erst nach dem
    // Aufbau aller Wochen, weil die Änderung die Last nur senkt und keine Wochenstruktur ändert.
    let mut weeks = generator.weeks;
    let mut notes = generator.notes;
    notes.extend(apply_swap(&mut weeks, ctx, rules));
    let mut plan = Plan {
        schema_version: PLAN_SCHEMA_VERSION,
        plan_id: ctx.plan_id.clone(),
        created_at: ctx.created_at,
        locale: ctx.locale,
        source: PlanSource::Rules,
        rules_version: rules.version.clone(),
        goal: generator.goal.clone(),
        weeks,
        warnings: Vec::new(),
        explanation: None,
    };
    let violations = validate_plan(&plan, ctx, rules);
    let mut warnings = violations_to_warnings(&violations);
    warnings.extend(notes);
    warnings.sort();
    warnings.dedup();
    plan.warnings = warnings;
    Ok(plan)
}
