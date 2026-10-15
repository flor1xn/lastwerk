// Einrichtung des lokalen Coachs in der Attrappe (T29): ein erfundenes Ollama, das gefunden,
// "installiert" und mit Modellen gefüllt werden kann, ohne Prozess, Netz oder Platte. Die
// Einstufung der Modelle ist eine Kopie der Faustregeln aus crates/coach/src/recommend.rs; maßgeblich
// bleibt der Rust-Teil. Die Größen stammen aus catalog/models.json (ungeprüft, siehe dort).
//
// Varianten (`?ollama=fehlt|laeuft|laedt` im Browser):
// - missing: Ollama ist nicht installiert, "Jetzt einrichten" läuft in Schritten durch.
// - running: Ollama läuft mit `qwen3:4b` und `llama3.2:3b` (Standard).
// - loading: Ollama läuft ohne Modell, das Startmodell lädt langsam im Hintergrund.

import type { MockCoach } from './mock-coach';
import {
  BackendError,
  type CoachBusy,
  type CoachCatalogEntry,
  type CoachFit,
  type CoachPullPhase,
  type CoachPullProgress,
  type CoachSetupFileState,
  type CoachSetupProgress,
  type CoachSetupStatus,
  type OllamaState,
} from './types';

export type OllamaVariant = 'missing' | 'running' | 'loading';

export function isOllamaVariant(v: string | null): v is OllamaVariant {
  return v === 'missing' || v === 'running' || v === 'loading';
}

export interface MockSetupOptions {
  variant: OllamaVariant;
  /** Pause je Schritt in Millisekunden. */
  stepMs: number;
  /** Hardware des erfundenen Rechners. Standard 16 GB ohne Grafikkarte. */
  hardware?: { ram_gb: number; vram_gb: number | null };
  /** Kann die Attrappe Ollama "einrichten" (Windows)? Standard ja. */
  canInstall?: boolean;
}

interface CatalogRow {
  name: string;
  size_gb: number;
  min_ram_gb: number;
  comfortable_vram_gb: number;
  label_key: string;
  starter: boolean;
}

// Kopie von catalog/models.json. Ein Test hält sie gleich.
export const MOCK_CATALOG: CatalogRow[] = [
  {
    name: 'qwen3:1.7b',
    size_gb: 1.4,
    min_ram_gb: 4,
    comfortable_vram_gb: 2.5,
    label_key: 'ui.coach_setup.model.qwen3_1_7b',
    starter: false,
  },
  {
    name: 'qwen3:4b',
    size_gb: 2.5,
    min_ram_gb: 8,
    comfortable_vram_gb: 4,
    label_key: 'ui.coach_setup.model.qwen3_4b',
    starter: true,
  },
  {
    name: 'qwen3:8b',
    size_gb: 5.2,
    min_ram_gb: 16,
    comfortable_vram_gb: 7,
    label_key: 'ui.coach_setup.model.qwen3_8b',
    starter: false,
  },
  {
    name: 'qwen3:14b',
    size_gb: 9.3,
    min_ram_gb: 24,
    comfortable_vram_gb: 12,
    label_key: 'ui.coach_setup.model.qwen3_14b',
    starter: false,
  },
];

/** Toleranz des Arbeitsspeichers und größte CPU-taugliche Größe, wie in recommend.rs. */
const RAM_MARGIN_GB = 0.5;
const CPU_FIT_MAX_GB = 3.0;

export function mockFit(
  hw: { ram_gb: number; vram_gb: number | null },
  m: Pick<CatalogRow, 'size_gb' | 'min_ram_gb' | 'comfortable_vram_gb'>,
): CoachFit {
  if (hw.ram_gb + RAM_MARGIN_GB < m.min_ram_gb) return 'too_big';
  const onGpu = hw.vram_gb !== null && hw.vram_gb >= m.comfortable_vram_gb;
  return onGpu || m.size_gb <= CPU_FIT_MAX_GB ? 'fits' : 'slow';
}

const err = (code: string) => new BackendError(code, `ui.error.${code}`);
const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));
const NAME = /^[a-z0-9][a-z0-9._-]*(:[a-z0-9._-]+)?$/;

/** Schritte der Attrappe bei der Einrichtung, wie die Zeilen des Skripts (Vertrag, T29). */
const INSTALL_STEPS: CoachSetupProgress[] = [
  { phase: 'detect', value: 'missing', percent: null },
  { phase: 'install', value: 'winget', percent: null },
  { phase: 'install', value: 'verify_ok', percent: null },
  { phase: 'start', value: 'ok', percent: null },
  { phase: 'done', value: 'ok', percent: null },
];

const PULL_PHASES: CoachPullPhase[] = ['manifest', 'download', 'verify', 'write', 'done'];

export class MockCoachSetup {
  private ollama: OllamaState;
  private models: string[];
  private busy: CoachBusy | null = null;
  private cancelled = false;
  private fileState: CoachSetupFileState | null = null;
  private readonly hardware: { ram_gb: number; vram_gb: number | null };
  private readonly canInstall: boolean;

