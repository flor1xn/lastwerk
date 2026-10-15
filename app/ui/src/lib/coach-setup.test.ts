import { get } from 'svelte/store';
import { afterEach, describe, expect, it } from 'vitest';
import models from '../../../../catalog/models.json';
import { setBackend } from './backend';
import { MockBackend } from './backend/mock';
import { MOCK_CATALOG, mockFit } from './backend/mock-coach-setup';
import type { CoachCatalogEntry, CoachSetupStatus } from './backend/types';
import {
  badgesFor,
  cancelPull,
  coachSetupError,
  coachSetupOp,
  coachSetupPulled,
  coachSetupStatus,
  gbText,
  installOllama,
  installPhaseText,
  percentOf,
  pullModel,
  refreshCoachSetup,
  resetCoachSetupState,
  stateErrorKey,
  stepState,
  useModel,
} from './coach-setup';
import { hasKey, translate } from './i18n';

afterEach(() => {
  setBackend(null);
  resetCoachSetupState();
});

function entry(over: Partial<CoachCatalogEntry> = {}): CoachCatalogEntry {
  return {
    name: 'qwen3:4b',
    size_gb: 2.5,
    label_key: 'ui.coach_setup.model.qwen3_4b',
    starter: true,
    fit: 'fits',
    recommended: false,
    installed: false,
    ...over,
  };
}

function status(over: Partial<CoachSetupStatus> = {}): CoachSetupStatus {
  return {
    ollama: 'running',
    can_install: true,
    models: [],
    model: null,
    hardware: { ram_gb: 16, vram_gb: null },
    catalog: [],
    recommended: null,
    state: null,
    busy: null,
    ...over,
  };
}

describe('Zustand des Schritts', () => {
  const idle = { kind: 'idle' } as const;

  it('folgt Ollama, Modell und laufendem Auftrag', () => {
    expect(stepState(null, idle)).toBe('loading');
    expect(stepState(status({ ollama: 'missing' }), idle)).toBe('installable');
    expect(stepState(status({ ollama: 'installed' }), idle)).toBe('installable');
    expect(stepState(status({ ollama: 'missing', can_install: false }), idle)).toBe('manual');
    expect(stepState(status({ ollama: 'installed', can_install: false }), idle)).toBe('manual');
    expect(stepState(status(), idle)).toBe('no_model');
    expect(stepState(status({ model: 'qwen3:4b', models: ['qwen3:4b'] }), idle)).toBe('ready');
  });

  it('ein laufender Auftrag geht vor, auch ohne Status', () => {
    expect(stepState(null, { kind: 'install', progress: null })).toBe('installing');
    expect(stepState(status(), { kind: 'pull', name: 'qwen3:4b', progress: null })).toBe('pulling');
  });
});

describe('Marken der Modellliste', () => {
  it('empfohlen nur, wenn das Modell auf dem Rechner läuft', () => {
    expect(badgesFor(entry({ recommended: true }), null)).toEqual(['recommended']);
    // Fällt die Empfehlung auf das Startmodell zurück, obwohl es zu groß ist: nicht empfehlen.
    expect(badgesFor(entry({ recommended: true, fit: 'too_big' }), null)).toEqual(['too_big']);
  });

  it('langsam und zu groß bleiben sichtbar, das Modell bleibt wählbar', () => {
    expect(badgesFor(entry({ fit: 'slow' }), null)).toEqual(['slow']);
    expect(badgesFor(entry({ fit: 'too_big' }), null)).toEqual(['too_big']);
  });

  it('geladen und im Einsatz schließen sich aus', () => {
    expect(badgesFor(entry({ installed: true }), null)).toEqual(['installed']);
    expect(badgesFor(entry({ installed: true }), 'llama3.2:3b')).toEqual(['installed']);
    expect(badgesFor(entry({ installed: true }), 'qwen3:4b')).toEqual(['active']);
    expect(badgesFor(entry({ installed: true, recommended: true }), 'qwen3:4b')).toEqual([
      'recommended',
      'active',
    ]);
  });
});

