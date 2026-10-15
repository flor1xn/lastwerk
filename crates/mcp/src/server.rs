//! MCP-Server auf Basis von `rmcp`: Werkzeugliste, Hinweise und Aufrufprotokoll.
//!
//! Die Werkzeuge selbst stehen in `tools.rs`. Hier liegt nur die Anbindung an das Protokoll:
//! Jedes Werkzeug trägt dieselben vier Hinweise (nur lesend, nicht zerstörend, wiederholbar,
//! geschlossene Welt), prüft nichts selbst und schreibt je Aufruf eine Zeile ins Protokoll.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::{DateTime, NaiveDate, Utc};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::CallToolResult;
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use serde::Serialize;
use serde_json::Value;

use crate::args::{
    ActivityArgs, FitnessArgs, ListActivitiesArgs, NoArgs, PlanWeekArgs, RecoveryArgs, RuleArgs,
    ThresholdsArgs, WeeklyLoadArgs,
};
use crate::calllog::{CallLog, CallRecord};
use crate::error::{ToolError, ToolResult};
use crate::tools::{self, Ctx};
use crate::views::{
    ActivityList, ActivityView, FitnessStatus, RecoveryDays, RowCount, RuleExplanation,
    ThresholdsView, WeeklyLoad,
};
use lastwerk_store::Store;

/// Uhr des Servers: liefert die aktuelle Zeit in UTC und das lokale Datum von heute. Ein
/// Parameter, damit Tests ein festes Datum vorgeben können.
pub type Clock = Arc<dyn Fn() -> (DateTime<Utc>, NaiveDate) + Send + Sync>;

/// Systemuhr: UTC für das Protokoll, lokales Datum für "heute".
pub fn system_clock() -> Clock {
    Arc::new(|| (Utc::now(), chrono::Local::now().date_naive()))
}

struct Inner {
    store: Mutex<Store>,
    log: CallLog,
    clock: Clock,
}

/// Der Server. Billig zu kopieren: Zustand und Router liegen hinter `Arc`.
#[derive(Clone)]
pub struct LastwerkMcp {
    inner: Arc<Inner>,
    tool_router: ToolRouter<Self>,
}

impl LastwerkMcp {
    /// Server über einer lesenden Verbindung. Die Uhr ist die Systemuhr.
    pub fn new(store: Store, log: CallLog) -> Self {
        Self::with_clock(store, log, system_clock())
    }

    /// Wie [`LastwerkMcp::new`] mit einer Uhr, die der Aufrufer vorgibt.
    pub fn with_clock(store: Store, log: CallLog, clock: Clock) -> Self {
        Self {
            inner: Arc::new(Inner {
                store: Mutex::new(store),
                log,
                clock,
            }),
            tool_router: Self::tool_router(),
        }
    }

