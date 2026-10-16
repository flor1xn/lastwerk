# T38: Lastwerk für ein öffentliches Repository vorbereiten

| Feld | Wert |
|---|---|
| Status | erledigt (Weg b, siehe "Umsetzung von Weg (b)") |
| Meilenstein | nach 1.0 |
| Voraussetzung | T30 |
| cloudfähig | ja; Umstellung der Sichtbarkeit und Einstellungen auf GitHub nur durch den Eigentümer |
| Bereich | `.github`, `scripts`, `docs`, `README.md`, `CLAUDE.md`, `crates` (Messung), `catalog` (Texte) |
| SPEC | 1.5, 10, 11.4, 11.6 |

## Ziel

Das Repository ist bereit, öffentlich zu werden: Was ein Fremder sieht, ist geprüft, was sich ohne Änderung der Geschichte beheben lässt, ist behoben, und was nur der Eigentümer entscheiden oder einstellen kann, steht als Liste am Ende. Die Sichtbarkeit selbst stellt der Eigentümer um.

## Warum

Mit der Umstellung wird nicht der Arbeitsbaum sichtbar, sondern das Rohmaterial: jede Version jeder Datei auf jedem Zweig, die Commit-Texte und Metadaten, Issues, Release-Notizen und Assets, Actions-Läufe mit Protokollen und Artefakten. Dazu kommt die Angriffsfläche der Workflows, sobald Fremde Pull Requests öffnen können. Das private Repository hatte außerdem das Freikontingent für Actions-Minuten aufgebraucht (Jobs starteten seit dem 15. Oktober 2026 nicht mehr); öffentliche Repositories nutzen die Standard-Runner kostenlos.

## Werkzeuge mit Version

Installiert im Scratchpad der Session, nicht im Repository. Quelle in Klammern.

| Werkzeug | Version | Zweck |
|---|---|---|
| gitleaks | 8.30.1 (`go install github.com/zricethezav/gitleaks/v8`) | Geheimnisse über alle Refs |
| trufflehog | 3.99.2 (aus dem Quelltext des Tags gebaut) | Geheimnisse, `--no-verification` |
| zizmor | 1.31.0 (PyPI) | Angriffsfläche der Workflows |
| actionlint | 1.7.12 (`go install github.com/rhysd/actionlint`) | Syntax der Workflows (die CI nutzt 1.7.7) |
| shellcheck | 0.11.0 (PyPI `shellcheck-py`) | Shell-Skripte |
| reuse | 6.2.0 (PyPI) | Lizenzzuordnung nach REUSE 3.3 |
| lychee | 0.24.2 (`cargo install`) | interne Links, `--offline` |
| hyperfine | 2.0.0 (`cargo install`) | Zeitmessung der Kommandozeile |
| cargo-machete | 0.9.2 | ungenutzte Abhängigkeiten |
| cargo-llvm-cov | 0.9.1 (`cargo install`, Komponente `llvm-tools`) | Abdeckung |
| cargo-deny | 0.20.2 | Lizenzen, Quellen |
| Rust, Node, pnpm, Python, uv, Go | 1.99.0, 22.22.0, 10.28.0, 3.13, 0.11.32, 1.24.7 | Bau und Prüfung |

Nicht verfügbar in der Cloud-Umgebung: Downloads von GitHub-Releases (gesperrt, daher aus dem Quelltext gebaut), `perf` (Kernel passt nicht zu den apt-Paketen), der Secret-Scanner von GitHub (`run_secret_scanning` meldet, dass für das private Repository Advanced Security fehlt).

## Teil A: Was ein Fremder sehen würde

Prüfgrundlage: Spiegelklon (`git clone --mirror`, nicht flach) von `origin` am 15. Oktober 2026: 246 Commits über alle Refs, 4 Zweige, 4 Tags, keine `refs/pull/*` (es gibt keinen Pull Request), 3554 Objekte, davon 1707 Blobs mit 36,9 MB in allen Versionen, Pack 2,8 MiB. Alle Funde stehen nur mit Datei, Commit und Art, nie mit dem Wert.

### Ergebnis der Scanner

| Prüfung | Ergebnis |
|---|---|
| gitleaks `git` über alle Refs | 230 Commits gescannt (die übrigen sind Merges ohne eigenen Inhalt), 5,97 MB, **kein Treffer** |
| trufflehog `git --no-verification` | 3856 Chunks, 6,1 MB, 0 bestätigte, 0 unbestätigte, **kein Treffer** |
| eigene Muster in allen Blobs (PEM, JWT, GitHub-, Google-, Slack-, AWS-, Anthropic-Token, Updater-Schlüssel, Token-Felder) | **kein Treffer** |
| GitHub-Secret-Scanner (`run_secret_scanning`) | nicht möglich: Advanced Security im privaten Repository nicht aktiv. Nach der Umstellung Secret-Scanning einschalten und den Bericht ansehen (Liste unten) |
| Plausibilität der Scanner | Ein künstliches GitHub-Token (zufällig erzeugt, nur im ersten von zwei Commits, im zweiten gelöscht) in einem Wegwerf-Repository wird von beiden gefunden: gitleaks 1 Treffer, trufflehog 1 unbestätigter Treffer (Exit 183). Die Nulltreffer am Spiegelklon sind damit aussagekräftig |

Einordnung der Dinge, die wie Geheimnisse aussehen und keine sind:

| Fund | Ort | Einordnung |
|---|---|---|
| Öffentlicher Schlüssel des Updaters (minisign, Base64) | `app/src-tauri/tauri.conf.json` | Fehlalarm: öffentlicher Teil, gewollt im Repository. Der private Schlüssel liegt nur als Actions-Secret (nicht im Repository, nicht prüfbar) |
| Kennungen der Garmin-Apps (`GARMIN_CONNECT_MOBILE_..._DI`, `GarminConnect`) | `crates/garmin` | Fehlalarm: öffentliche Anwendungskennungen, kein Geheimnis. Rechtlich siehe Garmin-Hinweis im README |
| JWT-ähnliche Zeichenketten, Token-Marker, `Bearer` | Tests in `crates/garmin`, `crates/mcp` | synthetisch: werden im Test aus Einzelteilen gebaut oder sind erkennbare Marker (`MARKER-...`) |
| IDs mit 9 bis 12 Ziffern | Tests, `docs/SPEC.md` | synthetisch (`12345678901`, `98765432101`, Prüfwerte von CRC-16) |
| Koordinaten `52.52...`, `13.40...` | `crates/mcp/tests`, `crates/cli/tests` | synthetisch: Testmarker, die nie in einer Ausgabe vorkommen dürfen |
| Seriennummer `4242424242` | `crates/mcp/tests` | synthetisch (Marker) |
| E-Mail-ähnliche Zeichenketten | Tests, Skills | synthetisch: reservierte Domains (`example.*`, `.invalid`), URL-Beispiele mit Benutzerinfo, Beispiele aus den Fremd-Skills |

### Binärreste

Jedes Blob jeder Version wurde nach Magic Bytes geprüft (`git rev-list --objects --all`, `git cat-file`), nicht nach Endung.

| Art | Anzahl Blobs | Pfade |
|---|---|---|
| FIT (".FIT" ab Byte 8) | 0 | |
| SQLite | 0 | |
| ZIP, gzip, 7z | 0 | |
| PNG, ICO, ICNS | 6 | `app/src-tauri/icons/*` (eigenes Symbol aus `app-icon.svg`) |
| sonstige Binärdateien | 0 | |

Die 30 größten Blobs sind alle `docs/SPEC.md` in verschiedenen Versionen (245 bis 321 KB). Nichts anderes ist größer als 245 KB. Kein Pfad unter `local/` außer `local/README.md`, keine `.env`, keine Token-Datei, keine Datenbank in irgendeiner Version.

### Personenbezug und Metadaten

| Prüfung | Ergebnis |
|---|---|
| Namen und Adresse des Eigentümers in allen Blob-Versionen | Windows-Kontoname im Pfad von SPEC 1.5 (siehe R1); der Klarname im Urheberrechtsvermerk von `LICENSE-MIT` (siehe R3); sonst keine |
| Orte, Koordinaten außerhalb synthetischer Werte | keine; die Koordinaten in Tests sind Marker |
| Geräteseriennummern, Garmin-Konto- oder Aktivitäts-IDs | keine echten; Tests nutzen erkennbar erfundene Werte |
| Körper- und Leistungswerte, die nach echten Messungen aussehen | keine; Beispiele sind runde Handrechenwerte (FTP 250 W, Ruhepuls 50) |
| `docs/tasks/L1-lokal-garmin-sonde.md` | enthält nur Ablauf und Zahlen der Aufrufe (271, 255, 14), keine Werte |
| `docs/garmin-feldabdeckung.md` | nur Feldnamen und Datentypen. Die Spalte "Historie ab" nannte genaue Daten aus dem echten Konto (4 verschiedene); jetzt auf das Jahr gerundet (R9) |
| Testdaten und Snapshots (`crates/*`, `bridge/tests`, `app/ui`) | synthetisch: feste Testdaten (13. Oktober 2026, UUID aus Nullen), keine FIT-Dateien, keine Datenbanken |
| Commit-Texte (alle Refs) | keine Namen, keine Adressen außer `noreply@anthropic.com`, keine Pfade; 214 Texte enthalten einen Link auf eine Claude-Sitzung (5 verschiedene Sitzungen) |
| Autor- und Committer-Adressen | 225 Commits mit `noreply@anthropic.com`; **21 Commits mit der privaten Adresse des Eigentümers** (Autor und Committer, bei 20 mit einem Kurznamen, bei 1 mit vollem Namen); 3 verschiedene Autorennamen |
| Trailer | 235 `Co-Authored-By`, 214 `Claude-Session` |

### Risikotabelle

Schwere: hoch = Geheimnis oder unwiderruflicher Schaden; mittel = Personenbezug in der Geschichte, nicht ohne Umschreiben zu entfernen; niedrig = sichtbar, harmlos oder leicht zu beheben.