  constructor(
    private readonly coach: MockCoach,
    private readonly options: MockSetupOptions,
  ) {
    this.hardware = options.hardware ?? { ram_gb: 16, vram_gb: null };
    this.canInstall = options.canInstall ?? true;
    this.ollama = options.variant === 'missing' ? 'missing' : 'running';
    this.models = options.variant === 'running' ? ['qwen3:4b', 'llama3.2:3b'] : [];
    this.coach.setAvailable(this.ollama === 'running');
    this.coach.setModels(this.models);
    if (options.variant === 'loading') {
      // Das Startmodell lädt schon, wenn die Seite öffnet. Fehler ignoriert die Attrappe.
      void this.pullModel('qwen3:4b', () => {}, this.options.stepMs * 4).catch(() => {});
    }
  }

  status(): CoachSetupStatus {
    const recommended = this.recommended();
    const catalog: CoachCatalogEntry[] = MOCK_CATALOG.map((m) => ({
      name: m.name,
      size_gb: m.size_gb,
      label_key: m.label_key,
      starter: m.starter,
      fit: mockFit(this.hardware, m),
      recommended: m.name === recommended,
      installed: this.models.includes(m.name),
    }));
    const model = this.coach.status().model;
    return {
      ollama: this.ollama,
      can_install: this.canInstall,
      models: [...this.models],
      model: this.ollama === 'running' ? model : null,
      hardware: { ...this.hardware },
      catalog,
      recommended,
      state: this.fileState,
      busy: this.busy ? { ...this.busy } : null,
    };
  }

  /** Das größte passende Modell, sonst das Startmodell (wie `recommend`). */
  private recommended(): string | null {
    const fits = MOCK_CATALOG.filter((m) => mockFit(this.hardware, m) === 'fits');
    const best = fits.sort((a, b) => b.size_gb - a.size_gb)[0];
    return best?.name ?? MOCK_CATALOG.find((m) => m.starter)?.name ?? null;
  }

  async install(onProgress: (p: CoachSetupProgress) => void): Promise<CoachSetupStatus> {
    if (!this.canInstall) throw err('coach_setup_unsupported');
    if (this.busy) throw err('coach_setup_busy');
    this.busy = { kind: 'install', name: null, phase: null, completed: 0, total: 0 };
    try {
      for (const step of INSTALL_STEPS) {
        await sleep(this.options.stepMs);
        onProgress(step);
      }
      this.ollama = 'running';
      this.coach.setAvailable(true);
      this.coach.setModels(this.models);
      this.fileState = {
        decision: 'yes',
        ollama_found: true,
        installed_via: 'winget',
        model: null,
        model_ready: false,
        error_code: null,
        updated_at: new Date().toISOString().replace(/\.\d+Z$/, 'Z'),
      };
    } finally {
      this.busy = null;
    }
    return this.status();
  }

  pull(name: string, onProgress: (p: CoachPullProgress) => void): Promise<CoachSetupStatus> {
    return this.pullModel(name, onProgress, this.options.stepMs);
  }

  private async pullModel(
    name: string,
    onProgress: (p: CoachPullProgress) => void,
    stepMs: number,
  ): Promise<CoachSetupStatus> {
    if (!NAME.test(name) || name.length > 200) throw err('coach_pull_invalid_model');
    if (!MOCK_CATALOG.some((m) => m.name === name) && !this.models.includes(name)) {
      throw err('coach_pull_invalid_model');
    }
    if (this.busy) throw err('coach_setup_busy');
    if (this.ollama !== 'running') throw err('coach_pull_unreachable');
    const size = (MOCK_CATALOG.find((m) => m.name === name)?.size_gb ?? 2) * 1e9;
    this.busy = { kind: 'pull', name, phase: 'manifest', completed: 0, total: 0 };
    this.cancelled = false;
    try {
      const steps = 20;
      const report = (phase: CoachPullPhase, completed: number, total: number) => {
        if (this.busy) Object.assign(this.busy, { phase, completed, total });
        onProgress({ name, phase, completed, total });
      };
      for (const phase of PULL_PHASES) {
        if (phase === 'download') {
          for (let i = 0; i <= steps; i += 1) {
            await sleep(stepMs);
            if (this.cancelled) throw err('coach_pull_cancelled');
            report('download', Math.round((size * i) / steps), size);
          }
          continue;
        }
        await sleep(stepMs);
        if (this.cancelled) throw err('coach_pull_cancelled');
        report(phase, phase === 'manifest' ? 0 : size, phase === 'manifest' ? 0 : size);
      }
      if (!this.models.includes(name)) this.models.push(name);
      this.coach.setModels(this.models);
      this.coach.setModel(name);
    } finally {
      this.busy = null;
    }
    return this.status();
  }

  cancelPull(): void {
    if (this.busy?.kind === 'pull') this.cancelled = true;
  }
}
