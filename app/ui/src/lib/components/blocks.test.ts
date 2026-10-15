import { cleanup, render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';
import Metric from './Metric.svelte';
import Section from './Section.svelte';
import StatRow from './StatRow.svelte';

// Bausteine der Gestaltung (SPEC 5.5, seit T24): Struktur und Klassen, auf die app.css zielt.

afterEach(cleanup);

const text = (value: string) =>
  createRawSnippet(() => ({
    render: () => `<p data-testid="inner">${value}</p>`,
  }));

describe('Metric', () => {
  it('zeigt Wert, Einheit und Beschriftung in dieser Reihenfolge', () => {
    const { container } = render(Metric, {
      value: '88,9',
      unit: 'TSS',
      label: 'Wochenlast',
      testid: 'm',
    });
    const metric = screen.getByTestId('m');
    expect(metric.classList.contains('metric')).toBe(true);
    expect(metric.classList.contains('size-md')).toBe(true);
    expect(container.querySelector('.metric-value')?.textContent).toContain('88,9');
    expect(container.querySelector('.metric-unit')?.textContent).toBe('TSS');
    expect(container.querySelector('.metric-label')?.textContent?.trim()).toBe('Wochenlast');
  });

  it('setzt ein echtes Minuszeichen vor negative Werte, Ziffern bleiben unberührt', () => {
    render(Metric, { value: '-12,5', label: 'Form', valueTestid: 'v' });
    expect(screen.getByTestId('v').textContent).toBe('−12,5');
  });

  it('lässt Platzhalter und Text ohne führende Ziffer unverändert', () => {
    render(Metric, { value: '-', label: 'Form', valueTestid: 'v' });
    expect(screen.getByTestId('v').textContent).toBe('-');
  });

  it('zeigt die Veränderung als Text mit Pfeil, die Bewertung als Klasse', () => {
    const { container } = render(Metric, {
      value: '33',
      label: 'Fitness',
      delta: { text: '+3 zur Vorwoche', direction: 'up', tone: 'good' },
    });
    const delta = container.querySelector('.metric-delta');
    expect(delta?.textContent).toContain('+3 zur Vorwoche');
    expect(delta?.classList.contains('good')).toBe(true);
    expect(delta?.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true');
  });

  it('kennt die Größen md, lg und xl', () => {
    render(Metric, { value: '1', label: 'a', size: 'xl', testid: 'm' });
    expect(screen.getByTestId('m').classList.contains('size-xl')).toBe(true);
  });
});

describe('Section', () => {
  it('trägt Marke und Aussage und rendert den Inhalt', () => {
    const { container } = render(Section, {
      label: 'Form',
      statement: 'Belastung und Erholung halten sich die Waage.',
      testid: 's',
      children: text('Inhalt'),
    });
    expect(screen.getByTestId('s').classList.contains('section')).toBe(true);
    expect(container.querySelector('.section-label')?.textContent).toBe('Form');
    expect(container.querySelector('.section-statement')?.textContent).toContain('Waage');
    expect(screen.getByTestId('inner').textContent).toBe('Inhalt');
  });

  it('lässt die Aussage weg, wenn keine übergeben wird', () => {
    const { container } = render(Section, { label: 'Form', children: text('x') });
    expect(container.querySelector('.section-statement')).toBeNull();
  });
});

describe('StatRow', () => {
  it('hüllt die Kennzahlen in .stat-row', () => {
    render(StatRow, { testid: 'row', children: text('a') });
    expect(screen.getByTestId('row').classList.contains('stat-row')).toBe(true);
    expect(screen.getByTestId('inner')).toBeTruthy();
  });
});
