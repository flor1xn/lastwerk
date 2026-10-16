# T44: Tagesentscheidung, Coach-Erkenntnisse und Wochen-Story auf Heute

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 |
| cloudfähig | ja |
| Voraussetzung | Teil A: T41. Teil B: T43 und Teil A |
| Bereich | Teil A `crates/core`; Teil B `app/src-tauri`, `app/ui`, `catalog/locales`, `catalog/changelog.json`, `docs/SPEC.md` |
| SPEC | 5.4, 5.5, 8.4, neu 8.7 |

## Ziel

Heute beantwortet zuerst die Frage "Was trainiere ich heute und warum?" mit einer regelbasierten Empfehlung samt Alternativen. Darunter höchstens drei Erkenntnisse mit Einstieg in den Coach und eine Wochen-Story statt dreier Wochenabschnitte.

## Warum

Auftrag des Nutzers vom 16. Oktober 2026: Dashboard wie die Konkurrenz, nur anders und besser. Heute zeigte neun gleichrangige Abschnitte ohne klare Entscheidung.

## Entscheidung des Nutzers (16. Oktober 2026)

- **E11** Oben eine Tagesentscheidung, danach Coach-Erkenntnisse und Wochen-Story.

## Umfang

### Teil A (`crates/core`)

1. `plan/today.rs`: `today_decision(&TodayInput, &Rules) -> TodayDecision`. Eingänge: aktiver Plan, `PlanContext`, `ReadinessState` (R12), `FormState` (R10, SPEC 5.5), Vorlagenkatalog. Ausgänge: Zustandscode (`no_plan`, `rest`, `already_adjusted`, `lighten`, `keep_easy`, `as_planned`), Hauptvariante und Alternativen (`planned`, `lighter`, `shorter`, `harder` mit Vorlagen-ID und Dauer), Gründe, Regelcodes, `missing`.
2. Entscheidungstabelle (Faustregel, abgeleitet aus R12, keine neue Zahl):

| Lage | Zustand | Empfohlen | Alternativen |
|---|---|---|---|
| kein aktiver Plan oder Tag außerhalb | `no_plan` | nichts | keine |
| keine Einheit | `rest` | Ruhe | keine |
| Einheit schon durch R12 getauscht | `already_adjusted` | geplant | keine |
| hart geplant, Bereitschaft niedrig oder Form überlastet | `lighten` | `easy_replacement`, gleiche Dauer | geplant |
| hart geplant, sonst | `as_planned` | geplant | leichter |
| locker geplant, Bereitschaft niedrig oder Form überlastet | `keep_easy` | locker | kürzer im Katalogfenster |
| locker geplant, Bereitschaft normal, Form frisch, keine Taper-, Wettkampf- oder Erholungswoche | `as_planned` | geplant | härter (`hard_replacement`), nur ohne neue harte Verletzung in `validate_plan` (R1, R3, R4, R11) |

3. `plan/recovery.rs::easy_replacement` wird `pub(crate)`, daneben `hard_replacement`. Last über `rules.load_model.session_load`.
4. `metrics/insights.rs`: `rank_insights(codes)`, feste Reihenfolge (Warnungen vor positiven Trends), höchstens drei (Faustregel).
5. Tests mit handgerechneten Konstanten (SPEC 11.1).

### Teil B (App)

1. `dto.rs`: `Dashboard.decision`, `Dashboard.insights`; `service/dashboard.rs` ruft `today_decision` und `rank_insights`. `ChatTopic` um `decision_*` erweitern.
2. `TodayView.svelte` neu: Leitsatz, Tagesentscheidung als einzige `.card` (Hauptaktion "Mit Coach besprechen"), Coach-Erkenntnisse (Texte aus `ui.dash.statement.<code>`, je Zeile "Besprechen"), Bereitschaft, Form, Wochen-Story (`today/WeekStorySection.svelte`, `lib/week-story.ts`), Raster mit Rampe und Verteilung. Effizienz und Leistungskurve ziehen nach Verlauf › Leistung, Belastungskalender nach Verlauf › Fortschritt.
3. Attrappe (`mock-dashboard.ts`, `?decision=`), Texte de und en, SPEC 8.7, 5.5, 5.4, Changelog.

## Nicht im Umfang

Die Alternativen ändern den Plan nicht; Übernahme läuft weiter über den Coach (SPEC 9.7).

## Abnahme

```
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo fmt --manifest-path app/src-tauri/Cargo.toml --check && cargo clippy --manifest-path app/src-tauri/Cargo.toml --all-targets --locked -- -D warnings && cargo test --manifest-path app/src-tauri/Cargo.toml --locked
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
sh scripts/check-all.sh
```

## Manuelle Schritte

Sichtprüfung im Tauri-Fenster mit echten Daten.

## Neue Abhängigkeiten

Keine.

## Blocker oder Vermerke

Keine.
