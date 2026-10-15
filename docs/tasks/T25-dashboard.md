# T25: Kennzahlen und Dashboard

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | 1.0 |
| Voraussetzung | T24 (Teil B), T22 (Erholung mit Body Battery) |
| cloudfähig | ja |
| Bereich | Teil A: `crates/core`, `crates/store` (Lesemethoden). Teil B: `app/src-tauri`, `app/ui`, `catalog/locales` |
| SPEC | 4.2 (F20, F35, F36), 5.5, 8.2, 8.4 (R1, R8, R12), Anhang A |

## Ziel

"Heute" wird ein Dashboard, das alle wichtigen Informationen zeigt und deutet, auch solche, die Garmin Connect nicht hat: wie sich Form, Rampe, Verteilung, Planerfüllung und Effizienz entwickeln und was das für heute heißt.

## Teil A: Kennzahlen in `core` (reine Funktionen)

Jede Funktion mit handgerechnetem Referenzwert als Konstante und Rechenweg im Kommentar (SPEC 11.1). Jede Zahl, Grenze und Formel mit Quelle aus Anhang A oder "Faustregel". Was sich nicht belegen lässt, entfällt.

| Kennzahl | Definition | Quelle |
|---|---|---|
| Rampe | CTL heute minus CTL vor 7 Tagen | Q1 bis Q3 (Modell), Grenze aus R1 (`rules.default.json`) |
| Intensitätsverteilung | Anteil der Zeit in Zone 1 bis 2 an der Zeit mit Zonen, je Woche und über 4 Wochen, je Schema getrennt oder nach Vorrang Leistung vor HF | Q5 (beschreibend), Schwelle 70 % aus R8 |
| Effizienzfaktor | NP geteilt durch mittlere HF je Radeinheit ab 45 min (Faustregel für die Mindestdauer) | Q16, nur Sekundärquelle, im Kommentar |
| Aerobe Entkopplung | Verhältnis Leistung zu HF erste gegen zweite Hälfte, in Prozent | Q16, nur Sekundärquelle. Braucht die Messreihe; nur berechnen, wo die Reihe schon gelesen wird (Detailansicht) oder beim Import, wenn ohne großen Umbau möglich, sonst in die Detailansicht verschieben und vermerken |
| Erholungsabweichung | HRV, Ruhepuls, Schlafdauer gegen eigenes 7-Tage-Mittel, in Prozent | Q6 (7-Tage-Mittel nur Sekundärquelle), Logik von `plan/recovery.rs` wiederverwenden, nicht duplizieren |
| Planerfüllung | Summe der erledigten Last durch geplante Last je Woche des aktiven Plans | Faustregel (reine Zählung) |
| Leistungskurve | Beste Leistung je Dauer (5, 60, 300, 1200, 3600 s) im Zeitraum und im Vorzeitraum | F35, Werte aus `activity_best_power` |
| Formprognose | Vorausberechnung aus dem aktiven Plan | vorhanden (`pmc`), wiederverwenden |

Monotonie und Strain nach Foster 1998 entfallen, solange die Quelle nicht in Anhang A steht (Q4 ist Foster 2001, Session-RPE).

Deutung: Je Kennzahl eine Funktion, die aus Wert und Schwellen einen Code liefert (`ramp.steep`, `ramp.ok`, `distribution.polarized`, `distribution.threshold_heavy`, `readiness.low`, …). Schwellen aus `rules.default.json` oder als Faustregel benannt. Texte entstehen erst in der Oberfläche aus `catalog/locales`.

## Teil B: Dashboard

