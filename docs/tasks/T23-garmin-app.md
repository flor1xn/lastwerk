# T23: Garmin in der App (Anmeldefenster, Abgleich, Erststart)

| Feld | Wert |
|---|---|
| Status | in Prüfung |
| Meilenstein | 1.0 |
| Voraussetzung | T22, T24 |
| cloudfähig | ja (Attrappen), **Livetest manuell** |
| Bereich | `app/src-tauri`, `app/ui`, `catalog/locales` |
| SPEC | 5.6, 10.1, 10.2, 10.4 |

## Ziel

Der Nutzer klickt "Mit Garmin verbinden", meldet sich auf der Garmin-Seite an, und Lastwerk gleicht Aktivitäten und Erholung ab. Keine Python-Installation, kein Terminal.

## Umfang

1. `app/src-tauri/Cargo.toml`: Pfadabhängigkeit auf `crates/garmin` (die CLI-Bibliothek bringt den Abgleich mit). `cargo deny` für die App grün.
2. Befehle (an allen Stellen eintragen, siehe `app/README.md`): `garmin_status` → `{ connected, last_sync, sync_on_start, locked_until }` (nie Token, nie `display_name`), `garmin_login` (öffnet das Fenster, Ergebnis als Ereignis `garmin-login` `{ ok, code }`), `garmin_sync` (Fortschritt als Ereignis `garmin-sync-progress`, Ergebnis `SyncSummary` ohne Pfade), `garmin_logout`, Einstellung `garmin_sync_on_start` über `set_settings`.
3. Anmeldefenster: `WebviewWindowBuilder` mit Label `garmin-login`, externe URL laut SPEC 5.6 (Sprache aus `app.locale`), Größe etwa 480 × 720, `on_navigation`: Adressen, für die `lastwerk_garmin::auth::ticket_from_redirect` ein Ticket liefert, werden abgefangen (Rückgabe `false`), das Fenster schließt, der Tausch läuft in `spawn_blocking`. Navigation nur zu `*.garmin.com` über `https` zulassen, alles andere ablehnen. Das Fenster bekommt keine Capability (`capabilities/default.json` gilt nur für `main`, Test prüft das). Schließt der Nutzer das Fenster, Ereignis `{ ok: false, code: "cancelled" }`.
4. Abgleich beim Start: Wenn verbunden und `garmin.sync_on_start`, einmal nach dem Laden im Hintergrundthread, Fortschritt über dasselbe Ereignis. Kein Zeitplan, keine Wiederholung.
5. Oberfläche:
   - Erststart (`views/onboarding/Wizard.svelte`): Schritt "garmin" mit Erklärung in zwei Sätzen, Hinweis auf den inoffiziellen Zugang (O18), Knopf "Mit Garmin verbinden", Status, "Überspringen". Kopierbefehl und `get_bridge_status` entfallen dort. `Wizard.test.ts` behält die Prüfung "kein Eingabefeld für Passwort".
   - Einstellungen: Abschnitt "Garmin" (verbunden, letzter Abgleich, Abgleich beim Start, Jetzt abgleichen, Trennen mit Bestätigung, Sperre mit Uhrzeit).
   - Abgleich-Anzeige in der Seitenleiste: kleiner Zustand (läuft, Fortschritt, fertig mit Zahl neuer Aktivitäten).
   - Attrappe `lib/backend/mock.ts`: alle neuen Befehle mit synthetischem Ablauf.
6. `get_bridge_status` bleibt als Befehl für Nutzer von `bridge/`, wird aber in der Oberfläche nur noch in den Einstellungen unter "Erweitert" gezeigt (oder entfernt, dann SPEC vermerken).

## Vorgaben

- Fehlercodes mit Texten de/en (`ui.garmin.*`, `error.garmin_*`).
- Rust-Test: Capability-Datei nennt nur `main`; CSP unverändert; Befehlsliste.
- Playwright: Erststart und Einstellungen mit Attrappe in de und en, kein externer Host (Guard).
- Kein Token und kein `display_name` in DTOs (Test).

## Abnahme

