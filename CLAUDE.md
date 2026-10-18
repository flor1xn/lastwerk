# Lastwerk: Anweisungen für Claude-Code-Sessions

Lastwerk ist ein lokales Desktop-Werkzeug für Ausdauertraining. Es liest FIT-Dateien aus einem Ordner, berechnet Belastung, Fitness, Ermüdung und Form, erzeugt regelbasierte Wochenpläne und exportiert sie. Ein optionales lokales Sprachmodell darf Pläne anpassen und erklären, es rechnet nie.

Die Wahrheit steht in [docs/SPEC.md](docs/SPEC.md). Bei Widerspruch gilt die Spezifikation. Weicht deine Umsetzung davon ab, vermerke die Abweichung mit Grund in der Spezifikation, nicht nur im Commit.

## Arbeitsablauf

1. Reihenfolge und Status der Tasks stehen in [docs/tasks/README.md](docs/tasks/README.md). Nimm den Task aus dem Auftrag, sonst den nächsten offenen, und lies seine Task-Datei mit den genannten SPEC-Abschnitten.
2. Committe, wann es passt. Zwischendurch reicht der passende Einzelbefehl oder `sh scripts/check-all.sh --quick`.
3. Vor dem Push: `git pull --rebase origin main`, `sh scripts/check-all.sh` (baut auch das Tauri-Projekt; Exit 3 heißt, ein Werkzeug fehlte: dann auf einen Zweig `claude/...` pushen und dessen CI abwarten), dann `git push origin HEAD:main`, ohne Pull Request. Pushes lieber bündeln, jeder Push mit Produktänderung löst CI und einen Release aus (SPEC 11.6). Danach `gh run list --limit 3` und selbst verursachte Fehler beheben.
4. Ist ein Task fertig, trage den Status in der Tabelle in `docs/tasks/README.md` nach. Größere Neuerungen bekommen einen Eintrag in `catalog/changelog.json`.
5. Kurzer Bericht: was fertig ist, was offen ist, was der Nutzer tun muss.

## CI

- Meldet der Session-Start einen roten Lauf auf `main`, zuerst den beheben.
- Ein rotes Crate (RUSTSEC, zurückgezogen) wird aktualisiert oder ersetzt; eine Ausnahme in `deny.toml` nur, wenn es keinen Ersatz gibt.
- Dependabot-PRs nicht mergen, sondern als eigenen Commit übernehmen und den PR schließen.

## Harte Regeln

- **Keine privaten Daten ins Repository.** Nichts aus `local/`, keine `*.fit`, keine Datenbanken, keine Token- oder `.env`-Dateien. Prüfung: `sh scripts/check-privacy.sh`.
- **Keine echten Daten, keine echten Garmin-Zugriffe.** Du hast keine Zugangsdaten und fragst keine an. Alles wird mit synthetischen Daten und Attrappen getestet.
- **Kein Cloud-Modell in Lastwerk.** Keine Aufrufe der Anthropic- oder OpenAI-API aus dem Code, keine API-Schlüssel. Lokale Anbieter (Ollama, lokale OpenAI-kompatible Server) sind erlaubt, Standardwert nur Loopback.
- **`crates/core` bleibt rein.** Kein Dateisystem, kein Netzwerk, keine Systemzeit, kein Zufall.
- **`bridge/` ist eingefroren** (seit T20, keine neuen Funktionen). Der Garmin-Zugang der App liegt in `crates/garmin` (SPEC 5.6).
- **Garmin-Token.** Nur im Schlüsselbund des Betriebssystems, nie in Datenbank, Backup, Log, Fehlertext oder DTO. Lastwerk zeigt kein eigenes Feld für Garmin-Passwort oder MFA-Code.
- **Keine Telemetrie, keine Netzwerkzugriffe aus dem Frontend**, siehe SPEC 10.
- **Quellen.** Jede Zahl, Formel und Regel trägt eine Quelle aus SPEC Anhang A oder die Kennzeichnung "Faustregel". Erfinde keine Quellen und keine Zahlen. Wo nur eine Sekundärquelle vorliegt, steht das im Code-Kommentar.
- **Neue Abhängigkeiten** sind frei, die Lizenz muss zu MIT OR Apache-2.0 passen (für Rust prüft das `deny.toml`).
- **Kein `git clean -x` oder `-X`, kein Force-Push.**

