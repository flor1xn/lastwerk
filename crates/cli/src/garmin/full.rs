//! Phasen "Konto" und "Tageswerte" des Abgleichs (T39, SPEC 5.6, 8.6).
//!
//! - **Konto:** Profil (Geschlecht, Geburtsjahr, VO2max), FTP, Laktatschwelle und
//!   Herzfrequenzzonen. Daraus werden Schwellen mit Herkunft `garmin` und, solange der Nutzer
//!   sie nicht selbst gesetzt hat, Geschlecht und Geburtsjahr.
//! - **Tageswerte:** Schritte, Bergwert und Garmins Last je Aktivität über Bereichsabfragen im
//!   ganzen Fenster; Tagesherzfrequenz, Atmung, SpO2, Intensitätsminuten, Stress,
//!   Trainingsbereitschaft und (nur mit Freigabe) Hydration je Tag für die letzten
//!   [`DETAIL_DAYS`] Tage; Fitnessalter und Ausdauerwert einmal je Lauf.
//!
//! Geschrieben wird erst nach allen Phasen ([`write_full`]), vor dem Import, damit neue Dateien
//! gleich mit den neuen Schwellen gerechnet werden. Was sich an den Schwellen ändert, rechnet
//! der Abgleich danach für bereits bekannte Aktivitäten neu.

use std::collections::BTreeMap;

use chrono::{NaiveDate, TimeDelta};
use lastwerk_core::Sex;
use lastwerk_garmin::{
    MAX_LOAD_SPAN_DAYS, MAX_SCORE_SPAN_DAYS, MAX_STEPS_SPAN_DAYS, SensitiveConsent,
};
use lastwerk_store::{
    DayRecord, GARMIN_APP_SOURCE, GarminActivityLoad, GarminThresholdOutcome,
    SETTING_ATHLETE_BIRTH_YEAR, SETTING_ATHLETE_BIRTH_YEAR_ORIGIN, SETTING_ATHLETE_SEX,
    SETTING_ATHLETE_SEX_ORIGIN, SensitiveDay, Store, StoreError, TrainingDay, WellnessDay,
};

use super::parse_full::{self as pf, GarminThreshold, ProfileValues};
use super::{Run, Stop, SyncOptions, SyncPhase, SyncProgress, chunks};

/// Tage am Ende des Fensters, für die Tageswerte mit einem Aufruf je Tag geholt werden
/// (Faustregel wie [`super::SLEEP_DAYS`]: beim ersten Abgleich wären es sonst über 2500
/// Aufrufe, rund 90 Minuten bei 2 s je Aufruf).
pub const DETAIL_DAYS: i64 = 28;

/// Was die Phasen "Konto" und "Tageswerte" gesammelt haben.
#[derive(Default)]
pub(super) struct FullFound {
    pub profile: ProfileValues,
    pub thresholds: Vec<GarminThreshold>,
    pub wellness: BTreeMap<NaiveDate, WellnessDay>,
    pub training: BTreeMap<NaiveDate, TrainingDay>,
    pub sensitive: BTreeMap<NaiveDate, SensitiveDay>,
    pub loads: Vec<GarminActivityLoad>,
}

/// Ergebnis von [`write_full`].
#[derive(Debug, Default)]
pub(super) struct FullWritten {
    pub thresholds_changed: u32,
    /// Frühester Tag, ab dem Belastungen neu zu rechnen sind.
    pub recompute_from: Option<NaiveDate>,
    pub wellness_days: u32,
    pub training_days: u32,
    pub sensitive_days: u32,
    pub garmin_loads: u32,
    pub dropped: u32,
}

