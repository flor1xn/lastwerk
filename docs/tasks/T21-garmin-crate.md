# T21: Crate `lastwerk-garmin` (Token, Client, Drosselung)

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | 1.0 |
| Voraussetzung | T20 |
| cloudfähig | ja (nur Attrappen), Livetest in T23 |
| Bereich | `crates/garmin`, `Cargo.toml` (Workspace), `deny.toml` nur falls nötig |
| SPEC | 5.6, 10.2, 13, 14 (O24, O30) |

## Ziel

Ein Rust-Crate tauscht ein Garmin-Ticket gegen Token, erneuert sie, legt sie im Schlüsselbund ab und ruft eine feste Liste lesender Endpunkte gedrosselt ab. Alles getestet gegen einen lokalen Attrappen-Server.

## Warum

SPEC 5.6: Garmin ohne Python. Dieses Crate ist die einzige Stelle mit Netzwerk zu Garmin.

## Referenz (nur lesen, kein Code kopieren)

- `bridge/.venv/lib/python3.12/site-packages/garminconnect/client.py` (MIT, 0.3.17): Konstanten `:99-180`, Native-Header `:165-169` und `:286-299`, Tausch `_exchange_service_ticket` um `:1317-1380`, Erneuern `:1382-1452`, Speicherformat `:1508-1594`, API-Aufruf `:1645-1740`, Pfadprüfung.
- `bridge/.venv/.../garminconnect/__init__.py`: Pfade der Endpunkte (siehe Liste unten), `displayName` aus `socialProfile`.
- `bridge/src/lastwerk_bridge/throttle.py`: Drosselwerte und Sperre.

## Umfang

1. Neues Crate `crates/garmin` (`lastwerk-garmin`), Mitglied im Workspace. Bibliothek, kein Binary.
2. `Endpoints` mit Basis-URLs (`sso`, `diauth`, `connectapi`), Standard die echten `https`-Adressen, in Tests injizierbar (`http://127.0.0.1:<port>`). Nur Tests dürfen `http` setzen (Konstruktor `for_tests` hinter `#[cfg(any(test, feature = "test-endpoints"))]` oder gleichwertig).
3. `auth`:
   - `ticket_from_redirect(url: &str) -> Option<Ticket>`: nimmt nur `https://connect.garmin.com/app` (auch mit `/` am Ende) mit Query `ticket=ST-…` (Zeichen `[A-Za-z0-9-]`, Länge begrenzt) an, alles andere `None`. Wird von T23 im Navigationsfilter genutzt. Ausführlich testen (fremde Hosts, `connect.garmin.com.evil`, Fragment, doppelte Parameter, `http`).
   - `exchange_ticket(&Ticket) -> Result<TokenSet>`: POST wie SPEC 5.6, Client-IDs der Reihe nach, Fehler pro ID sammeln, ohne Token-Inhalte in Fehlern.
   - `refresh(&TokenSet) -> Result<TokenSet>`; `needs_refresh(&TokenSet, now)` bei weniger als 900 s bis `exp` (aus der JWT-Nutzlast, base64url, nur `exp` und `client_id` lesen, keine Signaturprüfung nötig, Kommentar warum).
   - `TokenSet` mit eigenem `Debug`, das nur `"TokenSet { .. }"` ausgibt; Serialisierung nur für die Ablage.
4. `store`: Trait `TokenStore { load, save, delete }`, `KeyringStore` (Crate `keyring`, Dienst `lastwerk-garmin`, Konto `default`, JSON als Geheimnis) und `MemoryStore` für Tests. Plattformfeatures: Windows und macOS nativ; Linux nur so, dass die CI ohne neue Systembibliothek baut (zum Beispiel `linux-native`), Einschränkung vermerken.
5. `client::GarminClient`: hält Endpunkte, Token-Ablage, Drossel. Methoden je Endpunkt der Allowlist, keine freie Pfadmethode nach außen:
   - `social_profile() -> Profile { display_name }`
   - `activities(start, limit) -> Vec<ActivitySummary { activity_id, start_time_local, activity_type }>`
   - `activity_fit(id) -> Vec<u8>` (ZIP-Rohbytes, Größenlimit 256 MiB beim Lesen)
   - `hrv_range(start, end)`, `hrv_day(date)`, `sleep_day(display_name, date)`, `rhr_range(display_name, start, end)`, `body_battery(start, end)` als `serde_json::Value` (Auswertung in T22).
   - Vor jedem Aufruf: Token laden, bei Bedarf erneuern und speichern; auf 401 genau einmal erneuern und wiederholen.
   - Header wie die Bibliothek (Bearer, Native-Header). Zeitlimit 15 s je Aufruf, Anmeldung 30 s. Keine Weiterleitungen folgen.
   - Antworten: 204 → leer, 404 → `NotAvailable`, 429/403 → `Throttled`, andere 4xx/5xx → `Http(status)`. Antwortgröße begrenzen (JSON 8 MiB, FIT 256 MiB).