```
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
(cd app/src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
sh scripts/check-privacy.sh && sh scripts/check-isolation.sh && sh scripts/check-i18n.sh
```

## Manuelle Schritte (Nutzer, Windows)

1. App bauen (`pnpm --dir app/ui exec tauri build --debug`) oder installieren, mit einem Testbestand (`LASTWERK_DB` auf eine Kopie oder eine leere Datei) starten.
2. Erststart oder Einstellungen, Abschnitt Garmin: "Mit Garmin verbinden", auf der Garmin-Seite anmelden (MFA).
3. Erwartung: Fenster schließt, Hinweis "Anmeldung bei Garmin erfolgreich", Zustand "Verbunden", der Abgleich startet und die Seitenleiste zeigt "Abgleich läuft" mit Fortschritt, danach "Neue Aktivitäten: n". In der Windows-Anmeldeinformationsverwaltung steht ein Eintrag `lastwerk-garmin`. Im FIT-Ordner liegt ein Unterordner `garmin/` mit `<Kennung>.fit`; ohne gewählten Ordner steht in den Einstellungen unter Daten `%LOCALAPPDATA%\Lastwerk\fit`.
4. Meldet die App "Garmin hat nach der Anmeldung anders weitergeleitet als erwartet" (`unexpected_redirect`): In der Konsole (`tauri dev` oder Start aus einer Eingabeaufforderung) steht `garmin-login: unexpected redirect, path …` mit dem Pfad ohne Query. Dann sendet Garmin die Weiterleitung anders als in T21 angenommen (etwa ein weiterer Parameter); Blocker eintragen mit dem Pfad, nicht umgehen.
5. Scheitert der Tausch mit `auth_failed` und HTTP 403: O30 ist eingetreten, Blocker eintragen, nicht umgehen.
6. Bleibt die Anmeldeseite leer oder hängt bei einem Captcha: Der Navigationsfilter lässt nur `*.garmin.com` zu; einen fremden Captcha-Host als Blocker mit Host (ohne Query) eintragen.
7. Einstellungen: "Jetzt abgleichen" läuft ein zweites Mal ohne neue Dateien; "Beim Start der App abgleichen" aus, App neu starten: kein Abgleich; "Trennen" mit Bestätigung löscht den Eintrag in der Anmeldeinformationsverwaltung.
8. Wochenansicht und Heute zeigen die geholten Aktivitäten, Erholungswerte (HRV, Ruhepuls, Schlaf, Body Battery) stehen für die letzten Tage in der Datenbank (O9: Feldnamen der Bereichsendpunkte prüfen).
9. `git status` zeigt nichts Neues, `sh scripts/check-privacy.sh` grün.

## Vermerke

### Abhängigkeiten

Keine neue Fremdabhängigkeit. `app/src-tauri/Cargo.toml` bekommt die Pfadabhängigkeit `lastwerk-garmin` (für `GarminClient`, `KeyringStore`, `ticket_from_redirect`, `Endpoints::sign_in_url`) und als Entwicklungsabhängigkeit dasselbe Crate mit der Funktion `test-endpoints` (nur für `TokenSet::for_tests` in den Tests; der normale Bau der App enthält sie nicht). `app/src-tauri/Cargo.lock` hat 52 neue Pakete, alle aus Ketten, die T21 und T22 im Wurzel-Workspace schon geprüft haben: `keyring` 4.2.0 mit `keyring-core`, `zbus` samt `zvariant`, `zbus_names`, `enumflags2`, `endi`, die async-Kette von `smol` (`async-io`, `async-lock`, `async-executor`, `blocking`, `polling`, `event-listener` und weitere), `secret-service`, `aes`, `cbc`, `hkdf`, `hmac`, `apple-native-keyring-store`, `windows-native-keyring-store`, dazu `zip` und `typed-path` (T22) und `lastwerk-garmin`. `rustls` und `ring` standen schon im Baum (Updater). `cargo deny --manifest-path app/src-tauri/Cargo.toml --config deny.toml check`: `advisories ok, bans ok, licenses ok, sources ok` (nur Warnungen zu doppelten Versionen).

