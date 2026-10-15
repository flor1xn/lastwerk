//! Intensitätsverteilung: Anteil der Zeit in Zone 1 und 2 an der Zeit mit Zonen
//! (SPEC 4.2 F36, 8.4 R8).
//!
//! Die Kennzahl beschreibt, wie das Training verteilt war. Sie ist keine Forderung: Q5 (Seiler)
//! berichtet beschreibend etwa 80 Prozent der Einheiten im lockeren Bereich bei
//! Ausdauerathleten, der Grenzwert 70 Prozent ist die Faustregel aus R8 und kommt aus den
//! Regeln (`R08.min_easy_share`), nicht aus diesem Modul.
//!
//! Zonen 1 und 2 zählen als locker, unabhängig vom Schema: Bei `power7` (Q16, Coggan), `hr_lthr5`
//! und den übrigen Schemata des Katalogs sind das die beiden untersten Zonen. Die Zahl der Zonen
//! je Schema darf verschieden sein, gezählt wird nur "Zone 1 und 2 gegen alle Zonen".
//!
//! Eingang ist die Zeit in Zonen je Aktivität und Schema (`activity_zone_time`, SPEC 6.2).
//! Eine Aktivität hat oft mehrere Schemata (Leistung und Herzfrequenz). Damit ihre Zeit nie
//! doppelt zählt, wählt [`DistributionBasis`] genau eines je Aktivität: nach Vorrang Leistung vor
//! Herzfrequenz vor Geschwindigkeit (Konvention, Leistung ist das direktere Maß), nur ein Kanal
//! oder nur ein Schema.

use std::collections::BTreeMap;

use chrono::{NaiveDate, TimeDelta};
use serde::{Deserialize, Serialize};

use crate::plan::Rules;
use crate::zones::ZoneChannel;

/// Zonen 1 und 2 zählen als locker (R8: "Zone 1 und 2").
pub const EASY_ZONES: usize = 2;

/// Länge des Fensters "über vier Wochen" in Tagen.
pub const WINDOW_DAYS: u32 = 28;

/// Mindestens so viele Sekunden mit Zonen braucht eine Deutung (Faustregel, eine Stunde). Bei
/// weniger bestimmt eine einzelne Einheit den Anteil, und "100 Prozent hart" nach einer
/// zwanzigminütigen Einheit wäre keine Verteilung.
pub const MIN_ZONE_SECONDS: u64 = 3600;

/// Toleranz der Grenzprüfung, wie bei der Prüfung von R8 im Planer (`validate`, 1e-3).
const LIMIT_EPS: f64 = 1e-3;

/// Zeit in Zonen einer Aktivität für ein Schema. Index 0 ist Zone 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneTimeEntry {
    /// Lokaler Tag der Aktivität.
    pub date: NaiveDate,
    /// Kennung der Aktivität. Entscheidend ist nur die Gleichheit, damit eine Aktivität mit
    /// mehreren Schemata nur einmal zählt.
    pub activity_id: i64,
    /// Kennung des Schemas, zum Beispiel `power7`.
    pub scheme: String,
    pub channel: ZoneChannel,
    pub seconds: Vec<u32>,
}

/// Welche Zeiten in die Verteilung eingehen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum DistributionBasis {
    /// Je Aktivität ein Schema: Leistung vor Herzfrequenz vor Geschwindigkeit.
    Priority,
    /// Nur Schemata dieses Kanals.
    Channel(ZoneChannel),
    /// Nur dieses Schema (Kennung wie in `activity_zone_time.scheme`).
    Scheme(String),
}

impl DistributionBasis {
    fn admits(&self, e: &ZoneTimeEntry) -> bool {
        match self {
            Self::Priority => true,
            Self::Channel(c) => e.channel == *c,
            Self::Scheme(s) => e.scheme == *s,
        }
    }
}

/// Rang eines Kanals bei Vorrang (kleiner ist besser).
const fn rank(c: ZoneChannel) -> u8 {
    match c {
        ZoneChannel::Power => 0,
        ZoneChannel::HeartRate => 1,
        ZoneChannel::Speed => 2,
    }
}

