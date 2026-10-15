# T07: Regelbasierter Planer (lastwerk-core, catalog)

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M3 |
| Voraussetzung | T05 |
| cloudfähig | ja |
| Bereich | `crates/core`, `catalog/` |
| SPEC | 7.1, 7.4, 7.5, 8.3, 8.4, 11.4 (Property, Golden), 12 (M3) |

## Ziel

`generate_plan` erzeugt deterministisch einen Wochenplan (mit Termin rückwärts, fortlaufend in Blöcken), `validate_plan` prüft ihn gegen die Leitplanken R1 bis R11. Jede Einheit trägt einen Begründungsschlüssel.

## Umfang

- Typen `Plan`, `PlanWeek`, `PlanSession`, `Violation` nach `plan.schema.json` (SPEC 7.1), mit `serde`.
- `catalog/templates.json` (SPEC 7.4): Vorlagen je Sportart, Klasse (leicht, hart, lang, Kraft, Ruhe) und Dauerfenster, Texte in `catalog/locales/de.json` und `en.json`.
- `catalog/rules.default.json`: alle Parameter der Leitplanken R1 bis R11 (SPEC 8.4). R12 kommt in T14.
- `generate_plan(&PlanContext, &Rules)`: Ablauf nach SPEC 8.3 (Phasen, Wochenziel, Wochenstruktur, Vorlagenwahl, Vorausberechnung mit `pmc`, Validierung mit Neuwahl, Begründungsschlüssel). Gleichstände nach Katalogreihenfolge, kein Zufall, keine Systemzeit.
- `validate_plan(&Plan, &PlanContext, &Rules) -> Vec<Violation>`: harte Regeln lehnen ab, weiche erzeugen Warnungen mit Code `R**`.
- Property-Tests (proptest) nach SPEC 11.4: Ausgabe besteht `validate_plan`, keine harten Tage an Folgetagen, CTL-Anstieg höchstens R1, Verfügbarkeit nie überschritten. Mindestens 1000 Fälle.

## Nicht im Umfang

JSON-Schema-Datei und Export (T08), Oberfläche (T09), Modell-Anbindung (T15), Erholungsregel R12 (T14).

## Vorgaben

- `crates/core` bleibt rein (keine Ein- und Ausgabe). Der Katalog wird als Daten übergeben, nicht von `core` gelesen.
- Jede Regel nennt Quelle oder "Faustregel" (SPEC 8.4). Kein ACWR, keine Verletzungsprognose (SPEC 8.4, "Bewusst keine Leitplanke").
- Golden-Tests (`insta`) für Plan-JSON bei festen Eingaben.