### Festlegungen und Abweichungen

Ausführlich in SPEC 5.4 (`lastwerk-cli`, "Ort der Ablage"; `lastwerk-app`, "Umsetzung in T23"), 5.6 und 10.4. Kurz:

1. **Ort der FIT-Dateien (offene Frage aus T22).** Garmin-Dateien liegen in `<FIT-Ordner>/garmin/`, eingelesen wird der ganze FIT-Ordner (eine Wurzel für `source_file.rel_path`, `rebuild_cache` verliert nichts, 6.1 gilt). Ohne gewählten Ordner legt der erste Abgleich `<lokaler Datenordner>/Lastwerk/fit` an und merkt ihn als `app.fit_dir`; die Zusammenfassung meldet `fit_dir_assigned`, die Einstellungen zeigen den Ordner. Ein gewählter, aber fehlender Ordner wird nicht angelegt (`garmin_fit_dir_missing`). Dafür neu in `crates/cli` (außerhalb des Bereichs der Aufgabe, aber nötig, damit App und Kommandozeile dieselbe Regel haben): `garmin::fit_location`, `garmin::sync_into`, `SyncError::FitDirMissing`, `paths::default_fit_root` und `paths::garmin_fit_override`; `paths::default_fit_dir` entfällt. `lastwerk garmin sync` folgt derselben Regel, `LASTWERK_GARMIN_FIT` bleibt als ein Ordner für Ablage und Import (Tests).
2. **Abgleich beim Start** wird von der Oberfläche angestoßen (`garmin_sync("start")` nach dem Laden), nicht im `setup` des Rust-Teils, damit das Fortschrittsereignis nicht vor dem `listen` verloren geht. Der Rust-Teil entscheidet trotzdem allein: höchstens einmal je Programmstart, nur wenn eingeschaltet und Token vorhanden, sonst `null`.
3. **`garmin_sync` hat den Parameter `trigger`** (`manual` oder `start`) und gibt `GarminSyncSummary | null` zurück. Ein zweiter gleichzeitiger Aufruf endet mit `busy`. `manual` ohne Token endet mit `garmin_not_connected`, ein Abbruch durch Garmin steht als `aborted` in der Zusammenfassung (wie T22).
4. **Fehlercodes.** `busy`, `garmin_<code>` für die neun Codes von `lastwerk-garmin`, dazu `garmin_window_failed`, `garmin_fit_dir_missing`, `garmin_io`; Texte unter `ui.error.*`. Die Ereignisse tragen kurze Codes, Texte dazu unter `ui.garmin.code.*` (`connected`, `cancelled`, `unexpected_redirect`). Die Aufgabe nannte `error.garmin_*`; die App führt alle Fehlertexte unter `ui.error`, deshalb dort.
5. **Anmeldefenster.** Zusätzlich zur Aufgabe: inkognito, neue Fenster und Downloads abgelehnt, Entwicklerwerkzeuge nur im Debug-Build, Fenster nach vorn statt zweitem Fenster bei erneutem Klick. Erlaubt ist `https` zu `garmin.com` und `*.garmin.com` ohne Port und Zugangsdaten; `https://connect.garmin.com/app…` ohne gültiges Ticket meldet `unexpected_redirect` und protokolliert nur den Pfad. Der Navigationsfilter ist die reine Funktion `decide_navigation`.
6. **Sperrdatei** aus dem lokalen Datenordner von Tauri (`<…>/Lastwerk/garmin/lockout.json`), derselbe Pfad wie `lastwerk_cli::paths::default_lockout_file` (beide folgen 6.1).
7. **`get_bridge_status`** bleibt; die Oberfläche zeigt ihn nur noch in den Einstellungen im Abschnitt Garmin unter "Erweitert" und liest ihn erst beim Aufklappen. Die Texte dazu stehen unter `ui.garmin.bridge_*`, die alten Schlüssel `ui.onboarding.garmin.command`, `copy`, `copied`, `refresh`, `has_data`, `no_data`, `optional` sind entfallen.
8. **Seitenleiste** zeigt bei einem Abbruch nur "Abgleich unvollständig", der volle Text steht im Tooltip und in den Einstellungen (manche Fehlertexte sind zwei Sätze lang, die Seitenleiste ist 208 px breit).
9. **Kein Zeitplan, keine Wiederholung**: Ein Abgleich läuft nur auf Klick, nach erfolgreicher Anmeldung und einmal beim Start.