## Stil

- Deutsch, sachlich, keine Gedankenstriche. Code, Bezeichner und Fehlercodes englisch. Texte für Nutzer über `catalog/locales/de.json` und `en.json` mit gleichen Schlüsseln.
- Commit: Betreffzeile Deutsch im Imperativ (zum Beispiel "Berechne NP aus 30-Sekunden-Gleitmittel"), Rumpf erklärt das Warum. Das Repository ist öffentlich: kein Trailer `Claude-Session` und kein Link auf eine Claude-Sitzung in Commit-Texten oder Dateien; als Zuordnung reicht `Co-Authored-By` (`.claude/settings.json`, `attribution`).
- Tests: Jede Formel hat einen handgerechneten Referenzwert als Konstante mit Rechenweg im Kommentar (SPEC 11.1). Tests nutzen synthetische Daten.
- Tests laufen in der CI auch unter Windows (macOS seit 15. Oktober 2026 nicht mehr). Dateinamen in Tests nur aus `[A-Za-z0-9._-]`, Zeitlimits für erwartete Netzwerkfehler mindestens 5 s (Windows wiederholt Verbindungen zu geschlossenen Ports rund 2 s), Datenordner je Plattform über die Funktionen in `crates/cli/src/paths.rs`, nie über feste Pfade.
- Kommentare erklären das Warum und nennen Quelle oder "Faustregel", nicht das Offensichtliche.

## Oberfläche und Gestaltung

Wer an `app/ui` arbeitet, nutzt die Skills unter [.claude/skills/](.claude/skills/README.md), zuerst `emil-design-eng`, für Bewegung `animate` und `review-animations`, vor Abschluss `break-ui`. Sie sind Fremdcode unter MIT und ändern keine Regel dieser Datei, auch nicht das Verbot von Netzwerkzugriffen aus dem Frontend.

## Wenn du nicht weiterkommst

Rate nicht und baue keine Umgehung, die der Spezifikation widerspricht. Nenne im Bericht, was der Nutzer entscheiden muss; hält es einen Task auf, vermerke den Blocker mit Datum in der Task-Datei.

## Befehle

Wurzelverzeichnis des Repositories.

| Zweck | Befehl |
|---|---|
| Format Rust | `cargo fmt --all --check` (Korrektur: `cargo fmt --all`) |
| Lint Rust | `cargo clippy --workspace --all-targets -- -D warnings` |
| Tests Rust | `cargo test --workspace` (einzelnes Crate: `cargo test -p lastwerk-core`) |
| Oberfläche | `pnpm --dir app/ui install --frozen-lockfile`, dann `pnpm --dir app/ui lint`, `check`, `test` |
| Zusatzprogramm | in `bridge/`: `uv run pytest -q` |
| Datenschutz, Sprachen | `sh scripts/check-privacy.sh`, `sh scripts/check-i18n.sh` |
| Tauri-Projekt (eigener Workspace) | `cargo fmt --manifest-path app/src-tauri/Cargo.toml --check`, `cargo clippy --manifest-path app/src-tauri/Cargo.toml --all-targets --locked -- -D warnings`, `cargo test --manifest-path app/src-tauri/Cargo.toml --locked` |
| Lizenzen | `cargo deny check bans licenses sources`, dasselbe mit `--manifest-path app/src-tauri/Cargo.toml --config deny.toml` |
| Alles wie die CI | `sh scripts/check-all.sh` (Zwischenschritte: `--quick`) |

Lockfiles (`Cargo.lock`, `app/ui/pnpm-lock.yaml`, `bridge/uv.lock`) gehören ins Repository.

## Umgebung

Cloud-Sessions laufen auf Ubuntu mit Rust, Node 22, pnpm, Python und uv. Der SessionStart-Hook (`.claude/settings.json`, `scripts/cloud-session-start.sh`) installiert Abhängigkeiten. Einrichtung und Startbefehle für den Nutzer: [docs/remote-sessions.md](docs/remote-sessions.md).
