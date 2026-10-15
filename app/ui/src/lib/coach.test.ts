import { describe, expect, it } from 'vitest';
import { compareCell, ruleLabel } from './coach';
import { MISSING } from './format';

const fmt = (n: number) => String(n);

describe('compareCell', () => {
  it('zeigt gleiche Werte einmal und ungleiche als Vorher und Nachher', () => {
    expect(compareCell(480, 480, fmt)).toEqual({ text: '480', changed: false });
    expect(compareCell(480, 420, fmt)).toEqual({ text: '480 → 420', changed: true });
  });

  it('setzt für fehlende Werte den Platzhalter, nie 0', () => {
    expect(compareCell(null, 60, fmt)).toEqual({ text: `${MISSING} → 60`, changed: true });
    expect(compareCell(60, null, fmt)).toEqual({ text: `60 → ${MISSING}`, changed: true });
    expect(compareCell(null, null, fmt)).toEqual({ text: MISSING, changed: false });
    expect(compareCell(0, 0, fmt)).toEqual({ text: '0', changed: false });
  });
});

describe('ruleLabel', () => {
  it('nennt bei einem Regelcode auch den Namen, sonst nur den Code', () => {
    expect(ruleLabel('de', 'R03')).toMatch(/^R03 \S/);
    expect(ruleLabel('en', 'R03')).toMatch(/^R03 \S/);
    expect(ruleLabel('de', 'SCHEMA')).toBe('SCHEMA');
    expect(ruleLabel('de', 'R99')).toBe('R99');
    // Kein Freitext wird zu einem Schlüssel.
    expect(ruleLabel('de', 'rule.R03.name')).toBe('rule.R03.name');
  });
});