1. Befehl `get_dashboard(today)` liefert ein DTO mit allen Kennzahlen, Deutungscodes und Reihen (PMC 120 Tage mit Prognose des aktiven Plans, Rampe 12 Wochen, Verteilung 8 Wochen, Planerfüllung je Planwoche, EF-Reihe 90 Tage, Leistungskurve 90 Tage gegen die 90 davor, Erholung 60 Tage mit Basis, Tageslast 365 Tage für den Kalender). Bestehende Lesemethoden in `lastwerk-store` (`history.rs`, `recovery.rs`) wiederverwenden.
2. `TodayView.svelte` neu, mit den Bausteinen aus T24, kein Kachelraster:
   - Kopf: Empfehlung für heute (bestehende Logik `lib/today.ts`), Form als große Zahl mit Zustand, eine Aussage in einem Satz, die Form, Bereitschaft und Plan verbindet.
   - **Bereitschaft**: HRV, Ruhepuls, Schlaf, Body Battery als Zahl mit Abweichung zur eigenen Basis und Mini-Verlauf; R12-Hinweis "Lastwerk hat angepasst", wenn das Signal greift (SPEC 5.5, nur wenn der Plan es wirklich tat).
   - **Form-Verlauf**: PMC mit TSB-Bändern der Formzustände (5.5) und gestrichelter Prognose.
   - **Rampe**, **Verteilung 80/20**, **Planerfüllung** als Zahl plus kleines Diagramm, je mit Deutung in einem Satz.
   - **Effizienz** (EF-Trend) und **Leistungskurve** (jetzt gegen vorher).
   - **Belastungskalender**: Jahres-Heatmap der Tageslast, eigenes SVG, Farbe aus Tokens, Wert im Tooltip und als `title`.
   - Ohne Daten je Abschnitt ein Satz, was fehlt (zum Beispiel "Verbinde Garmin für Erholungswerte"), nichts erfinden.
3. Diagramme: uPlot für Zeitreihen (O11), schlankes SVG für Heatmap und Balken, keine neue Bibliothek. Farbe nie allein, immer Text oder Muster.
4. Attrappe mit synthetischen Werten für alle Abschnitte, Vitest für Deutungstexte, Playwright de/en.

## Vorgaben

- Skills `dataviz`, `emil-design-eng`, `break-ui` (leere Daten, extreme Werte, 0 Aktivitäten, 5 Jahre Daten).
- Keine medizinischen Aussagen (SPEC 2.3): Deutungen beschreiben Trainingsdaten, keine Gesundheit.

## Abnahme

```
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
(cd app/src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
sh scripts/check-privacy.sh && sh scripts/check-isolation.sh && sh scripts/check-i18n.sh
```

## Vermerke

### Teil A geliefert (15. Oktober 2026)

Modul `crates/core/src/metrics/`, nur reine Funktionen, keine neue Abhängigkeit, keine Änderung in `crates/store`. Jede Berechnung hat einen handgerechneten Referenzwert als Konstante mit Rechenweg im Testkommentar, jede Deutung einen stabilen Code (`code()`), keine Texte.