| Nr. | Fund | Ort | Schwere | Beleg | Maßnahme |
|---|---|---|---|---|---|
| R1 | Windows-Kontoname (Teil des Klarnamens) im Arbeitsordnerpfad | `docs/SPEC.md` Abschnitt 1.5, seit Commit c809720, in allen 70 Versionen | mittel | Blob-Scan (Muster Benutzerpfad), `check-history.sh` | Aktueller Stand: Platzhalter (erledigt, Teil B). Geschichte: **Blocker**, Wahl unten |
| R2 | Private Mailadresse als Autor und Committer | 21 von 246 Commits | mittel | `git log --all --format=raw`, nur gezählt | Ab jetzt noreply-Adresse einstellen (Eigentümer). Geschichte: **Blocker**, Wahl unten |
| R3 | Klarname im Urheberrechtsvermerk | `LICENSE-MIT` (1 Version) | niedrig, Entscheidung | Namensvergleich mit den Autoren | Unverändert gelassen: Der Vermerk ist die rechtliche Zuordnung. Eigentümer entscheidet, ob Klarname, Handle oder "Lastwerk contributors" |
| R4 | GitHub-Handle in Pfaden und URLs | `Cargo.toml`, `tauri.conf.json` (Update-Endpunkt), SPEC, Tasks | niedrig | Textsuche | Bleibt: technisch nötig, ohnehin öffentlich |
| R5 | Verweise auf Claude-Sitzungen in Commit-Texten | 214 Commits, 5 Sitzungen | niedrig | Zählung | Link braucht Anmeldung bei Anthropic, nennt aber private Sitzungen. Bleibt; Eigentümer kann den Trailer künftig abschalten |
| R6 | Commit-Texte nennen Kostenlimit des Kontos und fehlenden Mac | Commits 49eaa32, 1162896 | niedrig | Lesen der Texte | Bleibt (Geschichte); Hinweis im Bericht |
| R7 | Issue #1 mit freiem Text des Eigentümers (persönliche Wunschformulierung, Tippfehler) wird sichtbar; SPEC, T30 und `auto-release.yml` verweisen darauf | GitHub, Issue #1 | niedrig | `list_issues` | Eigentümer entscheidet: schließen, Text glätten oder lassen. Verweise im Repository sind in Teil D neutral gefasst |
| R8 | Veröffentlichte Release-Notizen (v1.0.0 bis 1.0.2) nennen noch macOS (Gatekeeper); Release v1.0.2 trägt ein `.dmg`; ein Entwurf v0.1.1 (nur für Schreibberechtigte sichtbar) | GitHub, Releases | niedrig | `list_releases`, `get_latest_release` | Falsche Aussage, kein Datenschutzproblem. Notiztext der Vorlage ist schon korrigiert; alte Notizen und das `.dmg` kann der Eigentümer bearbeiten bzw. löschen; Entwurf löschen |
| R9 | Genaue Daten aus dem echten Garmin-Konto (frühestes Datum je Datenart) | `docs/garmin-feldabdeckung.md` | niedrig | Lesen der Tabelle | Im aktuellen Stand auf das Jahr gerundet (erledigt). Alte Versionen bleiben in der Geschichte (gehört zur Wahl zu R1, geringes Gewicht) |
| R10 | Artefakte der CI: `ui-screenshots` (11,7 MB, Oberfläche mit Mock-Daten) und Debug-Binary für Linux (34,7 MB), 90 Tage | GitHub, Actions | niedrig | `list_workflow_run_artifacts` | Inhalt nicht prüfbar (Download-Host für die Session gesperrt). Screenshots entstehen aus dem Mock-Backend; Debug-Binary kann Pfade des Runners enthalten, keine Personendaten. Vor der Umstellung alte Artefakte löschen (Liste unten) |
| R11 | 105 Actions-Läufe mit Commit-Titeln, Akteur-Handle; die letzten Läufe ohne Protokoll (Jobs starten nicht) | GitHub, Actions | niedrig | `list_workflow_runs`, `get_job_logs` (404) | Läufe nennen nur Commit-Texte und den öffentlichen Handle. Alte Läufe auf Wunsch löschen |
| R12 | Workflows: 31 Actions nur per Tag gepinnt, `secrets: inherit`, drei Caches im Release-Build, Token in Checkouts gespeichert, `workflow_run` | `.github/workflows` | hoch (zizmor), nach Analyse mittel | zizmor 1.31.0: 58 Funde (35 hoch, 1 mittel, 14 niedrig, 1 Info, 7 unterdrückt) | Behoben in Teil B (siehe Tabelle unten) |
| R13 | Keine Geheimnisprüfung durch GitHub möglich | GitHub | niedrig | Meldung des Werkzeugs | Nach der Umstellung Secret-Scanning und Push-Protection einschalten (Eigentümer) |
| R14 | Installer aus dem Release nicht prüfbar | GitHub, Releases | niedrig | Download liefert 404 (privates Repository) | Ersatz: lokaler Release-Build der Kommandozeile mit `strings` (Teil C). Prüfung des echten Installers nach der Umstellung wiederholen (Liste unten) |
| R15 | Garmin-Zugang über eine inoffizielle Schnittstelle, Garmin-Name | `crates/garmin`, README | niedrig (rechtlich) | Lesen | Markenhinweis und Hinweis auf Nutzungsbedingungen im README (Teil D) |
| R16 | Actions-Jobs starten nicht (Zahlung oder Ausgabenlimit des Kontos, seit 15:33 Uhr am 15. Oktober 2026) | GitHub | Betrieb | Läufe 58 bis 63 der CI: Jobs scheitern nach 1 bis 3 Sekunden ohne Schritte | CI auf `main` bleibt rot, bis das Limit behoben oder das Repository öffentlich ist. Siehe Abnahme |

### CI als Angriffsfläche

Antwort auf die Frage zu `auto-release.yml`: Ein Fork-Pull-Request erreicht weder einen Release noch den Signaturschlüssel.

- `pull_request` läuft nur in `ci.yml` und `bridge.yml`, mit `contents: read` und ohne Geheimnisse (für Forks stellt GitHub keine bereit). Dort steht kein Schreibrecht.
- `workflow_run` in `auto-release.yml` startet nur nach einer CI, deren Ereignis ein **Push** war (`event == 'push'`, nie `pull_request`), auf `main`, mit einem Quell-Repository gleich dem eigenen. Ein Push in das Repository können nur Personen mit Schreibrecht auslösen. Der Checkout führt keinen Code aus dem Commit aus, er liest Tags, Dateinamen und `catalog/changelog.json`. Das Muster der Version wird in `release.yml` erneut geprüft.
- Die Geheimnisse (`TAURI_SIGNING_PRIVATE_KEY`, `..._PASSWORD`) lesen nur die Schritte "Konfiguration für den Release" und "Build" in `release.yml`, der Build läuft nur im Job `build` hinter `check-tag` und `advisories`. Aufrufer geben sie jetzt einzeln statt mit `secrets: inherit` weiter.
- Ausdrücke mit `github.event.*` stehen nur in `env` oder `if`, nie in `run`. Einziger Fund (`steps.config.outputs.overlay`, `matrix.bundles` in einer `run`-Zeile) ist auf `env` umgestellt.
- Restrisiko, das GitHub-Einstellungen betrifft: Wer Schreibrecht hat, kann `release.yml` auf einem Zweig ändern und über `workflow_dispatch` mit Secrets laufen lassen. Abhilfe ist eine **Umgebung** (Environment) `release` mit Zweigbeschränkung auf `main`, in die der Schlüssel verschoben wird (Liste unten).