### Tests

- Rust, Wurzel: `cargo test --workspace` 746 Tests grün, davon neu in `crates/cli/tests/garmin_sync.rs` `fit_location_uses_the_chosen_folder_or_assigns_the_default` und `one_root_keeps_own_and_garmin_files_through_a_rebuild` (eigene und geholte Datei unter einer Wurzel, `rel_path` `garmin/<Kennung>.fit`, Wiederaufbau behält alle, zweiter Lauf lädt nichts neu), in `paths.rs` zwei geänderte Tests.
- Rust, App: 55 Tests (21 in der Bibliothek, 8 in `tests/garmin.rs`, 10 Pläne, 16 Dienst). Neu: Navigationsfilter mit über 25 feindlichen Adressen (fremde Hosts, `garmin.com.evil.example`, `evilgarmin.com`, IP-Adressen, Port, Zugangsdaten, `http`, `data:`, `blob:`, `javascript:`, `about:`, `file:`), Ticket wird erkannt, App-Adresse ohne Ticket meldet `unexpected_redirect`; Capability nur für `main`, keine zweite Capability-Datei, kein `remote`; Befehlsliste mit 28 Befehlen; Sperre gegen zwei Abgleiche (auch nach Panik); Ausdünnen des Fortschritts; Zustand, Sperre mit Ende, Trennen, Einstellung beim Start; Abgleich mit Attrappe der Quelle (eine Wurzel, Aktivitätsliste zeigt beide Fahrten), Standardordner, fehlender Ordner, Abbruch als Ergebnis; Markertest: kein Token, kein Anzeigename, kein Pfad und kein Feldname mit `token`, `ticket`, `name`, `path` in Zustand, Zusammenfassung, Fortschritt und Anmeldeereignis.
- Vitest: 137 Tests, neu `lib/garmin.test.ts` (Texte zu allen Codes, unbekannter Code, Fortschritt je Phase, kein zweiter Abgleich, `busy` der Attrappe, Start höchstens einmal, Fehler ohne Verbindung), `views/settings/GarminSection.test.ts` (Zustand, Abgleich, Trennen mit Bestätigung, Einstellung, Sperre mit Uhrzeit, Abbruch als Text, Zusatzprogramm nur unter "Erweitert", Standardordner, Seitenleiste und Abgleich beim Start), `Wizard.test.ts` (kein Eingabefeld, kein Kopierbefehl, Verbinden mit Abgleich, abgebrochene Anmeldung).
- Playwright: 14 Tests, neu `e2e/garmin.spec.ts` (Einstellungen in de und en: verbinden, abgleichen, Seitenleiste, Erweitert, Trennen), `e2e/onboarding.spec.ts` verbindet im Erststart in de und en. Guard: kein fremder Host, keine Konsolenfehler.

### Nicht geprüft (Livetest)

- Ob Garmin die Weiterleitung genau als `https://connect.garmin.com/app?ticket=ST-…` sendet (T21 ist bewusst streng) und ob `diauth.garmin.com` den Tausch von rustls annimmt (O30).
- Ob die Anmeldeseite Inhalte auf fremden Hosts als Navigation lädt (Captcha); der Filter lehnt sie ab.
- Verhalten von `on_navigation`, `incognito` und Schließen im Navigationsfilter unter WebView2 (Windows) und WKWebView (macOS); gebaut und getestet ist nur Linux ohne Fenster.
- Schlüsselbund unter Linux ohne Secret Service: Die App meldet `garmin_store_failed` in den Einstellungen, beim Start bleibt sie still.
- Feldnamen der Bereichsendpunkte (O9) gegen ein echtes Konto, wie in T22.
