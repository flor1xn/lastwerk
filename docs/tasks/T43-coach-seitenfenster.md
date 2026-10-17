# T43: Coach als globales Seitenfenster mit Ansichtskontext

| Feld | Wert |
|---|---|
| Status | in Prüfung |
| Meilenstein | nach 1.0 |
| cloudfähig | ja, **Live manuell** (Gespräch mit echtem Ollama) |
| Voraussetzung | T42 |
| Bereich | `crates/coach`, `app/src-tauri`, `app/ui`, `catalog/locales`, `catalog/changelog.json`, `docs/SPEC.md` |
| SPEC | 5.4 (`lastwerk-app`), 5.5, 9.3, 9.7 |

## Ziel

Das Gespräch mit dem Coach lässt sich aus jeder Ansicht öffnen (Knopf in der Seitenleiste, Strg/Cmd+K) und weiß, welche Seite und welches Thema der Nutzer ansieht.

## Warum

Auftrag des Nutzers vom 16. Oktober 2026: "ein Coach mit KI Chatfenster". Bisher lag der Chat nur in der Planansicht.

## Entscheidung des Nutzers (16. Oktober 2026)

- **E9** Coach als globales Seitenfenster, nicht modal.
- **E10** Antworten weiter ohne Strom; Streaming als eigener Task (T45).

## Umfang

1. `crates/coach/src/chat.rs`: geschlossene Enums `ChatView` (`today`, `plan`, `history_calendar`, `history_activities`, `history_activity`, `history_progress`, `history_performance`, `history_body`, `settings`) und `ChatTopic` (1:1 zu vorhandenen Zustandscodes aus `core::metrics`, Test prüft das), `ChatFocus { view, topic }`. `ChatInput::build` schreibt `"focus"` in den Eingang. Nur Codes, keine ID, kein Datum, kein Name (SPEC 9.3). Prompts `chat.de.txt` und `chat.en.txt`: "focus" ist Datum, keine Anweisung.
2. `app/src-tauri`: `coach_chat(message, focus)` und `coach_preview(message, focus)`, unbekannter Code ergibt `bad_input`. Befehlsnamen bleiben, Registrierung unverändert.
3. `app/ui`: Gesprächszustand in `lib/coach-session.svelte.ts`; `CoachPanel` wird `lib/components/coach/CoachChat.svelte`, `ProposalCard` zieht mit; neues `CoachDrawer.svelte` in `App.svelte` (rechts, 400 px, nicht modal, unter 1100 px über dem Inhalt, `translateX` 180 ms, Esc schließt, Fokus kehrt zurück). `openCoach({ prefill, topic })`, Kontextzeile "Ich sehe: ...". Prefill sendet nie; Freigabe je Sitzung bleibt. Knopf `nav-coach`, Kürzel Strg/Cmd+K (nicht im Assistenten). `PlanView` einspaltig mit Knopf "Mit dem Coach über den Plan sprechen". Einrichtungshinweis ohne Modell im Fenster.
4. Tests in Rust (Serialisierung, Markertest ohne Aktivitätsdaten, Ablehnung ungültiger Codes), Vitest (`CoachDrawer`), Playwright.
5. SPEC 9.3, 9.7, 5.4, 5.5 und Changelog.

## Nicht im Umfang

Streaming (T45), neue Inhalte auf Heute (T44).

## Abnahme

```
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo fmt --manifest-path app/src-tauri/Cargo.toml --check && cargo clippy --manifest-path app/src-tauri/Cargo.toml --all-targets --locked -- -D warnings && cargo test --manifest-path app/src-tauri/Cargo.toml --locked
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
sh scripts/check-all.sh
```

## Manuelle Schritte

Strg+K in WebView2 unter Windows prüfen; Breite des Fensters neben Diagrammen; ein echtes Ollama-Gespräch mit Kontext.

## Neue Abhängigkeiten

Keine.

## Blocker oder Vermerke

- 2026-10-17: Umgesetzt, alle Befehle der Abnahme und `sh scripts/check-all.sh` bestehen. Status `in Prüfung`, weil das Gespräch mit einem echten Ollama und die Punkte unter "Manuelle Schritte" offen sind.
- 2026-10-17: Abweichung, in SPEC 9.7 vermerkt: Ein unbekannter Code im Fokus ergibt den vorhandenen Fehlercode `invalid_value`, keinen neuen Code `bad_input`.
- 2026-10-17: `ChatTopic` umfasst alle Zustandscodes aus `lastwerk_core` (Kennzahlen und `decision.*` der Tagesentscheidung), damit T44 Teil B Themen aus Erkenntnissen und Entscheidung setzen kann. Per Tastenkürzel öffnet und schließt das Fenster ohne Bewegung (Faustregel aus `emil-design-eng`), per Knopf mit 180 ms.
- 2026-10-17: Changelog-Eintrag unter 1.1.1 (nächste Version nach v1.1.0).
