#!/bin/bash
# Lastwerk: SessionStart-Hook für Cloud-Sessions (.claude/settings.json).
# Läuft bei jedem Start und bei jedem Fortsetzen, deshalb nur Leichtes und nur in der Cloud.
# Endet immer mit Exit-Code 0, damit ein Netzwerkfehler die Session nicht blockiert.

if [ "$CLAUDE_CODE_REMOTE" != "true" ]; then
  exit 0
fi

cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0

# Hooks gegen versehentliche Commits privater Daten (SPEC 1.5).
git config core.hooksPath .githooks || true

# Rust: Abhängigkeiten vorab holen, falls schon ein Lockfile existiert.
if [ -f Cargo.lock ] && command -v cargo >/dev/null 2>&1; then
  cargo fetch --locked >/dev/null 2>&1 || cargo fetch >/dev/null 2>&1 || true
fi

# Oberfläche: Abhängigkeiten installieren, falls das Projekt schon existiert.
if [ -f app/ui/package.json ] && command -v pnpm >/dev/null 2>&1; then
  if [ -f app/ui/pnpm-lock.yaml ]; then
    pnpm --dir app/ui install --frozen-lockfile >/dev/null 2>&1 || true
  else
    pnpm --dir app/ui install >/dev/null 2>&1 || true
  fi
fi

# Zusatzprogramm: Umgebung synchronisieren, falls das Projekt schon existiert.
if [ -f bridge/pyproject.toml ] && command -v uv >/dev/null 2>&1; then
  (cd bridge && uv sync >/dev/null 2>&1) || true
fi

exit 0
