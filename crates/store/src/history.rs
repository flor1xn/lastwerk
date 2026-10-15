//! Historie für den Planer: Wochenwerte je Sportart und der Stand von CTL und ATL an einem
//! Tag (SPEC 8.3, Eingaben des Planers).

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate, TimeDelta};
use lastwerk_core::plan::WeekHistory;
use lastwerk_core::{PmcParams, PmcPoint, Sport, pmc};

use crate::error::{Result, conversion};
use crate::rows::{fmt_date, parse_date, sport_from_str};
use crate::store::Store;

/// Einstellungen des Verlaufs (SPEC 8.2, 5.4 App): Zeitkonstanten in Tagen und Startwerte.
/// Sie stehen im Backup. Die Oberfläche schreibt sie, alle Leser gehen über
/// [`Store::pmc_params`].
pub const SETTING_PMC_TAU_CTL: &str = "pmc.tau_ctl_d";
pub const SETTING_PMC_TAU_ATL: &str = "pmc.tau_atl_d";
pub const SETTING_PMC_CTL0: &str = "pmc.ctl0";
pub const SETTING_PMC_ATL0: &str = "pmc.atl0";

/// Montag der ISO-Woche, in der `d` liegt.
pub fn monday_of(d: NaiveDate) -> NaiveDate {
    d - TimeDelta::days(i64::from(d.weekday().num_days_from_monday()))
}

/// Feste Reihenfolge der Sportarten in der Ausgabe, damit Listen reproduzierbar sind.
fn sport_rank(s: Sport) -> u8 {
    match s {
        Sport::Cycling => 0,
        Sport::Running => 1,
        Sport::Swimming => 2,
        Sport::Strength => 3,
        Sport::Other => 4,
    }
}

impl Store {
    /// Parameter des Verlaufs aus den Einstellungen `pmc.*`. Die einzige Stelle, die sie liest:
    /// Oberfläche, MCP-Server, `lastwerk plan` und `lastwerk validate` rechnen damit gleich.
    ///
    /// Ein fehlender, nicht lesbarer oder nicht endlicher Wert ergibt den Standard aus
    /// [`PmcParams::default`] (42 und 7 Tage, Start 0), je Wert einzeln. Weitere Grenzen prüft
    /// hier nichts: Die Oberfläche lässt beim Schreiben nur Zeitkonstanten über 0 und
    /// Startwerte ab 0 zu, und der Planer lehnt Werte außerhalb davon als Eingabefehler ab.
    pub fn pmc_params(&self) -> Result<PmcParams> {
        let d = PmcParams::default();
        let number = |key: &str, default: f32| -> Result<f32> {
            Ok(self
                .get_setting(key)?
                .and_then(|t| t.trim().parse::<f32>().ok())
                .filter(|v| v.is_finite())
                .unwrap_or(default))
        };
        Ok(PmcParams {
            tau_ctl_d: number(SETTING_PMC_TAU_CTL, d.tau_ctl_d)?,
            tau_atl_d: number(SETTING_PMC_TAU_ATL, d.tau_atl_d)?,
            ctl0: number(SETTING_PMC_CTL0, d.ctl0)?,
            atl0: number(SETTING_PMC_ATL0, d.atl0)?,
        })
    }

