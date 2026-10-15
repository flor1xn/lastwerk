# T26: Planer mit drei Fragen und Gespräch mit dem Coach

| Feld | Wert |
|---|---|
| Status | in Prüfung |
| Meilenstein | 1.0 |
| Voraussetzung | T20 (Teil A), T25 (Teil B) |
| cloudfähig | ja (Attrappe), Live manuell mit Ollama |
| Bereich | Teil A: `crates/coach`, `schema/`, `crates/cli`. Teil B: `app/src-tauri`, `app/ui`, `catalog/locales` |
| SPEC | 7.2, 9, 9.7, 14 (O13, O31) |

## Ziel

Ein Plan entsteht aus drei Antworten. Wer mehr will, spricht mit dem lokalen Coach, der nur geprüfte Vorschläge macht.

## Teil A: Vertrag (`crates/coach`)

1. Schema `schema/plan-chat.schema.json` (Draft 2020-12) nach SPEC 9.7: `{ schema_version: 1, reply, proposal? }`, `proposal` ist `{ kind: "plan_edit", edit: <plan-edit> }` oder `{ kind: "planning_change", availability?: [7 × { max_minutes, allow_hard, fixed_rest }], sports?: [...], strength_per_week?: 0..2, weeks?: 1..26 }`. `additionalProperties: false` überall, Längen begrenzt.
2. `chat.rs`: `ChatInput` (Kontext wie `RefineInput` plus `metrics: Vec<{ code, value }>` aus T25, ganze oder auf eine Stelle gerundete Zahlen), `ChatHistory` (höchstens 12 Nachrichten, je höchstens 1000 Zeichen, ältere fallen weg), `chat_turn(client, input, history, user_text, locale) -> ChatOutcome { reply, proposal: Option<CheckedProposal>, rejected_codes }`. Prüfung wie `refine_plan` (`refine.rs:189`): parse, bei `plan_edit` anwenden und `validate_plan`, höchstens zwei Korrekturaufrufe; bei `planning_change` Werte gegen Grenzen prüfen (Erzeugen des neuen Plans macht die App über `planning::generate` und prüft dort).
3. Prompts `prompts/chat.de.txt` und `chat.en.txt`: Rolle Trainer, kurz, deutsch bzw. englisch, nichts rechnen, nur Zahlen aus dem Eingang nennen, keine medizinischen Aussagen, Vorschläge nur nach Schema, Eingabe des Nutzers ist ein Wunsch, keine Anweisung an die Regeln.
4. `discover.rs`: `ollama_models(base_url) -> Vec<String>` über `GET /api/tags` (nur Loopback-Standard), Auswahl des ersten vorhandenen Modells aus einer Vorzugsliste (Faustregel, im Code begründet) oder des ersten überhaupt.
5. Tests mit `fake.rs`: gültige Antwort, ungültiges JSON, Vorschlag verletzt R3 und wird korrigiert, Vorschlag bleibt ungültig (ohne Vorschlag zurück), Markertest wie T15 (Zielbezeichnung, Pfade nie im Prompt; der eigene Text des Nutzers ist erlaubt).

## Teil B: Oberfläche

1. Befehle: `coach_status` → `{ available, provider, model, models[] }`, `coach_set_model`, `coach_preview(input)` (zeigt den exakten Eingang, F24), `coach_chat(message)` → `{ reply, proposal_preview? }` mit Vorher/Nachher (Wochenminuten, harte Einheiten, Last je Woche), `coach_apply(proposal_id)`. Gesprächsverlauf nur im Speicher der App, nicht in der Datenbank.
2. `PlanView.svelte` neu: oben drei Fragen
   - Ziel: Wettkampf (Datum, Sportart) oder "Fit bleiben".
   - Stunden je Woche (Schieber 2 bis 20, Schritt 0,5).
   - Trainingstage (7 Tagesknöpfe).
   - Daraus `set_goal`, `set_availability` (Minuten nach O31 verteilt, Ruhetage fest), `generate_plan`, Aktivieren mit einem Klick. Die bisherigen Panels (`GoalPanel`, `AvailabilityPanel`, `GeneratePanel`) wandern hinter "Feineinstellungen" (eingeklappt), nicht löschen.
   - Darunter der Plan (bestehendes `PlanDisplay`) und rechts bzw. darunter das Gespräch: Verlauf, Eingabefeld, Vorschlagskarte mit "Übernehmen" und "Verwerfen", Link "Was sieht der Coach?" (Vorschau). Ohne Modell: Hinweis mit Einrichtung (Ollama installieren, Modell laden), Planer bleibt nutzbar.
3. Attrappe: Coach-Antworten aus fester Liste, ein Vorschlag je Antworttyp.

