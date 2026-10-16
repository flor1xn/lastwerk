//! Phasen "Konto" und "Tageswerte" des Abgleichs (T39, SPEC 5.6, 8.6).
//!
//! - **Konto:** Profil (Geschlecht, Geburtsjahr, VO2max), FTP, Laktatschwelle (Herzfrequenz
//!   und seit T40 Schwellentempo), Herzfrequenzzonen und seit T40 der Verlauf von FTP,
//!   Laktatschwellen-Herzfrequenz und Schwellentempo im ganzen Fenster. Daraus werden Schwellen
//!   mit Herkunft `garmin` und, solange der Nutzer sie nicht selbst gesetzt hat, Geschlecht und
//!   Geburtsjahr.
//! - **Tageswerte:** Schritte, Bergwert, VO2max-Verlauf, Garmins Last je Aktivität und (nur mit
//!   Freigabe) Gewicht und Blutdruck über Bereichsabfragen im ganzen Fenster;
//!   Tagesherzfrequenz, Atmung, SpO2, Intensitätsminuten, Stress, Trainingsbereitschaft,
//!   Trainingsstatus, Etagen und (nur mit Freigabe) Hydration und Ernährung je Tag für die
//!   letzten [`DETAIL_DAYS`] Tage; Lauftoleranz und Rennprognosen für diese Tage, Fitnessalter,
//!   Ausdauerwert und Belastungsfokus einmal je Lauf.
//!
//! Seit T40 läuft die Phase "Konto" zuerst, und ihre Schwellen werden vor dem Import
//! geschrieben ([`write_account`]), damit neue Dateien gleich mit ihnen gerechnet werden und die
//! Last früh da ist. Was sich an den Schwellen ändert, rechnet der Abgleich danach für bereits
//! bekannte Aktivitäten neu. Die Tageswerte schreibt [`write_days`] am Ende.

use std::collections::BTreeMap;

use chrono::{NaiveDate, TimeDelta};
use lastwerk_core::Sex;
use lastwerk_garmin::{
    MAX_LOAD_SPAN_DAYS, MAX_RANGE_SPAN_DAYS, MAX_SCORE_SPAN_DAYS, MAX_STEPS_SPAN_DAYS,
    MAX_TOLERANCE_SPAN_DAYS, SensitiveConsent, SeriesSport, ThresholdSeries,
};
use lastwerk_store::{
    BodyDay, BpDay, DayRecord, GARMIN_APP_SOURCE, GarminActivityLoad, GarminThresholdOutcome,
    NutritionDay, SETTING_ATHLETE_BIRTH_YEAR, SETTING_ATHLETE_BIRTH_YEAR_ORIGIN,
    SETTING_ATHLETE_SEX, SETTING_ATHLETE_SEX_ORIGIN, SensitiveCategory, SensitiveDay, Store,
    StoreError, TrainingDay, WellnessDay,
};

use super::parse_full::{self as pf, GarminThreshold, ProfileValues};
use super::{Run, Stop, SyncOptions, SyncPhase, SyncProgress, chunks};

/// Tage am Ende des Fensters, für die Tageswerte mit einem Aufruf je Tag geholt werden
/// (Faustregel wie [`super::SLEEP_DAYS`]: beim ersten Abgleich wären es sonst über 2500
/// Aufrufe, rund 90 Minuten bei 2 s je Aufruf).
pub const DETAIL_DAYS: i64 = 28;

/// Verläufe der Schwellen, die die Phase "Konto" holt: Kennzahl und Sportart der Abfrage.
const THRESHOLD_SERIES: [(ThresholdSeries, SeriesSport); 3] = [
    (
        ThresholdSeries::FunctionalThresholdPower,
        SeriesSport::Cycling,
    ),
    (
        ThresholdSeries::LactateThresholdHeartRate,
        SeriesSport::Running,
    ),
    (ThresholdSeries::LactateThresholdSpeed, SeriesSport::Running),
];

