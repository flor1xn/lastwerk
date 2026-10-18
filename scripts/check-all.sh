#!/bin/sh
# Alle Prüfungen, die die CI macht, lokal in einem Lauf. Vor jedem Push ausführen (CLAUDE.md).
#
# Warum: Die Analyse vom 15. Oktober 2026 fand 13 rote CI-Läufe auf main, fast alle nach einem
# Push ohne vollständigen lokalen Lauf: das Tauri-Projekt (eigener Workspace) nicht mitgebaut,
# actionlint und die Lizenzprüfung nicht gelaufen, ein Schematest in bridge/ nicht ausgeführt.
#
# Aufruf im Wurzelverzeichnis: sh scripts/check-all.sh [--quick]
#   --quick  ohne Tauri-Tests, Playwright und bridge/ (für Zwischenschritte, nicht vor dem Push)
#
# Fehlende Werkzeuge (cargo-deny, actionlint, zizmor, gitleaks, uv, WebKitGTK) werden mit Hinweis
# übersprungen, Exit 3; den Rest prüft dann die CI auf dem Arbeitszweig. Exit 1 bei jedem Fehler.

set -u
QUICK=0
[ "${1:-}" = "--quick" ] && QUICK=1

failed=0
skipped=0
step() { printf '\n== %s\n' "$1"; }
run() {
  if ! "$@"; then
    printf 'FEHLER: %s\n' "$*"
    failed=1
  fi
}
skip() {
  printf 'ÜBERSPRUNGEN: %s\n' "$1"
  skipped=1
}
has() { command -v "$1" >/dev/null 2>&1; }

step "Datenschutz, Sprachdateien"
run sh scripts/check-privacy.sh
run sh scripts/test-check-privacy.sh
run sh scripts/test-check-history.sh
run sh scripts/check-i18n.sh

step "Rust: Wurzel-Workspace"
run cargo fmt --all --check
run cargo clippy --workspace --all-targets --locked -- -D warnings
run cargo test --workspace --locked

step "Rust: Tauri-Projekt (eigener Workspace, wird von --workspace nicht gebaut)"
run cargo fmt --manifest-path app/src-tauri/Cargo.toml --check
if [ "$QUICK" = 1 ]; then
  run cargo clippy --manifest-path app/src-tauri/Cargo.toml --all-targets --locked -- -D warnings
  skip "Tests des Tauri-Projekts (--quick)"
elif has pkg-config && pkg-config --exists webkit2gtk-4.1 2>/dev/null || [ "$(uname -s)" != "Linux" ]; then
  run cargo clippy --manifest-path app/src-tauri/Cargo.toml --all-targets --locked -- -D warnings
  run cargo test --manifest-path app/src-tauri/Cargo.toml --locked
else
  skip "Tauri-Projekt (WebKitGTK fehlt, siehe ci.yml Job tauri für die Pakete)"
fi

step "Lizenzen und Quellen (cargo-deny)"
if has cargo-deny; then
  run cargo deny check bans licenses sources
  run cargo deny --manifest-path app/src-tauri/Cargo.toml --config deny.toml check bans licenses sources
else
  skip "cargo-deny nicht installiert (cargo install cargo-deny@0.20.2 --locked)"
fi

step "Workflows (actionlint)"
if has actionlint; then
  run actionlint
else
  skip "actionlint nicht installiert"
fi

step "Angriffsfläche der Workflows (zizmor)"
if has zizmor; then
  run zizmor --offline --min-severity medium --format plain .github/workflows
else
  skip "zizmor nicht installiert (pip install zizmor==1.31.0)"
fi

# Wie der Job guards. Fehlalarm: Fingerprint in .gitleaksignore.
step "Secret-Scan (gitleaks)"
if has gitleaks; then
  run gitleaks detect --source . --no-banner --redact
else
  skip "gitleaks nicht installiert (Version 8.18.4 wie in ci.yml)"
fi

step "Oberfläche"
if has pnpm; then
  run pnpm --dir app/ui install --frozen-lockfile --silent
  run pnpm --dir app/ui lint
  run pnpm --dir app/ui check
  run pnpm --dir app/ui test
  run pnpm --dir app/ui audit --prod
  if [ "$QUICK" = 1 ]; then
    skip "Playwright-Rauchtest (--quick)"
  else
    run pnpm --dir app/ui build
    run pnpm --dir app/ui exec playwright test
  fi
else
  skip "pnpm nicht installiert"
fi

step "Zusatzprogramm (bridge/)"
if [ "$QUICK" = 1 ]; then
  skip "bridge/ (--quick)"
elif has uv; then
  # Unterschale für das Verzeichnis; `failed` wird außerhalb gesetzt.
  if ! (cd bridge && uv run ruff check . && uv run ruff format --check src tests && uv run pytest -q); then
    echo "FEHLER: bridge/ (ruff oder pytest)"
    failed=1
  fi
else
  skip "uv nicht installiert"
fi

printf '\n'
if [ "$failed" = 1 ]; then
  echo "Ergebnis: FEHLER. Nicht pushen."
  exit 1
fi
if [ "$skipped" = 1 ]; then
  echo "Ergebnis: grün, aber unvollständig (siehe ÜBERSPRUNGEN)."
  exit 3
fi
echo "Ergebnis: alles grün."
