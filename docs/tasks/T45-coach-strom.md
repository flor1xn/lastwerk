# T45: Coach-Antworten als Strom

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 |
| cloudfähig | ja, Live manuell |
| Voraussetzung | T43 |
| Bereich | `crates/coach`, `app/src-tauri`, `app/ui` |
| SPEC | 9.4, 9.7, 5.4 |

## Ziel

Antworten des lokalen Modells erscheinen Wort für Wort im Coach-Fenster (Ollama `stream: true`, Tauri-Event).

## Warum

Entscheidung des Nutzers vom 16. Oktober 2026 (E10): Streaming später als eigener Task.

## Offene Punkte vor dem Umfang

- Die Schema-Prüfung (SPEC 9.4, 9.7) geht erst nach der vollständigen Antwort; was wird bis dahin angezeigt?
- Korrekturaufrufe bei ungültiger Antwort.
- `coach_cancel` während eines Stroms.
- Neuer Event-Name in `lib.rs` und den Capabilities.

## Umfang

Noch festzulegen.

## Abnahme

Noch festzulegen.

## Blocker oder Vermerke

Keine.
