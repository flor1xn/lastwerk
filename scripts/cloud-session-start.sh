#!/bin/bash
# Lastwerk: SessionStart-Hook für Cloud-Sessions (.claude/settings.json).
# Läuft bei jedem Start und bei jedem Fortsetzen, deshalb nur Leichtes und nur in der Cloud.
# Endet immer mit Exit-Code 0, damit ein Netzwerkfehler die Session nicht blockiert.

cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0

# Lokal und in der Cloud: Ist der letzte Lauf eines Workflows auf main rot, steht das am Anfang
# der Session im Kontext (T46, docs/ci.md). Auch der tägliche Lauf der Sicherheitshinweise kann
# ohne Commit rot werden und blockiert dann jeden Release. Nur mit gh, höchstens 15 s.
if command -v gh >/dev/null 2>&1; then
  red=$(timeout 15 gh run list --branch main --limit 40 \
    --json conclusion,status,workflowName,createdAt,url \
    --jq '[.[] | select(.status == "completed")] | group_by(.workflowName)
      | map(max_by(.createdAt)) | .[] | select(.conclusion == "failure")
      | "\(.createdAt) \(.workflowName) \(.url)"' 2>/dev/null)
  if [ -n "$red" ]; then
    echo "CI: letzter Lauf auf main rot, zuerst beheben (docs/ci.md):"
    echo "$red"
  fi
fi

if [ "$CLAUDE_CODE_REMOTE" != "true" ]; then
  exit 0
fi

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