Ergebnis der Härtung (zizmor 1.31.0, Persona `regular`):

| Befund | vorher | nachher |
|---|---|---|
| `unpinned-uses` (Actions nur per Tag) | 31 (hoch) | 0 (alle auf Commit-SHA mit Versionskommentar) |
| `cache-poisoning` | 3 (hoch) | 0 (kein Cache im Release-Build) |
| `dangerous-triggers` (`workflow_run`) | 1 (hoch) | 0 offen; bewusst mit `# zizmor: ignore[...]` und Begründung im Workflow, zusätzliche Bedingung auf Herkunftsrepository |
| `secrets-inherit` | 1 (mittel) | 0 |
| `artipacked` (Token im Checkout) | 12 (niedrig) | 0 (`persist-credentials: false`) |
| `template-injection` | 1 (Info) | 0 |
| `self-repository` | 2 (niedrig) | 2 (bewusst offen: die Schreibweise `$/...` kennt actionlint noch nicht) |
| actionlint 1.7.12 | 0 Fehler | 0 Fehler |

Mit der Persona `auditor` bleiben nur Hinweise der Stufe "help" (Gleichzeitigkeitsgrenzen, Kommentare zu Rechten); sie sind keine Befunde. Minimale Rechte: Jeder Workflow setzt `contents: read` auf oberster Ebene, Schreibrechte haben nur die Jobs `release` (Anlegen des Entwurfs) und `publish`.

Die CI führt zizmor ab jetzt selbst aus (`--min-severity medium`, Job `guards`).

## Teil B: Umsetzung ohne Änderung der Geschichte

| Punkt | Ergebnis |
|---|---|
| B1 CI härten | siehe Tabelle oben. Verhalten für Pushes auf `main` und `claude/**` unverändert |
| B2 `scripts/check-privacy.sh` | prüft zusätzlich den Index nach Magic Bytes (FIT, SQLite, Archive) und Mustern (`scripts/privacy-patterns.txt`); POSIX sh, keine neue Abhängigkeit, läuft in der CI auch unter Windows (Git Bash); Tests mit zur Laufzeit erzeugten Fundstücken (`scripts/test-check-privacy.sh`, 30 Fälle) |
| B2 `scripts/check-history.sh` | wiederholt die Prüfung auf einem Spiegelklon: gitleaks, trufflehog, Magic Bytes aller Blobs, Muster, Benutzerpfade und Adressen in allen Diffs, Metadaten; Tests in `scripts/test-check-history.sh` (15 Fälle); geprüfte Altfunde stehen in `scripts/history-allowlist.txt` (nur Personenbezug, nie Schlüssel, Token oder Binärdateien) |
| B3 Rechtliches | Markenhinweis zu Garmin im README, `SECURITY.md` (private Schwachstellenmeldung auf GitHub, keine Mailadresse), `REUSE.toml` und `LICENSES/` (`reuse lint` ohne Befund, 539 von 539 Dateien), `THIRD-PARTY-NOTICES.md` (Schrift Cairo OFL-1.1, Skills von Emil Kowalski MIT) |
| B4 Personenbezug im aktuellen Stand | R1 (SPEC 1.5) und R9 (Feldabdeckung) bereinigt. R3 bewusst unverändert |

Neue Abhängigkeiten: keine (Laufzeit und Entwicklung). Werkzeuge der Prüfung stehen nur in dieser Datei.

Der Download der OFL-Lizenz der Schrift ins Installationspaket ist nicht umgesetzt (kein Windows-Bau in der Cloud möglich); der Installer enthält `LICENSE-MIT`. Offener Punkt für einen Lauf mit echtem Bau: `THIRD-PARTY-NOTICES.md` als Ressource in `tauri.release.conf.json` aufnehmen.

## Teil C: Codequalität und Effizienz

### Bestandsaufnahme (ohne Sperre)

| Prüfung | Ergebnis |
|---|---|
| `cargo clippy --workspace --all-targets -- -W clippy::pedantic` (Bericht) | rund 1450 Meldungen, davon 473 `must_use_candidate`, 384 `missing_errors_doc`, 150 `cast_possible_truncation`, 79 `cast_precision_loss`, 74 `assert_is_empty`, 35 `too_many_lines`; der Rest einstellig bis zweistellig. Es ist Stil und Dokumentation, kein Fehler: Kein Fund weist auf toten Code, Laufzeitfehler oder Rechenfehler hin. Die Casts in der Zahlenarbeit (Zeitreihen, Sekunden) sind bekannt und durch Referenzwerte abgesichert. Nicht in großem Umfang behoben (keine reinen Stiländerungen) |
| `cargo machete` (Wurzel und Tauri-Projekt) | keine ungenutzte Abhängigkeit |
| `pnpm lint`, `pnpm check` | 0 Fehler, 0 Warnungen (455 Dateien) |
| Vite-Build | 608 KB gesamt: JS 484 KB (157 KB gzip), CSS 64 KB (12 KB gzip), Schrift Cairo nur `latin` und `latin-ext` (51 KB) |
| Abdeckung (`cargo llvm-cov --workspace`, ohne Tests, Beispiele und testkit) | 93,9 % der Zeilen (17 519 Zeilen, 1077 nicht ausgeführt), 92,3 % der Regionen, 91,7 % der Funktionen. Die schwächsten Dateien sind Anbindung der Kommandozeile (`cli/src/chat.rs` 56 %, `commands.rs` 72 %, `garmin.rs` 79 %) und Fehlertypen; in `crates/core` liegt keine Datei mit Formel unter 78 % (nur `plan/error.rs`). Keine Formel ohne Test, deshalb kein Test ergänzt |

### Messung und Optimierung