| Kennzahl | Datei | Funktionen | Quelle der Zahlen |
|---|---|---|---|
| Rampe | `ramp.rs` | `ctl_ramp`, `ramp_series`, `ramp_state` (`RampState`: `ramp.no_data`, `.falling`, `.steady`, `.building`, `.steep`) | Modell Q1 bis Q3. Obergrenze `R01.max_ctl_rise_per_week` aus den Regeln (Faustregel). Band "gleichbleibend" ±1 CTL je Woche: Faustregel |
| Verteilung | `distribution.rs` | `easy_share`, `weekly_easy_share`, `window_easy_share`, `distribution_state` (`distribution.no_data`, `.easy_dominant`, `.intensity_heavy`) | Q5 beschreibend, Schwelle `R08.min_easy_share` aus den Regeln (Faustregel 70 Prozent). Mindestzeit mit Zonen für eine Deutung 1 h: Faustregel |
| Effizienz | `efficiency.rs` | `efficiency_factor`, `ef_points`, `ef_rolling_mean`, `ef_rolling_series`, `ef_change_percent`, `ef_trend` (`efficiency.*`) | Q16, nur Sekundärquelle (im Modulkopf vermerkt). Mindestdauer 45 min, Fenster 28 Tage, mindestens 2 Einheiten je Fenster, Band ±2 Prozent: Faustregeln |
| Entkopplung | `efficiency.rs` | `aerobic_decoupling`, `decoupling_state` (`decoupling.no_data`, `.coupled`, `.decoupled`) | Q16, nur Sekundärquelle, Zuschreibung der Form nicht geprüft. Mindestlänge 45 min, Grenze 5 Prozent: Faustregeln |
| Erholungsabweichung | `readiness.rs` | `recovery_deviation`, `readiness_state` (`readiness.no_data`, `.normal`, `.low`) | Q6, 7-Tage-Mittel nur Sekundärquelle. Schwellen und Basislinie aus `R12` der Regeln |
| Planerfüllung | `adherence.rs` | `planned_daily_loads`, `weekly_adherence`, `adherence_state` (`adherence.*`) | Reine Zählung. Grenzen 80 und 120 Prozent: Faustregel |
| Leistungskurve | `power_curve.rs` | `compare_power_curves`, `power_curve_trend_code` (`power_curve.*`) | F35, Werte aus `activity_best_power`. Band ±2 Prozent: Faustregel |

Gemeinsam in `metrics/mod.rs`: `TrendState`, `TREND_BAND_PERCENT`.

Entscheidungen und Abweichungen vom Wortlaut der Tabelle oben:

- **Erholung ohne Duplikat.** `plan/recovery.rs` stellt die Basislogik jetzt als `recovery_drops` bereit, `recovery_signal` (R12) und `metrics::readiness` rufen beide diese Funktion. Verhalten und Tests von R12 sind unverändert (alle bestehenden Tests grün), ein Test prüft, dass `readiness.low` und R12 bei gleichen Daten gleich urteilen. Das Dashboard rechnet den Alter-Check ab "heute" und lässt den heutigen Tag zu, R12 ab Planbeginn. Der Ruhepuls erscheint als Abweichung, hat aber keine Deutung, weil R12 ihn nicht verwendet und sich keine Schwelle belegen lässt. Body Battery gehört nicht in dieses Modul (kein Tageswert in `RecoveryDay`, kommt mit T22).
- **Codes der Verteilung.** Statt `distribution.polarized` und `distribution.threshold_heavy` gibt es `easy_dominant` und `intensity_heavy`. "Polarisiert" und "Schwellenlastig" setzen eine Aussage über die Mittelzonen voraus (Zone 3 gegen Zone 4 und höher), für die Anhang A keine Quelle und keine Schwelle liefert. Die Codes beschreiben nur, was R8 prüft: lockerer Anteil über oder unter der Grenze. Die Oberfläche kann in Teil B Texte dafür schreiben.
- **Codes der Rampe.** `ramp.ok` entfällt zugunsten von `steady` und `building`, weil "ok" innerhalb der R1-Grenze beides bedeuten würde.
- **Verteilung je Schema oder nach Vorrang.** `DistributionBasis` hat `Priority` (Leistung vor Herzfrequenz vor Geschwindigkeit, die Reihenfolge ist Konvention), `Channel(..)` und `Scheme(..)`. Je Aktivität zählt genau ein Schema, damit nichts doppelt zählt. Die Funktionen bekommen die Zeit in Zonen als `ZoneTimeEntry` (Tag, Aktivität, Schema, Kanal, Sekunden je Zone), die Oberfläche bildet sie aus `activity_zone_time`.
- **Aerobe Entkopplung.** Die Funktion rechnet über Leistungs- und HF-Reihe im Sekundentakt und mit der mittleren Leistung je Hälfte, nicht mit NP (Quellen uneinheitlich, Vereinfachung im Kommentar). Es wurde keine Berechnung beim Import eingebaut: Sie braucht die Messreihe, und die liest nur die Detailansicht. Teil B ruft `aerobic_decoupling` dort auf, wo die Reihe gelesen wird, oder lässt die Kennzahl im Dashboard weg. Eine Spalte in `activities` oder ein Wert im Dashboard-DTO setzt eine Migration voraus und ist hier nicht vorgenommen.
- **Planerfüllung.** Die laufende Woche wird nur gegen die bis `as_of` geplante Last gerechnet (`planned_to_date`), sonst wäre jeder Wochenanfang bei null Prozent. `planned_daily_loads(&plan.weeks)` bildet die geplante Tageslast aus dem Plan, die erledigte Last ist die vorhandene `DailyLoad`-Reihe.
- **Formprognose.** Unverändert `pmc` mit den geplanten Tageslasten, nichts Neues in `core`.
- **Entfallen.** Monotonie und Strain (Foster 1998 steht nicht in Anhang A).
- **Speicher.** Keine neuen Lesemethoden: `activities` (mit `np_w`, `avg_hr`), `zone_times` und `best_powers` sowie `recovery_days` reichen für die Eingänge. Gruppieren nach Tag und Zeitraum geschieht in `get_dashboard` (Teil B).
- **Status** stand nach Teil A auf `offen`, Teil B ist unten vermerkt.

