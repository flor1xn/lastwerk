#!/bin/sh
# Tests für scripts/check-privacy.sh (T38). POSIX sh, läuft auch in der Windows-Git-Bash der CI.
#
# Jeder Fall legt in einem frischen Wegwerf-Repository eine synthetische Datei an, nimmt sie in
# den Index auf (kein Commit) und erwartet das Ergebnis der Prüfung. Die Fundstücke (FIT-Kopf,
# Schlüssel, Token) entstehen erst hier aus Einzelteilen; im Repository steht keines davon, und
# der Quelltext dieses Tests trifft die Muster der Prüfung nicht.
#
# Aufruf im Wurzelverzeichnis: sh scripts/test-check-privacy.sh
# Exit 0 = alle Fälle wie erwartet, 1 = mindestens ein Fall weicht ab.

root=$(pwd)
[ -f "$root/scripts/check-privacy.sh" ] || {
  echo "Bitte im Wurzelverzeichnis des Repositories aufrufen." >&2
  exit 2
}

work=$(mktemp -d 2>/dev/null || mktemp -d -t lwprivtest) || exit 2
trap 'rm -rf "$work"' EXIT HUP INT TERM

failed=0
total=0

# Frisches Repository mit den Dateien, die die Prüfung braucht.
fresh() {
  rm -rf "$work/repo"
  mkdir -p "$work/repo/scripts" "$work/repo/local"
  cd "$work/repo" || exit 2
  git init -q .
  cp "$root/scripts/check-privacy.sh" "$root/scripts/privacy-patterns.txt" scripts/
  cp "$root/.gitignore" .gitignore
  printf '# Lastwerk\n' >README.md
  printf 'Platzhalter\n' >local/README.md
}

# Ein Fall: $1 = Name, $2 = erwarteter Exit (0 oder 1), $3 = Text, den die Meldung nennen muss
# (leer = egal). Die Datei legt die Funktion `setup` an, die der Aufrufer vorher definiert.
run_case() {
  name=$1
  want=$2
  must=$3
  total=$((total + 1))
  fresh
  setup
  git add -A -f . >/dev/null 2>&1
  out=$(sh scripts/check-privacy.sh 2>&1)
  got=$?
  ok=1
  [ "$got" -eq "$want" ] || ok=0
  if [ -n "$must" ]; then
    printf '%s' "$out" | grep -qF "$must" || ok=0
  fi
  # Ein Fund darf nie im Wortlaut ausgegeben werden.
  if printf '%s' "$out" | grep -qF "$SECRET_MARK"; then
    ok=0
  fi
  if [ "$ok" -eq 1 ]; then
    echo "ok       $name"
  else
    echo "FEHLER   $name (Exit $got, erwartet $want)"
    printf '%s\n' "$out" | sed 's/^/         /'
    failed=$((failed + 1))
  fi
  cd "$root" || exit 2
}

# Marker, der in allen Fundstücken vorkommt und in keiner Ausgabe stehen darf.
SECRET_MARK=ZZMARK

pad_zero() { # $1 = Anzahl Nullbytes
  i=0
  while [ "$i" -lt "$1" ]; do
    printf '\000'
    i=$((i + 1))
  done
}

fit_header() { # FIT-Kopf: Größe 14, Protokoll 0x20, Profil 0, Datengröße 0, ".FIT", CRC
  printf '\016\040\000\000\000\000\000\000.FIT\000\000'
}

# --- Sauberer Stand und erlaubte Inhalte ---------------------------------------------------

setup() { :; }
run_case "sauberer Stand" 0 ""

setup() {
  # Öffentlicher Schlüssel des Updaters ("minisign public key") ist gewollt im Repository.
  printf '%s\n' 'untrusted comment: minisign public key: 0000000000000000' >pub.txt
  printf 'RWQ%s\n' 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA' >>pub.txt
}
run_case "öffentlicher Schlüssel ist erlaubt" 0 ""

