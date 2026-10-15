# T30: Installer und Update immer aktuell (automatischer Release)

| Feld | Wert |
|---|---|
| Status | in Prüfung |
| Meilenstein | nach 1.0 |
| Voraussetzung | T17, T27 |
| cloudfähig | ja, erster echter Lauf nach dem nächsten Push mit Produktänderung |
| Bereich | `.github/workflows`, `scripts`, `docs`, `README.md`, `CLAUDE.md` |
| SPEC | 10.1, 11.6 |

## Ziel

Lastwerk wird unter Windows einmal installiert. Danach kommt jede Änderung am Programm über "Nach Update suchen" in die App, ohne Terminal und ohne Neuinstallation. Der Installer am neuesten Release entspricht immer dem Stand von `main`.

## Warum

Anforderung vom 15. Oktober 2026: Eine einmalige Windows-Installation soll auch während der Testphase automatische Updates bekommen, und der Windows-Installer soll immer nachgezogen und aktuell gehalten werden. Bis hier entstand ein Release nur, wenn eine Session ihn von Hand startete; zwischen zwei Releases fand die App kein Update. Entscheidung vom 15. Oktober 2026: automatisch veröffentlichen, Windows nur noch als `-setup.exe`.

## Umfang

- `.github/workflows/auto-release.yml`: nach grüner CI eines Pushes auf `main` Entscheidung, ob ein Release fällig ist, und Aufruf von `release.yml`.
- `release.yml`: Aufruf über `workflow_call` mit Tag, Commit und `publish`; Build und Notizen aus genau diesem Commit; Windows nur NSIS.
- `scripts/release-notes.mjs`: Betreffzeilen der Commits als Ersatz, wenn `catalog/changelog.json` keinen Eintrag für die Version hat.
- `scripts/build-installer.ps1`: nur NSIS.
- SPEC 11.6, README, CLAUDE.md (Schritt 6), Vermerke in T17 und T29.

## Nicht im Umfang

Hintergrundprüfung auf Updates (bleibt ein Klick, SPEC 10.1), Updates für macOS und Linux, Codesigning, Vorabversionen (der Updater liest nur `releases/latest`).

## Vorgaben

- Release nur nach grüner CI auf `main`, nur aus dem geprüften Commit (`workflow_run.head_sha`), nur wenn er noch die Spitze von `main` ist.
- Produktänderung heißt: Änderung unter `app/`, `crates/`, `catalog/`, `schema/`, an `Cargo.toml`, `Cargo.lock` oder `rust-toolchain.toml` seit dem letzten Tag. Dokumentation, Workflows, Skripte und `bridge/` lösen keinen Release aus.
- Version: Patch + 1 des höchsten Tags `vX.Y.Z`; nennt `catalog/changelog.json` eine höhere Version, gilt diese.
- Kein neues Secret, keine neue Abhängigkeit, keine fremde Action.

## Abnahme

- `actionlint` (v1.7.7) ohne Befund.
- `node scripts/release-notes.mjs v1.0.1 <ordner> <commits.txt>` schreibt die Commits, für `v1.0.0` bleibt es beim Changelog.
- `sh scripts/check-all.sh` mit Exit 0.
- Nach dem Push: `auto-release.yml` läuft nach der CI und meldet "Keine Änderung am Produkt", solange seit `v1.0.0` nur Dokumentation und Workflows geändert sind.
- Erster Push mit Produktänderung: Release `v1.0.1` mit `-setup.exe`, `.sig` und `latest.json` erscheint ohne Handgriff als neuester Release.

## Manuelle Schritte

1. Wer 0.1.x oder 1.0.0 per `.msi` installiert hat: einmal deinstallieren (Daten in `%LOCALAPPDATA%\Lastwerk` bleiben), `-setup.exe` des neuesten Releases installieren.
2. Danach nur noch in den Einstellungen "Nach Update suchen".

## Vermerke

- 2026-10-15: Umgesetzt. Seit `v1.0.0` (8e12ee7) gibt es keine Produktänderung auf `main`, der erste automatische Release entsteht also mit dem nächsten Commit, der `app/`, `crates/`, `catalog/` oder `schema/` ändert. Bis dahin bleibt der Status `in Prüfung`.
- Scheitert ein automatischer Release nach dem Anlegen des Entwurfs, bleibt ein Entwurf mit dem Tag liegen; der nächste Lauf versucht denselben Tag erneut. Den liegengebliebenen Entwurf dann von Hand löschen.
- Sicherheitshinweise (RUSTSEC) blockieren wie bisher jeden Release (`advisories.yml`), also auch den automatischen.