6. `throttle`: 1,5 s Mindestabstand plus Zufall bis 0,5 s, drei 429/403 in Folge schreiben eine Sperrdatei (Pfad vom Aufrufer) mit Ende in 6 h; solange sie gilt, endet jeder Aufruf sofort mit `Locked { until }`. Uhr und Schlafen injizierbar, damit Tests nicht warten.
7. Fehler (`thiserror`): stabile Codes für die App (`not_connected`, `auth_failed`, `throttled`, `locked`, `not_available`, `network`, `invalid_response`), nie mit URL samt Query, Token oder Antwortinhalt.

## Nicht im Umfang

Datenbank, FIT-Import, ZIP-Entpacken (T22), Tauri-Fenster (T23), schreibende Endpunkte, Login mit Passwort.

## Vorgaben

- HTTP: blockierende API (die CLI ist blockierend, die App ruft über `spawn_blocking`). Bevorzugt dieselbe TLS-Kette wie der App-Baum (rustls, Zertifikate des Betriebssystems über `rustls-platform-verifier`), damit `deny.toml` keine neue Lizenz braucht. Zulässig sind `ureq` 3 mit rustls oder `reqwest` blockierend mit rustls. Kein Proxy aus Umgebungsvariablen zwingend, aber auch nicht verbieten (Nutzer hinter Firmenproxy).
- Neue Abhängigkeiten mit Name, Version, Lizenz, Grund unter "Vermerke" und in der Tabelle SPEC 5.3. `cargo deny check` muss für den Workspace grün sein.
- Tests nur gegen einen Attrappen-Server auf Loopback (eigener kleiner `TcpListener`-Server wie in `crates/coach/tests` oder ein Dev-Crate mit Vermerk). Synthetische Antworten, keine echten Werte. Ein Markertest prüft, dass ein Token-Marker in keinem `Debug`, `Display` und keiner Fehlermeldung erscheint.
- `crates/core` bleibt unberührt.