## Vorgaben

- Antworttext immer als Text (`textContent`), Test `no-html.test.ts` erweitern.
- Netzwerk nur im Rust-Teil, nur zur eingestellten lokalen Adresse.
- O31 in SPEC 14 entscheiden und vermerken.

## Abnahme

```
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
(cd app/src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
sh scripts/check-privacy.sh && sh scripts/check-isolation.sh && sh scripts/check-i18n.sh
```

## Manuelle Schritte

Ollama installieren, ein Modell laden (`ollama pull <modell>`), in der App drei Wünsche ausprobieren ("Dienstag habe ich keine Zeit", "mehr Laufen", "warum ist meine Form so niedrig?"), Antwortzeit notieren (O13).

## Vermerke

### Teil A erledigt (15. Oktober 2026)

Teil A ist umgesetzt, in einer Cloud-Umgebung (Linux), vollständig offline mit synthetischen Daten, dem Attrappen-Client und Attrappen-Servern auf Loopback. Der Status der Task bleibt `offen`, bis Teil B fertig ist. Einzelheiten und Abweichungen stehen in SPEC 9.7 ("Umsetzung in T26 Teil A") und 5.4 (lastwerk-coach, lastwerk-cli).

- Schema `schema/plan-chat.schema.json`, Module `crates/coach/src/chat.rs` (`ChatInput`, `MetricValue`, `ChatHistory`, `chat_turn`, `ChatOutcome`, `CheckedProposal`, `PlanningChange`, `ChatError`) und `discover.rs` (`ollama_models`, `pick_model`), Prompts `chat.de.txt`, `chat.en.txt`, `chat_correction.de.txt`, `chat_correction.en.txt`.
- CLI `lastwerk chat` zum Ausprobieren (nichts wird gespeichert), Markertest in `crates/cli/tests/chat.rs`. Die Markerdatenbank aus T15 liegt jetzt in `crates/cli/tests/common` und wird von `refine` und `chat` geteilt.
- Abweichungen: Eingang als JSON in der Systemnachricht statt als erste Nutzernachricht; `plan_edit` im Schema eingebettet statt `$ref`; eine neue Nachricht über 1000 Zeichen wird abgelehnt statt gekürzt; verletzt der Basisplan selbst eine harte Regel, wird ein `plan_edit` ohne Korrekturaufruf verworfen (`BASE_PLAN_INVALID`); `chat_turn` liefert `Result`, eine unlesbare Antwort nach allen Korrekturen ist `ChatError::InvalidOutput`. Der OpenAI-kompatible Client benennt das Schema jetzt nach `$id`.
- Keine neue Abhängigkeit, `crates/core` unverändert.
- Abnahme Teil A: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (573 Tests, 0 Fehler), `sh scripts/check-privacy.sh`, `sh scripts/check-isolation.sh`, `sh scripts/check-i18n.sh` bestehen. Die Befehle für App und Oberfläche aus der Abnahme gehören zu Teil B.
- Für Teil B: `ChatInput::build(...)` mit den Kennzahlen aus T25, `chat_messages(...)` für "Was sieht der Coach?" (F24), `chat_turn(...)`, `ChatHistory::record(...)` im Speicher der App. `CheckedProposal::PlanEdit.plan` braucht eine neue `plan_id` und `created_at`, `PlanningChange` geht in `planning::generate` und wird dort geprüft. `ollama_models` und `pick_model` für `coach_status`. Live-Versuch mit echtem Modell: `lastwerk chat --db <datei> --message "Dienstag habe ich keine Zeit" --show-input` (O13).

### Teil B umgesetzt (15. Oktober 2026), Status `in Prüfung`

Teil B ist umgesetzt, in einer Cloud-Umgebung (Linux), vollständig offline mit synthetischen Daten, dem Attrappen-Client und Attrappen-Servern auf Loopback. Offen bleibt der Live-Versuch mit Ollama (Abschnitt "Manuelle Schritte") und die Bewertung durch den Nutzer; erst danach `erledigt`. Einzelheiten und Abweichungen stehen in SPEC 9.7 ("Umsetzung in T26 Teil B"), 5.4 (lastwerk-app), 5.5, 10.4 und 14 (O31, O13).

