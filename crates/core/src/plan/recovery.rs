//! R12: Erholungsdaten (SPEC 8.4, weich, ab M4d).
//!
//! Eingang ist eine Liste von Erholungstagen im `PlanContext`. Der jüngste Erholungstag vor
//! dem Planbeginn wird mit seiner Basislinie verglichen, dem Mittel der `baseline_days` Tage
//! davor (gleitendes Mittel, Q6; die Empfehlung von sieben Tagen stammt nur aus einem
//! Sekundärbericht, O8). Liegt die HRV um mindestens `hrv_drop_fraction` oder die Schlafdauer um
//! mindestens `sleep_deficit_fraction` unter der Basislinie, ersetzt der Planer die erste harte
//! Einheit des Plans durch eine lockere derselben Sportart. Alle Schwellen sind Faustregeln und
//! stehen in `catalog/rules.default.json`.
//!
//! Die Regel greift nur bei vorhandenen Daten: Ohne Erholungstage, mit veralteten Tagen (älter
//! als `max_age_days`) oder mit zu kurzer Basislinie (weniger als `min_baseline_days` Tage mit
//! Wert) bleibt der Plan unverändert. Ruhepuls und Schlafwert fließen nicht ein, weil die SPEC
//! sie nicht als Auslöser nennt.
//!
//! Die Grenzen gelten "ab": genau 15 Prozent unter der Basislinie lösen aus, wie bei R2.

use std::collections::BTreeMap;

use chrono::{NaiveDate, TimeDelta};
use serde::{Deserialize, Serialize};

use super::catalog::{IntensityClass, Template, TemplateCatalog};
use super::context::PlanContext;
use super::generate::keys;
use super::rules::{R12, Rules, round1};
use super::types::{PlanSession, PlanSport, PlanWarning, PlanWeek, Severity};

/// Code der Regel in Warnungen und Verletzungen.
pub const RULE_CODE: &str = "R12";

/// Ein Tag mit Erholungswerten (SPEC 6.2 `recovery_day`, 7.3). Fehlende Werte sind `None`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RecoveryDay {
    pub date: NaiveDate,
    #[serde(default)]
    pub hrv_overnight_ms: Option<f32>,
    #[serde(default)]
    pub resting_hr_bpm: Option<f32>,
    #[serde(default)]
    pub sleep_duration_s: Option<u32>,
    #[serde(default)]
    pub sleep_score: Option<u8>,
}

/// Auslöser von R12: der Tag und um welchen Anteil die Werte unter der Basislinie liegen
/// (positiv heißt darunter, `None` heißt nicht berechenbar).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecoverySignal {
    /// Jüngster Erholungstag vor dem Planbeginn.
    pub date: NaiveDate,
    pub hrv_drop: Option<f64>,
    pub sleep_deficit: Option<f64>,
    pub hrv_low: bool,
    pub sleep_short: bool,
}

/// Anteil, um den der Wert des Tages `latest` unter dem Mittel der Tage davor liegt
/// (positiv heißt darunter, negativ darüber). Gemeinsame Basislogik von R12 und der
/// Erholungsabweichung im Dashboard (`metrics::readiness`): Das Mittel läuft über die
/// `baseline_days` Tage vor `latest` (ohne `latest`) und braucht mindestens
/// `min_baseline_days` Tage mit Wert.
fn relative_drop(
    by_date: &BTreeMap<NaiveDate, &RecoveryDay>,
    latest: NaiveDate,
    r: &R12,
    value: impl Fn(&RecoveryDay) -> Option<f64>,
) -> Option<f64> {
    let current = value(by_date.get(&latest)?)?;
    let mean = baseline_mean(by_date, latest, r, value)?;
    Some((mean - current) / mean)
}

