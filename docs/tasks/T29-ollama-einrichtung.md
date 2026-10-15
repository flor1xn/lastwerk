# T29: Ollama im Installer und im Erststart einrichten

| Feld | Wert |
|---|---|
| Status | in Prüfung |
| Meilenstein | 1.0 |
| Voraussetzung | T26, T27 |
| cloudfähig | ja (Attrappen, Trockenlauf), **Livetest unter Windows manuell** |
| Bereich | Teil A: `app/src-tauri/resources`, `app/src-tauri/windows`, `app/src-tauri/tauri.release.conf.json`, `.github/workflows`. Teil B: `catalog/models.json`, `crates/coach`, `app/src-tauri/src`, `app/ui`, `catalog/locales` |
| SPEC | 9.2, 10.1, 10.2, 14 (O13) |

## Ziel

Nach der Installation läuft der Coach ohne weitere Einrichtung: Der Installer bringt Ollama und ein leichtes Startmodell mit oder erkennt ein vorhandenes Ollama. Im Erststart wählt der Nutzer bei Bedarf ein anderes Modell, mit Empfehlung nach seiner Hardware.

## Warum

Entscheidung vom 15. Oktober 2026: Ollama wird in Installer und Erststart von Anfang an mitgedacht und eingerichtet. Gewünscht sind Erkennung einer vorhandenen Installation, für den Anfang das beste leichtgewichtige Modell, später selbst wählen mit Empfehlung, und als Installationsweg zuerst winget, sonst den offiziellen Download mit Prüfung der Signatur.

## Vertrag zwischen Teil A und Teil B

- Skript `app/src-tauri/resources/ollama-setup.ps1`, als Tauri-Ressource gebündelt (in der App über `BaseDirectory::Resource`, im Installer unter `$INSTDIR\resources\ollama-setup.ps1`, Pfad in Teil A prüfen und vermerken).
- Parameter: `-Mode installer|app` (Pflicht), `-Model <name>` (optional, ohne: kein Modell laden), `-DryRun` (nur ausgeben, nichts ändern, kein Netz), `-StatePath <datei>` (Standard `%LOCALAPPDATA%\Lastwerk\ollama-setup.json`).
- Ausgabe auf der Standardausgabe, eine Zeile je Schritt, maschinenlesbar: `LASTWERK:<phase>:<wert>`. Phasen: `detect` (`found`, `missing`, `running`), `install` (`winget`, `download`, `verify_ok`, `skipped`), `start` (`ok`, `timeout`), `pull` (`start`, `<prozent>`, `done`, `skipped`), `done` (`ok`), `error` (`<code>`). Codes: `no_windows`, `winget_failed`, `download_failed`, `signature_invalid`, `install_failed`, `start_timeout`, `pull_failed`, `bad_args`. Alles andere auf der Standardausgabe ignoriert die App.
- Exit-Code 0 bei Erfolg, 1 bei Fehler (letzte Zeile `LASTWERK:error:<code>`), 2 bei falschen Parametern.
- Zustandsdatei (JSON, atomar geschrieben): `{ "schema_version": 1, "decision": "yes"|"no"|null, "ollama_found": bool, "installed_via": "existing"|"winget"|"download"|null, "model": string|null, "model_ready": bool, "error_code": string|null, "updated_at": "<ISO 8601 UTC>" }`. Keine Pfade, kein Text aus fremden Antworten.
- Startmodell: `qwen3:4b` (Faustregel bis O13 gemessen ist). Es steht in `catalog/models.json` als `"starter": true` und im Hook. Ein Test nach dem Zusammenführen prüft, dass beide gleich sind.

### Nachträge zum Vertrag aus Teil A (15. Oktober 2026)

Alle Nachträge sind Erweiterungen, der Vertrag oben gilt unverändert.

