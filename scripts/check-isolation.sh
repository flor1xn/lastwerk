#!/bin/sh
# Lastwerk: Isolationsprüfung (SPEC 1.5, 11.4).
# bridge/ importiert nichts aus crates/ und app/, und umgekehrt.
# Austausch nur über Dateien (SPEC 5.4). Prüfung per Pfadsuche in Code und Manifesten.
# Exit 0 = sauber, 1 = Verstoß.

fail=0

# Treffer in Dateien suchen, die zum Code oder zu Manifesten gehören.
# $1 = Ordner, $2 = Muster, $3 = Meldung
scan() {
  [ -d "$1" ] || return 0
  hits=$(grep -rEn --include='*.py' --include='*.rs' --include='*.ts' --include='*.svelte' \
    --include='*.js' --include='Cargo.toml' --include='package.json' --include='pyproject.toml' \
    --exclude-dir=node_modules --exclude-dir=target --exclude-dir=.venv \
    "$2" "$1" 2>/dev/null)
  if [ -n "$hits" ]; then
    echo "VERSTOSS: $3" >&2
    echo "$hits" >&2
    fail=1
  fi
}

scan bridge '(\.\./)+(crates|app)/' "bridge/ verweist auf crates/ oder app/"
scan bridge 'lastwerk-(core|fit|store|coach|cli|mcp|app|testkit)' "bridge/ nennt einen Lastwerk-Crate"
scan crates '(\.\./)+bridge/' "crates/ verweist auf bridge/"
scan app '(\.\./)+bridge/' "app/ verweist auf bridge/"
scan crates 'lastwerk_bridge' "crates/ importiert lastwerk_bridge"
scan app 'lastwerk_bridge' "app/ importiert lastwerk_bridge"

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "Isolationsprüfung: ok"