/// Basislinie für den Tag `day`: Mittel der Werte der `baseline_days` Tage davor (ohne `day`),
/// mindestens `min_baseline_days` Tage mit Wert, sonst `None`. Einzige Stelle dieser Rechnung,
/// R12, das Dashboard und die Seite "Erholung" (T34) rufen sie über diese Funktionen.
fn baseline_mean(
    by_date: &BTreeMap<NaiveDate, &RecoveryDay>,
    day: NaiveDate,
    r: &R12,
    value: impl Fn(&RecoveryDay) -> Option<f64>,
) -> Option<f64> {
    let from = day - TimeDelta::days(i64::from(r.baseline_days));
    let base: Vec<f64> = by_date
        .range(from..day)
        .filter_map(|(_, d)| value(d))
        .collect();
    if base.len() < r.min_baseline_days as usize {
        return None;
    }
    let mean = base.iter().sum::<f64>() / base.len() as f64;
    (mean > 0.0).then_some(mean)
}

/// Basislinien eines Tages für HRV, Ruhepuls und Schlafdauer (T34), wie R12 sie bildet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Baselines {
    pub hrv: Option<f64>,
    pub resting_hr: Option<f64>,
    pub sleep: Option<f64>,
}

/// Basislinien für jeden Tag von `from` bis `to` (einschließlich). Der Tag selbst braucht keinen
/// Wert; mehrere Einträge zu einem Datum: der spätere in der Liste gilt (wie bei R12).
pub(crate) fn baselines(
    days: &[RecoveryDay],
    from: NaiveDate,
    to: NaiveDate,
    r: &R12,
) -> Vec<(NaiveDate, Baselines)> {
    let by_date: BTreeMap<NaiveDate, &RecoveryDay> = days.iter().map(|d| (d.date, d)).collect();
    from.iter_days()
        .take_while(|d| *d <= to)
        .map(|d| {
            (
                d,
                Baselines {
                    hrv: baseline_mean(&by_date, d, r, hrv_value),
                    resting_hr: baseline_mean(&by_date, d, r, resting_hr_value),
                    sleep: baseline_mean(&by_date, d, r, sleep_value),
                },
            )
        })
        .collect()
}

/// HRV in ms, nur endlich und positiv.
fn hrv_value(d: &RecoveryDay) -> Option<f64> {
    d.hrv_overnight_ms
        .map(f64::from)
        .filter(|v| v.is_finite() && *v > 0.0)
}

/// Schlafdauer in s. Eine Schlafdauer von 0 gilt als fehlender Wert, nicht als durchwachte Nacht.
fn sleep_value(d: &RecoveryDay) -> Option<f64> {
    d.sleep_duration_s.filter(|s| *s > 0).map(f64::from)
}

/// Ruhepuls in bpm, nur endlich und positiv.
fn resting_hr_value(d: &RecoveryDay) -> Option<f64> {
    d.resting_hr_bpm
        .map(f64::from)
        .filter(|v| v.is_finite() && *v > 0.0)
}

/// Abstand der Werte des jüngsten Erholungstags zur eigenen Basislinie, jeweils als Anteil
/// "darunter" (positiv) oder "darüber" (negativ). `None` heißt nicht berechenbar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RecoveryDrops {
    /// Jüngster Erholungstag.
    pub date: NaiveDate,
    pub hrv: Option<f64>,
    pub resting_hr: Option<f64>,
    pub sleep: Option<f64>,
}

/// Jüngster Erholungstag mit Datum vor `before` (ausschließlich), nicht älter als
/// `r.max_age_days` Tage gemessen von `age_ref`, mit seinen Abständen zur Basislinie.
/// R12 ruft das mit dem Planbeginn für beide Argumente, das Dashboard mit dem Tag nach
/// "heute" für `before` und mit "heute" für `age_ref`, damit der heutige Tag mitzählt.
pub(crate) fn recovery_drops(
    days: &[RecoveryDay],
    before: NaiveDate,
    age_ref: NaiveDate,
    r: &R12,
) -> Option<RecoveryDrops> {
    // Mehrere Einträge zu einem Datum: der spätere in der Liste gilt.
    let by_date: BTreeMap<NaiveDate, &RecoveryDay> = days
        .iter()
        .filter(|d| d.date < before)
        .map(|d| (d.date, d))
        .collect();
    let latest = *by_date.keys().next_back()?;
    if (age_ref - latest).num_days() > i64::from(r.max_age_days) {
        return None;
    }
    Some(RecoveryDrops {
        date: latest,
        hrv: relative_drop(&by_date, latest, r, hrv_value),
        resting_hr: relative_drop(&by_date, latest, r, resting_hr_value),
        sleep: relative_drop(&by_date, latest, r, sleep_value),
    })
}