/// Freigaben der S3-Bereiche (Entscheidung E7). Ein Marker existiert nur, wenn der Nutzer den
/// Bereich in den Einstellungen freigegeben hat; ohne ihn gibt es keinen Aufruf.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Consents {
    pub hydration: Option<SensitiveConsent>,
    pub body: Option<SensitiveConsent>,
    pub blood_pressure: Option<SensitiveConsent>,
    pub nutrition: Option<SensitiveConsent>,
}

impl Consents {
    /// Liest die Freigaben aus den Einstellungen (nur genau `true` gibt frei).
    pub fn read(store: &Store) -> Result<Self, StoreError> {
        let one = |c| -> Result<Option<SensitiveConsent>, StoreError> {
            Ok(store
                .sensitive_released(c)?
                .then(SensitiveConsent::granted_by_user))
        };
        Ok(Self {
            hydration: one(SensitiveCategory::Hydration)?,
            body: one(SensitiveCategory::Body)?,
            blood_pressure: one(SensitiveCategory::BloodPressure)?,
            nutrition: one(SensitiveCategory::Nutrition)?,
        })
    }

    pub fn any(&self) -> bool {
        self.hydration.is_some()
            || self.body.is_some()
            || self.blood_pressure.is_some()
            || self.nutrition.is_some()
    }
}

/// Was die Phasen "Konto" und "Tageswerte" gesammelt haben.
#[derive(Default)]
pub(super) struct FullFound {
    pub profile: ProfileValues,
    pub thresholds: Vec<GarminThreshold>,
    pub wellness: BTreeMap<NaiveDate, WellnessDay>,
    pub training: BTreeMap<NaiveDate, TrainingDay>,
    pub sensitive: BTreeMap<NaiveDate, SensitiveDay>,
    pub body: BTreeMap<NaiveDate, BodyDay>,
    pub bp: BTreeMap<NaiveDate, BpDay>,
    pub nutrition: BTreeMap<NaiveDate, NutritionDay>,
    pub loads: Vec<GarminActivityLoad>,
}

