# Remote-Sessions mit Claude Code

Anleitung, um die Tasks aus [tasks/README.md](tasks/README.md) in Claude-Code-Cloud-Sessions auf GitHub abzuarbeiten. Stand der Plattform: Dokumentation unter code.claude.com (Seiten "Use Claude Code in the cloud" und "Configure cloud environments"), abgerufen am 13. Oktober 2026. Plattformdetails können sich ändern, dann gilt die Dokumentation.

## Was das Repository schon mitbringt

| Datei | Zweck |
|---|---|
| `CLAUDE.md` | Wird von jeder Session geladen: Regeln, Arbeitsablauf, Befehle |
| `docs/tasks/` | Ein Arbeitsauftrag je Task mit Abnahmebefehlen |
| `.claude/settings.json` | SessionStart-Hook, ruft `scripts/cloud-session-start.sh` (nur in der Cloud) |
| `scripts/cloud-env-setup.sh` | Inhalt für das Setup-Skript der Cloud-Umgebung (einmal einfügen) |
| `.github/workflows/` | CI auf Windows und Linux und für `bridge/` |

## Einmalige Einrichtung

1. **GitHub verbinden.** Entweder die Claude-GitHub-App für das Repository `flor1xn/lastwerk` installieren (Onboarding auf claude.ai/code, nötig für Auto-Fix) oder im Terminal `/web-setup` ausführen. Die Sitzung braucht Schreibrechte, weil sie nach `main` pusht.
2. **Umgebung anlegen** (claude.ai/code, Umgebung bearbeiten):
   - Name: `lastwerk`
   - Netzwerk: **Trusted** (erreicht crates.io, npm, PyPI, GitHub, Ubuntu-Archiv).
   - Umgebungsvariablen: `BASH_DEFAULT_TIMEOUT_MS=600000` (längere Builds).
   - Setup-Skript: Inhalt von [`scripts/cloud-env-setup.sh`](../scripts/cloud-env-setup.sh) einfügen. Es installiert die Systembibliotheken für Tauri, `rustfmt` und `clippy` und Python 3.12 und wird zwischengespeichert, wenn es unter etwa fünf Minuten bleibt.
3. **Für T06 und T09 (Playwright):** Lädt der Browser-Download nicht (Host nicht in "Trusted"), den Download-Host unter "Custom" ergänzen. Die Session trägt sonst einen Blocker in die Task-Datei ein.
4. **Lokale Datenschutzhaken (optional, einmal pro Klon):** `git config core.hooksPath .githooks`. In der Cloud setzt der SessionStart-Hook das selbst.

Keine Zugangsdaten für Garmin, Strava oder Cloud-Modelle in die Umgebung eintragen. Umgebungsvariablen sind für alle sichtbar, die die Umgebung nutzen.

## Eine Session starten

Browser (claude.ai/code) oder Handy (Code-Tab der Claude-App): Repository `lastwerk`, Umgebung `lastwerk`, Branch `main`, Auftrag:

```
Bearbeite docs/tasks/T01-core-belastung.md. Beachte CLAUDE.md.
```

Terminal (im Repository, zuvor `git push`):

```
claude --cloud "Bearbeite docs/tasks/T01-core-belastung.md. Beachte CLAUDE.md."
```

Nächsten Schritt in eine laufende Session geben:

```
claude -p "Weiter mit der Abnahme, dann Status nachführen." --cloud <session-id>
```

## Welcher Task als Nächstes

Siehe Tabelle und Reihenfolge in [tasks/README.md](tasks/README.md). Ein Task ist startklar, wenn seine Voraussetzungen `erledigt` sind. Die Sessions tragen den Status selbst nach.

Aktuell startklar nach T00: T01, T02 und T11. T01 (`crates/core`), T02 (`crates/testkit`) und T11 (`bridge/`) berühren verschiedene Ordner und dürfen gleichzeitig laufen. Zwei Sessions im selben Ordner starten nur nacheinander.

## Was Sessions nie tun

- Garmin-Anmeldung, echte Abrufe, echte FIT-Dateien. Das ist lokal (L1 und die manuellen Schritte).
- Etwas aus `local/` lesen oder committen. In der Cloud existiert `local/` ohnehin nicht.
- Force-Push, `git clean -x`, Veröffentlichen des Repositories.
- Cloud-APIs aus Lastwerk aufrufen oder Schlüssel speichern.

## Ablauf und Kontrolle

- Jede Session pusht möglichst nur einmal nach `main`, am Ende, und prüft die CI. Der Grund ist nicht der Preis (Standard-Runner öffentlicher Repositories sind kostenlos), sondern die CI-Last und der Release-Takt: Jeder Push mit Produktänderung löst CI und einen automatischen Release aus. Stand: `gh run list --limit 5` oder der Reiter "Actions" auf GitHub.
- Endet eine Session mit "blockiert", steht der Grund unter "Blocker" in der Task-Datei, und der Abschlussbericht nennt die Entscheidung, die der Nutzer treffen muss.
- Eine Session zeigt ihre Änderungen im Diff (claude.ai/code). Den Verlauf prüft man wie gewohnt mit `git log`.
- Eine fehlerhafte Änderung auf `main` wird durch eine neue Session mit dem Auftrag "Repariere die CI auf main" behoben oder per `git revert` zurückgenommen.

## Grenzen

- Cloud-Sessions teilen sich das Nutzungslimit mit allem anderen Claude-Code-Gebrauch. Parallele Sessions verbrauchen entsprechend schneller.
- Eine inaktive Cloud-Umgebung läuft ab. Die Unterhaltung lässt sich wieder öffnen, laufende Hintergrundprozesse gehen verloren.
- Lange Builds (Tauri, `cargo test` mit proptest) brauchen das erhöhte Zeitlimit aus der Einrichtung.