## Abnahme

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
sh scripts/check-privacy.sh && sh scripts/check-isolation.sh && sh scripts/check-i18n.sh
```

## Vermerke

### Abhängigkeiten (Stand 15. Oktober 2026, crates.io)

Neue direkte Abhängigkeiten von `crates/garmin`. `deny.toml` ist unverändert, `cargo deny check` meldet `advisories ok, bans ok, licenses ok, sources ok` (nur Warnungen zu doppelten Versionen, `multiple-versions = "warn"`).

| Name | Version | Lizenz | Grund |
|---|---|---|---|
| ureq | 3.4.2 | MIT OR Apache-2.0 | Schon in `coach` (ohne TLS). Hier zusätzlich mit den Funktionen `rustls-no-provider` und `platform-verifier`, weiter ohne Standardfeatures. Die Funktion `rustls` hätte `webpki-roots` (CDLA-Permissive-2.0, eigenes Paket, nicht in `deny.toml`) mitgebracht. Gewählt statt `reqwest` blockierend, weil es leichter ist (kein `tokio`, kein `hyper`) und schon im Baum steht |
| rustls | 0.23.46 | Apache-2.0 OR ISC OR MIT | Direkte Abhängigkeit nur, um die Funktion `ring` zu schalten und den Anbieter ureq zu übergeben (`unversioned_rustls_crypto_provider`). Stand wie im Sicherheitsupdate zu RUSTSEC-2026-0346 bis 0348 |
| keyring | 4.2.0 | MIT OR Apache-2.0 | Schlüsselbund des Betriebssystems. Funktion `v1` (Standard) |
| base64 | 0.23 (Lock: 0.23.1) | MIT OR Apache-2.0 | JWT-Nutzlast lesen, Basic-Authentifizierung. Schon im Baum durch ureq |
| serde, serde_json, chrono, thiserror | wie im Workspace | MIT OR Apache-2.0 | wie in `coach` und `cli`; `chrono` ohne Standardfeatures, nur `std` |

Neue Pakete in `Cargo.lock` (102 Einträge, davon rund 60 nur für `zbus`): über `ureq` und `rustls` die Kette `rustls-platform-verifier` 0.7.1, `rustls-webpki` 0.103.15 (ISC), `rustls-native-certs` 0.8.5, `openssl-probe` 0.2.1, `ring` 0.17.14 (Apache-2.0 AND ISC), `untrusted` 0.9.0 (ISC), `subtle` 2.6.1 (BSD-3-Clause), `webpki-root-certs` 1.0.9 (CDLA-Permissive-2.0, Ausnahme besteht seit T17); über `keyring` die Pakete `keyring-core`, `zbus-secret-service-keyring-store`, `secret-service`, `zbus` 5.19.0 samt `zvariant`, `endi`, `winnow` (MIT), die async-Kette von `smol` (`async-io`, `async-lock`, `event-listener` und weitere, MIT OR Apache-2.0), `aes`, `cbc`, `hkdf`, `hmac` (MIT OR Apache-2.0) sowie für macOS und Windows `apple-native-keyring-store` und `windows-native-keyring-store`. Alle Lizenzen sind in der Zulassungsliste. Vollständige Zeilen stehen in SPEC 5.3.

Folgen, auf die zu achten ist:

- **Funktionsvereinigung im Workspace.** `ureq` bekommt durch dieses Crate auch in `coach` die TLS-Funktionen. `coach` bleibt bei `http` (`ProviderError::TlsNotSupported` für `https`), die Hinweise in SPEC 5.4 zu `coach` ("ureq läuft ohne TLS-Feature") gelten damit nur noch für das Verhalten, nicht für die Bauweise.
- **`ring` braucht beim Bau einen C-Compiler.** Er steht schon im Baum von `app/src-tauri` (Updater), dessen Release-Build ihn auf allen drei Systemen baut. Die Rust-CI baut das ganze Workspace auf Ubuntu, Windows und macOS; der Bau unter Windows und macOS (`windows-native-keyring-store`, `apple-native-keyring-store`, `ring`) ist in dieser Cloud-Umgebung nicht geprüft und zeigt sich erst im ersten CI-Lauf.
- **`app/src-tauri` hat ein eigenes `Cargo.lock` und eine eigene `cargo deny`-Prüfung.** Dieses Crate ist dort noch nicht eingebunden. T23 bindet es ein und muss dort `cargo deny --manifest-path app/src-tauri/Cargo.toml --config deny.toml check` neu ausführen (dann kommt die `keyring`-Kette dazu).

### Festlegungen und Abweichungen

Ausführlich in SPEC 5.4 unter `lastwerk-garmin`. Kurz:

1. **`exchange_ticket` und `refresh` sind keine freien Funktionen**, sondern laufen über `GarminClient::connect` und den Datenabruf, damit jede Anfrage durch Drossel und Sperre geht.
2. **Zwei Fehlercodes mehr** als die sieben der Aufgabe: `store_failed` und `invalid_input`. `Http(status)` hat den Code `network`.
3. **`stress_day`** zusätzlich zu den Endpunkten der Aufgabe, weil SPEC 5.6 Stress in der Allowlist nennt. **`Endpoints::sign_in_url`** zusätzlich, damit `sso` verwendet wird und T23 die Adresse nicht selbst zusammensetzt.
4. **Der Eintrag im Schlüsselbund lässt den Zugangstoken weg**, wenn er über 2400 Bytes käme (Windows: 2560 Bytes je Eintrag). Das Erneuerungstoken bleibt.
5. **Linux nutzt den Secret Service über `zbus`** (reines Rust) statt `linux-native` (Kernel-Schlüsselbund, überlebt keinen Neustart). Ohne laufenden Secret Service ist die Ablage unter Linux nicht möglich; die App muss `store_failed` verständlich melden (T23).
6. **Wartezeit der Drossel auf 30 s begrenzt**, damit ein Sprung der Wanduhr nicht zu stundenlangem Schlafen führt. **30 s Pause nach 429 oder 403** (aus `throttle.py`) ist in der Aufgabe nicht genannt, aber dort Teil der Drosselwerte.
7. **ureq-Eigenheit:** `BodyExceedsLimit` kommt schon bei genau `limit` gelesenen Bytes; das Limit ist deshalb um eins erhöht (Test: genau 8 MiB geht durch, ein Byte mehr nicht).

### Tests

84 Tests plus 3 Doctests (`compile_fail`), ein weiterer Test (`keyring_round_trip_live`) ist `#[ignore]` und läuft nur von Hand gegen den echten Schlüsselbund. Alle gegen den Attrappen-Server auf `127.0.0.1`, mit falscher Uhr und ohne echtes Warten (Gesamtdauer unter einer Sekunde je Datei). Der Markertest wurde gegengeprüft: Mit absichtlich undichtem `Debug` schlägt er fehl.

### Offene Risiken

- **O30:** Ob `diauth.garmin.com` Anfragen von rustls (statt eines Browsers oder `curl_cffi`) annimmt, ist ungeprüft; die Python-Bibliothek nutzt `curl_cffi` mit Browser-Fingerabdruck, wenn vorhanden. Entscheidet der Livetest in T23.
- **Schlüsselbund unter Linux** setzt einen laufenden Secret Service voraus.
- **Größe der Token** ist nur geschätzt (Zugangstoken über 1000 Bytes); der Ausweg über das Weglassen steht im Code, ein echter Wert fehlt bis zum Livetest.
- **Ticket-Länge 256** und die Annahme, dass Garmin nur `ticket=` in der Weiterleitung sendet, sind Faustregeln; ein zusätzlicher Parameter würde das Ticket ablehnen (bewusst streng, T23 meldet dann `None`).