    /// Die Verbindung, die auch die Werkzeuge nutzen. Für Diagnose und Tests: Ein Test
    /// versucht darüber ein `INSERT` und erwartet einen Fehler, weil die Verbindung
    /// schreibgeschützt geöffnet ist (SPEC 11.4).
    pub fn lock_store(&self) -> std::sync::MutexGuard<'_, Store> {
        self.inner
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Alle Werkzeuge in der Reihenfolge von SPEC 5.4 (der Router liefert sie sortiert nach
    /// Name). Ein Werkzeug außerhalb von [`tools::TOOL_NAMES`] kommt ans Ende, damit es
    /// auffällt.
    pub fn tool_list() -> Vec<rmcp::model::Tool> {
        let mut list = Self::tool_router().list_all();
        list.sort_by_key(|t| {
            tools::TOOL_NAMES
                .iter()
                .position(|n| *n == t.name)
                .unwrap_or(usize::MAX)
        });
        list
    }

    /// Führt ein Werkzeug aus und protokolliert den Aufruf. Die Parameter sind schon gegen
    /// ihren Typ geprüft. Das Protokoll bekommt die Parameter, die Zahl der Zeilen und den
    /// Ausgang, nie die Antwort.
    fn run<P, T>(
        &self,
        tool: &str,
        params: &P,
        f: impl FnOnce(Ctx<'_>, &P) -> ToolResult<T>,
        rows: impl FnOnce(&T) -> usize,
    ) -> Result<T, String>
    where
        P: Serialize,
    {
        let started = Instant::now();
        let (now, today) = (self.inner.clock)();
        let outcome = match self.inner.store.lock() {
            Ok(store) => f(
                Ctx {
                    store: &store,
                    today,
                },
                params,
            ),
            Err(_) => Err(ToolError::Internal),
        };
        let (count, status) = match &outcome {
            Ok(v) => (rows(v), "ok"),
            Err(e) => (0, e.code()),
        };
        self.inner.log.record(&CallRecord::new(
            now,
            tool,
            serde_json::to_value(params).unwrap_or(Value::Null),
            count,
            started.elapsed().as_millis() as u64,
            status,
        ));
        outcome.map_err(|e| e.message())
    }

    /// Wie [`Self::run`] für Werkzeuge, deren Antwort ein Typ mit [`RowCount`] ist.
    fn run_typed<P, T>(
        &self,
        tool: &str,
        params: &P,
        f: impl FnOnce(Ctx<'_>, &P) -> ToolResult<T>,
    ) -> Result<Json<T>, String>
    where
        P: Serialize,
        T: RowCount,
    {
        self.run(tool, params, f, RowCount::row_count).map(Json)
    }

    /// Wie [`Self::run`] für Werkzeuge, die JSON nach `plan.schema.json` liefern.
    fn run_plan<P>(
        &self,
        tool: &str,
        params: &P,
        f: impl FnOnce(Ctx<'_>, &P) -> ToolResult<Value>,
    ) -> Result<CallToolResult, String>
    where
        P: Serialize,
    {
        self.run(tool, params, f, tools::plan_rows)
            .map(CallToolResult::structured)
    }
}

// Die Hinweise (SPEC 5.4): alle Werkzeuge lesen nur, ändern nichts, liefern bei gleichen
// Argumenten dasselbe und sprechen nur mit der lokalen Datenbank.
#[tool_router]
impl LastwerkMcp {
    #[tool(
        name = "get_fitness_status",
        description = "Fitness (CTL), fatigue (ATL) and form (TSB) for each day in a date \
                       range of at most 366 days. TSB is CTL minus ATL of the previous day. \
                       Days before the first recorded load and days in the future are left out.",
        annotations(
            title = "Fitness status",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub fn get_fitness_status(
        &self,
        Parameters(args): Parameters<FitnessArgs>,
    ) -> Result<Json<FitnessStatus>, String> {
        self.run_typed("get_fitness_status", &args, tools::get_fitness_status)
    }

    #[tool(
        name = "get_weekly_load",
        description = "Training load and duration per calendar week and sport for the last \
                       1 to 52 weeks including the current week. Weeks without counted \
                       activities are left out.",
        annotations(
            title = "Weekly load",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub fn get_weekly_load(
        &self,
        Parameters(args): Parameters<WeeklyLoadArgs>,
    ) -> Result<Json<WeeklyLoad>, String> {
        self.run_typed("get_weekly_load", &args, tools::get_weekly_load)
    }

    #[tool(
        name = "list_activities",
        description = "Activities in a date range, oldest first, at most 200. Each has date, \
                       sport, duration, load, intensity factor and minutes per zone. No names, \
                       locations or device data.",
        annotations(
            title = "List activities",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub fn list_activities(
        &self,
        Parameters(args): Parameters<ListActivitiesArgs>,
    ) -> Result<Json<ActivityList>, String> {
        self.run_typed("list_activities", &args, tools::list_activities)
    }

    #[tool(
        name = "get_activity",
        description = "One activity by its number, like list_activities and additionally the \
                       best average power over 5 s, 1 min, 5 min, 20 min and 1 h.",
        annotations(
            title = "Get activity",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub fn get_activity(
        &self,
        Parameters(args): Parameters<ActivityArgs>,
    ) -> Result<Json<ActivityView>, String> {
        self.run_typed("get_activity", &args, tools::get_activity)
    }

    #[tool(
        name = "get_current_plan",
        description = "The active training plan as JSON (plan.schema.json): weeks, days, \
                       sessions with template, duration, planned load and reason keys. The \
                       free-text explanation is left out.",
        annotations(
            title = "Current plan",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub fn get_current_plan(
        &self,
        Parameters(args): Parameters<NoArgs>,
    ) -> Result<CallToolResult, String> {
        self.run_plan("get_current_plan", &args, tools::get_current_plan)
    }

    #[tool(
        name = "get_plan_week",
        description = "One week of the active training plan (the week that contains the \
                       given date).",
        annotations(
            title = "Plan week",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub fn get_plan_week(
        &self,
        Parameters(args): Parameters<PlanWeekArgs>,
    ) -> Result<CallToolResult, String> {
        self.run_plan("get_plan_week", &args, tools::get_plan_week)
    }

    #[tool(
        name = "get_thresholds",
        description = "Training thresholds (FTP, threshold heart rate, maximum and resting \
                       heart rate, running threshold speed, critical swim speed) valid on a \
                       date, per sport, each with the date it is valid from.",
        annotations(
            title = "Thresholds",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub fn get_thresholds(
        &self,
        Parameters(args): Parameters<ThresholdsArgs>,
    ) -> Result<Json<ThresholdsView>, String> {
        self.run_typed("get_thresholds", &args, tools::get_thresholds)
    }

    #[tool(
        name = "explain_rule",
        description = "Explains one planning rule (R01 to R12): title, text, whether it is \
                       hard or soft, its numeric parameters and its source or the label \
                       'Faustregel' (rule of thumb).",
        annotations(
            title = "Explain rule",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub fn explain_rule(
        &self,
        Parameters(args): Parameters<RuleArgs>,
    ) -> Result<Json<RuleExplanation>, String> {
        self.run_typed("explain_rule", &args, tools::explain_rule)
    }

    #[tool(
        name = "get_recovery_days",
        description = "Overnight HRV, resting heart rate and sleep duration per day, at most \
                       90 days. Blocked unless the user has enabled it; then it returns an \
                       error.",
        annotations(
            title = "Recovery days",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub fn get_recovery_days(
        &self,
        Parameters(args): Parameters<RecoveryArgs>,
    ) -> Result<Json<RecoveryDays>, String> {
        self.run_typed("get_recovery_days", &args, tools::get_recovery_days)
    }
}

// Name und Version (CARGO_PKG_VERSION) setzt der Makro, der Hinweistext steht hier.
#[tool_handler(
    router = self.tool_router,
    name = "lastwerk-mcp",
    instructions = "Read-only training data of one athlete: fitness (CTL), fatigue (ATL) and \
                    form (TSB), weekly load, single activities without names or locations, the \
                    active training plan, thresholds and the planning rules. Dates are \
                    YYYY-MM-DD. Missing data is reported as null or left out, never as zero."
)]
impl ServerHandler for LastwerkMcp {}
