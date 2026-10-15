# T00: Gerüst, CI und Datenschutzprüfungen

| Feld | Wert |
|---|---|
| Status | erledigt (13. Oktober 2026, CI auf ubuntu, windows, macos grün) |
| Meilenstein | M0 |
| Voraussetzung | keine |
| cloudfähig | ja |
| Bereich | Wurzel, `crates/*`, `app/ui`, `.github`, `scripts` |
| SPEC | 1.3 bis 1.5, 5.2, 11.6, 12 (M0) |

## Ziel

Ein Repository, in dem `cargo test --workspace`, `pnpm lint/check/test` und die Datenschutzprüfungen auf Windows, macOS und Linux grün laufen und in dem Cloud-Sessions ohne Vorarbeit starten können.

## Umfang (bereits umgesetzt)

- Cargo-Workspace mit sieben Crates (`core`, `fit`, `store`, `coach`, `cli`, `mcp`, `testkit`), je mit einem trivialen Test. `rust-toolchain.toml`, `deny.toml`.
- `app/ui`: Svelte 5, Vite, TypeScript, pnpm, ESLint, Prettier, Vitest, svelte-check. Kein Tauri.
- `scripts/`: Datenschutz-, Isolations- und Sprachprüfung, Cloud-Skripte.
- `.github/workflows/ci.yml`, `bridge.yml`, `dependabot.yml`.
- `CLAUDE.md`, `docs/tasks/`, `docs/remote-sessions.md`, `.claude/settings.json`.

## Nicht im Umfang

- Tauri-Shell (kommt mit T06, als eigenes Cargo-Projekt außerhalb des Wurzel-Workspace).
- Funktionale Logik in den Crates.

## Abweichungen von der Spezifikation

- `app/src-tauri` entsteht erst in T06 (SPEC 5.2 nennt "ab M0"). Grund: Systembibliotheken (webkit2gtk) und Icons würden die Workspace-CI auf drei Systemen vorab belasten. Die M0-Abnahme in SPEC 12 verlangt es nicht.
- Die Oberflächen-Prüfungen laufen nur auf Linux (plattformunabhängig), die Rust-Prüfungen auf drei Systemen.
- Lockfiles entstehen beim ersten CI-Lauf und werden aus dem Artefakt eingecheckt. Danach gilt `--frozen-lockfile`.

## Abnahme

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --dir app/ui install --frozen-lockfile
pnpm --dir app/ui lint
pnpm --dir app/ui check
pnpm --dir app/ui test
sh scripts/check-privacy.sh
sh scripts/check-isolation.sh
sh scripts/check-i18n.sh
```

Erfolg: Alle Befehle enden mit Exit-Code 0, der CI-Lauf auf `main` ist auf ubuntu, windows und macos grün, `Cargo.lock` und `app/ui/pnpm-lock.yaml` sind eingecheckt.

## Manuelle Schritte

Keine. Die lokalen Toolchains (MSVC, rustup, Node, pnpm, SPEC 1.1) braucht nur, wer lokal bauen will. Remote-Sessions brauchen sie nicht.

## Blocker

Keine.
