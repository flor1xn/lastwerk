# T17 Windows-Installer und Update auf Knopfdruck

Status: in Prüfung
Meilenstein: nach M3
Voraussetzung: T09, T18
cloudfähig: nein (lokaler Build, Secret und erster Release manuell)
Bereich: app, scripts, .github
SPEC: 10.1, 10.4, 11.6

## Ziel

Lastwerk lässt sich unter Windows als normale App installieren und aus der App heraus auf eine neue Version aktualisieren, nur auf ausdrücklichen Klick.

## Warum

Der Nutzer will die App mit eigenen Daten nutzen und spätere Änderungen ohne Neubau erhalten.

## Umfang

- `scripts/build-installer.ps1`: lokaler Build von msi und nsis.
- Updater: Plugin, Befehle `check_update` und `install_update`, Abschnitt im Einstellungsdialog, Texte de und en.
- Konfiguration: `plugins.updater` (fester Endpunkt, öffentlicher Schlüssel), `createUpdaterArtifacts` im Release-Overlay.
- `release.yml`: Signaturschlüssel aus Secrets, `.sig` und `latest.json` am Release-Entwurf.
- SPEC 10.1 und die Liste der Nicht-Ziele: Abweichung vermerkt.

## Nicht im Umfang

Windows-Codesigning, Hintergrundcheck, Updates für macOS und Linux, `bridge/`.

## Vorgaben

- Netzwerkzugriff nur im Rust-Teil, nur nach Klick, nur auf den festen Endpunkt. Das Frontend bekommt keine URL.
- Neue Abhängigkeit: `tauri-plugin-updater` 2.13 (Lizenz MIT OR Apache-2.0), Grund: signaturgeprüftes Update. Gleiche Tauri-Familie wie `tauri-plugin-dialog`.
- Der private Schlüssel liegt außerhalb des Repositories (`%USERPROFILE%\.lastwerk-signing\updater.key`, ohne Passwort erzeugt) und ist als Secret einzutragen.

## Abnahme

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` in `app/src-tauri`.
- `pnpm --dir app/ui lint`, `check`, `test`.
- `sh scripts/check-privacy.sh`, `check-isolation.sh`, `check-i18n.sh`.
- `scripts/build-installer.ps1` erzeugt msi, nsis und mit Schlüssel die `.sig`.

## Manuelle Schritte

1. Secret `TAURI_SIGNING_PRIVATE_KEY` (Inhalt von `updater.key`) im Repository anlegen, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` leer oder weglassen.
2. Installer ausführen (SmartScreen: "Weitere Informationen", "Trotzdem ausführen").
3. FIT-Ordner im Einstellungsdialog wählen.
4. Für das erste Update: Tag `v*` setzen, Entwurf veröffentlichen, in der installierten App "Nach Update suchen".

## Vermerke

- Der Endpunkt `releases/latest/download/latest.json` liefert nur veröffentlichte Releases, nie den Entwurf.
- Installer und Update der ersten Version (0.1.0) wurden lokal gebaut, ein Update von 0.1.0 auf eine neuere Version ist noch nicht live geprüft.
- 2026-10-15 (T30): Releases entstehen nach jeder grünen CI auf main mit Produktänderung automatisch und veröffentlicht, Windows nur noch als `-setup.exe`. Wer 0.1.x per `.msi` installiert hat, deinstalliert einmal und installiert die `-setup.exe`; danach genügt "Nach Update suchen".
- 2026-10-14: `deny.toml` erlaubt `CDLA-Permissive-2.0` nur für das Crate `webpki-root-certs` (Wurzelzertifikate, permissiv), das über das Updater-Plugin in die Kette kommt.