Ausführlich in [T38-anhang-messung.md](T38-anhang-messung.md): Messaufbau, Befehle zum Nachmessen (`sh scripts/bench.sh`), Hotspots, Änderungen und Gleichheitstests. Zusammenfassung der maßgeblichen Nachmessung auf `main` mit T33 und T37 (4 Kerne, Release-Build, synthetische Daten aus `crates/testkit`, Median, in Klammern Minimum, ms):

| Messung | vorher | nachher |
|---|---|---|
| Erstimport 10 Fahrten zu 5 h (180 000 Messpunkte) | 66,0 (60,9) | 40,9 (35,4) |
| Erstimport 50 Fahrten zu 5 h (900 000 Messpunkte) | 336,2 (300,4) | 140,2 (135,3) |
| Erstimport 1 Jahr, 300 Einheiten (1,1 Mio. Messpunkte) | 387,7 (323,5) | 182,5 (154,3) |
| Erstimport 5 Jahre, 1500 Einheiten (5,6 Mio. Messpunkte) | 1896,8 (1558,2) | 794,6 (760,8) |
| zweiter Import, 1500 unveränderte Dateien | 100,0 (82,4) | 98,8 (82,4) |
| `pmc` über 5 Jahre | 6,7 (6,1) | 6,4 (5,3) |
| `plan` über 12 Wochen | 10,2 (9,1) | 11,2 (8,8) |

Hotspots und Maßnahmen: Die FIT-Prüfsumme (CRC-16) kostete gut ein Fünftel des Imports (acht Tabellen je Schritt statt Halbbyte-Verfahren), die Plausibilitätsprüfung der Samples lief je Aktivität bis zu achtmal mit Kopien (jetzt einmal, `*_prepared` in `crates/core`), und der Import bereitete rund ein Dutzend SQL-Anweisungen je Aktivität neu vor (kleiner Zwischenspeicher `statements::Db`, keine neue Abhängigkeit). Quadratische Verfahren gibt es nicht: NP ist ein gleitendes Mittel in O(n), die Leistungskurve nutzt Präfixsummen, die Mean-Max-Kurve über alle Dauern existiert im Produkt nicht. Alle abgeleiteten Tabellen (`activity_load`, `activity_zone_time`, `activity_best_power`, `activity_best_effort`) sind zwischen altem und neuem Binary bitgleich (1500 Aktivitäten, 20 630 Zonenzeilen, 14 738 Leistungswerte, 1183 Bestzeiten). Kein Referenzwert, Snapshot oder Test wurde geändert; 809 Tests grün. `crates/core` ist weiter rein (kein Dateisystem, Netz, Zeit, Zufall), `bridge/` unberührt, keine neue Abhängigkeit.

Bewusst nicht optimiert: Programmstart und Datenbank dominieren `pmc`, `plan` und den zweiten Import; der Parser je Nachricht (drei Versuche ohne messbaren Gewinn, zurückgenommen); SHA-256 (unter 10 % des Imports).

## Teil D: Texte und Doku

| Punkt | Ergebnis |
|---|---|
| D1 README | Neu als Eingang für Fremde: was Lastwerk ist, Status mit ehrlicher Angabe zur Erprobung, Installation, Bauanleitung, Datenschutz, Garmin- und Strava-Hinweis (nicht verbunden, inoffizielle Schnittstelle, eigenes Risiko), Lizenz, Sicherheit, Verweise auf `docs/`, englischer Schluss. Kommandozeile und MCP-Server stehen unverändert in `docs/cli.md` und `docs/mcp.md`. Die Bauanleitung wurde in einem frischen Klon abgearbeitet (siehe Abnahme) |
| D2 Kosten und Minuten | `CLAUDE.md` (Schritt 6), `docs/remote-sessions.md`, `docs/README.md`, SPEC 11.6 und die Workflow-Kommentare sagen nicht mehr, ein Push verbrauche Minuten des privaten Repositorys. "Ein Push je Session" bleibt, begründet mit CI-Last und Release-Takt. Historisches steht als datierte Notiz. macOS-Builds sind nicht wieder eingeführt; **Option:** `macos-latest` lässt sich als dritte Zeile der Matrix in `ci.yml` und `release.yml` wieder aufnehmen, in öffentlichen Repositories ohne Minutenkosten (der Code für Datenordner und Schlüsselbund ist vorhanden, aber ungeprüft) |
| D3 SPEC und Aufträge | Wörtliche Zitate und Hinweise auf den Eigentümer als Person sind sachlich gefasst ("Entscheidung vom 15. Oktober 2026"), Wahrheit, Quellen und Abnahmewerte unverändert. SPEC 1.5 nennt die neuen Prüfungen in der Veröffentlichungs-Checkliste. Links: `lychee --offline` ohne Fehler im Bestand (siehe Abnahme) |
| D4 Sprachdateien | `catalog/locales/de.json` und `en.json` geprüft: kein Personenbezug, keine interne Notiz, keine Gedankenstriche, kein veralteter macOS-Hinweis; unverändert. `scripts/check-i18n.sh`: 997 Schlüssel gleich |

Offene Entscheidung dazu (nicht umgesetzt, weil es die App betrifft): Soll die App den Hinweis "nicht mit Garmin verbunden" in den Einstellungen zeigen? Bisher steht er in README und Release-Notizen.

## Abnahme

Stand 15. Oktober 2026, Zweig `claude/happy-ptolemy-ach4fq`. Erfüllt heißt: mit Befehl geprüft.