## Abnahme

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p lastwerk-core plan::
cargo test --workspace
```

Erfolg (SPEC 12, M3, soweit zu diesem Task gehörig): Property-Tests laufen mit mindestens 1000 Fällen ohne Gegenbeispiel, Golden-Pläne stabil, Plan mit absichtlich verletzter harter Regel wird von `validate_plan` abgelehnt, de.json und en.json haben gleiche Schlüssel.

## Manuelle Schritte

O16 (Taper- und Rampenparameter für den Autor persönlich): Standardwerte gegen die eigene Erfahrung prüfen.

## Blocker oder Vermerke

O8: Taper-Dauer und -Reduktion nur aus Sekundärquellen. Als Parameter, nicht als Code.

Keine Blocker. Erledigt am 13. Oktober 2026.

Neue Abhängigkeiten (nur Entwicklung in `crates/core`, passend zu `deny.toml`, `cargo-deny` lief in dieser Umgebung nicht):

| Name | Version | Lizenz | Grund |
|---|---|---|---|
| serde_json | 1.0.154 (schon im Workspace) | MIT OR Apache-2.0 | lädt `catalog/*.json` in den Tests und prüft die Plan-Form |
| insta, Feature `json` | 1.49.0 (schon im Workspace) | Apache-2.0 | Golden-Tests `assert_json_snapshot!`. Das Feature zieht nur `serde` mit |

Produktivcode von `core` bleibt bei `serde`, `chrono` (nur Datumstypen) und `thiserror`. Keine Datei-, Netzwerk-, Zeit- oder Zufallsaufrufe im Modul `plan`, `include_str!` nur in `plan/tests.rs`.

Ergebnis der Abnahme: `cargo test -p lastwerk-core plan::` mit 58 Tests, davon die Property-Tests `generated_plans_satisfy_the_hard_rules` mit 1000 Fällen (Debug-Profil rund 3 Sekunden, einmalig mit 30000 Fällen im Release-Profil ohne Gegenbeispiel geprüft) und `generation_is_deterministic_for_random_contexts` mit 200 Fällen. Golden-Pläne: `crates/core/src/plan/snapshots/`.

Entscheidungen und Abweichungen von der SPEC (jeweils im betroffenen Abschnitt vermerkt):

- 5.4 core: `PlanContext`, `Violation`, `PlanError` und die Hilfstypen sind ausformuliert. `ctl0` und `atl0` stehen im Kontext, `pmc_params` liefert nur die Zeitkonstanten. `PlanSport` (mit `rest`) ist ein eigener Typ neben `Sport`.
- 7.1: `warnings` steht immer im Plan. Der Wettkampf ist keine Einheit: Tag vor dem Termin, Termin und Folgetage bleiben leer, der Plan endet mit der Rennwoche.
- 7.4: `rest.day` hat Dauer 0 und wird nie als Einheit geplant (Schema verlangt mindestens 10 Minuten), ein Ruhetag ist `sessions: []`. Schritte gelten bei der Nenndauer und werden bei anderer Dauer gedehnt (Intervallblöcke fest). Die Last einer Einheit folgt aus der Klasse (IF 0,65, 0,8, 0,95) bzw. bei Kraft aus der RPE, alles Faustregel und Parameter.
- 8.3 und 8.4: Messgrößen und Konventionen je Regel, insbesondere R2 als Trainingszeit mit Bezug "letzte Woche vor der Erholungswoche" und `min_reference_minutes` (60), R6 an der geplanten Last, R9 als Tage vom Montag der ersten Taperwoche bis zum Termin (mit einer Taperwoche 7 bis 13 Tage).
- Quelle der Regelparameter: R1, R3 bis R7 und R10 sind Faustregeln, R2 Faustregel mit Q12 und Q13, R8 Q5 (Grenzwert Faustregel), R9 Q10 (Prinzip, Dauer Faustregel, Zahlen aus Q11 nur sekundär, O8), R11 Nutzereingabe. Wochenzuwachs (7 und 3 Prozent), Anteile für Erholung (65), Taper (70) und Rennwoche (50 Prozent), `long_weight`, `min_week_load`, `planning_margin` und `min_reference_minutes` sind Konvention oder Faustregel und stehen in `rules.default.json`.

Hinweise für spätere Tasks:

- T08: Das Plan-JSON entspricht Feld für Feld SPEC 7.1 (Test `generated_plan_matches_the_schema_shape` prüft Pflicht- und erlaubte Felder von Hand, ersetzt aber nicht die Schemaprüfung mit `jsonschema`). `Plan` lässt sich zurücklesen (`Deserialize`, `schema_version` muss 1 sein). `Violation` ist serialisierbar, `message_key` ist `rule.R**.explanation`. Regeln und Katalog lädt der Aufrufer: `serde_json::from_str::<Rules>` und `::<TemplateCatalog>` auf den Dateien in `catalog/`. `plan_id` und `created_at` muss die CLI beim Aufruf vergeben (UUID und Uhr gehören in `cli`, nicht in `core`). Die Historie (`WeekHistory`) kommt aus den Tageslasten und Dauern der Aktivitäten je ISO-Woche (Montag), `ctl0` und `atl0` aus `pmc` am Tag vor `week_start`.
- T09: Texte nachschlagen unter `template.<id>.name` und `.description` (Schlüssel `text_key` der Vorlage ist `template.<id>`), `reason.week.*`, `reason.session.*` und `rule.R**.name` und `.explanation`. Die Sprachdateien sind verschachtelt (Punkte im Schlüssel sind Ebenen). Warnungen tragen nur den Code, Parameter liefert `validate_plan` im `detail`. Wochen können leer sein (`reason.week.fallback_rest`, `reason.week.no_training_days`), Rennwoche und Folgetage ebenfalls.
- T14: R12 hat im Schema der Regeln den Platz `reserved: ["R12"]`. Unbekannte Schlüssel in `rules.default.json` werden beim Lesen ignoriert. Die Prüfung `validate_weeks` ist der Ort für die neue Regel.
- O16 (Taper- und Rampenparameter für den Autor persönlich) bleibt offen: Die Standardwerte sind generisch und stehen in `catalog/rules.default.json`.

Nachtrag 14. Oktober 2026 (Integrationsprüfung, Fund A): Die Rennwoche blieb ohne Hinweis leer, wenn eine harte Einheit allein mehr Last brauchte als das Wochenziel (Abwurfreihenfolge leicht vor hart). Korrigiert: Einheiten, die zusammen mit der Langeinheit das Ziel allein übersteigen, entfallen zuerst; eine trotz positivem Ziel leere Woche trägt `reason.week.fallback_rest`, eine Woche mit weniger harten Einheiten als vorgesehen `reason.week.hard_reduced`; die leichte Einheit nach einem entfallenen harten Tag heißt nicht mehr "Locker nach einer harten Einheit"; die Neuwahl verkleinert das Ziel nur bei R2 und R10. Abweichungsvermerk in SPEC 8.3, Tests `race_week_below_the_minimum_of_a_hard_session_keeps_easy_sessions`, `taper_below_hard_plus_long_drops_the_hard_session_before_the_easy_ones`, `week_left_empty_despite_a_positive_target_says_so`.
