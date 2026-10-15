import { describe, expect, it } from 'vitest';
import { APP_NAME, windowTitle } from './title';

describe('windowTitle', () => {
  it('gibt ohne Abschnitt nur den Namen zurück', () => {
    expect(windowTitle()).toBe(APP_NAME);
  });

  it('setzt den Abschnitt vor den Namen', () => {
    expect(windowTitle('Woche')).toBe('Woche · Lastwerk');
  });
});