- Pfad geprüft: Die Ressource steht in `tauri.release.conf.json` als `"resources": { "resources/ollama-setup.ps1": "resources/ollama-setup.ps1" }`. tauri-bundler 2.10.2 (zur CLI 2.12.2) legt sie unter `$INSTDIR\resources\ollama-setup.ps1` ab; unter Windows ist `BaseDirectory::Resource` der Ordner der exe, also `$INSTDIR` (tauri-utils 2.10.1, `platform::resource_dir`). In der App: `resolve("resources/ollama-setup.ps1", BaseDirectory::Resource)`. Weil die Ressource nur im Release-Overlay steht, fehlt sie in `tauri dev` und im Debug-Build der CI; Teil B muss das als Fehler behandeln, nicht als Absturz.
- `-Decision yes|no` (optional): Schreibt die Antwort aus dem Installer in `decision`. `no` merkt nur: Erkennung wie sonst (lokal und `/api/version` auf Loopback), dann `install:skipped`, `pull:skipped`, `done:ok`, keine Installation, kein Start, kein Modell. `-Decision no` zusammen mit `-Model` ist `bad_args`. Ohne `-Decision` bleibt eine gemerkte Entscheidung erhalten (die App ruft ohne `-Decision` auf).
- Zustandsdatei: kompaktes JSON in einer Zeile mit abschließendem Zeilenumbruch, UTF-8 ohne BOM, Schlüssel in der Reihenfolge des Vertrags. Der Hook liest die erste Zeile und sucht `"decision":"yes"` oder `"decision":"no"`. Wer die Datei sonst schreibt, muss dieses Format halten (in Rust `serde_json::to_string`, nicht `to_string_pretty`).
- Übernahme früherer Werte: `decision` bleibt, `installed_via` bleibt `winget` oder `download`, solange Ollama gefunden wird (sonst `null`), `model` und `model_ready` bleiben, wenn kein `-Model` übergeben wird (`model_ready` wird `false`, wenn Ollama fehlt). Eine unlesbare Datei zählt als nicht vorhanden.
- Reihenfolge der Zeilen im Erfolgsfall: `detect`, `install`, `start`, `pull`, `done`. `install` kann zweimal kommen (`winget`, dann `download`, wenn winget scheitert), nach `download` folgt `verify_ok`. `pull:<n>` nur steigend, ganze Prozent über alle Schichten. `winget_failed` bleibt reserviert und erscheint nicht als Endzustand, weil auf ein Scheitern von winget immer der Download folgt.
- `-DryRun`: keine Erkennung (nimmt `missing` an), kein Netz, kein Zugriff auf Registry oder Ollama, gibt die Schritte des ungünstigsten Falls aus (`detect:missing`, `install:winget`, `start:ok`, `pull:start`/`pull:done` oder `pull:skipped`, `done:ok`; mit `-Decision no` wie oben). Er schreibt die Zustandsdatei nur, wenn `-StatePath` ausdrücklich gesetzt ist, und dann nur den Stand ohne Wirkung (`ollama_found` und `model_ready` `false`, `installed_via` `null`). Der Trockenlauf läuft auch außerhalb von Windows.
- Parameter: Modellname zusätzlich höchstens 100 Zeichen (Faustregel), Muster mit Groß- und Kleinschreibung und `\z` statt `$` (kein angehängter Zeilenumbruch). `-Mode` und `-Decision` nur in Kleinbuchstaben. `-StatePath` muss absolut sein. Unbekannte Parameter sind `bad_args`. Bei `bad_args` wird nichts geschrieben.
- Unerwartete Fehler: Code nach der Phase, `start` ergibt `start_timeout`, `pull` ergibt `pull_failed`, alles andere `install_failed`. Die Meldung steht nur als Hinweiszeile im Protokoll.
- Hinweiszeilen beginnen mit `ollama-setup: ` und sind ASCII (nsExec liest in der Codepage der Konsole). Eine davon nennt beim Download den Herausgeber (`publisher: ...`) für den manuellen Schritt 5.
- Modellliste und Laden über die Loopback-Schnittstelle (`GET /api/tags`, `POST /api/pull` mit `stream: true`) statt `ollama list` und `ollama pull`: Windows PowerShell 5.1 wertet Fehlerausgaben nativer Programme unter `$ErrorActionPreference = 'Stop'` beim Umleiten als Abbruch, und die Fortschrittsbalken von `ollama pull` enthalten Steuerzeichen. Die Schnittstelle liefert `completed` und `total` als Zahlen und funktioniert auch, wenn nur der Dienst läuft.
- Hook: Die Zeile `!define LASTWERK_OLLAMA_STARTER_MODEL "qwen3:4b"` in `app/src-tauri/windows/hooks.nsh` ist die Stelle für den Gleichheitstest mit `catalog/models.json`.

