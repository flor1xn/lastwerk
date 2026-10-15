#!/bin/sh
# Lastwerk: Schlüsselgleichheit der Sprachdateien (SPEC 7.5, 11.4).
# Vergleicht die Schlüsselmengen von catalog/locales/de.json und en.json.
# Exit 0 = gleich, 1 = Abweichung, 2 = Eingabefehler.

PY=python3
command -v python3 >/dev/null 2>&1 || PY=python

"$PY" - catalog/locales/de.json catalog/locales/en.json <<'EOF'
import json
import sys


def keys(obj, prefix=""):
    out = set()
    if isinstance(obj, dict):
        for k, v in obj.items():
            path = f"{prefix}.{k}" if prefix else k
            if isinstance(v, dict):
                out |= keys(v, path)
            else:
                out.add(path)
    return out


try:
    with open(sys.argv[1], encoding="utf-8") as f:
        de = keys(json.load(f))
    with open(sys.argv[2], encoding="utf-8") as f:
        en = keys(json.load(f))
except (OSError, ValueError) as e:
    print(f"FEHLER: {e}", file=sys.stderr)
    sys.exit(2)

only_de = sorted(de - en)
only_en = sorted(en - de)
if only_de or only_en:
    for k in only_de:
        print(f"nur in de.json: {k}", file=sys.stderr)
    for k in only_en:
        print(f"nur in en.json: {k}", file=sys.stderr)
    sys.exit(1)
print(f"i18n-Pruefung: ok ({len(de)} Schluessel)")
EOF