impl Run<'_> {
    fn wellness_day(&mut self, date: NaiveDate) -> &mut WellnessDay {
        let now = self.now;
        self.full
            .wellness
            .entry(date)
            .or_insert_with(|| WellnessDay::empty(date, GARMIN_APP_SOURCE, now))
    }

    fn training_day(&mut self, date: NaiveDate) -> &mut TrainingDay {
        let now = self.now;
        self.full
            .training
            .entry(date)
            .or_insert_with(|| TrainingDay::empty(date, GARMIN_APP_SOURCE, now))
    }

    /// Phase "Konto": vier Aufrufe ohne Datum.
    pub(super) fn account(
        &mut self,
        today: NaiveDate,
        progress: &mut dyn FnMut(SyncProgress),
    ) -> Result<(), Stop> {
        let total = 4;
        let tick = |done: u32, progress: &mut dyn FnMut(SyncProgress)| {
            progress(SyncProgress {
                phase: SyncPhase::Account,
                done,
                total: Some(total),
            });
        };
        tick(0, progress);
        if let Some(v) = self.call(|s| s.user_settings())? {
            self.full.profile = pf::profile(&v, today);
            let (run, bike) = (
                self.full.profile.vo2max_running,
                self.full.profile.vo2max_cycling,
            );
            let day = self.training_day(today);
            day.vo2max_running = run;
            day.vo2max_cycling = bike;
        }
        tick(1, progress);
        if let Some(v) = self.call(|s| s.cycling_ftp())?
            && let Some(t) = pf::ftp(&v, &mut self.dropped)
        {
            self.full.thresholds.push(t);
        }
        tick(2, progress);
        if let Some(v) = self.call(|s| s.lactate_threshold())? {
            let found = pf::lactate_threshold(&v, &mut self.dropped);
            self.full.thresholds.extend(found);
        }
        tick(3, progress);
        if let Some(v) = self.call(|s| s.heart_rate_zones())? {
            let found = pf::heart_rate_zones(&v, &mut self.dropped);
            self.full.thresholds.extend(found);
        }
        tick(total, progress);
        Ok(())
    }

    /// Phase "Tageswerte". `hydration` ist nur gesetzt, wenn der Nutzer S3 freigegeben hat.
    pub(super) fn wellness(
        &mut self,
        start: NaiveDate,
        today: NaiveDate,
        hydration: Option<SensitiveConsent>,
        progress: &mut dyn FnMut(SyncProgress),
    ) -> Result<(), Stop> {
        let detail_start = start.max(today - TimeDelta::days(DETAIL_DAYS - 1));
        let steps = chunks(start, today, MAX_STEPS_SPAN_DAYS);
        let hills = chunks(start, today, MAX_SCORE_SPAN_DAYS);
        let loads = chunks(start, today, MAX_LOAD_SPAN_DAYS);
        let endurance_start = start.max(today - TimeDelta::days(MAX_SCORE_SPAN_DAYS - 1));
        let per_day = 6 + u32::from(hydration.is_some());
        let detail_days = (today - detail_start).num_days() + 1;
        let total =
            (steps.len() + hills.len() + loads.len() + 2) as u32 + per_day * detail_days as u32;
        let mut done = 0u32;
        let mut tick = |progress: &mut dyn FnMut(SyncProgress)| {
            progress(SyncProgress {
                phase: SyncPhase::Wellness,
                done,
                total: Some(total),
            });
            done += 1;
        };

        for (from, to) in steps {
            tick(progress);
            if let Some(v) = self.call(|s| s.daily_steps(from, to))? {
                for (date, n, m) in pf::steps_days(&v) {
                    if (from..=to).contains(&date) {
                        let day = self.wellness_day(date);
                        day.steps = n.or(day.steps);
                        day.step_distance_m = m.or(day.step_distance_m);
                    }
                }
            }
        }
        for (from, to) in hills {
            tick(progress);
            if let Some(v) = self.call(|s| s.hill_score(from, to))? {
                for (date, score) in pf::hill_scores(&v) {
                    if (from..=to).contains(&date) {
                        self.training_day(date).hill_score = Some(score);
                    }
                }
            }
        }
        for (from, to) in loads {
            tick(progress);
            if let Some(v) = self.call(|s| s.training_load_activities(from, to))? {
                for (id, date, load, effect) in pf::training_loads(&v) {
                    if (from..=to).contains(&date) {
                        self.full.loads.push(GarminActivityLoad {
                            garmin_activity_id: id,
                            date,
                            training_load: load,
                            training_effect: effect,
                            source: GARMIN_APP_SOURCE.to_owned(),
                            imported_at: self.now,
                        });
                    }
                }
            }
        }
        tick(progress);
        if let Some(v) = self.call(|s| s.endurance_score(endurance_start, today))?
            && let Some((date, score)) = pf::endurance_score(&v)
            && (endurance_start..=today).contains(&date)
        {
            self.training_day(date).endurance_score = Some(score);
        }
        tick(progress);
        if let Some(v) = self.call(|s| s.fitness_age(today))?
            && let Some(age) = pf::field(&v, "fitnessAge")
        {
            self.training_day(today).fitness_age = Some(age);
        }

        let name = self.display_name.clone();
        let mut date = detail_start;
        while date <= today {
            // Eine Tagesantwort mit anderem `calendarDate` als dem angefragten Tag kommt nie in
            // den Speicher (wie bei den Erholungswerten).
            let same_day = |v: &serde_json::Value| pf::calendar_date(v).is_none_or(|d| d == date);
            tick(progress);
            if let Some(name) = &name
                && let Some(v) = self.call(|s| s.heart_rate_day(name, date))?
                && same_day(&v)
            {
                let day = self.wellness_day(date);
                day.hr_min_bpm = pf::field(&v, "minHeartRate");
                day.hr_max_bpm = pf::field(&v, "maxHeartRate");
            }
            tick(progress);
            if let Some(v) = self.call(|s| s.respiration_day(date))?
                && same_day(&v)
            {
                let day = self.wellness_day(date);
                day.respiration_waking_brpm = pf::field(&v, "avgWakingRespirationValue");
                day.respiration_sleep_brpm = pf::field(&v, "avgSleepRespirationValue");
            }
            tick(progress);
            if let Some(v) = self.call(|s| s.spo2_day(date))?
                && same_day(&v)
            {
                let day = self.wellness_day(date);
                day.spo2_avg_pct = pf::field(&v, "averageSpO2");
                day.spo2_low_pct = pf::field(&v, "lowestSpO2");
            }
            tick(progress);
            if let Some(v) = self.call(|s| s.intensity_minutes_day(date))?
                && same_day(&v)
            {
                let day = self.wellness_day(date);
                day.intensity_moderate_min = pf::field(&v, "moderateMinutes");
                day.intensity_vigorous_min = pf::field(&v, "vigorousMinutes");
            }
            tick(progress);
            if let Some(v) = self.call(|s| s.stress_day(date))?
                && same_day(&v)
            {
                let day = self.wellness_day(date);
                day.stress_avg = pf::field(&v, "avgStressLevel");
                day.stress_max = pf::field(&v, "maxStressLevel");
            }
            tick(progress);
            if let Some(v) = self.call(|s| s.training_readiness(date))?
                && let Some((score, level, load)) = pf::training_readiness(&v)
            {
                let day = self.training_day(date);
                day.readiness_score = score;
                day.readiness_level = level;
                day.garmin_acute_load = load;
            }
            if let Some(consent) = hydration {
                tick(progress);
                if let Some(v) = self.call(|s| s.hydration_day(consent, date))?
                    && same_day(&v)
                {
                    let now = self.now;
                    self.full
                        .sensitive
                        .entry(date)
                        .or_insert_with(|| SensitiveDay::empty(date, GARMIN_APP_SOURCE, now))
                        .hydration_ml = pf::field(&v, "valueInML");
                }
            }
            date += TimeDelta::days(1);
        }
        progress(SyncProgress {
            phase: SyncPhase::Wellness,
            done: total,
            total: Some(total),
        });
        Ok(())
    }
}