    /// Last und Trainingszeit je ISO-Woche und Sportart für die `weeks` Wochen vor der Woche,
    /// in der `before` liegt (die Woche von `before` selbst gehört nicht dazu).
    ///
    /// Es zählen nur primäre Aktivitäten (Duplikatregel, SPEC 6.4). Die Zeit ist die Summe von
    /// `duration_s` aller dieser Aktivitäten in Minuten, auch ohne gültige Last: R2 vergleicht
    /// die Trainingszeit aller geplanten Einheiten (SPEC 8.4) mit der Vorwoche, und der Plan
    /// enthält Kraft, deren Last aus FIT nie berechenbar ist (`missing_channel`, RPE nur von
    /// Hand). Die Last ist wie bei [`Store::daily_loads`] nur die Summe der Belastungen im
    /// Status `ok`, damit sie zur CTL passt; hat keine Aktivität der Sportart in der Woche
    /// eine solche, ist sie 0. Wochen oder Sportarten ohne primäre Aktivität fehlen in der
    /// Antwort. Sortiert nach Woche (älteste zuerst) und Sportart.
    pub fn week_history(&self, before: NaiveDate, weeks: u32) -> Result<Vec<WeekHistory>> {
        let end = monday_of(before);
        let start = end - TimeDelta::days(7 * i64::from(weeks));
        let mut stmt = self.conn.prepare(
            "SELECT a.local_date, a.sport, a.duration_s, SUM(l.value) FROM activity a \
             LEFT JOIN activity_load l ON l.activity_id = a.id AND l.status = 'ok' \
             WHERE a.is_primary = 1 \
               AND a.local_date >= ?1 AND a.local_date < ?2 \
             GROUP BY a.id ORDER BY a.local_date, a.id",
        )?;
        let rows = stmt
            .query_map([fmt_date(start), fmt_date(end)], |r| {
                let date: String = r.get(0)?;
                let sport: String = r.get(1)?;
                Ok((
                    conversion(0, parse_date(&date))?,
                    conversion(1, sport_from_str(&sport))?,
                    r.get::<_, i64>(2)?,
                    // Ohne Belastung im Status `ok` liefert SUM über keine Zeile NULL.
                    r.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        // Summen in f64, erst am Ende auf f32 runden.
        let mut sums: BTreeMap<(NaiveDate, u8), (Sport, f64, f64)> = BTreeMap::new();
        for (date, sport, duration_s, load) in rows {
            let e = sums
                .entry((monday_of(date), sport_rank(sport)))
                .or_insert((sport, 0.0, 0.0));
            e.1 += load;
            e.2 += duration_s as f64 / 60.0;
        }
        Ok(sums
            .into_iter()
            .map(|((week_start, _), (sport, load, minutes))| WeekHistory {
                week_start,
                sport,
                load: load as f32,
                minutes: minutes as f32,
            })
            .collect())
    }

    /// CTL und ATL am Ende des Tages `date`, gerechnet aus den Tageslasten ab dem ersten Tag
    /// mit zählender Last (SPEC 8.2: Start mit `ctl0` und `atl0` am ersten Tag mit Daten).
    ///
    /// Liegt `date` vor dem ersten Tag mit Last oder gibt es keine Last, sind die Werte die
    /// Startwerte `p.ctl0` und `p.atl0`. Belastungen nach `date` bleiben unberücksichtigt.
    /// Die Zeitkonstanten und Startwerte kommen vom Aufrufer, sie sind einstellbar.
    pub fn pmc_state_at(&self, date: NaiveDate, p: &PmcParams) -> Result<PmcPoint> {
        let Some(first) = self.first_load_date()?.filter(|f| *f <= date) else {
            return Ok(PmcPoint {
                date,
                ctl: p.ctl0,
                atl: p.atl0,
                tsb: p.ctl0 - p.atl0,
            });
        };
        let daily = self.daily_loads(first, date)?;
        let points = pmc(&daily, p, first, date);
        Ok(points.last().copied().unwrap_or(PmcPoint {
            date,
            ctl: p.ctl0,
            atl: p.atl0,
            tsb: p.ctl0 - p.atl0,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monday_of_maps_every_weekday_to_its_monday() {
        // 4. Januar 2027 ist ein Montag.
        let monday = NaiveDate::from_ymd_opt(2027, 1, 4).unwrap();
        for k in 0..7 {
            assert_eq!(monday_of(monday + TimeDelta::days(k)), monday);
        }
        assert_eq!(
            monday_of(monday - TimeDelta::days(1)),
            monday - TimeDelta::days(7)
        );
    }

    #[test]
    fn pmc_params_read_the_settings_with_defaults_per_value() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(store.pmc_params().unwrap(), PmcParams::default());
        store.set_setting(SETTING_PMC_TAU_CTL, " 50 ").unwrap();
        store.set_setting(SETTING_PMC_TAU_ATL, "unsinn").unwrap();
        store.set_setting(SETTING_PMC_CTL0, "12.5").unwrap();
        store.set_setting(SETTING_PMC_ATL0, "inf").unwrap();
        assert_eq!(
            store.pmc_params().unwrap(),
            PmcParams {
                tau_ctl_d: 50.0,
                // Nicht lesbar und nicht endlich: je Wert der Standard.
                tau_atl_d: 7.0,
                ctl0: 12.5,
                atl0: 0.0,
            }
        );
    }
}