/// Zeit in lockeren Zonen und insgesamt, mit Anteil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EasyShare {
    /// Sekunden in Zone 1 und 2.
    pub easy_s: u64,
    /// Sekunden in allen Zonen.
    pub total_s: u64,
}

impl EasyShare {
    /// Anteil in 0 bis 1, `None` ohne Zeit mit Zonen.
    pub fn share(&self) -> Option<f64> {
        (self.total_s > 0).then(|| self.easy_s as f64 / self.total_s as f64)
    }
}

/// Eine Woche der Verteilung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeekShare {
    pub week_start: NaiveDate,
    pub share: EasyShare,
}

/// Wählt je Aktivität höchstens einen Eintrag im Datumsbereich `from..=to`. Bei Gleichstand
/// des Rangs gewinnt der frühere Eintrag der Eingabe, der Aufrufer ordnet also nach Katalog.
fn select<'a>(
    entries: &'a [ZoneTimeEntry],
    from: NaiveDate,
    to: NaiveDate,
    basis: &DistributionBasis,
) -> Vec<&'a ZoneTimeEntry> {
    let mut chosen: BTreeMap<i64, &ZoneTimeEntry> = BTreeMap::new();
    for e in entries
        .iter()
        .filter(|e| e.date >= from && e.date <= to && basis.admits(e))
    {
        match chosen.get(&e.activity_id) {
            Some(cur) if rank(cur.channel) <= rank(e.channel) => {}
            _ => {
                chosen.insert(e.activity_id, e);
            }
        }
    }
    chosen.into_values().collect()
}

/// Zeit in lockeren Zonen und insgesamt über `from..=to` (beide einschließlich).
pub fn easy_share(
    entries: &[ZoneTimeEntry],
    from: NaiveDate,
    to: NaiveDate,
    basis: &DistributionBasis,
) -> EasyShare {
    let mut out = EasyShare {
        easy_s: 0,
        total_s: 0,
    };
    for e in select(entries, from, to, basis) {
        out.easy_s += e
            .seconds
            .iter()
            .take(EASY_ZONES)
            .map(|&s| u64::from(s))
            .sum::<u64>();
        out.total_s += e.seconds.iter().map(|&s| u64::from(s)).sum::<u64>();
    }
    out
}

/// Verteilung je Woche für `weeks` aufeinanderfolgende Wochen ab `first_week_start`. Die Woche
/// beginnt am übergebenen Tag (der Aufrufer nimmt den Montag, wie der Planer).
pub fn weekly_easy_share(
    entries: &[ZoneTimeEntry],
    first_week_start: NaiveDate,
    weeks: u32,
    basis: &DistributionBasis,
) -> Vec<WeekShare> {
    (0..weeks)
        .filter_map(|w| {
            let start = first_week_start.checked_add_signed(TimeDelta::days(7 * i64::from(w)))?;
            let end = start.checked_add_signed(TimeDelta::days(6))?;
            Some(WeekShare {
                week_start: start,
                share: easy_share(entries, start, end, basis),
            })
        })
        .collect()
}

/// Verteilung über die letzten `days` Tage bis einschließlich `end` (Standard: [`WINDOW_DAYS`]).
pub fn window_easy_share(
    entries: &[ZoneTimeEntry],
    end: NaiveDate,
    days: u32,
    basis: &DistributionBasis,
) -> EasyShare {
    let Some(start) = end.checked_sub_signed(TimeDelta::days(i64::from(days.max(1)) - 1)) else {
        return EasyShare {
            easy_s: 0,
            total_s: 0,
        };
    };
    easy_share(entries, start, end, basis)
}

/// Deutung der Verteilung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributionState {
    /// Zu wenig Zeit mit Zonen für eine Deutung (unter [`MIN_ZONE_SECONDS`]).
    NoData,
    /// Der lockere Anteil erreicht die Grenze aus R8.
    EasyDominant,
    /// Der lockere Anteil liegt unter der Grenze aus R8: mittlere und harte Zonen überwiegen
    /// mehr, als R8 für den Plan zulässt.
    IntensityHeavy,
}