/// Parameter aus `f32` ohne Darstellungsrest (0,15 als `f32` ist 0,15000000596), damit Grenzen
/// wie "ab 15 Prozent" genau greifen.
pub(crate) fn param(x: f32) -> f64 {
    (f64::from(x) * 1e6).round() / 1e6
}

/// Wertet die Erholungstage vor `week_start` aus. `Some` heißt: HRV oder Schlafdauer des
/// jüngsten Tages liegt deutlich unter der Basislinie.
pub fn recovery_signal(
    days: &[RecoveryDay],
    week_start: NaiveDate,
    r: &R12,
) -> Option<RecoverySignal> {
    let drops = recovery_drops(days, week_start, week_start, r)?;
    let hrv_low = drops
        .hrv
        .is_some_and(|x| x >= param(r.hrv_drop_fraction) - 1e-9);
    let sleep_short = drops
        .sleep
        .is_some_and(|x| x >= param(r.sleep_deficit_fraction) - 1e-9);
    (hrv_low || sleep_short).then_some(RecoverySignal {
        date: drops.date,
        hrv_drop: drops.hrv,
        sleep_deficit: drops.sleep,
        hrv_low,
        sleep_short,
    })
}

/// Erste harte Einheit in Planreihenfolge als (Woche, Tag, Einheit).
pub(crate) fn first_hard_session(weeks: &[PlanWeek]) -> Option<(usize, usize, usize)> {
    for (wi, w) in weeks.iter().enumerate() {
        for (di, d) in w.days.iter().enumerate() {
            if let Some(si) = d.sessions.iter().position(|s| s.hard) {
                return Some((wi, di, si));
            }
        }
    }
    None
}

/// Erste Einheit in Planreihenfolge, die hart ist oder von R12 schon durch eine lockere ersetzt
/// wurde (Begründungsschlüssel `reason.session.recovery_swap`). Das Ergebnis sagt, ob sie
/// ersetzt wurde. So erkennt die Prüfung auch nach dem Austausch, dass die erste harte Einheit
/// erledigt ist und die nächste harte Einheit nicht mehr gemeint ist.
pub(crate) fn first_hard_or_swapped(weeks: &[PlanWeek]) -> Option<(usize, usize, bool)> {
    for (wi, w) in weeks.iter().enumerate() {
        for (di, d) in w.days.iter().enumerate() {
            for s in &d.sessions {
                let swapped = s
                    .reason_keys
                    .iter()
                    .any(|k| k == keys::SESSION_RECOVERY_SWAP);
                if s.hard || swapped {
                    return Some((wi, di, swapped && !s.hard));
                }
            }
        }
    }
    None
}

/// Vorlage einer Sportart und Klasse, deren Dauerfenster `minutes` enthält. Unter mehreren
/// gewinnt das engste Fenster (kleinste Obergrenze), bei Gleichstand die frühere im Katalog
/// (wie bei der Wahl lockerer Einheiten im Planer, Konvention).
fn narrowest_containing(
    catalog: &TemplateCatalog,
    sport: PlanSport,
    class: IntensityClass,
    minutes: u32,
) -> Option<&Template> {
    let mut best: Option<&Template> = None;
    for t in catalog.select(sport, class) {
        let w = t.duration_min;
        let contains = w.min <= minutes && minutes <= w.max;
        if contains && best.is_none_or(|b| t.duration_min.max < b.duration_min.max) {
            best = Some(t);
        }
    }
    best
}

