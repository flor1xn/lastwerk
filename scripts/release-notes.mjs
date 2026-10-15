// Erzeugt die Release-Notizen aus catalog/changelog.json, derselben Quelle, die die App in den
// Einstellungen zeigt. Aufruf: node scripts/release-notes.mjs <tag> <ausgabeordner> [commits.txt]
// Schreibt notes.md (Beschreibung des Release-Entwurfs) und manifest-notes.txt (Feld `notes`
// in latest.json). Fehlt die Version im Changelog, stehen dort die Betreffzeilen aus
// commits.txt (eine je Zeile, automatische Patch-Versionen seit T30); fehlt auch die, bleibt
// der Abschnitt weg.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const [tag, outDir, commitsFile] = process.argv.slice(2);
if (!tag || !outDir) {
  console.error('Aufruf: node scripts/release-notes.mjs <tag> <ausgabeordner>');
  process.exit(2);
}
const version = tag.replace(/^v/, '');
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const changelog = JSON.parse(readFileSync(join(root, 'catalog', 'changelog.json'), 'utf8'));
const items = (lang) => (Array.isArray(changelog[lang]?.[version]) ? changelog[lang][version] : []);
const list = (lang) => items(lang).map((line) => `- ${line}`);

const de = list('de');
const en = list('en');
// Commits sind deutsch (CLAUDE.md, Stil), deshalb nur im deutschen Abschnitt.
const commits =
  de.length || en.length || !commitsFile
    ? []
    : readFileSync(commitsFile, 'utf8')
        .split('\n')
        .map((line) => line.trim())
        .filter(Boolean)
        .map((line) => `- ${line}`);
const notes = [
  'Lastwerk, lokales Werkzeug für Ausdauertraining.',
  '',
  ...(de.length ? [`## Neu in Version ${version}`, '', ...de, ''] : []),
  ...(en.length ? [`## New in version ${version}`, '', ...en, ''] : []),
  ...(commits.length
    ? [
        `## Änderungen in Version ${version}`,
        '',
        ...commits,
        '',
        '_Changes since the previous version (commit subjects, German)._',
        '',
      ]
    : []),
  '**Ohne Signierung, ohne Support.** Die Installer sind nicht signiert:',
  '',
  '- Windows: SmartScreen warnt vor einer unbekannten App. "Weitere Informationen", dann "Trotzdem ausführen".',
  '- Linux: AppImage ausführbar machen (`chmod +x`) oder das deb-Paket installieren.',
  '',
  'Daten bleiben auf dem Rechner, es gibt keine Telemetrie und keine Netzwerkzugriffe. Lastwerk wird nicht von Garmin oder Strava unterstützt oder autorisiert.',
  '',
  'English: unsigned builds, no support. Windows SmartScreen will warn on first start.',
  '',
].join('\n');

mkdirSync(outDir, { recursive: true });
writeFileSync(join(outDir, 'notes.md'), notes);
writeFileSync(join(outDir, 'manifest-notes.txt'), [...de, ...en, ...commits].join('\n'));