## Teil A: Skript, Installer, Sicherheit

1. `ollama-setup.ps1` nach dem Vertrag. Ollama erkennen: `Get-Command ollama`, sonst `$env:LOCALAPPDATA\Programs\Ollama\ollama.exe`, sonst Deinstallationseintrag in der Registry, und ob `http://127.0.0.1:11434/api/version` antwortet. Installieren nur, wenn nicht vorhanden: zuerst `winget install --id Ollama.Ollama --exact --silent --accept-package-agreements --accept-source-agreements --disable-interactivity`; ohne winget oder bei Fehlschlag `https://ollama.com/download/OllamaSetup.exe` per `Invoke-WebRequest` nach einem eigenen Ordner unter `%TEMP%`, `Get-AuthenticodeSignature` muss `Valid` sein und der Herausgeber (`SignerCertificate.Subject`) muss den erwarteten Namen tragen (vor dem ersten Livetest nur prüfen, dass `O=` Ollama enthält; der genaue Name wird beim Livetest vermerkt), sonst Datei löschen und `signature_invalid`. Danach `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`. Starten, falls der Dienst nicht antwortet (`ollama serve` versteckt im Hintergrund oder die App von Ollama), warten bis zu 60 s (Faustregel). Modell: nur wenn `-Model` gesetzt und `ollama list` es nicht nennt, `ollama pull <Model>`, Prozent aus der Ausgabe als `LASTWERK:pull:<n>`. Modellname vorher gegen das Muster `^[a-z0-9][a-z0-9._-]*(:[a-z0-9._-]+)?$` prüfen (keine Einschleusung von Befehlen). Idempotent und schnell, wenn alles vorhanden ist. Strikter Modus (`Set-StrictMode -Version Latest`, `$ErrorActionPreference = 'Stop'`), keine Ausführung von Text aus dem Netz.
2. NSIS-Hook `app/src-tauri/windows/hooks.nsh`, eingebunden über `bundle.windows.nsis.installerHooks` in `tauri.release.conf.json`; Ressource über `bundle.resources`. `NSIS_HOOK_POSTINSTALL`: Steht in der Zustandsdatei schon `decision`, nicht fragen. Im stillen oder passiven Modus (Update, `${Silent}` und der Passivmodus von Tauri) nicht fragen und nur `-Model` übergeben, wenn `decision` `yes` ist. Sonst eine Ja/Nein-Abfrage (deutsch und englisch nach Sprache des Installers): "Lokalen Coach (Ollama) einrichten? Lädt etwa 2,5 GB." Antwort `no` wird gemerkt (Aufruf mit `-Mode installer` ohne `-Model` und ein Merker, oder ein eigener Schalter `-Decision no`, im Vertrag nachtragen). Aufruf mit `nsExec::ExecToLog`. Ein Fehler bricht die Installation von Lastwerk nie ab.
3. CI: Job auf `windows-latest`, der `ollama-setup.ps1 -Mode app -Model qwen3:4b -DryRun -StatePath <temp>` ausführt und die Ausgabezeilen prüft, dazu PSScriptAnalyzer (falls installierbar, sonst Parser-Prüfung). Kein echter Download in der CI.
4. SPEC 10.1 und 10.2: neue Verbindungen (winget-Quelle, ollama.com) nur im Installer oder nach Klick, Bedrohung "manipulierter Installer von Ollama" mit Gegenmaßnahmen. SPEC 9.2: Ollama wird auf Wunsch eingerichtet. `scripts/build-installer.ps1` und `release.yml` prüfen, ob die Ressource und der Hook mitgebaut werden.