/// Lockere Vorlage derselben Sportart, deren Dauerfenster die Dauer der Einheit enthält. Die
/// Dauer bleibt dann gleich, damit Verfügbarkeit (R11) und Wochenvolumen (R2) unberührt
/// bleiben. Unter mehreren Vorlagen gewinnt das engste Fenster, bei Gleichstand die frühere
/// im Katalog (wie bei der Wahl lockerer Einheiten im Planer). Auch die Tagesentscheidung
/// (`plan::today`, SPEC 8.7) wählt so die leichtere Variante.
pub(crate) fn easy_replacement<'c>(
    catalog: &'c TemplateCatalog,
    session: &PlanSession,
) -> Option<&'c Template> {
    narrowest_containing(
        catalog,
        session.sport,
        IntensityClass::Easy,
        session.duration_min,
    )
}

/// Gegenstück zu [`easy_replacement`] für die härtere Variante der Tagesentscheidung (SPEC
/// 8.7): harte Vorlage derselben Sportart mit derselben Dauer, gleiche Wahlregel. Ob die harte
/// Einheit an diesem Tag zulässig ist, prüft der Aufrufer mit `validate_plan`.
pub(crate) fn hard_replacement<'c>(
    catalog: &'c TemplateCatalog,
    session: &PlanSession,
) -> Option<&'c Template> {
    narrowest_containing(
        catalog,
        session.sport,
        IntensityClass::Hard,
        session.duration_min,
    )
}

