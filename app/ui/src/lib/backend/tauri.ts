// Umsetzung des Backends über Tauri: ein `invoke` je Befehl, Fortschritt über ein Ereignis.
// Die Befehle und ihre Namen stehen in app/src-tauri/src/lib.rs (COMMAND_NAMES), das
// Frontend darf nur diese aufrufen (Capabilities, SPEC 10.4).

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  BackendError,
  type ActivityDetail,
  type ActivityFilter,
  type ActivityItem,
  type AddFitFilesResult,
  type AthleteInput,
  type AthleteSet,
  type AvailabilityDay,
  type Backend,
  type CoachPullProgress,
  type CoachReply,
  type CoachSetupProgress,
  type CoachSetupStatus,
  type CoachStatus,
  type Dashboard,
  type Performance,
  type PerfRange,
  type Progress,
  type RecoveryPage,
  type RecoveryRange,
  type HealthPage,
  type SensitiveCategory,
  type HealthRange,
  type ExportResult,
  type GeneratePlanInput,
  type GoalInput,
  type PlanDetail,
  type PlanListItem,
  type PlanStatus,
  type PlanningInputs,
  type PromptPreview,
  type QuickPlanInput,
  type RestoreResult,
  type BridgeStatus,
  type UpdateInfo,
  type ImportProgress,
  type ImportSummary,
  type Settings,
  type SettingsPatch,
  type ThresholdInput,
  type ThresholdList,
  type ThresholdSet,
  type WeekOverview,
  type GarminLoginResult,
  type GarminStatus,
  type GarminSyncProgress,
  type GarminSyncSummary,
  type GarminSyncTrigger,
} from './types';

export const EVENT_IMPORT_PROGRESS = 'import-progress';
export const EVENT_GARMIN_LOGIN = 'garmin-login';
export const EVENT_GARMIN_SYNC_PROGRESS = 'garmin-sync-progress';
export const EVENT_COACH_SETUP_PROGRESS = 'coach-setup-progress';
export const EVENT_COACH_PULL_PROGRESS = 'coach-pull-progress';

/** Wandelt das Fehlerobjekt `{ code, message_key }` des Rust-Teils in einen BackendError. */
export function toBackendError(e: unknown): BackendError {
  if (e && typeof e === 'object') {
    const o = e as { code?: unknown; message_key?: unknown };
    if (typeof o.code === 'string') {
      return new BackendError(
        o.code,
        typeof o.message_key === 'string' ? o.message_key : `ui.error.${o.code}`,
      );
    }
  }
  return new BackendError('internal', 'ui.error.internal');
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    throw toBackendError(e);
  }
}

export class TauriBackend implements Backend {
  readonly kind = 'tauri' as const;

  getSettings(): Promise<Settings> {
    return call('get_settings');
  }

  setSettings(patch: SettingsPatch): Promise<Settings> {
    return call('set_settings', { patch });
  }

  async importFolder(onProgress: (p: ImportProgress) => void): Promise<ImportSummary> {
    return this.withImportProgress(onProgress, () => call<ImportSummary>('import_folder'));
  }

  addFitFiles(onProgress: (p: ImportProgress) => void): Promise<AddFitFilesResult> {
    return this.withImportProgress(onProgress, () => call<AddFitFilesResult>('add_fit_files'));
  }

  /** Hört während des Aufrufs auf das Fortschrittsereignis des Imports. */
  private async withImportProgress<T>(
    onProgress: (p: ImportProgress) => void,
    run: () => Promise<T>,
  ): Promise<T> {
    const unlisten = await listen<ImportProgress>(EVENT_IMPORT_PROGRESS, (e) =>
      onProgress(e.payload),
    );
    try {
      return await run();
    } finally {
      unlisten();
    }
  }

  getWeekOverview(weekStart: string, pmcWeeks: number): Promise<WeekOverview> {
    return call('get_week_overview', { week_start: weekStart, pmc_weeks: pmcWeeks });
  }

  getDashboard(today: string): Promise<Dashboard> {
    return call('get_dashboard', { today });
  }

  getProgress(today: string): Promise<Progress> {
    return call('get_progress', { today });
  }

  getPerformance(today: string, range: PerfRange): Promise<Performance> {
    return call('get_performance', { today, range });
  }

  getRecovery(today: string, days: RecoveryRange): Promise<RecoveryPage> {
    return call('get_recovery', { today, days });
  }

  getHealth(today: string, days: HealthRange): Promise<HealthPage> {
    return call('get_health', { today, days });
  }

  deleteSensitiveData(category?: SensitiveCategory): Promise<number> {
    return call('delete_sensitive_data', { category: category ?? null });
  }

  listActivities(filter: ActivityFilter): Promise<ActivityItem[]> {
    return call('list_activities', { filter });
  }

  getActivity(id: number): Promise<ActivityDetail> {
    return call('get_activity', { id });
  }