setup() {
  # Text, dessen Zeile mit acht Zeichen und ".FIT" beginnt, ist kein FIT-Inhalt (Textdatei).
  printf 'Beispiel.FIT ist ein Dateiname in der Anleitung.\n' >doku.md
}
run_case "Text mit .FIT in der Zeile ist kein Fund" 0 ""

setup() {
  # Icons und andere Binärdateien ohne Magic Bytes von FIT, SQLite oder Archiv.
  printf '\211PNG\r\n\032\n' >icon.png
  pad_zero 32 >>icon.png
}
run_case "PNG ist erlaubt" 0 ""

# --- Magic Bytes, unabhängig von der Endung -------------------------------------------------

setup() { fit_header >daten.bin; pad_zero 20 >>daten.bin; }
run_case "FIT-Inhalt unter fremder Endung" 1 "FIT-Inhalt (Magic Bytes) in: daten.bin"

setup() { mkdir -p "Ordner mit Leerzeichen"; fit_header >"Ordner mit Leerzeichen/Fahrt eins.txt"; pad_zero 20 >>"Ordner mit Leerzeichen/Fahrt eins.txt"; }
run_case "FIT-Inhalt, Pfad mit Leerzeichen" 1 "Fahrt eins.txt"

setup() { fit_header >aktivitaet.fit; }
run_case "FIT-Endung" 1 "FIT-Dateien im Index"

setup() { printf 'SQLite format 3\000' >notizen.txt; pad_zero 40 >>notizen.txt; }
run_case "SQLite-Inhalt unter fremder Endung" 1 "SQLite-Datenbank (Magic Bytes) in: notizen.txt"

setup() { printf 'PK\003\004' >export.dat; pad_zero 30 >>export.dat; }
run_case "ZIP-Inhalt" 1 "Archiv (ZIP, gzip oder 7z) in: export.dat"

setup() { printf '\037\213\010' >export.dat; pad_zero 30 >>export.dat; }
run_case "gzip-Inhalt" 1 "Archiv (ZIP, gzip oder 7z) in: export.dat"

setup() {
  printf 'PK\003\004' >fixture.dat
  pad_zero 30 >>fixture.dat
  printf '# geprüft: synthetisches Archiv\nfixture.dat\n' >scripts/privacy-allowlist.txt
}
run_case "ZIP-Inhalt in der Ausnahmeliste" 0 ""

setup() {
  fit_header >fixture.dat
  pad_zero 20 >>fixture.dat
  printf 'fixture.dat\n' >scripts/privacy-allowlist.txt
}
run_case "FIT-Inhalt kommt auch über die Ausnahmeliste nicht durch" 1 "FIT-Inhalt (Magic Bytes) in: fixture.dat"

# --- Dateinamen (unverändert aus der alten Prüfung) ----------------------------------------

setup() { printf 'x' >local/geheim.txt; }
run_case "Datei unter local/" 1 "local/geheim.txt"

setup() { printf '{}' >garmin_tokens.json; }
run_case "Token-Datei" 1 "garmin_tokens.json"

setup() { printf 'x' >lastwerk.db; }
run_case "Datenbankdatei" 1 "lastwerk.db"

# --- Muster in Textdateien -----------------------------------------------------------------

mk() { # $1 = Dateiname, übrige = Teile des Fundstücks, die ohne Trenner zusammengesetzt werden
  f=$1
  shift
  : >"$f"
  for part in "$@"; do
    printf '%s' "$part" >>"$f"
  done
  printf '\n' >>"$f"
}

setup() { mk key.txt '-----BEGIN ' 'RSA ' 'PRIVATE KEY-----' ZZMARK; }
run_case "privater Schlüssel (PEM)" 1 "Privater Schlüssel (PEM) in:"

setup() { mk key.txt '-----BEGIN ' 'PRIVATE KEY-----' ZZMARK; }
run_case "privater Schlüssel (PEM, ohne Algorithmus)" 1 "Privater Schlüssel (PEM) in:"