## Teil B: Katalog, Empfehlung, App

1. `catalog/models.json`: wenige Einträge (Vorschlag `qwen3:1.7b`, `qwen3:4b` mit `starter: true`, `qwen3:8b`, `qwen3:14b`), je `name`, `size_gb`, `min_ram_gb`, `comfortable_vram_gb`, Textschlüssel. Werte als Faustregel, Größen gegen die Ollama-Bibliothek geprüft und mit Datum vermerkt (falls aus der Cloud nicht abrufbar, als ungeprüft kennzeichnen).
2. `crates/coach/src/recommend.rs`: reine Funktion `recommend(&Hardware, &[CatalogModel]) -> Recommendation` mit Einstufung `fits`/`slow`/`too_big` je Modell und Empfehlung (größtes `fits`, sonst Startmodell). Handgerechnete Tests (8, 16, 32 GB RAM, mit und ohne GPU-Speicher).
3. `crates/coach`: Pull über `POST /api/pull` mit `stream: true` (Loopback, gleicher HTTP-Aufbau wie `client.rs`: keine Weiterleitungen, kein Proxy aus der Umgebung), Fortschritt als Rückruf (`completed`/`total`), Abbruch über ein Flag. Tests gegen Attrappen-Server.
4. App: `service/coach_setup.rs` und Befehle `coach_setup_status` (Ollama gefunden/läuft, Modelle, gewähltes Modell, Hardware: RAM über `sysinfo` mit Vermerk, GPU-Speicher über `nvidia-smi` wenn vorhanden, sonst `null`; Katalog mit Einstufung und Empfehlung; Inhalt der Zustandsdatei), `coach_install_ollama` (nur Windows: startet das gebündelte Skript mit `-Mode app` ohne `-Model`, liest die `LASTWERK:`-Zeilen, Ereignis `coach-setup-progress`; sonst Fehlercode `coach_setup_unsupported`), `coach_pull_model(name)` (nur Katalog oder installierte Namen, Ereignis `coach-pull-progress`, setzt danach `coach.model`), `coach_pull_cancel`. Doppelter Lauf: `coach_setup_busy`. Registrierung an allen Stellen nach `app/README.md`.
5. Oberfläche: Erststart-Schritt "Coach" nach "Ziel" (überspringbar): Zustand, Knopf "Jetzt einrichten", Modellliste mit "Empfohlen für deinen PC", "langsam auf diesem PC", "zu groß für diesen PC" (wählbar), Fortschritt, Download läuft im Hintergrund weiter. Einstellungen: Abschnitt "Coach" mit demselben Baustein. Planansicht: Hinweis ohne Modell verweist auf den Abschnitt. Texte de/en, Bausteine aus T24, Attrappe mit Varianten (`?ollama=fehlt|laeuft|laedt`).
6. SPEC 9.2 und 9.7 Umsetzungsvermerk, O13 ergänzen, `catalog/changelog.json` 1.0.0 um einen Punkt ergänzen.

## Abnahme

```
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
(cd app/src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)
cargo deny check && cargo deny --manifest-path app/src-tauri/Cargo.toml --config deny.toml check
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test && pnpm --dir app/ui build
sh scripts/check-privacy.sh && sh scripts/check-isolation.sh && sh scripts/check-i18n.sh
```

## Manuelle Schritte (Nutzer, Windows)

1. NSIS-Installer auf einem Rechner ohne Ollama: Abfrage mit Ja, Ollama wird über winget installiert, `qwen3:4b` lädt (Protokoll im Installer), danach startet Lastwerk mit bereitem Coach.
2. Installer erneut (oder Update): keine Abfrage, nach Sekunden fertig.
3. Rechner mit vorhandenem Ollama: wird erkannt, nichts installiert.
4. Erststart, Schritt "Coach": Empfehlung passt zum Rechner, ein anderes Modell laden.
5. Herausgeber aus `Get-AuthenticodeSignature` von `OllamaSetup.exe` hier vermerken und im Skript fest prüfen.