/// Ergebnis von [`write_account`] und [`write_days`].
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

    /// Phase "Konto": vier Aufrufe ohne Datum und die Verläufe der Schwellen von `start` bis
    /// `today`.
    pub(super) fn account(
        &mut self,
        start: NaiveDate,
        today: NaiveDate,
        progress: &mut dyn FnMut(SyncProgress),
    ) -> Result<(), Stop> {
        let ranges = chunks(start, today, MAX_RANGE_SPAN_DAYS);
        let total = 4 + (THRESHOLD_SERIES.len() * ranges.len()) as u32;
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
        let mut done = 4;
        for (series, sport) in THRESHOLD_SERIES {
            for &(from, to) in &ranges {
                tick(done, progress);
                done += 1;
                if let Some(v) = self.call(|s| s.threshold_range(series, sport, from, to))? {
                    let found = pf::threshold_series(&v, series, &mut self.dropped);
                    self.full.thresholds.extend(
                        found
                            .into_iter()
                            .filter(|t| t.date.is_some_and(|d| (from..=to).contains(&d))),
                    );
                }
            }
        }
        tick(total, progress);
        Ok(())
    }

    /// Phase "Tageswerte". S3-Bereiche nur mit ihrem Marker in `consents`.
    pub(super) fn wellness(
        &mut self,
        start: NaiveDate,
        today: NaiveDate,
        consents: Consents,
        progress: &mut dyn FnMut(SyncProgress),
    ) -> Result<(), Stop> {
        let detail_start = start.max(today - TimeDelta::days(DETAIL_DAYS - 1));
        let steps = chunks(start, today, MAX_STEPS_SPAN_DAYS);
        let hills = chunks(start, today, MAX_SCORE_SPAN_DAYS);
        let loads = chunks(start, today, MAX_LOAD_SPAN_DAYS);
        let ranges = chunks(start, today, MAX_RANGE_SPAN_DAYS);
        let tolerance = chunks(detail_start, today, MAX_TOLERANCE_SPAN_DAYS);
        let endurance_start = start.max(today - TimeDelta::days(MAX_SCORE_SPAN_DAYS - 1));
        let per_day =
            8 + u32::from(consents.hydration.is_some()) + u32::from(consents.nutrition.is_some());
        let s3_ranges =
            u32::from(consents.body.is_some()) + u32::from(consents.blood_pressure.is_some());
        let detail_days = (today - detail_start).num_days() + 1;
        // Einmalig: Ausdauerwert, Fitnessalter, Belastungsfokus, Rennprognosen (zwei Aufrufe).
        let total = (steps.len() + hills.len() + loads.len() + tolerance.len() + 5) as u32
            + (1 + s3_ranges) * ranges.len() as u32
            + per_day * detail_days as u32;
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
        for &(from, to) in &ranges {
            tick(progress);
            if let Some(v) = self.call(|s| s.max_metrics_range(from, to))? {
                for (date, run, bike) in pf::vo2max_days(&v) {
                    if (from..=to).contains(&date) {
                        let day = self.training_day(date);
                        day.vo2max_running = run.or(day.vo2max_running);
                        day.vo2max_cycling = bike.or(day.vo2max_cycling);
                    }
                }
            }
        }
        tick(progress);
        if let Some(v) = self.call(|s| s.load_balance(today))?
            && let Some(b) = pf::load_balance(&v)
        {
            let day = self.training_day(today);
            [
                day.load_aerobic_low,
                day.load_aerobic_low_min,
                day.load_aerobic_low_max,
            ] = b.aerobic_low;
            [
                day.load_aerobic_high,
                day.load_aerobic_high_min,
                day.load_aerobic_high_max,
            ] = b.aerobic_high;
            [
                day.load_anaerobic,
                day.load_anaerobic_min,
                day.load_anaerobic_max,
            ] = b.anaerobic;
            day.load_balance = b.phrase;
        }
        for &(from, to) in &tolerance {
            tick(progress);
            if let Some(v) = self.call(|s| s.running_tolerance(from, to))? {
                for t in pf::running_tolerance(&v) {
                    if (from..=to).contains(&t.date) {
                        let day = self.training_day(t.date);
                        day.run_acute_distance_m = t.acute_distance_m;
                        day.run_impact_load = t.impact_load;
                        day.run_tolerance_m = t.tolerance_m;
                        day.run_tolerance_level = t.level;
                    }
                }
            }
        }
        // Rennprognosen: der Verlauf der Detailtage, danach der jüngste Stand, der für seinen Tag
        // gewinnt. Ein leerer oder fehlender Verlauf ist kein Fehler.
        let name = self.display_name.clone();
        for latest in [false, true] {
            tick(progress);
            let Some(name) = &name else { continue };
            let reply = if latest {
                self.call(|s| s.race_predictions_latest(name))?
            } else {
                self.call(|s| s.race_predictions_daily(name, detail_start, today))?
            };
            for (date, times) in reply.iter().flat_map(pf::race_predictions) {
                if (detail_start..=today).contains(&date) {
                    let day = self.training_day(date);
                    [
                        day.race_5k_s,
                        day.race_10k_s,
                        day.race_half_s,
                        day.race_marathon_s,
                    ] = times;
                }
            }
        }
        for &(from, to) in &ranges {
            if let Some(consent) = consents.body {
                tick(progress);
                if let Some(v) = self.call(|s| s.weight_range(consent, from, to))? {
                    for (date, values) in pf::body_days(&v) {
                        if (from..=to).contains(&date) {
                            let now = self.now;
                            let day =
                                self.full.body.entry(date).or_insert_with(|| {
                                    BodyDay::empty(date, GARMIN_APP_SOURCE, now)
                                });
                            [
                                day.weight_kg,
                                day.bmi,
                                day.body_fat_pct,
                                day.body_water_pct,
                                day.bone_kg,
                                day.muscle_kg,
                            ] = values;
                        }
                    }
                }
            }
            if let Some(consent) = consents.blood_pressure {
                tick(progress);
                if let Some(v) = self.call(|s| s.blood_pressure_range(consent, from, to))? {
                    for (date, values) in pf::blood_pressure_days(&v) {
                        if (from..=to).contains(&date) {
                            let now = self.now;
                            let day = self
                                .full
                                .bp
                                .entry(date)
                                .or_insert_with(|| BpDay::empty(date, GARMIN_APP_SOURCE, now));
                            [day.systolic_mmhg, day.diastolic_mmhg, day.pulse_bpm] = values;
                        }
                    }
                }
            }
        }

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
            tick(progress);
            if let Some(v) = self.call(|s| s.training_status_day(date))?
                && let Some(t) = pf::training_status(&v)
                && t.date.is_none_or(|d| d == date)
            {
                let day = self.training_day(date);
                day.training_status = t.status;
                day.garmin_acute_load = day.garmin_acute_load.or(t.acute);
                day.garmin_chronic_load = t.chronic;
                day.garmin_acwr = t.acwr;
            }
            tick(progress);
            if let Some(v) = self.call(|s| s.floors_day(date))?
                && let Some((up, down)) = pf::floors(&v)
            {
                let day = self.wellness_day(date);
                day.floors_up = Some(up);
                day.floors_down = Some(down);
            }
            if let Some(consent) = consents.nutrition {
                tick(progress);
                if let Some(v) = self.call(|s| s.nutrition_day(consent, date))?
                    && let Some((meal_date, values)) = pf::nutrition_day(&v)
                    && meal_date.is_none_or(|d| d == date)
                {
                    let now = self.now;
                    let day = self
                        .full
                        .nutrition
                        .entry(date)
                        .or_insert_with(|| NutritionDay::empty(date, GARMIN_APP_SOURCE, now));
                    [
                        day.calories_kcal,
                        day.carbs_g,
                        day.fat_g,
                        day.protein_g,
                        day.calories_goal_kcal,
                    ] = values;
                }
            }
            if let Some(consent) = consents.hydration {
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

/// Schreibt Schwellen und Athletendaten (siehe Moduldokumentation und SPEC 8.6), vor dem
/// Import. Leert `found.thresholds`.
pub(super) fn write_account(
    store: &mut Store,
    found: &mut FullFound,
    window_start: NaiveDate,
    opts: &SyncOptions,
) -> Result<FullWritten, StoreError> {
    let mut w = FullWritten::default();
    let earliest = |w: &mut FullWritten, d: NaiveDate| {
        w.recompute_from = Some(w.recompute_from.map_or(d, |e| e.min(d)));
    };

    for t in &std::mem::take(&mut found.thresholds) {
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
    Ok(w)
}

/// Bereinigt Zeilen im Fenster und mischt sie in ihre Tabelle. Gibt die Zahl der geänderten
/// Zeilen zurück und zählt verworfene Werte in `dropped`.
fn merge_window<T: DayRecord>(
    store: &mut Store,
    rows: impl IntoIterator<Item = T>,
    in_window: impl Fn(&T) -> bool,
    dropped: &mut u32,
) -> Result<u32, StoreError> {
    let mut rows: Vec<T> = rows.into_iter().filter(|r| in_window(r)).collect();
    *dropped += rows.iter_mut().map(DayRecord::sanitize).sum::<u32>();
    store.merge_garmin_rows(&rows)
}

/// Schreibt Tages-, Trainings- und S3-Werte und Garmins Last je Aktivität, nach allen Phasen.
pub(super) fn write_days(
    store: &mut Store,
    found: FullFound,
    window_start: NaiveDate,
    opts: &SyncOptions,
    w: &mut FullWritten,
) -> Result<(), StoreError> {
    let in_window = |d: NaiveDate| (window_start..=opts.today).contains(&d);
    let d = &mut w.dropped;
    w.wellness_days = merge_window(
        store,
        found.wellness.into_values(),
        |r| in_window(r.date),
        d,
    )?;
    w.training_days = merge_window(
        store,
        found.training.into_values(),
        |r| in_window(r.date),
        d,
    )?;
    w.sensitive_days =
        merge_window(
            store,
            found.sensitive.into_values(),
            |r| in_window(r.date),
            d,
        )? + merge_window(store, found.body.into_values(), |r| in_window(r.date), d)?
            + merge_window(store, found.bp.into_values(), |r| in_window(r.date), d)?
            + merge_window(
                store,
                found.nutrition.into_values(),
                |r| in_window(r.date),
                d,
            )?;
    w.garmin_loads = merge_window(store, found.loads, |r| in_window(r.date), d)?;
    Ok(())
}
