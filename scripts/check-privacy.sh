#!/bin/sh
# Lastwerk: Datenschutzprüfung für den Git-Index (SPEC 1.5, 11.4, 11.6).
# Läuft ohne Toolchain, lokal und in der CI (Linux und Windows-Git-Bash). Exit 0 = sauber,
# 1 = Verstoß.
#
# Geprüft wird der Index (`git ls-files`, `git grep --cached`), nicht der Arbeitsbaum: Was
# `git add` aufgenommen hat, gilt, auch vor dem Commit.
#   1 bis 5  Dateinamen (FIT, local/, Zugangsdateien, Datenbanken, .gitignore)
#   6        Inhalte nach Magic Bytes, unabhängig von der Endung: FIT-Datei (".FIT" ab Byte 8),
#            SQLite-Datenbank, Archive (ZIP, gzip, 7z)
#   7        Inhalte nach Muster (scripts/privacy-patterns.txt): private Schlüssel, JWT, Token von
#            GitHub, Google OAuth, Slack, AWS und Anthropic, Signaturschlüssel des Updaters
#            (rsign, geheimer Teil), Token-Felder mit Wert
# Treffer nennen nur Datei und Art, nie den Fund selbst, damit ein Token nicht im CI-Protokoll
# landet. Ausnahmen, die ein Mensch geprüft hat (zum Beispiel ein synthetisches Archiv als
# Testdatei), stehen als Pfad pro Zeile in scripts/privacy-allowlist.txt.
#
# Die Muster sind so geschrieben, dass keine Zeile im Repository sie selbst trifft.
# Tests: sh scripts/test-check-privacy.sh (erzeugt die Fundstücke zur Laufzeit, checkt nichts ein).

fail=0

report() {
  echo "VERSTOSS: $1" >&2
  fail=1
}

# Pfade ohne Anführungszeichen-Umschreibung (Umlaute), sonst passen die Listen nicht zusammen.
git_() { git -c core.quotepath=off "$@"; }

tracked=$(git_ ls-files)

# 1. Keine FIT-Dateien im Index.
fit=$(printf '%s\n' "$tracked" | grep -E '\.(fit|FIT)$')
[ -n "$fit" ] && report "FIT-Dateien im Index:
$fit"

# 2. Unter local/ darf nur README.md im Index stehen.
loc=$(printf '%s\n' "$tracked" | grep -E '^local/' | grep -v '^local/README\.md$')
[ -n "$loc" ] && report "Dateien unter local/ im Index:
$loc"

# 3. Keine Zugangsdaten.
cred=$(printf '%s\n' "$tracked" | grep -E '(^|/)(garmin_tokens\.json|\.env|\.env\..+)$')
[ -n "$cred" ] && report "Zugangsdaten im Index:
$cred"

# 4. Keine Datenbankdateien.
db=$(printf '%s\n' "$tracked" | grep -E '\.(db|db-wal|db-shm|sqlite)$')
[ -n "$db" ] && report "Datenbankdateien im Index:
$db"

# 5. .gitignore muss local/ weiterhin ausschließen (Abnahme M0).
if ! git check-ignore -q local/test.fit; then
  report "local/test.fit wird von .gitignore nicht ausgeschlossen"
fi

# Geprüfte Ausnahmen (Pfad pro Zeile, # beginnt einen Kommentar).
allow=scripts/privacy-allowlist.txt
allowed() {
  [ -f "$allow" ] && grep -v '^[[:space:]]*#' "$allow" | grep -qxF "$1"
}

tmp=$(mktemp -d 2>/dev/null || mktemp -d -t lwprivacy) || exit 2
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

# 6. Magic Bytes. Nur Binärdateien kommen infrage: FIT und SQLite enthalten NUL-Bytes in den
# ersten Bytes, Git erkennt sie daran als binär. Die Textdateien liefert `git grep -I`; was
# übrig bleibt (auch leere Dateien), wird an den ersten 16 Bytes geprüft.
printf '%s\n' "$tracked" | LC_ALL=C sort -u >"$tmp/all"
git_ grep -I --cached --name-only -e '' | LC_ALL=C sort -u >"$tmp/text"
LC_ALL=C comm -23 "$tmp/all" "$tmp/text" >"$tmp/binary"
while IFS= read -r f; do
  [ -n "$f" ] || continue
  hex=$(git cat-file blob ":$f" 2>/dev/null | head -c 16 | od -An -tx1 | tr -d ' \n')
  case "$hex" in
    ????????????????2e464954*) report "FIT-Inhalt (Magic Bytes) in: $f" ;;
    53514c69746520666f726d6174203300* | 53514c69746520666f726d61742033*)
      report "SQLite-Datenbank (Magic Bytes) in: $f" ;;
    504b0304* | 504b0506* | 1f8b08* | 377abcaf271c*)
      # Nur Archive können auf die Ausnahmeliste; FIT und SQLite nie.
      allowed "$f" || report "Archiv (ZIP, gzip oder 7z) in: $f; geprüfte Ausnahmen in $allow" ;;
  esac
done <"$tmp/binary"

# 7. Muster in Textdateien (Index). Die Pfade der Ausnahmeliste werden aus der Suche genommen.
set -- .
if [ -f "$allow" ]; then
  while IFS= read -r p; do
    case "$p" in '' | '#'*) continue ;; esac
    set -- "$@" ":(exclude)$p"
  done <"$allow"
fi

# $1 = Art des Fundes, $2 = Muster (erweiterter regulärer Ausdruck), Rest = Pfadangaben.
scan_pattern() {
  kind=$1
  pattern=$2
  shift 2
  hits=$(git_ grep --cached -I -l -E -e "$pattern" -- "$@")
  [ -n "$hits" ] && report "$kind in:
$hits"
}

# Muster und Arten stehen in scripts/privacy-patterns.txt (gemeinsam mit check-history.sh).
patterns=scripts/privacy-patterns.txt
if [ ! -f "$patterns" ]; then
  report "$patterns fehlt"
else
  tab=$(printf '\t')
  while IFS="$tab" read -r kind pattern; do
    case "$kind" in '' | '#'*) continue ;; esac
    scan_pattern "$kind" "$pattern" "$@"
  done <"$patterns"
fi

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "Datenschutzprüfung: ok"