fn sex_text(sex: Sex) -> &'static str {
    match sex {
        Sex::Male => "male",
        Sex::Female => "female",
    }
}

/// Setzt eine Athleteneinstellung aus Garmin, solange der Nutzer sie nicht selbst gesetzt hat
/// (SPEC 8.6). `true`, wenn sich der Wert geändert hat.
fn athlete_setting(
    store: &mut Store,
    key: &str,
    origin_key: &str,
    value: &str,
) -> Result<bool, StoreError> {
    if store.get_setting(origin_key)?.as_deref() == Some("manual") {
        return Ok(false);
    }
    // Ohne Herkunft, aber mit Wert stammt er aus der Zeit vor T39 und damit vom Nutzer.
    let current = store.get_setting(key)?;
    if store.get_setting(origin_key)?.is_none() && current.is_some() {
        store.set_setting(origin_key, "manual")?;
        return Ok(false);
    }
    store.set_setting(origin_key, "garmin")?;
    if current.as_deref() == Some(value) {
        return Ok(false);
    }
    store.set_setting(key, value)?;
    Ok(true)
}

/// Schreibt Schwellen, Athletendaten und Tageswerte (siehe Moduldokumentation und SPEC 8.6).
pub(super) fn write_full(
    store: &mut Store,
    found: FullFound,
    window_start: NaiveDate,
    opts: &SyncOptions,
) -> Result<FullWritten, StoreError> {
    let mut w = FullWritten::default();
    let earliest = |w: &mut FullWritten, d: NaiveDate| {
        w.recompute_from = Some(w.recompute_from.map_or(d, |e| e.min(d)));
    };

    for t in &found.thresholds {
        let date = match t.date {
            Some(d) => d,
            // Ohne Datum: erstes Auftreten ab Fensterbeginn, eine Änderung ab heute, ein
            // gleicher Wert schreibt nichts (Konvention, SPEC 8.6).
            None => match store.latest_garmin_threshold(t.sport, t.kind)? {
                Some((_, v)) if (v - t.value).abs() < 1e-9 => continue,
                Some(_) => opts.today,
                None => window_start,
            },
        };
        let outcome = store.upsert_garmin_threshold(t.sport, t.kind, t.value, date)?;
        if outcome != GarminThresholdOutcome::Unchanged {
            w.thresholds_changed += 1;
            earliest(&mut w, date);
        }
    }

    let mut athlete_changed = false;
    if let Some(sex) = found.profile.sex {
        athlete_changed |= athlete_setting(
            store,
            SETTING_ATHLETE_SEX,
            SETTING_ATHLETE_SEX_ORIGIN,
            sex_text(sex),
        )?;
    }
    if let Some(year) = found.profile.birth_year {
        athlete_changed |= athlete_setting(
            store,
            SETTING_ATHLETE_BIRTH_YEAR,
            SETTING_ATHLETE_BIRTH_YEAR_ORIGIN,
            &year.to_string(),
        )?;
    }
    if athlete_changed {
        // Geschlecht und Alter gelten für jede Aktivität (wie `athlete set`, SPEC 5.4).
        earliest(
            &mut w,
            NaiveDate::from_ymd_opt(1900, 1, 1).unwrap_or(window_start),
        );
    }

    let in_window = |d: NaiveDate| (window_start..=opts.today).contains(&d);
    let mut wellness: Vec<WellnessDay> = found
        .wellness
        .into_values()
        .filter(|r| in_window(r.date))
        .collect();
    let mut training: Vec<TrainingDay> = found
        .training
        .into_values()
        .filter(|r| in_window(r.date))
        .collect();
    let mut sensitive: Vec<SensitiveDay> = found
        .sensitive
        .into_values()
        .filter(|r| in_window(r.date))
        .collect();
    let mut loads: Vec<GarminActivityLoad> = found
        .loads
        .into_iter()
        .filter(|r| in_window(r.date))
        .collect();
    w.dropped += wellness.iter_mut().map(DayRecord::sanitize).sum::<u32>();
    w.dropped += training.iter_mut().map(DayRecord::sanitize).sum::<u32>();
    w.dropped += sensitive.iter_mut().map(DayRecord::sanitize).sum::<u32>();
    w.dropped += loads.iter_mut().map(DayRecord::sanitize).sum::<u32>();
    w.wellness_days = store.merge_garmin_rows(&wellness)?;
    w.training_days = store.merge_garmin_rows(&training)?;
    w.sensitive_days = store.merge_garmin_rows(&sensitive)?;
    w.garmin_loads = store.merge_garmin_rows(&loads)?;
    Ok(w)
}