### Teil B geliefert (15. Oktober 2026)

Status `in Prüfung`: Die Sichtprüfung im echten Tauri-Fenster mit einer echten Datenbank steht aus (nur Attrappe im Browser geprüft).

**Befehl `get_dashboard(today)`.** `app/src-tauri/src/service/dashboard.rs`, DTO `Dashboard` in `dto.rs`, an allen Stellen aus `app/README.md` eingetragen (`lib.rs` mit `COMMAND_NAMES` und `generate_handler!`, `build.rs`, `capabilities/default.json`, `Backend.getDashboard` in `types.ts`, `tauri.ts`, `mock.ts`). Der Service liest aus dem Speicher (`activities_between`, `zone_times`, `best_powers`, `recovery_days`, `daily_loads`, `active_plan`), bildet die Eingabetypen von `core::metrics` (Zeit in Zonen je Aktivität mit Kanal aus dem Katalog, Effizienz-Einheiten, Bestwerte je Zeitraum, Erholungstage), ruft die Funktionen aus Teil A und rundet auf eine Nachkommastelle (Effizienzfaktor zwei). Keine neue Lesemethode im Speicher nötig, keine neue Abhängigkeit.

| Block | Inhalt |
|---|---|
| `form` | TSB, CTL, ATL, Code `form.*`, `None` ohne Last bis heute |
| `pmc`, `projection`, `plan` | 120 Tage Verlauf, Prognose nach heute bis Planende, aktiver Plan (ID, erster und letzter Tag) |
| `ramp` | heute, Code, Grenze aus R1, Tagesreihe 84 Tage |
| `distribution` | Anteil 28 Tage, Zeit, Grenze aus R8, Code, 8 Wochen je Anteil und Zeit |
| `adherence` | Verhältnis der jüngsten Woche mit Soll, Code, Wochen des Plans |
| `efficiency` | 28-Tage-Mittel, Änderung, Code, Einheiten, Reihe 90 Tage |
| `power_curve` | je Dauer Zeitraum, Vorzeitraum, Änderung, Code (nur Radeinheiten) |
| `readiness` | Code, jüngster Tag, HRV, Ruhepuls, Schlaf (Wert, Basis, Abweichung), `plan_adjusted`, Reihe 60 Tage, `body_battery_high` und `_low` immer `None` (TODO T22) |
| `load_days` | Last je Tag der letzten 365 Tage, nur Tage mit Last |

Entscheidungen und Abweichungen:

- **Prognose.** Tatsächliche Last bis heute, geplante danach (`planned_daily_loads`), ein Lauf von `pmc`. Die Planansicht rechnet ab Planbeginn nur aus dem Plan; mitten im Plan gäbe das einen Sprung an der Nahtstelle. Ohne zählende Last bis heute gibt es keine Prognose (SPEC 5.4).
- **Formzustand in `core`.** Neu `metrics::form` (`form_state`, Codes `form.fresh` bis `form.overloaded`, Grenze -30 aus `R10.tsb_warn_below`, +5 und -10 Faustregel aus SPEC 5.5) mit Test der Grenzfälle. Die Oberfläche hält dieselben Grenzen für die Bänder im Diagramm in `lib/today.ts` (Konstanten).
- **Planerfüllung bis gestern.** `as_of` ist der Vortag, damit die Einheit von heute am Morgen nicht als verpasst zählt. Die Deutung folgt der jüngsten begonnenen Woche mit Verhältnis.
- **Basis der Erholung** folgt aus Wert und Abweichung (`Wert / (1 + Abweichung)`), die Basisrechnung bleibt allein in `core`.
- **R12-Hinweis.** `plan_adjusted` ist wahr, wenn eine Einheit des aktiven Plans den Schlüssel `reason.session.recovery_swap` trägt und heute oder in den letzten sechs Tagen liegt. Die Oberfläche zeigt "Lastwerk hat angepasst" nur bei `readiness.low` und `plan_adjusted`.
- **Entkopplung** erscheint nicht im Dashboard (braucht die Messreihe, die nur die Detailansicht liest).
- **Zonenverteilung** nach Vorrang Leistung vor Herzfrequenz vor Geschwindigkeit (`DistributionBasis::Priority`).
- **Texte.** `ui.dash.*` in beiden Sprachen, je Code ein Satz unter `ui.dash.statement.<code>`. Die Oberfläche kennt keine Schwellen und rechnet nichts neu. Ein Test prüft, dass keine Gesundheitsbegriffe vorkommen (SPEC 2.3).
- **Oberfläche.** `views/TodayView.svelte` mit Abschnitten unter `views/today/`, Kennzahlen, Balken und Kalender als SVG in `lib/components/` (`Sparkline`, `ShareBars`, `AdherenceBars`, `PowerCurveBars`, `LoadHeatmap`), Zeitreihen mit uPlot (`lib/charts-dashboard.ts`: TSB-Bänder, Prognose, Rampe mit Grenze, Effizienz). Keine neue Bibliothek, `--page-max-wide` neu als Token. `PmcChart` ersetzt die Legende der Bibliothek durch eine eigene (Punkt, Name, Wert, folgt dem Zeiger) in Heute, Woche und Plan.
- **Attrappe.** `mock-dashboard.ts` mit Varianten `rich`, `low`, `sparse`, `empty`, `extreme`, `gaps` (`MockBackend.dashboardVariant`, im Browser `?dashboard=<name>`).
- **Verifikation.** Screenshots (dunkel und hell, Deutsch, 1280 und 1600 Pixel, dazu Englisch, 820 und 560 Pixel, Varianten leer, wenig, extrem, niedrige Erholung) im Scratchpad, nicht im Repository. `break-ui`: leere Daten, sehr große Werte (CTL 446, Last 147.868), Tageslast in jedem Tag des Jahres, lange Texte und schmale Fenster brechen um, nichts läuft aus Spalte oder Karte. Fünf Jahre Daten ändern die Größe des DTO nicht (feste Fenster), der Service liest nur 180 Tage Aktivitäten.
- **Nicht gemacht.** Keine Bewegung für die neuen Abschnitte (Diagramme animieren nicht), Tastaturbedienung der Kalenderzellen (Wert steht in `title` und als Text unter dem Raster beim Überfahren).
- **Abnahme.** Rust im Wurzelverzeichnis 617 Tests, `app/src-tauri` 54 Tests (14 neu in `tests/dashboard.rs`, 3 neu in `service/dashboard.rs`, 2 neu in `metrics::form`), Oberfläche 158 Vitest, 12 Playwright, Datenschutz, Isolation und Sprachen grün.
