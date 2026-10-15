#!/bin/sh
# Tests für scripts/check-history.sh (T38). POSIX sh.
#
# Jeder Fall baut ein Wegwerf-Repository mit synthetischem Verlauf (Fundstücke entstehen hier aus
# Einzelteilen und werden im nächsten Commit wieder gelöscht: Der Fund steckt nur in der
# Geschichte) und ruft die Prüfung mit --ohne-scanner auf. Gitleaks und Trufflehog prüft
# check-history.sh selbst; hier geht es um Magic Bytes, Muster, Personenbezug und Metadaten.
#
# Aufruf im Wurzelverzeichnis: sh scripts/test-check-history.sh

root=$(pwd)
[ -f "$root/scripts/check-history.sh" ] || {
  echo "Bitte im Wurzelverzeichnis des Repositories aufrufen." >&2
  exit 2
}

work=$(mktemp -d 2>/dev/null || mktemp -d -t lwhisttest) || exit 2
trap 'rm -rf "$work"' EXIT HUP INT TERM

failed=0
total=0
MARK=ZZMARK
BS=$(printf '\134')

pad_zero() {
  i=0
  while [ "$i" -lt "$1" ]; do
    printf '\000'
    i=$((i + 1))
  done
}

mk() { # $1 = Datei, übrige Argumente werden ohne Trenner zusammengesetzt
  f=$1
  shift
  : >"$f"
  for part in "$@"; do
    printf '%s' "$part" >>"$f"
  done
  printf '\n' >>"$f"
}

# Neues Repository mit einem sauberen ersten Commit. Zweiter Parameter: Autoradresse.
fresh() {
  rm -rf "$work/src"
  mkdir -p "$work/src/scripts"
  cd "$work/src" || exit 2
  git init -q .
  git config user.name Tester
  git config user.email "${1:-tester@users.noreply.github.com}"
  cp "$root/scripts/privacy-patterns.txt" scripts/
  printf '# Lastwerk\n' >README.md
  git add -A . && git commit -q -m "Anfang"
}

# $1 = Name, $2 = erwarteter Exit, $3 = Text, den die Ausgabe nennen muss, $4 = weitere Optionen
check() {
  name=$1
  want=$2
  must=$3
  opts=$4
  total=$((total + 1))
  # shellcheck disable=SC2086  # $opts sind bewusst getrennte Optionen
  out=$(sh "$root/scripts/check-history.sh" --ohne-scanner $opts "$work/src" 2>&1)
  got=$?
  ok=1
  [ "$got" -eq "$want" ] || ok=0
  if [ -n "$must" ]; then
    printf '%s' "$out" | grep -qF "$must" || ok=0
  fi
  if printf '%s' "$out" | grep -qF "$MARK"; then
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

# Fundstück anlegen, committen, im nächsten Commit löschen: nur die Geschichte trägt es.
only_in_history() { # $1 = Datei
  git add -A -f . && git commit -q -m "Fund hinzufügen"
  git rm -q "$1" && git commit -q -m "Fund entfernen"
}

fresh
check "sauberer Verlauf" 0 "Ergebnis: ohne Fund." ""

fresh
printf '\016\040\000\000\000\000\000\000.FIT\000\000' >daten.bin
pad_zero 20 >>daten.bin
only_in_history daten.bin
check "FIT-Inhalt nur in der Geschichte" 1 "FIT-Inhalt in daten.bin" ""

fresh
printf 'SQLite format 3\000' >notizen.txt
pad_zero 40 >>notizen.txt
only_in_history notizen.txt
check "SQLite-Inhalt nur in der Geschichte" 1 "SQLite-Inhalt in notizen.txt" ""

fresh
printf 'PK\003\004' >export.dat
pad_zero 30 >>export.dat
only_in_history export.dat
check "ZIP-Inhalt nur in der Geschichte" 1 "ZIP-Inhalt in export.dat" ""

fresh
printf '\211PNG\r\n\032\n' >bild.png
pad_zero 32 >>bild.png
git add -A . && git commit -q -m "Bild"
check "Bild ist nur ein Hinweis" 0 "bild.png" ""

fresh
mk gh.txt 'gh' 'p_' 'aaaaaaaaaabbbbbbbbbbccccccccccdddddd' "$MARK"
only_in_history gh.txt
check "Token nur in der Geschichte" 1 "GitHub-Token in der Geschichte" ""

fresh
mk key.txt '-----BEGIN ' 'RSA PRIVATE KEY-----' "$MARK"
only_in_history key.txt
check "privater Schlüssel nur in der Geschichte" 1 "Privater Schlüssel (PEM) in der Geschichte" ""

fresh
mk pfad.md 'Arbeitsordner: C:' "$BS" 'Users' "$BS" 'maxmustermann' "$BS" 'Code'
only_in_history pfad.md
check "Benutzerpfad mit Namen nur in der Geschichte" 1 "Benutzerpfad mit Namen" ""

fresh
mk pfad.md 'Arbeitsordner: C:' "$BS" 'Users' "$BS" 'Name' "$BS" 'Code'
git add -A . && git commit -q -m "Platzhalter"
check "Benutzerpfad mit Platzhalter ist erlaubt" 0 "" ""

fresh
mk mail.md 'Kontakt: erika.' 'muster@mail.' 'xyz'
only_in_history mail.md
check "E-Mail-Adresse nur in der Geschichte" 1 "E-Mail-Adresse außerhalb reservierter Domains" ""
# Geprüfter Altfund: "Kurzhash Pfad" in der Liste nimmt genau diesen Fund heraus.
printf '# geprüft: synthetisches Beispiel\n%s mail.md\n' "$(git -C "$work/src" rev-parse --short=7 HEAD~1)" >"$work/allow.txt"
check "E-Mail-Adresse als geprüfter Altfund" 0 "Ergebnis: ohne Fund." "--erlaubt $work/allow.txt"

fresh
mk mail.md 'Kontakt: erika.muster@mail.' 'test'
git add -A . && git commit -q -m "Reservierte Endung"
check "E-Mail-Adresse mit reservierter Endung ist erlaubt" 0 "" ""

fresh
mk mail.md 'Kontakt: erika.muster@example.' 'org'
git add -A . && git commit -q -m "Beispieladresse"
check "E-Mail-Adresse einer reservierten Domain ist erlaubt" 0 "" ""

fresh "erika.muster@mail.test"
printf 'x\n' >neu.txt
git add -A . && git commit -q -m "Zweiter Commit"
check "Autoradresse ohne noreply" 1 "ohne noreply" ""
check "Autoradresse ohne noreply, bewusst angenommen" 0 "Ergebnis: ohne Fund." "--metadaten-ok"

echo
if [ "$failed" -ne 0 ]; then
  echo "$failed von $total Fällen weichen ab."
  exit 1
fi
echo "Alle $total Fälle wie erwartet."