  listThresholds(): Promise<ThresholdList> {
    return call('list_thresholds');
  }

  setThreshold(input: ThresholdInput): Promise<ThresholdSet> {
    return call('set_threshold', { input });
  }

  setAthlete(input: AthleteInput): Promise<AthleteSet> {
    return call('set_athlete', { input });
  }

  listPlans(): Promise<PlanListItem[]> {
    return call('list_plans');
  }

  getPlan(id: string): Promise<PlanDetail> {
    return call('get_plan', { id });
  }

  generatePlan(input: GeneratePlanInput): Promise<PlanDetail> {
    return call('generate_plan', { input });
  }

  setPlanStatus(id: string, status: PlanStatus): Promise<PlanListItem[]> {
    return call('set_plan_status', { id, status });
  }

  getPlanningInputs(): Promise<PlanningInputs> {
    return call('get_planning_inputs');
  }

  setGoal(input: GoalInput): Promise<PlanningInputs> {
    return call('set_goal', { input });
  }

  setAvailability(days: AvailabilityDay[]): Promise<PlanningInputs> {
    return call('set_availability', { days });
  }

  quickPlan(input: QuickPlanInput): Promise<PlanDetail> {
    return call('quick_plan', { input });
  }

  coachStatus(): Promise<CoachStatus> {
    return call('coach_status');
  }

  coachSetModel(model: string): Promise<CoachStatus> {
    return call('coach_set_model', { model });
  }

  coachPreview(message: string): Promise<PromptPreview> {
    return call('coach_preview', { message });
  }

  coachChat(message: string): Promise<CoachReply> {
    return call('coach_chat', { message });
  }

  coachApply(proposalId: string): Promise<PlanDetail> {
    return call('coach_apply', { proposal_id: proposalId });
  }

  coachReset(): Promise<void> {
    return call('coach_reset');
  }

  coachCancel(): Promise<void> {
    return call('coach_cancel');
  }

  coachSetupStatus(): Promise<CoachSetupStatus> {
    return call('coach_setup_status');
  }

  async coachInstallOllama(onProgress: (p: CoachSetupProgress) => void): Promise<CoachSetupStatus> {
    const unlisten = await listen<CoachSetupProgress>(EVENT_COACH_SETUP_PROGRESS, (e) =>
      onProgress(e.payload),
    );
    try {
      return await call<CoachSetupStatus>('coach_install_ollama');
    } finally {
      unlisten();
    }
  }

  async coachPullModel(
    name: string,
    onProgress: (p: CoachPullProgress) => void,
  ): Promise<CoachSetupStatus> {
    const unlisten = await listen<CoachPullProgress>(EVENT_COACH_PULL_PROGRESS, (e) =>
      onProgress(e.payload),
    );
    try {
      return await call<CoachSetupStatus>('coach_pull_model', { name });
    } finally {
      unlisten();
    }
  }

  coachPullCancel(): Promise<void> {
    return call('coach_pull_cancel');
  }

  exportPlanIcs(id: string): Promise<ExportResult> {
    return call('export_plan_ics', { id });
  }

  exportPlanJson(id: string): Promise<ExportResult> {
    return call('export_plan_json', { id });
  }

  exportBackup(): Promise<ExportResult> {
    return call('export_backup');
  }

  restoreBackup(confirmed: boolean): Promise<RestoreResult> {
    return call('restore_backup', { confirmed });
  }

  getBridgeStatus(): Promise<BridgeStatus> {
    return call('get_bridge_status');
  }

  checkUpdate(): Promise<UpdateInfo> {
    return call('check_update');
  }

  installUpdate(confirmed: boolean): Promise<void> {
    return call('install_update', { confirmed });
  }

  // --- Garmin (T23) ----------------------------------------------------------------------

  garminStatus(): Promise<GarminStatus> {
    return call('garmin_status');
  }

  /** Der Befehl öffnet nur das Fenster, das Ergebnis kommt als Ereignis `garmin-login`. */
  async garminLogin(): Promise<GarminLoginResult> {
    let resolve: (r: GarminLoginResult) => void = () => {};
    const result = new Promise<GarminLoginResult>((r) => (resolve = r));
    const unlisten = await listen<GarminLoginResult>(EVENT_GARMIN_LOGIN, (e) => resolve(e.payload));
    try {
      await call<void>('garmin_login');
      return await result;
    } finally {
      unlisten();
    }
  }

  async garminSync(
    trigger: GarminSyncTrigger,
    onProgress: (p: GarminSyncProgress) => void,
  ): Promise<GarminSyncSummary | null> {
    const unlisten = await listen<GarminSyncProgress>(EVENT_GARMIN_SYNC_PROGRESS, (e) =>
      onProgress(e.payload),
    );
    try {
      return await call<GarminSyncSummary | null>('garmin_sync', { trigger });
    } finally {
      unlisten();
    }
  }

  garminLogout(): Promise<void> {
    return call('garmin_logout');
  }
}
