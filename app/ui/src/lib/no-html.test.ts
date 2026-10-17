import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

/** Alle Svelte- und TypeScript-Dateien unter src (ohne Tests). */
function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return sources(path);
    return /\.(svelte|ts)$/.test(name) && !name.endsWith('.test.ts') ? [path] : [];
  });
}

describe('Text ist nie HTML (SPEC 9.5)', () => {
  it('nutzt weder {@html} noch innerHTML noch eine Markdown-Umsetzung', () => {
    const root = join(__dirname, '..');
    const offenders: string[] = [];
    for (const file of sources(root)) {
      const text = readFileSync(file, 'utf8');
      if (/\{@html\b|\.innerHTML\b|\.outerHTML\b|insertAdjacentHTML|document\.write/.test(text)) {
        offenders.push(file);
      }
    }
    expect(offenders).toEqual([]);
  });

  // Antworten des Coachs (T26, SPEC 9.5, 9.7): Text, nie Markdown mit Verweisen. Es gibt keine
  // Bibliothek, die Text in Elemente verwandelt, und die Ansichten des Gesprächs setzen den
  // Text als Textknoten (Svelte maskiert ihn) statt über ein Element-API.
  it('rendert Antworten des Coachs nur als Text, ohne Markdown- oder HTML-Bibliothek', () => {
    const root = join(__dirname, '..');
    const forbidden =
      /\b(marked|markdown-it|snarkdown|showdown|micromark|remark|dompurify|sanitize-html)\b|\.innerText\s*=|createContextualFragment|DOMParser|\bnew Function\b|\beval\(/i;
    const offenders = sources(root).filter((f) => forbidden.test(readFileSync(f, 'utf8')));
    expect(offenders).toEqual([]);

    const pkg = readFileSync(join(root, '..', 'package.json'), 'utf8');
    expect(pkg).not.toMatch(/marked|markdown|remark|dompurify|sanitize/i);

    // Die Komponenten des Gesprächs setzen Antwort und Erklärung als Textknoten.
    const panel = readFileSync(join(root, 'lib/components/coach/CoachChat.svelte'), 'utf8');
    expect(panel).toContain('{m.text}');
    expect(panel).toContain('{m.content}');
    const card = readFileSync(join(root, 'lib/components/coach/ProposalCard.svelte'), 'utf8');
    expect(card).toContain('{proposal.explanation}');
  });
});