| Kriterium | Ergebnis |
|---|---|
| Spiegelklon vom Server nach dem Push (`sh scripts/check-history.sh https://github.com/flor1xn/lastwerk.git`): gitleaks und trufflehog ohne echten Treffer | **erfüllt.** gitleaks: 249 Commits, 6,15 MB, kein Leck. trufflehog: 4072 Chunks, 0 bestätigt, 0 unbestätigt. Eigene Muster: keine Fundstelle |
| Magic-Byte-Prüfung ohne FIT, Datenbank oder Archiv | **erfüllt.** 1781 Blobs: 0 FIT, 0 SQLite, 0 ZIP/gzip/7z; 5 Bilder und 1 ICNS (App-Symbole). Keine bekannten Fixtures nötig, weil es keine gibt |
| Gesamtergebnis des Spiegelklon-Laufs | **Fund, wie im Blocker beschrieben:** Windows-Kontoname im Pfad von SPEC 1.5 (Commits c809720, 9f8f5c2, Datei `docs/SPEC.md`) und 42 Adressangaben ohne noreply in 21 Commits. Mit `--metadaten-ok` bleibt allein der SPEC-Pfad. Beides ist nur durch Umschreiben oder Neuanfang aus der Geschichte zu entfernen (Wahl des Eigentümers) |
| zizmor ohne Befund der Stufe medium oder höher | **erfüllt.** `zizmor --offline --min-severity medium`: keine Befunde (1 bewusst unterdrückt: `workflow_run` mit Begründung im Workflow). Die CI führt zizmor jetzt selbst aus |
| actionlint ohne Fehler | **erfüllt.** 1.7.12 lokal, 1.7.7 in der CI (gleiche Syntax) |
| Messwerte vorher und nachher, keine Verschlechterung, Referenzwerte unverändert | **erfüllt.** Tabelle in Teil C und im Anhang. Erstimport 5 Jahre 1558 ms auf 761 ms (Minimum), übrige Pfade gleich innerhalb des Rauschens. 809 Tests grün, kein Snapshot und keine Referenzkonstante geändert, alle abgeleiteten Tabellen bitgleich |
| Frischer Klon baut nach README | **erfüllt.** Klon des Zweigs vom Server (Commit 1f88de8) in einem leeren Ordner: `cargo build --workspace --locked` (1 min 4 s), `cargo test -p lastwerk-core --locked` (202 Tests), `cargo run -p lastwerk-cli -- --help`, `pnpm --dir app/ui install --frozen-lockfile`, `pnpm --dir app/ui build`. Das Tauri-Projekt baut `check-all.sh` (siehe unten) |
| `sh scripts/check-all.sh` mit Exit 0 | **erfüllt.** "Ergebnis: alles grün", nichts übersprungen: Datenschutz, Tests der Prüfungen, Isolation, Sprachdateien, Rust fmt/clippy/Tests im Wurzel-Workspace, Tauri-Projekt (fmt, clippy, Tests, mit WebKitGTK), cargo-deny (beide Lockdateien), actionlint, zizmor, Oberfläche (lint, check ohne Fehler, Tests, Build, Playwright), bridge (ruff, 347 Tests) |
| CI auf `main` grün unter Linux und Windows | **nicht erfüllt, nicht durch den Code verursacht.** Seit dem 15. Oktober 2026 um 15:33 Uhr starten Actions-Jobs des Kontos nicht mehr (Zahlung oder Ausgabenlimit): Alle Jobs der Läufe 58 bis 63 auf `main` und 65 bis 68 auf diesem Zweig enden nach 1 bis 6 Sekunden ohne ausgeführten Schritt und ohne Protokoll (404). Der letzte grüne Lauf war Nr. 51 (Linux, Windows, macOS). Deshalb wurde `main` nicht angefasst: CLAUDE.md gibt `main` erst nach grüner CI auf dem Zweig frei. Nach der Umstellung auf öffentlich (kostenlose Standard-Runner) oder nach Behebung des Limits den Zweig neu starten, bei grüner CI nach `main` übernehmen |
| `auto-release` geprüft | **teilweise.** Logik gelesen und gehärtet (Bedingungen, Rechte, explizite Geheimnisse, Abfrage der Spitze über die API), actionlint und zizmor sauber. Der letzte Lauf (Nr. 10 auf `main`, 959546b) war "skipped", weil die CI davor rot war. Ein echter Durchlauf mit den geänderten Schritten (kein Cache, Werte über `env`, `gh api` statt `git ls-remote`) ist erst nach grüner CI auf `main` beobachtbar; **den ersten Lauf danach ansehen** |

## Manuelle Schritte des Eigentümers

In dieser Reihenfolge; die Schritte 1 bis 4 vor der Umstellung der Sichtbarkeit.