/// Ersetzt die erste harte Einheit durch eine lockere, wenn die Erholungsdaten es verlangen.
/// Gibt die Information für die Warnungen des Plans zurück (Code `R12`, Stufe `info`, Datum der
/// ersetzten Einheit). Ohne Signal, ohne harte Einheit oder ohne passende lockere Vorlage
/// bleibt der Plan unverändert (dann meldet `validate_plan` die Verletzung).
pub(crate) fn apply_swap(
    weeks: &mut [PlanWeek],
    ctx: &PlanContext,
    rules: &Rules,
) -> Option<PlanWarning> {
    if ctx.recovery.is_empty() {
        return None;
    }
    recovery_signal(&ctx.recovery, ctx.week_start, &rules.r12)?;
    let (wi, di, si) = first_hard_session(weeks)?;
    if first_hard_or_swapped(weeks)?.2 {
        return None;
    }
    let template = easy_replacement(&ctx.catalog, &weeks[wi].days[di].sessions[si])?;
    let date = weeks[wi].days[di].date;
    let session = &mut weeks[wi].days[di].sessions[si];
    session.template_id = template.id.clone();
    session.hard = false;
    session.planned_load = rules
        .load_model
        .session_load(template, session.duration_min);
    session.reason_keys = vec![keys::SESSION_RECOVERY_SWAP.to_string()];
    let week = &mut weeks[wi];
    if week.planned_load.is_some() {
        let total: f64 = week
            .days
            .iter()
            .flat_map(|d| d.sessions.iter())
            .map(|s| f64::from(s.planned_load))
            .sum();
        week.planned_load = Some(round1(total));
    }
    Some(PlanWarning {
        code: RULE_CODE.to_string(),
        severity: Severity::Info,
        date: Some(date),
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::{ctx, date, monday, open_ctx, rules};
    use super::super::*;
    use super::*;
    use proptest::prelude::*;

    fn day(offset: i64, hrv: Option<f32>, sleep_s: Option<u32>) -> RecoveryDay {
        RecoveryDay {
            date: monday() + TimeDelta::days(offset),
            hrv_overnight_ms: hrv,
            resting_hr_bpm: Some(50.0),
            sleep_duration_s: sleep_s,
            sleep_score: Some(80),
        }
    }

    /// Sieben Tage vor dem Montag (Offsets -8 bis -2) als Basislinie und der Sonntag (-1) als
    /// jüngster Tag.
    ///
    /// Basislinie HRV: (60 + 62 + 58 + 61 + 59 + 60 + 60) / 7 = 420 / 7 = 60 ms.
    /// Basislinie Schlaf: 7 mal 25200 s (7 h) / 7 = 25200 s.
    fn series(latest_hrv: f32, latest_sleep: u32) -> Vec<RecoveryDay> {
        let hrv = [60.0, 62.0, 58.0, 61.0, 59.0, 60.0, 60.0];
        let mut days: Vec<RecoveryDay> = hrv
            .iter()
            .enumerate()
            .map(|(i, v)| day(-8 + i as i64, Some(*v), Some(25200)))
            .collect();
        days.push(day(-1, Some(latest_hrv), Some(latest_sleep)));
        days
    }

    fn r12() -> R12 {
        rules().r12
    }

    #[test]
    fn hrv_exactly_15_percent_below_the_baseline_triggers() {
        // 51 ms: (60 - 51) / 60 = 9 / 60 = 0,15. Die Grenze gilt "ab".
        let s = recovery_signal(&series(51.0, 25200), monday(), &r12()).expect("Signal");
        assert_eq!(s.date, monday() - TimeDelta::days(1));
        assert!((s.hrv_drop.unwrap() - 0.15).abs() < 1e-12);
        assert!(s.hrv_low && !s.sleep_short);
        assert_eq!(s.sleep_deficit, Some(0.0));
    }

    #[test]
    fn hrv_just_above_the_threshold_does_not_trigger() {
        // 52 ms: 8 / 60 = 0,1333 liegt unter 0,15.
        assert!(recovery_signal(&series(52.0, 25200), monday(), &r12()).is_none());
    }

    #[test]
    fn sleep_exactly_20_percent_below_the_baseline_triggers() {
        // 20160 s: (25200 - 20160) / 25200 = 5040 / 25200 = 0,2.
        let s = recovery_signal(&series(60.0, 20160), monday(), &r12()).expect("Signal");
        assert!(s.sleep_short && !s.hrv_low);
        assert!((s.sleep_deficit.unwrap() - 0.2).abs() < 1e-12);
        // 20400 s: 4800 / 25200 = 0,1905 liegt darunter.
        assert!(recovery_signal(&series(60.0, 20400), monday(), &r12()).is_none());
    }

    #[test]
    fn a_higher_hrv_or_longer_sleep_never_triggers() {
        assert!(recovery_signal(&series(75.0, 30000), monday(), &r12()).is_none());
    }

    #[test]
    fn baseline_needs_enough_days_with_values() {
        // Drei Tage Basislinie (min_baseline_days ist 4) und ein auffälliger Tag.
        let few = vec![
            day(-4, Some(60.0), Some(25200)),
            day(-3, Some(60.0), Some(25200)),
            day(-2, Some(60.0), Some(25200)),
            day(-1, Some(40.0), Some(10000)),
        ];
        assert!(recovery_signal(&few, monday(), &r12()).is_none());
        // Mit vier Tagen Basislinie greift sie.
        let mut four = few;
        four.insert(0, day(-5, Some(60.0), Some(25200)));
        let s = recovery_signal(&four, monday(), &r12()).expect("Signal");
        assert!(s.hrv_low && s.sleep_short);
    }

    #[test]
    fn only_days_inside_the_baseline_window_count() {
        // Basislinie: Tage -8 bis -2 (sieben Tage vor dem jüngsten Tag -1). Ein Ausreißer
        // bei -9 liegt außerhalb und ändert nichts.
        let mut days = series(51.0, 25200);
        days.push(day(-9, Some(500.0), Some(80000)));
        let s = recovery_signal(&days, monday(), &r12()).expect("Signal");
        assert!((s.hrv_drop.unwrap() - 0.15).abs() < 1e-12);
    }

    #[test]
    fn stale_days_and_days_from_the_plan_period_are_ignored() {
        // Der jüngste Tag liegt 5 Tage vor dem Montag, erlaubt sind 3.
        let stale: Vec<RecoveryDay> = series(40.0, 10000)
            .into_iter()
            .map(|mut d| {
                d.date -= TimeDelta::days(4);
                d
            })
            .collect();
        assert!(recovery_signal(&stale, monday(), &r12()).is_none());
        // Ein Tag am oder nach dem Montag gehört nicht zum Signal.
        let mut days = series(60.0, 25200);
        days.push(day(0, Some(10.0), Some(1000)));
        days.push(day(2, Some(10.0), Some(1000)));
        assert!(recovery_signal(&days, monday(), &r12()).is_none());
        assert!(recovery_signal(&[], monday(), &r12()).is_none());
    }

    #[test]
    fn missing_values_of_the_latest_day_are_not_a_signal() {
        let mut days = series(60.0, 25200);
        let last = days.last_mut().unwrap();
        last.hrv_overnight_ms = None;
        last.sleep_duration_s = None;
        assert!(recovery_signal(&days, monday(), &r12()).is_none());
        // Schlafdauer 0 gilt als fehlend.
        let mut days = series(60.0, 25200);
        days.last_mut().unwrap().sleep_duration_s = Some(0);
        assert!(recovery_signal(&days, monday(), &r12()).is_none());
    }

    #[test]
    fn unsorted_input_and_duplicates_give_the_same_signal() {
        let mut shuffled = series(51.0, 25200);
        shuffled.reverse();
        let a = recovery_signal(&series(51.0, 25200), monday(), &r12());
        assert_eq!(a, recovery_signal(&shuffled, monday(), &r12()));
        // Der spätere Eintrag zum selben Datum gilt.
        let mut dup = series(70.0, 25200);
        dup.push(day(-1, Some(51.0), Some(25200)));
        assert_eq!(a, recovery_signal(&dup, monday(), &r12()));
    }

    fn first_hard(plan: &Plan) -> Option<(NaiveDate, String)> {
        let (wi, di, si) = first_hard_session(&plan.weeks)?;
        let d = &plan.weeks[wi].days[di];
        Some((d.date, d.sessions[si].template_id.clone()))
    }

    fn with_recovery(mut c: PlanContext, recovery: Vec<RecoveryDay>) -> PlanContext {
        c.recovery = recovery;
        c
    }

    #[test]
    fn empty_recovery_changes_nothing() {
        let r = rules();
        let base = generate_plan(&ctx(), &r).unwrap();
        let again = generate_plan(&with_recovery(ctx(), Vec::new()), &r).unwrap();
        assert_eq!(
            serde_json::to_string(&base).unwrap(),
            serde_json::to_string(&again).unwrap()
        );
        assert!(base.warnings.iter().all(|w| w.code != "R12"));
        assert!(!has_r12(&base, &ctx()));
    }

    fn has_r12(plan: &Plan, c: &PlanContext) -> bool {
        validate_plan(plan, c, &rules())
            .iter()
            .any(|v| v.code == "R12")
    }

    #[test]
    fn healthy_recovery_data_changes_nothing() {
        let r = rules();
        let base = generate_plan(&ctx(), &r).unwrap();
        let healthy = generate_plan(&with_recovery(ctx(), series(62.0, 26000)), &r).unwrap();
        assert_eq!(
            serde_json::to_string(&base).unwrap(),
            serde_json::to_string(&healthy).unwrap()
        );
    }

    #[test]
    fn a_signal_swaps_only_the_first_hard_session() {
        let r = rules();
        let base = generate_plan(&ctx(), &r).unwrap();
        let c = with_recovery(ctx(), series(51.0, 25200));
        let plan = generate_plan(&c, &r).unwrap();

        let (date0, template0) = first_hard(&base).expect("der Basisplan hat harte Einheiten");
        // Die Einheit an diesem Tag ist jetzt locker, mit Begründung und Warnung.
        let (wi, di, si) = first_hard_session(&base.weeks).unwrap();
        let swapped = &plan.weeks[wi].days[di].sessions[si];
        let original = &base.weeks[wi].days[di].sessions[si];
        assert_eq!(plan.weeks[wi].days[di].date, date0);
        assert!(!swapped.hard);
        assert_eq!(swapped.sport, original.sport);
        assert_eq!(swapped.duration_min, original.duration_min);
        assert_ne!(swapped.template_id, template0);
        assert_eq!(swapped.reason_keys, vec![keys::SESSION_RECOVERY_SWAP]);
        assert!(swapped.planned_load < original.planned_load);
        let easy = c.catalog.get(&swapped.template_id).unwrap();
        assert_eq!(easy.intensity_class, IntensityClass::Easy);
        assert!(
            plan.warnings
                .iter()
                .any(|w| w.code == "R12" && w.severity == Severity::Info && w.date == Some(date0))
        );

        // Alles andere bleibt: dieselben Einheiten, nur die erste harte fehlt als hart.
        let hard_count = |p: &Plan| {
            p.weeks
                .iter()
                .flat_map(|w| &w.days)
                .flat_map(|d| &d.sessions)
                .filter(|s| s.hard)
                .count()
        };
        assert_eq!(hard_count(&plan), hard_count(&base) - 1);
        let (date1, _) = first_hard(&plan).expect("weitere harte Einheiten bleiben");
        assert!(date1 > date0);
        for (wb, wp) in base.weeks.iter().zip(&plan.weeks) {
            for (db, dp) in wb.days.iter().zip(&wp.days) {
                for (sb, sp) in db.sessions.iter().zip(&dp.sessions) {
                    if db.date != date0 || sb.template_id != template0 {
                        assert_eq!(sb, sp, "{}", db.date);
                    }
                }
            }
        }
        // Die Wochenlast der betroffenen Woche folgt der Einheit.
        let sum: f32 = plan.weeks[wi].sessions_load();
        assert!((plan.weeks[wi].planned_load.unwrap() - sum).abs() < 0.11);
        // Das Ergebnis ist annehmbar, R12 meldet nichts mehr.
        let violations = validate_plan(&plan, &c, &r);
        assert!(is_acceptable(&violations), "{violations:?}");
        assert!(violations.iter().all(|v| v.code != "R12"));
    }

    #[test]
    fn sleep_alone_also_swaps_and_the_result_is_deterministic() {
        let r = rules();
        let c = with_recovery(ctx(), series(60.0, 20160));
        let a = generate_plan(&c, &r).unwrap();
        let b = generate_plan(&c, &r).unwrap();
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
        assert!(a.warnings.iter().any(|w| w.code == "R12"));
    }

    #[test]
    fn validate_reports_r12_softly_when_the_first_hard_session_stayed_hard() {
        let r = rules();
        // Plan ohne Erholungsdaten erzeugt, danach mit Signal geprüft.
        let plan = generate_plan(&ctx(), &r).unwrap();
        let c = with_recovery(ctx(), series(51.0, 25200));
        let violations = validate_plan(&plan, &c, &r);
        let r12: Vec<&Violation> = violations.iter().filter(|v| v.code == "R12").collect();
        assert_eq!(r12.len(), 1, "{violations:?}");
        let (first_date, _) = first_hard(&plan).unwrap();
        assert_eq!(r12[0].date, Some(first_date));
        assert_eq!(r12[0].hardness, Hardness::Soft);
        assert_eq!(r12[0].message_key, "rule.R12.explanation");
        assert_eq!(r12[0].detail["recovery_date"], "2027-01-03");
        assert_eq!(r12[0].detail["hrv_drop"], "0.150");
        // Weich: der Plan bleibt annehmbar.
        assert!(is_acceptable(&violations));
        let warnings = violations_to_warnings(&violations);
        assert!(
            warnings
                .iter()
                .any(|w| w.code == "R12" && w.severity == Severity::Warn)
        );
        // Ohne Signal keine Meldung.
        assert!(!has_r12(&plan, &ctx()));
        assert!(!has_r12(&plan, &with_recovery(ctx(), series(62.0, 26000))));
    }

    #[test]
    fn a_plan_without_hard_sessions_is_left_alone() {
        let r = rules();
        let mut c = ctx();
        c.availability = std::array::from_fn(|_| DayAvailability {
            max_minutes: 90,
            allow_hard: false,
            fixed_rest: false,
        });
        c.availability[4].fixed_rest = true;
        let base = generate_plan(&c, &r).unwrap();
        assert!(first_hard_session(&base.weeks).is_none());
        let plan = generate_plan(&with_recovery(c.clone(), series(40.0, 10000)), &r).unwrap();
        assert_eq!(
            serde_json::to_string(&base).unwrap(),
            serde_json::to_string(&plan).unwrap()
        );
    }

    #[test]
    fn swap_works_in_event_plans_and_keeps_the_rest_valid() {
        let r = rules();
        let mut c = open_ctx();
        c.goal = GoalSpec::Event {
            event_date: date(2027, 2, 14),
            sport: PlanSport::Cycling,
            priority: Priority::A,
        };
        c.horizon_weeks = 8;
        let base = generate_plan(&c, &r).unwrap();
        let plan = generate_plan(&with_recovery(c.clone(), series(45.0, 15000)), &r).unwrap();
        assert!(is_acceptable(&validate_plan(
            &plan,
            &with_recovery(c, series(45.0, 15000)),
            &r
        )));
        assert_ne!(first_hard(&base), first_hard(&plan));
    }

    #[test]
    fn rules_reject_unusable_r12_parameters() {
        for edit in [
            (|r: &mut Rules| r.r12.hrv_drop_fraction = 0.0) as fn(&mut Rules),
            |r| r.r12.hrv_drop_fraction = 1.5,
            |r| r.r12.sleep_deficit_fraction = f32::NAN,
            |r| r.r12.baseline_days = 0,
            |r| r.r12.min_baseline_days = 0,
            |r| r.r12.min_baseline_days = r.r12.baseline_days + 1,
            |r| r.r12.max_age_days = 0,
            |r| r.r12.meta.source = "Erfindung".to_string(),
        ] {
            let mut r = rules();
            edit(&mut r);
            assert_eq!(r.validate().unwrap_err().code(), "invalid_rules");
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        /// Mit beliebigen Erholungsdaten bleibt der Plan annehmbar, und er unterscheidet sich vom
        /// Plan ohne Daten höchstens in der ersten harten Einheit.
        #[test]
        fn recovery_data_never_breaks_the_hard_rules(
            hrv in prop::collection::vec(prop::option::of(10.0f32..120.0), 1..12),
            sleep in prop::collection::vec(prop::option::of(10_000u32..40_000), 1..12),
            gap in 1i64..6,
        ) {
            let r = rules();
            let n = hrv.len().min(sleep.len());
            let recovery: Vec<RecoveryDay> = (0..n)
                .map(|i| day(-(gap + (n - 1 - i) as i64), hrv[i], sleep[i]))
                .collect();
            let c = with_recovery(ctx(), recovery);
            let plan = generate_plan(&c, &r).unwrap();
            let violations = validate_plan(&plan, &c, &r);
            prop_assert!(is_acceptable(&violations), "{violations:?}");
            prop_assert!(violations.iter().all(|v| v.code != "R12"));
            let base = generate_plan(&ctx(), &r).unwrap();
            let mut differing = 0;
            for (wb, wp) in base.weeks.iter().zip(&plan.weeks) {
                for (db, dp) in wb.days.iter().zip(&wp.days) {
                    prop_assert_eq!(db.sessions.len(), dp.sessions.len());
                    for (sb, sp) in db.sessions.iter().zip(&dp.sessions) {
                        if sb != sp {
                            differing += 1;
                            prop_assert!(sb.hard && !sp.hard);
                            prop_assert_eq!(sb.duration_min, sp.duration_min);
                        }
                    }
                }
            }
            prop_assert!(differing <= 1);
            let signal = recovery_signal(&c.recovery, c.week_start, &r.r12);
            if signal.is_none() {
                prop_assert_eq!(differing, 0);
            }
        }
    }
}