## Vermerke

### Teil A (15. Oktober 2026)

- Dateien: `app/src-tauri/resources/ollama-setup.ps1` (UTF-8 mit BOM, damit Windows PowerShell 5.1 die Umlaute der Kommentare richtig liest), `app/src-tauri/windows/hooks.nsh`, `app/src-tauri/tauri.release.conf.json` (Ressource, Hook, Installersprachen), `scripts/test-ollama-setup.ps1` (Prüfung im Trockenlauf), `.github/workflows/ci.yml` (Jobs `ollama-setup` und `installer-hook`). Nachträge zum Vertrag stehen oben.
- Hook geprüft gegen die Quellen von tauri-bundler 2.10.2 (`src/bundle/windows/nsis/installer.nsi`, `settings.rs`): Makronamen `NSIS_HOOK_PREINSTALL`, `NSIS_HOOK_POSTINSTALL`, `NSIS_HOOK_PREUNINSTALL`, `NSIS_HOOK_POSTUNINSTALL`. `NSIS_HOOK_POSTINSTALL` steht im Abschnitt `Install` nach dem Kopieren der Ressourcen und dem Anlegen der Verknüpfungen, `$PassiveMode` und `$UpdateMode` sind dort gesetzt (`/P`, `/UPDATE`), `${StrLoc}` ist in `installer.nsi` deklariert. makensis läuft mit `-INPUTCHARSET UTF8`, Umlaute im Hook sind deshalb erlaubt. Der Pfad in `installerHooks` und in `resources` gilt relativ zu `app/src-tauri` (die CLI wechselt vor dem Bündeln dorthin), ebenso wenn das Overlay wie in `release.yml` und `scripts/build-installer.ps1` aus einem temporären Ordner kommt. Beide brauchen deshalb keine Änderung.
- Installersprachen: Ohne Eintrag baut Tauri nur Englisch, dann wäre `$LANGUAGE` immer 1033 und die deutsche Frage käme nie. Das Overlay setzt `"languages": ["English", "German"]`; NSIS wählt die Sprache des Systems, sonst die erste (Englisch). Folge: Der ganze Installer erscheint auf deutschen Systemen deutsch.
- Installationsart: Tauri baut den NSIS-Installer standardmäßig je Benutzer ohne Adminrechte, `$INSTDIR` ist `%LOCALAPPDATA%\Lastwerk` und damit derselbe Ordner wie die Zustandsdatei. Die Deinstallation löscht nur ihre eigenen Dateien und den Ordner, wenn er leer ist; die Zustandsdatei bleibt, eine spätere Installation fragt also nicht erneut. Das MSI-Paket hat keinen Hook und fragt nicht; dort richtet die App Ollama ein (Teil B). Seit T30 gibt es kein MSI-Paket mehr am Release.
- Der Installer ist ein 32-Bit-Programm. Der Hook startet deshalb `%WINDIR%\Sysnative\WindowsPowerShell\v1.0\powershell.exe` (64 Bit), sonst `$SYSDIR\...`, mit `-NoProfile -NonInteractive -ExecutionPolicy Bypass -File`. Still, passiv oder Update ohne gemerktes "Ja": der Hook ruft das Skript gar nicht auf, damit ohne Zustimmung nichts installiert wird.
- Skript, Abweichungen von der Beschreibung oben: `winget install` zusätzlich mit `--source winget` (nur die Community-Quelle, nicht `msstore`). Der Installer von Ollama wird mit `Start-Process -PassThru` und `WaitForExit` (900 s, Faustregel) abgewartet, nicht mit `-Wait`: `-Wait` wartet in 5.1 auch auf Kindprozesse, und der Installer startet am Ende die dauerhaft laufende App von Ollama. Gestartet wird bevorzugt `ollama app.exe` neben `ollama.exe`, sonst `ollama serve` ohne Fenster. Anfragen an `127.0.0.1:11434` ohne Proxy und ohne Weiterleitung.
- Herausgeberprüfung vor dem Livetest: `Status` muss `Valid` sein und `SignerCertificate.Subject` muss `O=Ollama` als ganzes Wort am Anfang der Organisation enthalten (Regex `(^|,\s*)O="?Ollama\b`, mit Groß- und Kleinschreibung). Den genauen Namen trägt der Livetest ein (manueller Schritt 5).
- Werkzeuge nur in der CI, keine Abhängigkeit von Lastwerk: PSScriptAnalyzer 1.23.0 (MIT, PowerShell Gallery), Grund: statische Prüfung des Skripts, Fehler und Syntax, die 5.1 nicht kennt, brechen den Job ab, Warnungen stehen im Protokoll. NSIS 3.09 aus Ubuntu (zlib/libpng-Lizenz, nur zum Übersetzen des Hooks im Job `installer-hook`).
- Geprüft in der Cloud-Sitzung (Linux): Hook mit makensis 3.09 im Rumpf des CI-Jobs übersetzt, mit `-WX` ohne Warnung. Zusammengeführte Konfiguration (`tauri.conf.json` plus Overlay, wie `release.yml` sie baut) gegen `config.schema.json` der Tauri-CLI 2.12.2 geprüft: gültig. actionlint 1.7.7 mit shellcheck über alle Workflows: sauber. `check-privacy.sh`, `check-isolation.sh`, `check-i18n.sh`: sauber.
- Nicht ausgeführt in der Cloud-Sitzung: das PowerShell-Skript selbst. `pwsh` für Linux lag bereit, die Sitzung ließ es aber nicht starten. Der erste Lauf ist der CI-Job `ollama-setup` unter Windows PowerShell 5.1 und PowerShell 7; das Skript ist bis dahin nur gelesen und geprüft. Der echte Lauf (winget, Download, Signatur, Start, Laden des Modells) ist nur im Livetest unter Windows prüfbar (manuelle Schritte 1 bis 3 und 5).

