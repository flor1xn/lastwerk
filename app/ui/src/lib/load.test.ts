import { describe, expect, it } from 'vitest';
import type { LoadInfo } from './backend/types';
import { hasKey, translate } from './i18n';
import { missingText, TRAINING_EFFECTS, trainingEffectText } from './load';

const base: LoadInfo = {
  status: 'missing_threshold',
  value: null,
  method: null,
  intensity_factor: null,
  estimated: false,
  missing: 'ftp_w',
};

describe('Fehlender Wert der Last (T39)', () => {
  it('nennt den fehlenden Wert und den Ort zum Eintragen', () => {
    const m = missingText(base);
    expect(m).not.toBeNull();
    expect(translate('de', m!.what)).toBe('FTP fehlt');
    expect(translate('de', m!.hint)).toContain('Einstellungen, Schwellenwerte');
    expect(translate('de', missingText({ ...base, missing: 'sex' })!.hint)).toContain('Athlet');
  });

  it('jede Art hat einen Text in beiden Sprachen', () => {
    for (const kind of [
      'ftp_w',
      'lthr_bpm',
      'hr_max_bpm',
      'hr_rest_bpm',
      'run_speed_mps',
      'css_mps',
      'sex',
    ] as const) {
      const m = missingText({ ...base, missing: kind })!;
      expect(hasKey('de', m.what) && hasKey('en', m.what), kind).toBe(true);
    }
  });

  it('ohne Art, mit unbekannter Art oder anderem Status gibt es keinen eigenen Text', () => {
    expect(missingText({ ...base, missing: null })).toBeNull();
    expect(missingText({ ...base, status: 'ok', value: 50 })).toBeNull();
    expect(
      missingText({ ...base, missing: 'etwas_neues' as unknown as LoadInfo['missing'] }),
    ).toBeNull();
  });
});

describe('Garmins Trainingseffekt', () => {
  it('bekannte Schlüssel haben Texte, unbekannte werden lesbar', () => {
    const tr = (k: string) => translate('de', k);
    for (const e of TRAINING_EFFECTS) {
      expect(
        hasKey('de', `ui.detail.garmin.effect.${e}`) &&
          hasKey('en', `ui.detail.garmin.effect.${e}`),
      ).toBe(true);
    }
    expect(trainingEffectText('AEROBIC_BASE', tr)).toBe('Grundlage');
    expect(trainingEffectText('MIXED_INTENSITY', tr)).toBe('MIXED INTENSITY');
  });
});
