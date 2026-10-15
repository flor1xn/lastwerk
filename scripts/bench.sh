#!/bin/sh
# Laufzeitmessung mit synthetischen Daten (T38, docs/tasks/T38-anhang-messung.md).
#
# Aufruf aus dem Wurzelverzeichnis: sh scripts/bench.sh
# Voraussetzung: hyperfine im PATH. Mit BIN_DIR=<ordner> misst das Skript fertige Binaries
# (lastwerk, gen-bench, bench_phases) aus diesem Ordner, ohne neu zu bauen; so lassen sich
# zwei Stände mit denselben Daten vergleichen. Die Daten entstehen in einem temporären
# Ordner (mktemp) und werden am Ende gelöscht, nichts davon landet im Repository.
# RUNS legt die Zahl der Läufe je Messung fest (Standard 7, mindestens 5 für Median und
# Minimum).
set -eu

RUNS="${RUNS:-7}"

if [ -z "${BIN_DIR:-}" ]; then
    cargo build --release --locked -p lastwerk-cli -p lastwerk-testkit --bins
    cargo build --release --locked -p lastwerk-store --example bench_phases
    BIN_DIR="$(pwd)/target/release"
    PHASES="$BIN_DIR/examples/bench_phases"
else
    PHASES="$BIN_DIR/bench_phases"
fi
LW="$BIN_DIR/lastwerk"
GEN="$BIN_DIR/gen-bench"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT INT TERM

echo "Rechner: $(nproc) Kerne, Last: $(uptime)"

# hyperfine mit JSON-Ausgabe, danach eine Zeile mit Median und Minimum in Millisekunden.
# Aufruf: measure <name> <befehl> [<vorbereitung>]
measure() {
    json="$WORK/result.json"
    if [ -n "${3:-}" ]; then
        hyperfine --warmup 1 --runs "$RUNS" --style none --export-json "$json" \
            --prepare "$3" -n "$1" "$2"
    else
        hyperfine --warmup 1 --runs "$RUNS" --style none --export-json "$json" -n "$1" "$2"
    fi
    awk -v name="$1" '
        /"median":/ { gsub(/[",]/, "", $2); median = $2 * 1000 }
        /"min":/ { gsub(/[",]/, "", $2); min = $2 * 1000 }
        END { printf "%-16s median %10.1f ms   min %10.1f ms\n", name, median, min }
    ' "$json"
}

"$GEN" --out "$WORK/long10" --set long --count 10
"$GEN" --out "$WORK/long50" --set long --count 50
"$GEN" --out "$WORK/hist1" --set history --years 1 --count 300
"$GEN" --out "$WORK/hist5" --set history --years 5 --count 1500

# Schwellen für alle Sportarten, damit jede Belastung wirklich gerechnet wird.
BASE="$WORK/base.db"
"$LW" thresholds set --db "$BASE" --sport cycling --ftp 250 --lthr 165 --hr-max 190 \
    --hr-rest 50 --from 2020-01-01 >/dev/null
"$LW" thresholds set --db "$BASE" --sport running --run-speed 3.6 --lthr 168 --hr-max 190 \
    --hr-rest 50 --from 2020-01-01 >/dev/null
"$LW" thresholds set --db "$BASE" --sport swimming --css 1.2 --from 2020-01-01 >/dev/null
"$LW" athlete set --db "$BASE" --sex male >/dev/null 2>&1

for set in long10 long50 hist1 hist5; do
    # Erstimport in eine frische Kopie der Datenbank mit Schwellen.
    measure "import $set" "'$LW' import --db '$WORK/run.db' --dir '$WORK/$set'" \
        "cp '$BASE' '$WORK/run.db'"
    # Zweiter Lauf: alle Dateien unverändert, nur Lesen, Prüfsumme und Abgleich.
    cp "$BASE" "$WORK/$set.db"
    "$LW" import --db "$WORK/$set.db" --dir "$WORK/$set" >/dev/null
    measure "reimport $set" "'$LW' import --db '$WORK/$set.db' --dir '$WORK/$set'"
done

DB="$WORK/hist5.db"
measure "pmc hist5" "'$LW' pmc --db '$DB' --from 2021-01-04 --to 2025-12-31 --format csv"
measure "plan hist5" \
    "'$LW' plan --db '$DB' --week-start 2026-01-05 --weeks 12 --out '$WORK/plan.json'"

"$PHASES" --set long --count 10 --runs "$RUNS"
"$PHASES" --set long --count 50 --runs "$RUNS"
"$PHASES" --set history --years 1 --count 300 --runs "$RUNS"
"$PHASES" --set history --years 5 --count 1500 --runs "$RUNS"