### Teil B umgesetzt (15. Oktober 2026), Status unverändert

Teil B ist umgesetzt, in einer Cloud-Umgebung (Linux), vollständig offline mit synthetischen Daten, Attrappen-Servern auf Loopback und der Attrappe der Oberfläche. Das Skript `ollama-setup.ps1` (Teil A) lag im Arbeitszweig nicht vor; die App codiert gegen den Vertrag und ist ohne das Skript robust (`can_install: false`, `coach_setup_missing_script`). Einzelheiten und Abweichungen: SPEC 5.4 (lastwerk-app), 9.2, 10.4, 14 (O13).

- **Neue Abhängigkeit:** `sysinfo` 0.39.6, Lizenz MIT, in `app/src-tauri` ohne Standardfeatures mit der Funktion `system`. Grund: Gesamter Arbeitsspeicher für die Empfehlung, ohne eigenen Code je Plattform. Neue Pakete: `ntapi` 0.4.3, `objc2-io-kit` 0.3.2. Eingetragen in SPEC 5.3. `cargo deny check` und `cargo deny --manifest-path app/src-tauri/Cargo.toml --config deny.toml check` sind grün, `deny.toml` unverändert.
- **Katalog `catalog/models.json`:** `qwen3:1.7b` (1,4 GB, 4 GB RAM, 2,5 GB VRAM), `qwen3:4b` (Startmodell, 2,5 GB, 8 GB, 4 GB), `qwen3:8b` (5,2 GB, 16 GB, 7 GB), `qwen3:14b` (9,3 GB, 24 GB, 12 GB). Alle Werte Faustregel. **Größen ungeprüft:** `ollama.com` war aus der Cloud-Umgebung nicht erreichbar (Verbindung vom Proxy abgelehnt, auch `WebFetch`); sie stammen aus dem Gedächtnis und sind vor dem Livetest gegen `ollama.com/library/qwen3` zu prüfen.
- **Einstufung (`crates/coach/src/recommend.rs`):** `too_big`, wenn RAM + 0,5 GB < `min_ram_gb`; `fits`, wenn VRAM >= `comfortable_vram_gb` oder Größe <= 3 GB; sonst `slow`. Empfohlen: das größte `fits`, sonst das Startmodell (die Oberfläche zeigt "Empfohlen" nur bei `fits`). Begründung im Modulkopf.
- **Abweichungen vom Auftrag:**
  - `coach_setup_status` ist zusätzlich `can_install` (Windows und Skript da) und `busy` (laufender Auftrag mit Fortschritt) mitgegeben, damit die Oberfläche bei fehlendem Skript die manuelle Anleitung zeigt und einen Download nach dem Wechsel der Ansicht wiederfindet.
  - `-StatePath` wird immer übergeben (Zustandsdatei neben der Datenbank, mit `LASTWERK_DB` verschoben), damit App und Skript dieselbe Datei meinen; der Vertrag erlaubt das.
  - Das Laden fasst den Fortschritt über alle Dateien des Modells zusammen (Summe je `digest`), nicht je Datei.
  - Der Strom wird auf einem Hilfsthread gelesen, weil `ureq` keinen Zeitlimit je Lesen kennt und ein Abbruch sonst bis zur nächsten Zeile warten müsste.
  - Zusätzlich eine Zeile in der Seitenleiste (`CoachSetupLine`), die einen laufenden Download oder die Einrichtung zeigt, wenn der Erststart schon geschlossen ist.
  - Fehlercodes: 22 neue (`coach_setup_*`, `coach_pull_*`), Texte de und en.
