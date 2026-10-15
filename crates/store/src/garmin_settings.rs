//! Einstellungen des Abgleichs mit Garmin (SPEC 5.6, T22).
//!
//! Sie liegen wie alle Einstellungen in der Tabelle `setting` und stehen im Backup. Lesen
//! und Schreiben laufen nur über diese Methoden, damit CLI und App dieselben Regeln und
//! Standardwerte nehmen.

use chrono::NaiveDate;

use crate::error::{Result, StoreError};
use crate::rows::{fmt_date, parse_date};
use crate::store::Store;

/// Datum des letzten vollständigen Abgleichs (`JJJJ-MM-TT`).
pub const SETTING_GARMIN_LAST_SYNC: &str = "garmin.last_sync";
/// Abgleich beim Start der App (`true` oder `false`).
pub const SETTING_GARMIN_SYNC_ON_START: &str = "garmin.sync_on_start";
/// Zahl der Tage, die der erste Abgleich zurückgeht.
pub const SETTING_GARMIN_BACKFILL_DAYS: &str = "garmin.backfill_days";

/// Standard der Rückschau beim ersten Abgleich (Faustregel: ein Jahr deckt eine volle
/// Saison für CTL und die Basislinie der HRV ab).
pub const DEFAULT_BACKFILL_DAYS: u32 = 365;
/// Kleinste und größte einstellbare Rückschau (Faustregel: 1 Tag bis rund fünf Jahre).
pub const MIN_BACKFILL_DAYS: u32 = 1;
pub const MAX_BACKFILL_DAYS: u32 = 1825;

/// Die Einstellungen des Abgleichs mit ihren Standardwerten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GarminSettings {
    pub last_sync: Option<NaiveDate>,
    pub sync_on_start: bool,
    pub backfill_days: u32,
}

impl Store {
    /// Liest die drei Einstellungen. Ein fehlender oder nicht lesbarer Wert ergibt den
    /// Standard (kein Abgleich bisher, Abgleich beim Start an, 365 Tage), je Wert einzeln.
    /// Eine Rückschau außerhalb von 1 bis 1825 gilt als nicht lesbar.
    pub fn garmin_settings(&self) -> Result<GarminSettings> {
        let last_sync = self
            .get_setting(SETTING_GARMIN_LAST_SYNC)?
            .and_then(|t| parse_date(t.trim()).ok());
        let sync_on_start = self
            .get_setting(SETTING_GARMIN_SYNC_ON_START)?
            .is_none_or(|t| t.trim() != "false");
        let backfill_days = self
            .get_setting(SETTING_GARMIN_BACKFILL_DAYS)?
            .and_then(|t| t.trim().parse::<u32>().ok())
            .filter(|d| (MIN_BACKFILL_DAYS..=MAX_BACKFILL_DAYS).contains(d))
            .unwrap_or(DEFAULT_BACKFILL_DAYS);
        Ok(GarminSettings {
            last_sync,
            sync_on_start,
            backfill_days,
        })
    }

    pub fn set_garmin_last_sync(&mut self, date: NaiveDate) -> Result<()> {
        self.set_setting(SETTING_GARMIN_LAST_SYNC, &fmt_date(date))
    }

    pub fn set_garmin_sync_on_start(&mut self, on: bool) -> Result<()> {
        self.set_setting(
            SETTING_GARMIN_SYNC_ON_START,
            if on { "true" } else { "false" },
        )
    }

    /// Setzt die Rückschau. Werte außerhalb von 1 bis 1825 sind `InvalidInput`.
    pub fn set_garmin_backfill_days(&mut self, days: u32) -> Result<()> {
        if !(MIN_BACKFILL_DAYS..=MAX_BACKFILL_DAYS).contains(&days) {
            return Err(StoreError::InvalidInput(
                "garmin.backfill_days out of range".into(),
            ));
        }
        self.set_setting(SETTING_GARMIN_BACKFILL_DAYS, &days.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn defaults_apply_without_settings() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(
            store.garmin_settings().unwrap(),
            GarminSettings {
                last_sync: None,
                sync_on_start: true,
                backfill_days: 365
            }
        );
    }

    #[test]
    fn values_round_trip() {
        let mut store = Store::open_in_memory().unwrap();
        store.set_garmin_last_sync(d(2026, 10, 14)).unwrap();
        store.set_garmin_sync_on_start(false).unwrap();
        store.set_garmin_backfill_days(1825).unwrap();
        assert_eq!(
            store.garmin_settings().unwrap(),
            GarminSettings {
                last_sync: Some(d(2026, 10, 14)),
                sync_on_start: false,
                backfill_days: 1825
            }
        );
        store.set_garmin_backfill_days(1).unwrap();
        assert_eq!(store.garmin_settings().unwrap().backfill_days, 1);
    }

    #[test]
    fn out_of_range_backfill_is_refused_on_write_and_ignored_on_read() {
        let mut store = Store::open_in_memory().unwrap();
        for bad in [0, 1826, u32::MAX] {
            assert!(matches!(
                store.set_garmin_backfill_days(bad),
                Err(StoreError::InvalidInput(_))
            ));
        }
        assert_eq!(store.garmin_settings().unwrap().backfill_days, 365);
        // Ein von Hand geschriebener Wert (zum Beispiel aus einer Sicherung) wird nicht
        // übernommen, wenn er außerhalb liegt.
        for text in ["0", "1826", "-5", "abc", ""] {
            store
                .set_setting(SETTING_GARMIN_BACKFILL_DAYS, text)
                .unwrap();
            assert_eq!(
                store.garmin_settings().unwrap().backfill_days,
                365,
                "{text}"
            );
        }
        store
            .set_setting(SETTING_GARMIN_LAST_SYNC, "kein Datum")
            .unwrap();
        assert_eq!(store.garmin_settings().unwrap().last_sync, None);
    }
}