impl DistributionState {
    /// Stabiler Code, Schlüssel für Texte in `catalog/locales`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::NoData => "distribution.no_data",
            Self::EasyDominant => "distribution.easy_dominant",
            Self::IntensityHeavy => "distribution.intensity_heavy",
        }
    }
}

/// Deutet einen Anteil gegen `R08.min_easy_share` der Regeln (Q5 beschreibend, Grenze 70 Prozent
/// Faustregel, SPEC 8.4). Genau auf der Grenze gilt als erreicht, wie bei der Prüfung von R8.
pub fn distribution_state(share: &EasyShare, rules: &Rules) -> DistributionState {
    if share.total_s < MIN_ZONE_SECONDS {
        return DistributionState::NoData;
    }
    match share.share() {
        Some(s) if s >= f64::from(rules.r08.min_easy_share) - LIMIT_EPS => {
            DistributionState::EasyDominant
        }
        Some(_) => DistributionState::IntensityHeavy,
        None => DistributionState::NoData,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::default_rules;

    fn day(n: i64) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, 2).unwrap() + TimeDelta::days(n)
    }

    fn entry(
        date: i64,
        activity_id: i64,
        scheme: &str,
        channel: ZoneChannel,
        seconds: &[u32],
    ) -> ZoneTimeEntry {
        ZoneTimeEntry {
            date: day(date),
            activity_id,
            scheme: scheme.into(),
            channel,
            seconds: seconds.to_vec(),
        }
    }

    /// Eine Woche (Tag 0 bis 6), drei Aktivitäten:
    ///
    /// - Aktivität 1 hat Leistung (power7: 1800 + 1800 + 600 s) und Herzfrequenz (hr5, nicht
    ///   gewählt, sonst zählte die Zeit doppelt).
    /// - Aktivität 2 hat nur Herzfrequenz (hr5: 2400 + 600 + 600 s).
    /// - Aktivität 3 liegt in der Folgewoche (Tag 7) und zählt nicht.
    fn week() -> Vec<ZoneTimeEntry> {
        vec![
            entry(
                0,
                1,
                "hr_lthr5",
                ZoneChannel::HeartRate,
                &[100, 100, 100, 0, 0],
            ),
            entry(
                0,
                1,
                "power7",
                ZoneChannel::Power,
                &[1800, 1800, 600, 0, 0, 0, 0],
            ),
            entry(
                2,
                2,
                "hr_lthr5",
                ZoneChannel::HeartRate,
                &[2400, 600, 600, 0, 0],
            ),
            entry(
                7,
                3,
                "power7",
                ZoneChannel::Power,
                &[0, 0, 0, 0, 3600, 0, 0],
            ),
        ]
    }

    /// Rechenweg (Vorrang Leistung vor Herzfrequenz):
    /// locker = (1800 + 1800) + (2400 + 600) = 6600 s,
    /// gesamt = (1800 + 1800 + 600) + (2400 + 600 + 600) = 4200 + 3600 = 7800 s,
    /// Anteil = 6600 / 7800 = 0,846154.
    const PRIORITY_EASY_S: u64 = 6600;
    const PRIORITY_TOTAL_S: u64 = 7800;

    #[test]
    fn priority_counts_each_activity_once_with_power_first() {
        let s = easy_share(&week(), day(0), day(6), &DistributionBasis::Priority);
        assert_eq!(s.easy_s, PRIORITY_EASY_S);
        assert_eq!(s.total_s, PRIORITY_TOTAL_S);
        assert!((s.share().unwrap() - 0.846_154).abs() < 1e-6);
    }

    #[test]
    fn priority_does_not_depend_on_the_input_order() {
        let mut e = week();
        e.reverse();
        let s = easy_share(&e, day(0), day(6), &DistributionBasis::Priority);
        assert_eq!((s.easy_s, s.total_s), (PRIORITY_EASY_S, PRIORITY_TOTAL_S));
    }

    #[test]
    fn channel_and_scheme_bases_keep_the_schemes_apart() {
        let e = week();
        // Nur Herzfrequenz: Aktivität 1 mit 200 / 300 s, Aktivität 2 mit 3000 / 3600 s.
        // Locker 200 + 3000 = 3200, gesamt 300 + 3600 = 3900.
        let hr = easy_share(
            &e,
            day(0),
            day(6),
            &DistributionBasis::Channel(ZoneChannel::HeartRate),
        );
        assert_eq!((hr.easy_s, hr.total_s), (3200, 3900));
        // Nur power7: nur Aktivität 1, 3600 / 4200 s.
        let p = easy_share(
            &e,
            day(0),
            day(6),
            &DistributionBasis::Scheme("power7".into()),
        );
        assert_eq!((p.easy_s, p.total_s), (3600, 4200));
    }

    #[test]
    fn weeks_are_split_at_the_given_start_and_windows_count_back() {
        let e = week();
        let w = weekly_easy_share(&e, day(0), 2, &DistributionBasis::Priority);
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].week_start, day(0));
        assert_eq!(w[0].share.total_s, PRIORITY_TOTAL_S);
        assert_eq!(w[1].week_start, day(7));
        // Woche 2: nur Aktivität 3, 3600 s in Zone 5, nichts locker.
        assert_eq!((w[1].share.easy_s, w[1].share.total_s), (0, 3600));
        // Fenster von 28 Tagen bis Tag 7 deckt beide Wochen: 6600 / 11400.
        let win = window_easy_share(&e, day(7), WINDOW_DAYS, &DistributionBasis::Priority);
        assert_eq!((win.easy_s, win.total_s), (6600, 11400));
        // Fenster von einem Tag bis Tag 0: nur die Leistung von Aktivität 1.
        let one = window_easy_share(&e, day(0), 1, &DistributionBasis::Priority);
        assert_eq!((one.easy_s, one.total_s), (3600, 4200));
    }

    #[test]
    fn empty_input_has_no_share() {
        let s = easy_share(&[], day(0), day(6), &DistributionBasis::Priority);
        assert_eq!(s.share(), None);
        assert_eq!(
            distribution_state(&s, &default_rules()),
            DistributionState::NoData
        );
    }

    #[test]
    fn state_uses_the_r8_threshold_from_the_rules() {
        let mut rules = default_rules();
        assert_eq!(rules.r08.min_easy_share, 0.7);
        // 7 von 10 Stunden locker = genau 0,7, gilt als erreicht (wie bei R8).
        let at_limit = EasyShare {
            easy_s: 7 * 3600,
            total_s: 10 * 3600,
        };
        assert_eq!(
            distribution_state(&at_limit, &rules),
            DistributionState::EasyDominant
        );
        // 69 von 100 Minuten: 0,69 liegt unter der Grenze.
        let below = EasyShare {
            easy_s: 69 * 60,
            total_s: 100 * 60,
        };
        assert_eq!(
            distribution_state(&below, &rules),
            DistributionState::IntensityHeavy
        );
        // Wer die Grenze in den Regeln senkt, ändert die Deutung ohne Codeänderung.
        rules.r08.min_easy_share = 0.6;
        assert_eq!(
            distribution_state(&below, &rules),
            DistributionState::EasyDominant
        );
        // Unter einer Stunde Zonenzeit keine Deutung.
        let short = EasyShare {
            easy_s: 0,
            total_s: 3599,
        };
        assert_eq!(
            distribution_state(&short, &rules),
            DistributionState::NoData
        );
    }

    #[test]
    fn codes_are_stable() {
        assert_eq!(
            DistributionState::EasyDominant.code(),
            "distribution.easy_dominant"
        );
        assert_eq!(
            DistributionState::IntensityHeavy.code(),
            "distribution.intensity_heavy"
        );
        assert_eq!(DistributionState::NoData.code(), "distribution.no_data");
    }
}