- **Zu prüfen nach dem Zusammenführen mit Teil A:** (1) Der Test `the_starter_model_matches_the_installer_hook_if_present` in `coach_setup.rs` prüft, dass `windows/hooks.nsh` das Startmodell aus dem Katalog nennt; er tut nichts, solange die Datei fehlt. (2) Der Pfad `resources/ollama-setup.ps1` und die Ressource in `tauri.release.conf.json` müssen zu `SCRIPT_RESOURCE` passen. (3) Gibt das Skript auf der Standardausgabe nur die Zeilen des Vertrags aus, liest der Parser sie; Ausgaben von winget oder Invoke-WebRequest werden ignoriert.
- **Tests:** Rust `crates/coach` (Katalog und Muster der Namen, Einstufung für 8, 16 und 32 GB mit und ohne Grafikkarte von Hand gerechnet, Laden gegen Attrappen-Server: Fortschritt, Abbruch sofort und mit geschlossener Verbindung, Fehlerzeile als Code ohne Text, Weiterleitung nicht befolgt, Status, Stillstand, ungültiger Name nie gesendet), `app/src-tauri` (Zeilenparser mit Rauschen, Leser der Zustandsdatei, Positivliste, `nvidia-smi`-Ausgabe, Sperre und Abbruch, Ausdünnung, Kindprozess mit `sh` statt PowerShell einschließlich Zeitlimit, Status und Laden gegen Attrappen-Ollama, Befehlsliste, Texte); Vitest (Zustand des Schritts, Marken, Prozent, Zeilen, Attrappe gleich dem Katalog, Abläufe, Baustein, Erststart, Seitenleiste); Playwright (Erststart, Einstellungen, Plan, Varianten `fehlt|laeuft|laedt`, de und en, Wächter grün).
- **Nicht geprüft:** der echte Ablauf unter Windows (PowerShell, winget, `CREATE_NO_WINDOW`, `\\?\`-Pfad der Ressource), ein echtes Ollama, `nvidia-smi` mit echter Karte, die Schwellen der Einstufung gegen Messwerte (O13), das Verhalten im Tauri-Fenster (WebView2), schmale Fenster unter 820 Pixel nur durch die Zeilenumbrüche der Liste (`flex-wrap`, `overflow-wrap`), kein Screenshot im hellen Thema.