describe('Zahlen und Texte', () => {
  it('Prozent: abgerundet, begrenzt, ohne Gesamtgröße null', () => {
    expect(percentOf(0, 0)).toBeNull();
    expect(percentOf(5, -1)).toBeNull();
    expect(percentOf(Number.NaN, 10)).toBeNull();
    expect(percentOf(0, 1000)).toBe(0);
    expect(percentOf(999, 1000)).toBe(99);
    expect(percentOf(1000, 1000)).toBe(100);
    expect(percentOf(2000, 1000)).toBe(100);
    expect(percentOf(-5, 1000)).toBe(0);
    // 1 234 567 890 von 2 500 000 000 sind 49,38 Prozent.
    expect(percentOf(1_234_567_890, 2_500_000_000)).toBe(49);
  });

  it('Gigabyte dezimal mit einer Stelle, Komma auf Deutsch', () => {
    // 2 500 000 000 Byte sind 2,5 GB.
    expect(gbText(2_500_000_000, 'de')).toBe('2,5');
    expect(gbText(2_500_000_000, 'en')).toBe('2.5');
    expect(gbText(1_449_999_999, 'de')).toBe('1,4');
  });

  it('Zeilen des Skripts haben einen Text in beiden Sprachen', () => {
    const lines = [
      ['detect', 'found'],
      ['detect', 'missing'],
      ['detect', 'running'],
      ['install', 'winget'],
      ['install', 'download'],
      ['install', 'verify_ok'],
      ['install', 'skipped'],
      ['start', 'ok'],
      ['start', 'timeout'],
      ['pull', 'start'],
      ['pull', 'done'],
      ['pull', 'skipped'],
      ['done', 'ok'],
    ] as const;
    for (const [phase, value] of lines) {
      const [key] = installPhaseText({ phase, value, percent: null });
      expect(key, `${phase}:${value}`).not.toBe('ui.coach_setup.installing');
      expect(hasKey('de', key) && hasKey('en', key), key).toBe(true);
    }
    const [key, params] = installPhaseText({ phase: 'pull', value: 'progress', percent: 37 });
    expect(translate('de', key, params)).toBe('Modell wird geladen: 37 %');
  });

  it('ein unbekannter Wert zeigt den allgemeinen Text, nie sich selbst', () => {
    const [key] = installPhaseText({ phase: 'install', value: 'C:\\geheim', percent: null });
    expect(key).toBe('ui.coach_setup.installing');
  });

  it('Fehlercodes der Zustandsdatei', () => {
    for (const code of [
      'winget_failed',
      'download_failed',
      'signature_invalid',
      'install_failed',
      'start_timeout',
      'pull_failed',
      'bad_args',
      'no_windows',
    ]) {
      expect(stateErrorKey(code), code).toBe(`ui.error.coach_setup_${code}`);
      expect(hasKey('de', stateErrorKey(code)) && hasKey('en', stateErrorKey(code))).toBe(true);
    }
    expect(stateErrorKey('unknown')).toBe('ui.error.coach_setup_failed');
    expect(stateErrorKey('../x')).toBe('ui.error.coach_setup_failed');
  });
});