1. **Wahl zur Geschichte** (siehe Blocker): (a), (b) oder (c). Bei (b) oder (c) danach `sh scripts/check-history.sh <Quelle>` auf einem Spiegelklon wiederholen.
2. **Noreply-Adresse für Commits einstellen:** auf GitHub unter Settings, Emails "Keep my email addresses private" und "Block command line pushes that expose my email" einschalten, lokal `git config user.email <ID>+<Handle>@users.noreply.github.com` setzen. Gilt nur für künftige Commits.
3. **Actions wieder lauffähig machen:** Zahlungs- oder Ausgabenlimit des Kontos prüfen (oder die Umstellung abwarten), danach den Zweig `claude/happy-ptolemy-ach4fq` neu starten und bei grüner CI nach `main` übernehmen.
4. **Aufräumen auf GitHub vor der Umstellung:** Entwurf `v0.1.1` löschen; Release-Notizen von v1.0.0 bis v1.0.2 und das `.dmg` an v1.0.2 bearbeiten bzw. entfernen (nennen macOS, den es nicht mehr gibt); alte Actions-Artefakte (`ui-screenshots`, Debug-Binary) und bei Bedarf alte Läufe löschen; Issue #1 schließen, glätten oder lassen; Urheberrechtsvermerk in `LICENSE-MIT` (Klarname, Handle oder "Lastwerk contributors") festlegen.
5. **Sichtbarkeit umstellen** (Settings, General, Danger Zone).
6. **Danach einschalten:** Secret scanning und Push protection; Private vulnerability reporting (Settings, Code security; ohne diesen Schalter hat `SECURITY.md` keinen Meldeweg); Dependabot alerts (die Konfiguration für Updates liegt in `.github/dependabot.yml`).
7. **Actions-Einstellungen:** "Fork pull request workflows from outside collaborators": Genehmigung für alle Externen verlangen; Workflow permissions: Standard nur Lesen; "Allow GitHub Actions to create and approve pull requests" aus.
8. **Schlüssel absichern:** `TAURI_SIGNING_PRIVATE_KEY` und sein Passwort aus den Repository-Geheimnissen in eine **Umgebung** `release` verschieben, deren Zweigregel nur `main` erlaubt, und in `release.yml` am Job `build` `environment: release` eintragen. Dann kann auch ein Zweig mit geändertem Workflow den Schlüssel nicht lesen.
9. **Issues und Pull Requests von außen:** entscheiden, ob Issues (und Discussions) offen sind; der README sagt "kein Support". Wer keine Beiträge will: Settings, Moderation, Interaction limits ("Limit to collaborators") oder Issues abschalten. Pull Requests von Forks laufen nur mit `contents: read` und ohne Geheimnisse.
10. **Branch-Schutz für `main`:** Der Ablauf pusht Sessions direkt nach `main` und der automatische Release hängt am Push-Ereignis. Eine Regel "Require status checks" oder "Require a pull request" würde den Direkt-Push blockieren; sie passt nur, wenn Sessions künftig über den Zweig und einen Pull Request mergen (Auto-Release läuft dann nach dem Merge). Verträglich mit dem heutigen Ablauf sind ein Ruleset mit "Block force pushes" und "Restrict deletions". Empfehlung: diese beiden Regeln, Statusprüfung als Pflicht erst, wenn der Ablauf auf Pull Requests umgestellt ist.
11. **Nach der Umstellung prüfen:** den Bericht des GitHub-Secret-Scanners ansehen (Security, Secret scanning); einen echten Release-Installer herunterladen und mit `strings` auf eingebettete Pfade, Namen und Adressen prüfen (R14); `sh scripts/check-history.sh https://github.com/flor1xn/lastwerk.git` wiederholen; den ersten automatischen Release beobachten.
12. **Optional:** Hinweis "nicht mit Garmin verbunden" in den Einstellungen der App (steht bisher nur im README); `THIRD-PARTY-NOTICES.md` als Ressource in den Installer aufnehmen (Lizenzhinweis der Schrift, braucht einen Windows-Bau zum Prüfen); macOS wieder in die Matrix aufnehmen (kostenlos im öffentlichen Repository, Code ungeprüft).

## Umsetzung von Weg (b)

Entscheidung des Eigentümers vom 15. Oktober 2026: Weg (b), neues Repository mit einem sauberen Anfangsstand. Weitere Entscheidungen dazu:

| Punkt | Entscheidung |
|---|---|
| Altes Repository | umbenannt in `lastwerk-archiv`, bleibt privat (Geschichte, Releases bis 1.0.2, Issue #1, Arbeitszweige) |
| Neues Repository | `flor1xn/lastwerk`, ein Anfangscommit ohne Vorgänger mit dem Stand dieses Zweigs; der Update-Endpunkt bleibt gleich |
| Signaturschlüssel | neues Schlüsselpaar (der alte private Schlüssel lag nicht mehr vor). Öffentlicher Teil in `tauri.conf.json`, privater Teil nur in der Umgebung `release` (Zweigregel `main`). Versionen bis 1.0.2 werden einmal von Hand neu installiert; Hinweis im Changelog 1.0.3 |
| Urheberrechtsvermerk | "Lastwerk contributors" (`LICENSE-MIT`, wie in `REUSE.toml`) |
| Autor der Commits | Handle mit noreply-Adresse von GitHub |
| Issues, Pull Requests | Issues, Wiki, Projekte und Discussions aus; Interaction limit "nur Collaborators" (läuft nach 6 Monaten ab) |
| Erster Release | v1.0.3 über `auto-release.yml`, das ohne früheren Tag jetzt die höchste Version aus `catalog/changelog.json` nimmt |
| macOS | bleibt draußen |
| Trailer `Claude-Session` | abgeschaltet (`.claude/settings.json`, `attribution`; Regel in `CLAUDE.md`) |

Commit-Hashes in dieser Datei und in älteren Aufträgen beziehen sich auf das Archiv. `scripts/history-allowlist.txt` ist leer, weil die Altfunde nur dort liegen.

### Abnahme von Weg (b)

Stand 16. Oktober 2026, lokal unter Windows mit gitleaks 8.30.1, trufflehog 3.99.2, actionlint 1.7.12, zizmor 1.31.0 (scoop, uv).

| Kriterium | Ergebnis |
|---|---|
| `check-history.sh` auf einem Repository nur mit dem Anfangscommit, vor dem Push | **erfüllt.** gitleaks kein Leck, trufflehog 810 Chunks ohne Treffer, keine Muster, kein Personenbezug, 0 von 2 Adressen ohne noreply, 0 Sitzungsverweise |
| `check-history.sh https://github.com/flor1xn/lastwerk.git` nach dem Push | **erfüllt.** Ohne Fund (4 Commits: Anfang und drei Zweige von Dependabot), 0 von 8 Adressen ohne noreply |
| Secret-Scanner von GitHub | eingeschaltet mit Push-Protection, 0 Meldungen |
| `sh scripts/check-all.sh` | alles grün außer zwei Tests in `bridge/`, die Symlinks anlegen: Unter Windows ohne Entwicklermodus fehlt dafür das Recht (WinError 1314). `bridge/` ist unverändert und läuft in der CI unter Ubuntu, dort grün |
| CI auf `main` (Linux, Windows) und Bridge | **erfüllt**, beide grün beim ersten Lauf |
| `auto-release.yml` | **erfüllt.** Ohne früheren Tag die Version aus dem Changelog, v1.0.3 mit Installer für Windows (NSIS) und Linux (deb, AppImage) veröffentlicht |
| `latest.json` | erreichbar unter `releases/latest/download/latest.json`, Version 1.0.3, Signatur mit der Schlüssel-ID des neuen öffentlichen Schlüssels |
| Echter Installer (R14) | `Lastwerk_1.0.3_x64-setup.exe` mit 7-Zip entpackt, `strings` (ASCII und UTF-16) über Installer und alle Dateien: keine Benutzerpfade, kein Klarname, keine private Adresse (einzige Adresse aus OpenSSL) |
| Einstellungen | Issues, Wiki, Projekte, Discussions aus; Interaction limit nur Collaborators bis 15. April 2027; Workflows nur Lesen, keine Freigabe von Pull Requests durch Actions; Fork-PRs aller Externen nur nach Freigabe; Ruleset "main schützen" (kein Force-Push, kein Löschen); private Schwachstellenmeldung und Dependabot-Warnungen an; Schlüssel nur in der Umgebung `release` (Zweigregel `main`), keine Repository-Secrets |

Korrektur dabei: `scripts/check-history.sh` übergab trufflehog unter Git Bash einen MSYS-Pfad (`/tmp/...`), den das Windows-Programm nicht kennt; der Abbruch mit Exit 1 galt als Treffer. Jetzt Windows-Pfad über `cygpath`, und nur Exit 183 gilt als Treffer.

Offen beim Eigentümer: E-Mail-Datenschutz im GitHub-Konto ("Keep my email addresses private", "Block command line pushes that expose my email"), Freigabe des neuen Repositorys für die Claude-GitHub-App, falls sie auf einzelne Repositories beschränkt ist, Sicherung von Schlüssel und Passwort aus dem Benutzerordner `.tauri`, einmalige Neuinstallation von 1.0.3 über installierte 1.0.x, Verlängerung des Interaction limit im April 2027. Ob der Trailer `Claude-Session` wirklich wegfällt, zeigt der nächste Commit einer Cloud-Session.

## Blocker

- 15. Oktober 2026 (aufgelöst am selben Tag mit Weg (b), siehe oben): Die Geschichte enthielt Personenbezug, der sich nicht ohne Umschreiben entfernen lässt (R1, R2, mit R3 und R9 am Rand): der Windows-Kontoname im Pfad von SPEC 1.5 ab dem ersten Commit und die private Mailadresse in 21 Commits. Geheimnisse und Binärreste gibt es nicht. Die Geschichte wurde nicht verändert, es gab keinen Force-Push. Wahl des Eigentümers:

| Weg | Vorteile | Nachteile |
|---|---|---|
| (a) Geschichte bleibt, der Wert wird ungültig gemacht | Kein Eingriff, alle Hashes, Tags, Release-Verknüpfungen und Begründungen bleiben. Für Geheimnisse der richtige Weg (Token widerrufen) | Kontoname und Mailadresse sind kein widerrufbarer Wert: Sie bleiben dauerhaft abrufbar, auch in Forks, Archiven und Suchindizes; Mailadressen in Commits werden von Sammlern ausgelesen. Abhilfe nur für die Zukunft (noreply-Adresse, "Keep my email address private", "Block command line pushes that expose my email") |
| (b) Neues Repository mit einem sauberen Anfangsstand | Sauberste Lösung, nichts Altes bleibt erreichbar. Das Repository ist jung (alle Commits vom 13. bis 15. Oktober 2026), hat 1 Issue, keine Pull Requests, keine Forks und keine Sterne: der Verlust ist klein. `check-history.sh` bestätigt den Neuanfang. Der Update-Endpunkt der App (`.../releases/latest/download/latest.json`) bleibt gültig, wenn das neue Repository denselben Namen unter demselben Eigentümer trägt (altes vorher umbenennen oder löschen) | Die 246 Commits mit ihren Begründungen im Rumpf gehen verloren (die Spezifikation und die Aufträge bleiben als Dateien). Tags und Releases neu anlegen (Installer neu bauen), Secrets (`TAURI_SIGNING_PRIVATE_KEY` und Passwort) im neuen Repository neu setzen, Issue #1 ggf. neu anlegen. Bereits installierte Versionen finden Updates nur, wenn der Pfad gleich bleibt |
| (c) Umschreiben der Geschichte plus Löschantrag beim GitHub-Support | Die Geschichte bleibt als Text erhalten, Kontoname, Mailadresse und Klarname verschwinden aus allen Commits (z. B. mit `git filter-repo`: `--mailmap` für die Autoren, `--replace-text` für den Pfad in SPEC.md) | Alle Commit-Hashes ändern sich: die 4 Tags (v0.1.1, v1.0.0, v1.0.1, v1.0.2) und die 3 veröffentlichten Releases zeigen auf neue Commits, Actions-Läufe und das Issue verweisen auf alte Hashes. Force-Push durch den Eigentümer nötig (Branch-Schutz kurz abschalten). Alte Objekte bleiben auf GitHub per Hash abrufbar, bis der Support sie löscht und den Cache leert; ohne Löschantrag ist nichts gewonnen. Mehr Aufwand und mehr Fehlerquellen als (b) |

Empfehlung: (b), weil das Repository so jung ist und kaum etwas daran hängt; (a), wenn Kontoname und Mailadresse als unkritisch gelten. (c) lohnt nur, wenn die Geschichte wichtiger ist als der Aufwand. Alle drei Wege lassen die Arbeit dieser Session unberührt: Die Änderungen betreffen nur den aktuellen Stand und lassen sich in jedes neue Repository übernehmen. Nach jeder Wahl `sh scripts/check-history.sh` auf einem Spiegelklon wiederholen; ein Fund bei den Metadaten wird mit `--metadaten-ok` bewusst angenommen.