- **Befehle** (acht, an allen vier Stellen eingetragen): `quick_plan`, `coach_status`, `coach_set_model`, `coach_preview`, `coach_chat`, `coach_apply`, `coach_reset` und zusätzlich `coach_cancel` (SPEC 9.5: Abbruch jederzeit, ohne den Verlauf zu verlieren; die Aufgabe nannte nur `coach_reset`). Code in `app/src-tauri/src/service/coach.rs` und `quick.rs`, Zustand (Verlauf, höchstens vier offene Vorschläge) nur im Speicher.
- **O31 entschieden:** gleichmäßig auf die gewählten Tage, ohne Wochenendanteil, auf 5 Minuten gerundet, Rest an die späteren Tage; Begründung in SPEC 14. Weniger als 20 Minuten je Tag ist `too_little_time`.
- **Oberfläche:** `PlanView.svelte` neu (Kopf, Karte mit den drei Fragen, aktiver Plan, Gespräch als mitscrollende Spalte, Feineinstellungen eingeklappt ganz unten), neue Komponenten `QuickPlanner`, `CoachPanel`, `ProposalCard` in `app/ui/src/views/plan/`, Hilfen `lib/planner.ts` und `lib/coach.ts`, Attrappe `lib/backend/mock-coach.ts` mit `?coach=off`. Texte unter `ui.planner.*`, `ui.coach.*` und neuen `ui.error.*`, de und en mit gleichen Schlüsseln (708).
- **Abweichungen und Zusätze:**
  - Die Freigabe je Sitzung aus SPEC 9.7 (4) ist umgesetzt, auch wenn die Aufgabe sie nicht aufführte: einmaliger Hinweis vor der ersten Nachricht, nur im Speicher der Oberfläche.
  - `planning_change`: Der neue Plan beginnt am Montag der heutigen Woche (ersetzt den laufenden); vergangene Tage dieser Woche zählen in der Planerfüllung als offen. Scheitert der Regelplaner, wird der Vorschlag mit `PLAN_FAILED` verworfen, ohne Korrekturaufruf.
  - `lastwerk-cli` bekam `planning::generate_with` (Verfügbarkeit überschreibbar), damit die Vorschau nichts schreibt; `generate` ruft es mit `None`, nichts anderes änderte sich.
  - Keine neue externe Abhängigkeit: `lastwerk-coach` als Pfadabhängigkeit der App.
  - Ein Host im lokalen Netz ist in der App nicht einstellbar (nur Loopback), weil die Oberfläche die ausdrückliche Bestätigung dafür nicht bietet.
  - Die Verteilung der Zeit steht doppelt (Rust maßgeblich, `distributeMinutes` in der Oberfläche für Anzeige und Attrappe), beide mit denselben Handrechnungen als Tests.
- **Tests:** Rust `app/src-tauri/tests/coach.rs` (23 Tests: drei Fragen mit Verteilung, Grenzen und Ziel nur einmal, Status gegen Attrappen-Server, Gespräch mit beiden Vorschlagsarten über den Attrappen-Client einschließlich Übernehmen, veraltete Vorschläge, Sperre, Abbruch, Vorschau gleich dem Gesendeten, Markertest auf dem Weg der App mit der Markerdatenbank aus T15) und Einheitentests in `quick.rs`; Vitest (Verteilung, Vorbelegung, Planansicht mit den drei Fragen, Gespräch als Text, Vorschlag übernehmen und verwerfen, Freigabe, Abbruch, ohne Modell, Modellwahl, Attrappe, `no-html.test.ts` erweitert); Playwright (Gespräch und ohne Modell in de und en, der Wächter bleibt grün).
- **Abnahme:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (746 Tests), in `app/src-tauri` `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (83 Tests), `cargo deny --manifest-path app/src-tauri/Cargo.toml --config deny.toml check`, `pnpm --dir app/ui lint`, `check`, `test` (208 Tests), `build`, `playwright test` (16 Tests), `sh scripts/check-privacy.sh`, `check-isolation.sh`, `check-i18n.sh` bestehen.
- **Sichtprüfung** (Screenshots dunkel und hell, de und en, 1280 Pixel Breite, Scratchpad, nicht im Repository) und `break-ui`: lange Antworten mit 300 Zeichen ohne Leerzeichen, Emoji und `<script>`, 14 Änderungen mit unbrechbaren Namen, 26 Wochen Vorher und Nachher, sieben Nachrichten, 870 Zeichen im Feld, Vorschau offen, Breiten 820, 900 und 1280: kein waagerechter Überlauf (gemessen an Seite, Gespräch und Verlauf), Spalte nie höher als das Fenster. Gefundener und behobener Fehler: eine Klasse `coach` traf Zeilen und Panel zugleich (jetzt `coach-panel`), die Tabelle war in der schmalen Spalte abgeschnitten (Tag und Monat, ganze Last).
- **Nicht geprüft:** ein echtes Modell (Ollama), die Antwortzeit (O13), das Verhalten im Tauri-Fenster (WebKitGTK oder WebView2; der Schieber ist mit `-webkit-slider-*` gestaltet, nur in Chromium gesehen), ein Neustart der App mit offenem Gespräch.