describe('Attrappe', () => {
  it('der Katalog der Attrappe ist der von catalog/models.json', () => {
    expect(
      MOCK_CATALOG.map((m) => ({
        name: m.name,
        size_gb: m.size_gb,
        min_ram_gb: m.min_ram_gb,
        comfortable_vram_gb: m.comfortable_vram_gb,
        label_key: m.label_key,
        starter: m.starter,
      })),
    ).toEqual(
      models.models.map((m) => ({
        starter: false,
        ...m,
      })),
    );
    expect(MOCK_CATALOG.filter((m) => m.starter).map((m) => m.name)).toEqual(['qwen3:4b']);
  });

  /** Dieselben Handrechnungen wie in crates/coach/src/recommend.rs. */
  it('stuft wie die Funktion im Rust-Teil ein', () => {
    const grade = (ram: number, vram: number | null) =>
      MOCK_CATALOG.map((m) => mockFit({ ram_gb: ram, vram_gb: vram }, m));
    expect(grade(8, null)).toEqual(['fits', 'fits', 'too_big', 'too_big']);
    expect(grade(7.6, null)).toEqual(['fits', 'fits', 'too_big', 'too_big']);
    expect(grade(7.4, null)).toEqual(['fits', 'too_big', 'too_big', 'too_big']);
    expect(grade(16, null)).toEqual(['fits', 'fits', 'slow', 'too_big']);
    expect(grade(16, 8)).toEqual(['fits', 'fits', 'fits', 'too_big']);
    expect(grade(32, null)).toEqual(['fits', 'fits', 'slow', 'slow']);
    expect(grade(32, 6)).toEqual(['fits', 'fits', 'slow', 'slow']);
    expect(grade(32, 12)).toEqual(['fits', 'fits', 'fits', 'fits']);
    expect(grade(3, null)).toEqual(['too_big', 'too_big', 'too_big', 'too_big']);
  });

  const recommendedOf = async (hw: { ram_gb: number; vram_gb: number | null }) => {
    setBackend(new MockBackend({ setupHardware: hw }));
    return (await refreshAndGet()).recommended;
  };

  async function refreshAndGet(): Promise<CoachSetupStatus> {
    await refreshCoachSetup();
    const s = get(coachSetupStatus);
    if (!s) throw new Error('kein Status');
    return s;
  }

  it('empfiehlt das größte Modell, das passt, sonst das Startmodell', async () => {
    expect(await recommendedOf({ ram_gb: 8, vram_gb: null })).toBe('qwen3:4b');
    expect(await recommendedOf({ ram_gb: 16, vram_gb: 8 })).toBe('qwen3:8b');
    expect(await recommendedOf({ ram_gb: 32, vram_gb: 12 })).toBe('qwen3:14b');
    expect(await recommendedOf({ ram_gb: 3, vram_gb: null })).toBe('qwen3:4b');
  });
});