setup() { mk jwt.txt 'ey' 'JhbGciOiJSUzI1NiJ9' '.' 'ey' 'JleHAiOjE3NjAwMDAwMDB9' '.' "c2lnbmF0dXJl${SECRET_MARK}"; }
run_case "JWT" 1 "JWT (zum Beispiel ein Garmin-Token) in:"

setup() { mk gh.txt 'gh' 'p_' 'aaaaaaaaaabbbbbbbbbbccccccccccdddddd' ZZMARK; }
run_case "GitHub-Token (klassisch)" 1 "GitHub-Token in:"

setup() { mk gh.txt 'github' '_pat_' '11AAAAAAAAAAAAAAAAAAAA_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB' ZZMARK; }
run_case "GitHub-Token (fein granular)" 1 "GitHub-Token in:"

setup() { mk g.txt 'ya' '29.' 'aaaaaaaaaabbbbbbbbbbcccccc' ZZMARK; }
run_case "Google-OAuth-Token" 1 "Google-OAuth-Token in:"

setup() { mk s.txt 'xo' 'xb-' '1234567890-abcdefghij' ZZMARK; }
run_case "Slack-Token" 1 "Slack-Token in:"

setup() { mk a.txt 'AK' 'IA' 'ABCDEFGHIJKLMNOP' ZZMARK; }
run_case "AWS-Zugangsschlüssel" 1 "AWS-Zugangsschlüssel in:"

setup() { mk c.txt 'sk' '-ant-' 'abcdefghijklmnopqrstuvwxyz' ZZMARK; }
run_case "Anthropic-Schlüssel" 1 "Anthropic-Schlüssel in:"

setup() { mk u.key 'untrusted comment: rsign encrypted ' 'secret key' ZZMARK; }
run_case "geheimer Signaturschlüssel (Klartext)" 1 "Geheimer Signaturschlüssel des Updaters in:"

setup() { mk u.b64 'dW50cnVz' 'dGVkIGNvbW1lbnQ6IHJzaWduIGVuY3J5cHRlZCBzZWNyZXQga2V5OiBaWk1BUks='; }
run_case "geheimer Signaturschlüssel (Base64)" 1 "Geheimer Signaturschlüssel des Updaters in:"

setup() { printf '{"refresh_token": "abcdefghijklmnop%s"}\n' ZZMARK >t.json; }
run_case "Token-Feld mit Wert" 1 "Token-Feld mit Wert (JSON) in:"

setup() { printf '{"refresh_token": "kurz"}\n' >t.json; }
run_case "Token-Feld mit kurzem Platzhalter" 0 ""

setup() {
  mk jwt.txt 'ey' 'JhbGciOiJSUzI1NiJ9' '.' 'ey' 'JleHAiOjE3NjAwMDAwMDB9' '.' 'c2lnbmF0dXJl'
  printf 'jwt.txt\n' >scripts/privacy-allowlist.txt
}
run_case "Muster in der Ausnahmeliste" 0 ""

# Unstaged zählt nicht, Staged zählt: Datei nur im Arbeitsbaum, nicht im Index.
total=$((total + 1))
fresh
mk key.txt '-----BEGIN ' 'PRIVATE KEY-----' ZZMARK
git add -A . >/dev/null 2>&1
git rm -q --cached key.txt >/dev/null 2>&1
if sh scripts/check-privacy.sh >/dev/null 2>&1; then
  echo "ok       nur im Arbeitsbaum, nicht im Index"
else
  echo "FEHLER   nur im Arbeitsbaum, nicht im Index"
  failed=$((failed + 1))
fi
cd "$root" || exit 2

echo
if [ "$failed" -ne 0 ]; then
  echo "$failed von $total Fällen weichen ab."
  exit 1
fi
echo "Alle $total Fälle wie erwartet."
