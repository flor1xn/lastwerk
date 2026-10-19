import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

// Lastwerk-CI (docs/gestaltung.md): Die Regeln, die sich am Quelltext prüfen lassen. Was hier
// rot wird, verstößt gegen die Gestaltung, nicht gegen den Geschmack. Ausnahmen stehen mit Grund
// in der Liste ALLOWED und in docs/gestaltung.md, nirgends sonst.

const root = join(__dirname, '..');

/** CSS aus app.css und aus den <style>-Blöcken aller Svelte-Dateien, mit Dateiname. */
function styles(): { file: string; css: string }[] {
  const out: { file: string; css: string }[] = [];
  const walk = (dir: string) => {
    for (const name of readdirSync(dir)) {
      const path = join(dir, name);
      if (statSync(path).isDirectory()) walk(path);
      else if (name.endsWith('.css'))
        out.push({ file: rel(path), css: readFileSync(path, 'utf8') });
      else if (name.endsWith('.svelte')) {
        const text = readFileSync(path, 'utf8');
        const css = [...text.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)]
          .map((m) => m[1])
          .join('\n');
        if (css) out.push({ file: rel(path), css });
      }
    }
  };
  walk(root);
  return out;
}

const rel = (path: string) => relative(root, path).replaceAll('\\', '/');

/** Regeln als Paare aus Selektor und Rumpf, ohne Kommentare; @media wird aufgelöst. */
function rules(css: string): { selector: string; body: string }[] {
  const clean = css.replace(/\/\*[\s\S]*?\*\//g, '');
  return [...clean.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({
    selector: m[1].trim(),
    body: m[2],
  }));
}

// Signalfarben, Zustands- und Sportfarben: Sie dürfen nie als Kante an einer Seite stehen.
const COLORED = /var\(--(accent|warn|danger|ok|sport|form|power|heart|pmc)/;

// Ausnahmen mit Grund. Datei relativ zu src.
const ALLOWED = {
  // Umschalter ist eine Kapsel nach dem Vorbild der Segmented Controls von Apple.
  pill: ['lib/components/Segmented.svelte'],
  // Nur app.css setzt bei reduzierter Bewegung alles zurück.
  important: ['app.css'],
};

describe('Lastwerk-CI: Gestaltungsregeln im Quelltext (docs/gestaltung.md)', () => {
  const all = styles();

  it('setzt keine Versalien mit Sperrung (Regel Schrift)', () => {
    const hits = all.filter((s) => /text-transform:\s*uppercase/.test(s.css)).map((s) => s.file);
    expect(hits).toEqual([]);
  });

  it('sperrt Schrift nur über die Tokens --ls-* und nie positiv von Hand (Regel Schrift)', () => {
    const hits = all.flatMap((s) =>
      [...s.css.matchAll(/letter-spacing:\s*([^;]+);/g)]
        .map((m) => m[1].trim())
        .filter((v) => !/^var\(--ls-[a-z]+\)$/.test(v) && v !== '0' && !v.startsWith('-'))
        .map((v) => `${s.file}: ${v}`),
    );
    expect(hits).toEqual([]);
  });

  it('trägt keine farbige Kante an einer Seite (Regel Flächen)', () => {
    const hits: string[] = [];
    for (const s of all) {
      for (const r of rules(s.css)) {
        for (const m of r.body.matchAll(
          /border-(left|right|inline-start|inline-end)(-color)?\s*:\s*([^;]+);/g,
        )) {
          if (COLORED.test(m[3])) hits.push(`${s.file} ${r.selector}: ${m[0]}`);
        }
        // Streifen als Schatten: waagerecht versetzt, ohne Unschärfe.
        if (/inset\s+-?[1-9]\d*px\s+0(px)?\s+0(px)?\s+(0(px)?\s+)?var/.test(r.body))
          hits.push(`${s.file} ${r.selector}: Streifen als inset-Schatten`);
        // Streifen als Pseudoelement: schmal, über die volle Höhe, in einer Signalfarbe.
        if (
          /::(before|after)/.test(r.selector) &&
          /\bwidth:\s*[1-4]px/.test(r.body) &&
          /\btop:/.test(r.body) &&
          /\bbottom:/.test(r.body) &&
          COLORED.test(r.body)
        )
          hits.push(`${s.file} ${r.selector}: Streifen als Pseudoelement`);
      }
    }
    expect(hits).toEqual([]);
  });

  it('zeigt Kennzeichen nicht als Pille (Regel Kennzeichen)', () => {
    const hits = all
      .filter((s) => !ALLOWED.pill.includes(s.file))
      .filter((s) => /border-radius:\s*(999|9999)px/.test(s.css))
      .map((s) => s.file);
    expect(hits).toEqual([]);
  });

  it('nimmt Farben, Schatten und Schrift nur aus Tokens (Regel Tokens)', () => {
    const hits: string[] = [];
    for (const s of all) {
      if (s.file === 'app.css' || s.file === 'fonts.css') continue;
      for (const m of s.css.matchAll(/#[0-9a-fA-F]{3,8}\b|\b(rgb|rgba|hsl|hsla)\(/g))
        hits.push(`${s.file}: ${m[0]}`);
      for (const m of s.css.matchAll(/font-family:\s*([^;]+);/g))
        if (!/^var\(--font-(sans|mono)\)$/.test(m[1].trim())) hits.push(`${s.file}: ${m[0]}`);
    }
    expect(hits).toEqual([]);
  });

  it('bewegt sich nur gezielt: kein transition: all, kein ease-in (Regel Bewegung)', () => {
    const hits = all.flatMap((s) =>
      [...s.css.matchAll(/transition(-property)?:\s*all\b|\bease-in\b(?!-out)/g)].map(
        (m) => `${s.file}: ${m[0]}`,
      ),
    );
    expect(hits).toEqual([]);
  });

  it('nutzt !important nur in app.css', () => {
    const hits = all
      .filter((s) => !ALLOWED.important.includes(s.file) && /!important/.test(s.css))
      .map((s) => s.file);
    expect(hits).toEqual([]);
  });

  // Gegenprobe: Die Prüfung auf Kanten erkennt einen Streifen wirklich.
  it('erkennt eine farbige Kante in einem Beispiel', () => {
    const sample =
      '.box::before { position: absolute; top: 0; bottom: 0; width: 3px; background: var(--accent); }';
    const r = rules(sample)[0];
    expect(/::(before|after)/.test(r.selector) && COLORED.test(r.body)).toBe(true);
    expect(COLORED.test('3px solid var(--warn)')).toBe(true);
    expect(COLORED.test('1px solid var(--line)')).toBe(false);
  });
});