describe('Abläufe', () => {
  it('Ollama einrichten: Schritte laufen durch, danach läuft Ollama ohne Modell', async () => {
    setBackend(new MockBackend({ ollama: 'missing' }));
    await refreshCoachSetup();
    expect(get(coachSetupStatus)?.ollama).toBe('missing');
    const seen: string[] = [];
    const unsub = coachSetupOp.subscribe((op) => {
      if (op.kind === 'install' && op.progress)
        seen.push(`${op.progress.phase}:${op.progress.value}`);
    });
    await installOllama();
    unsub();
    expect(seen).toEqual([
      'detect:missing',
      'install:winget',
      'install:verify_ok',
      'start:ok',
      'done:ok',
    ]);
    expect(get(coachSetupOp)).toEqual({ kind: 'idle' });
    const s = get(coachSetupStatus);
    expect(s?.ollama).toBe('running');
    expect(s?.model).toBeNull();
    expect(s?.state?.decision).toBe('yes');
    expect(get(coachSetupError)).toBeNull();
  });

  it('ohne Skript (kein Windows) steht ein Fehlertext, kein Absturz', async () => {
    setBackend(new MockBackend({ ollama: 'missing', setupCanInstall: false }));
    await refreshCoachSetup();
    await installOllama();
    expect(get(coachSetupError)).toBe('ui.error.coach_setup_unsupported');
    expect(get(coachSetupOp)).toEqual({ kind: 'idle' });
  });

  it('Modell laden: Fortschritt in Phasen, danach ist es das Modell des Coachs', async () => {
    const backend = new MockBackend({ ollama: 'running' });
    setBackend(backend);
    await refreshCoachSetup();
    const phases: string[] = [];
    let lastPercent = -1;
    const unsub = coachSetupOp.subscribe((op) => {
      if (op.kind !== 'pull' || !op.progress) return;
      if (phases.at(-1) !== op.progress.phase) phases.push(op.progress.phase);
      const p = percentOf(op.progress.completed, op.progress.total);
      if (p !== null) {
        // Der Fortschritt läuft nur vorwärts.
        expect(p).toBeGreaterThanOrEqual(lastPercent);
        lastPercent = p;
      }
    });
    await pullModel('qwen3:8b');
    unsub();
    expect(phases).toEqual(['manifest', 'download', 'verify', 'write', 'done']);
    expect(lastPercent).toBe(100);
    expect(get(coachSetupPulled)).toBe('qwen3:8b');
    expect(get(coachSetupStatus)?.model).toBe('qwen3:8b');
    expect((await backend.coachStatus()).model).toBe('qwen3:8b');
    expect(get(coachSetupStatus)?.catalog.find((e) => e.name === 'qwen3:8b')?.installed).toBe(true);
  });

  it('ein Name außerhalb von Katalog und installierten Modellen wird abgelehnt', async () => {
    const backend = new MockBackend({ ollama: 'running' });
    setBackend(backend);
    for (const bad of ['evil:latest', 'qwen3:32b', 'a b', '../x', '']) {
      await expect(backend.coachPullModel(bad, () => {})).rejects.toMatchObject({
        code: 'coach_pull_invalid_model',
      });
    }
  });

  it('ohne laufenden Ollama scheitert das Laden mit einem Text', async () => {
    setBackend(new MockBackend({ ollama: 'missing' }));
    await pullModel('qwen3:4b');
    expect(get(coachSetupError)).toBe('ui.error.coach_pull_unreachable');
    expect(get(coachSetupPulled)).toBeNull();
    expect(get(coachSetupOp)).toEqual({ kind: 'idle' });
  });

  it('Abbrechen beendet das Laden ohne Modell und mit dem Text "Abgebrochen"', async () => {
    const backend = new MockBackend({ ollama: 'running', setupStepMs: 5 });
    setBackend(backend);
    await refreshCoachSetup();
    const run = pullModel('qwen3:8b');
    await new Promise((r) => setTimeout(r, 30));
    await cancelPull();
    await run;
    expect(get(coachSetupError)).toBe('ui.error.coach_pull_cancelled');
    expect(get(coachSetupStatus)?.models).not.toContain('qwen3:8b');
    expect(get(coachSetupOp)).toEqual({ kind: 'idle' });
  });

  it('ein zweiter Auftrag während des ersten startet nicht', async () => {
    const backend = new MockBackend({ ollama: 'running', setupStepMs: 5 });
    setBackend(backend);
    const first = pullModel('qwen3:8b');
    await pullModel('qwen3:1.7b');
    await first;
    const s = await backend.coachSetupStatus();
    expect(s.models).toContain('qwen3:8b');
    expect(s.models).not.toContain('qwen3:1.7b');
    // Auf der Seite des Backends gilt dieselbe Sperre.
    const slow = new MockBackend({ ollama: 'running', setupStepMs: 5 });
    const a = slow.coachPullModel('qwen3:8b', () => {});
    await expect(slow.coachPullModel('qwen3:1.7b', () => {})).rejects.toMatchObject({
      code: 'coach_setup_busy',
    });
    await a;
  });

  it('ein geladenes Modell lässt sich als Modell des Coachs wählen', async () => {
    const backend = new MockBackend({ ollama: 'running' });
    setBackend(backend);
    await refreshCoachSetup();
    expect(get(coachSetupStatus)?.model).toBe('qwen3:4b');
    await useModel('llama3.2:3b');
    expect(get(coachSetupStatus)?.model).toBe('llama3.2:3b');
  });

  it('die Variante "laedt": Die Oberfläche folgt dem laufenden Auftrag', async () => {
    const backend = new MockBackend({ ollama: 'loading', setupStepMs: 5 });
    setBackend(backend);
    await refreshCoachSetup();
    const op = get(coachSetupOp);
    expect(op.kind).toBe('pull');
    if (op.kind === 'pull') expect(op.name).toBe('qwen3:4b');
    expect(get(coachSetupStatus)?.busy?.kind).toBe('pull');
    // Nach dem Ende liest die Oberfläche weiter und räumt auf.
    await new Promise((r) => setTimeout(r, 1500));
    expect(get(coachSetupOp)).toEqual({ kind: 'idle' });
    expect(get(coachSetupStatus)?.models).toEqual(['qwen3:4b']);
  });
});
