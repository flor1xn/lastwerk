#!/bin/sh
# Lastwerk: Prüfung des ganzen Verlaufs auf einem Spiegelklon, vor jeder Veröffentlichung (T38).
#
# check-privacy.sh prüft nur den Index. Veröffentlicht wird aber das Rohmaterial: jede Version
# jeder Datei auf jedem Zweig, in jedem Tag und in jedem Pull-Request-Verweis. Dieses Skript
# klont deshalb mit `git clone --mirror` (alle Refs, nie flach) und prüft den Klon:
#
#   1  Gitleaks über alle Refs
#   2  Trufflehog ohne Verifikation (kein Aufruf fremder Dienste mit gefundenen Werten)
#   3  Magic Bytes jedes Blobs jeder Version, unabhängig von der Endung: FIT, SQLite, Archive;
#      dazu die Zahl der Bilder und sonstigen Binärdateien und die 30 größten Blobs
#   4  Muster aus scripts/privacy-patterns.txt in allen Versionen (Schlüssel, Token)
#   5  Personenbezug in allen Versionen: Benutzerpfade mit Namen, E-Mail-Adressen außerhalb
#      reservierter Domains
#   6  Metadaten: Autor- und Committer-Adressen ohne "noreply", Sitzungsverweise
#
# Ausgabe: nur Hash, Datei und Art, nie der Fund selbst und nie eine Adresse.
# Aufruf im Wurzelverzeichnis:
#   sh scripts/check-history.sh [--metadaten-ok] [--ohne-scanner] [--erlaubt DATEI] [QUELLE]
#   QUELLE  URL oder Pfad des Repositories (Standard: origin dieses Klons)
#   --metadaten-ok  Adressen ohne noreply in Commits sind vom Nutzer bewusst angenommen
#   --ohne-scanner  Gitleaks und Trufflehog auslassen (für Tests des Skripts selbst)
#   --erlaubt DATEI geprüfte Altfunde (Standard: scripts/history-allowlist.txt): eine Zeile je
#                   Fund, "Kurzhash Pfad" (mindestens 7 Zeichen des Commits), # beginnt einen
#                   Kommentar. Gilt nur für Personenbezug in Diffs (Abschnitt 5), nie für
#                   Schlüssel, Token, FIT, SQLite oder Archive.
# Exit 0 = sauber, 1 = Fund, 2 = Aufruffehler, 3 = grün, aber ein Scanner fehlte (Teilprüfung).
#
# Werkzeuge: git, awk, od (POSIX); optional gitleaks und trufflehog (Versionen im Task T38).

metadaten_ok=0
scanner=1
source_repo=
hist_allow=scripts/history-allowlist.txt
want_allow=0
for arg in "$@"; do
  if [ "$want_allow" -eq 1 ]; then
    hist_allow=$arg
    want_allow=0
    continue
  fi
  case "$arg" in
    --metadaten-ok) metadaten_ok=1 ;;
    --ohne-scanner) scanner=0 ;;
    --erlaubt) want_allow=1 ;;
    -*) echo "Unbekannte Option: $arg" >&2; exit 2 ;;
    *) source_repo=$arg ;;
  esac
done
[ -n "$source_repo" ] || source_repo=$(git config --get remote.origin.url)
[ -n "$source_repo" ] || { echo "Keine Quelle: origin fehlt, bitte QUELLE angeben." >&2; exit 2; }
root=$(pwd)

failed=0
skipped=0
step() { printf '\n== %s\n' "$1"; }
finding() { echo "FUND: $1"; failed=1; }
has() { command -v "$1" >/dev/null 2>&1; }

tmp=$(mktemp -d 2>/dev/null || mktemp -d -t lwhistory) || exit 2
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

step "Spiegelklon (alle Branches, Tags und Verweise)"
git clone --mirror --no-local -q "$source_repo" "$tmp/mirror.git" || { echo "Klon fehlgeschlagen." >&2; exit 2; }
g() { git --git-dir="$tmp/mirror.git" -c core.quotepath=off "$@"; }
if [ "$(g rev-parse --is-shallow-repository)" != false ]; then
  finding "Der Klon ist flach, die Prüfung wäre unvollständig"
fi
g for-each-ref --format='%(refname)' | awk '
  { n = split($0, p, "/"); k = p[2]; if (k == "") k = "sonstige"; c[k]++ }
  END { for (k in c) printf "  refs/%s: %d\n", k, c[k] }' | sort
echo "  Commits über alle Refs: $(g rev-list --all --count)"
if [ "$(g for-each-ref --format='%(refname)' 'refs/pull/*' | wc -l)" -eq 0 ]; then
  echo "  Hinweis: keine refs/pull/*; verbirgt der Server sie, sind PR-Stände nicht geprüft."
fi

step "Gitleaks (alle Refs)"
if [ "$scanner" -eq 0 ]; then
  echo "ausgelassen (--ohne-scanner)"
elif has gitleaks; then
  gitleaks git --redact --no-banner --log-opts="--all" "$tmp/mirror.git" >"$tmp/gitleaks.log" 2>&1
  code=$?
  tail -n 3 "$tmp/gitleaks.log" | sed 's/^/  /'
  [ "$code" -eq 0 ] || finding "gitleaks meldet Treffer (Exit $code); Bericht lokal mit --report-path wiederholen"
else
  echo "ÜBERSPRUNGEN: gitleaks nicht installiert"
  skipped=1
fi

step "Trufflehog (ohne Verifikation)"
if [ "$scanner" -eq 0 ]; then
  echo "ausgelassen (--ohne-scanner)"
elif has trufflehog; then
  # Trufflehog will ein Arbeitsverzeichnis mit .git: Kopie des Spiegels ohne Dateien.
  mkdir "$tmp/th" && cp -R "$tmp/mirror.git" "$tmp/th/.git" || exit 2
  git -C "$tmp/th" config core.bare false
  git -C "$tmp/th" read-tree --empty
  # Unter Git Bash (Windows) kennt das native trufflehog den MSYS-Pfad /tmp/... nicht.
  th_path=$tmp/th
  if command -v cygpath >/dev/null 2>&1; then th_path=$(cygpath -m "$th_path"); fi
  trufflehog git "file://$th_path" --no-verification --no-update --fail \
    --results=verified,unverified,unknown >"$tmp/trufflehog.log" 2>&1
  code=$?
  # Die Zusammenfassung steht je nach Ausgabeform als JSON oder als Text in der Zeile.
  grep 'finished scanning' "$tmp/trufflehog.log" | sed 's/^.*finished scanning[^{]*//' | cut -c1-120 | sed 's/^/  /'
  # Mit --fail heißt Exit 183 Treffer; jeder andere Code ist ein Fehler des Laufs.
  if [ "$code" -eq 183 ]; then
    finding "trufflehog meldet Treffer (Exit $code)"
  elif [ "$code" -ne 0 ]; then
    grep -i 'error' "$tmp/trufflehog.log" | head -n 3 | cut -c1-200 | sed 's/^/  /'
    finding "trufflehog lief nicht durch (Exit $code)"
  fi
else
  echo "ÜBERSPRUNGEN: trufflehog nicht installiert"
  skipped=1
fi

step "Magic Bytes aller Blobs aller Versionen"
allow=$root/scripts/privacy-allowlist.txt
g rev-list --objects --all |
  g cat-file --batch-check='%(objecttype) %(objectname) %(objectsize) %(rest)' |
  awk '$1 == "blob"' >"$tmp/blobs.txt"
echo "  Blobs: $(wc -l <"$tmp/blobs.txt")"
: >"$tmp/kinds.txt"
while read -r _ hash size path; do
  [ "$size" -ge 4 ] || continue
  hex=$(g cat-file blob "$hash" | head -c 16 | od -An -tx1 | tr -d ' \n')
  kind=
  case "$hex" in
    ????????????????2e464954*) kind=FIT ;;
    53514c69746520666f726d61742033*) kind=SQLite ;;
    504b0304* | 504b0506*) kind=ZIP ;;
    1f8b08*) kind=gzip ;;
    377abcaf271c*) kind=7z ;;
    89504e47*) kind=Bild ;;
    ffd8ff*) kind=Bild ;;
    47494638*) kind=Bild ;;
    0000010000*) kind=Bild ;;
    69636e73*) kind=Bild ;;
  esac
  if [ -z "$kind" ]; then
    # Unbekannte Binärdatei? NUL in den ersten 8000 Byte, wie Git es erkennt.
    if g cat-file blob "$hash" | head -c 8000 | od -An -tx1 | grep -q ' 00'; then
      kind=binaer-sonstige
    else
      continue
    fi
  fi
  printf '%s %s %s %s\n' "$kind" "$(printf '%.10s' "$hash")" "$size" "$path" >>"$tmp/kinds.txt"
done <"$tmp/blobs.txt"
awk '{ c[$1]++ } END { for (k in c) printf "  %s: %d Blobs\n", k, c[k] }' "$tmp/kinds.txt" | sort
while read -r kind hash size path; do
  case "$kind" in
    FIT | SQLite)
      finding "$kind-Inhalt in $path (Blob $hash, $size Byte)"
      ;;
    ZIP | gzip | 7z)
      if [ -f "$allow" ] && grep -v '^[[:space:]]*#' "$allow" | grep -qxF "$path"; then
        continue
      fi
      finding "$kind-Inhalt in $path (Blob $hash, $size Byte)"
      ;;
  esac
done <"$tmp/kinds.txt"
echo "  Binäre Pfade (Bilder und sonstige):"
awk '$1 == "Bild" || $1 == "binaer-sonstige" { $1 = $2 = $3 = ""; print }' "$tmp/kinds.txt" | sort -u | sed 's/^ */    /'
echo "  30 größte Blobs (Byte, Pfad):"
sort -k3,3 -n -r "$tmp/blobs.txt" | head -n 30 | awk '{ s = $3; $1 = $2 = $3 = ""; printf "    %9d %s\n", s, $0 }' | sed 's/  */ /2'

step "Schlüssel- und Token-Muster in allen Versionen"
patterns=$root/scripts/privacy-patterns.txt
tab=$(printf '\t')
if [ ! -f "$patterns" ]; then
  finding "$patterns fehlt"
else
  while IFS="$tab" read -r kind pattern; do
    case "$kind" in '' | '#'*) continue ;; esac
    # -G sucht Zeilen, die in einem Diff vorkommen; ohne --pickaxe-all nennt --name-only nur die
    # Dateien, deren Diff trifft. Die Zeile selbst wird nie ausgegeben.
    hits=$(g log --all -m -G"$pattern" --format='@%h' --name-only | awk '
      /^@/ { h = substr($0, 2); next } NF { print h " " $0 }' | sort -u)
    if [ -n "$hits" ]; then
      finding "$kind in der Geschichte:"
      printf '%s\n' "$hits" | head -n 20 | sed 's/^/    /'
    fi
  done <"$patterns"
fi

step "Personenbezug in allen Versionen (Benutzerpfade, E-Mail-Adressen)"
# Platzhalter und reservierte Domains, die keine Person bezeichnen. Gefunden wird in
# hinzugefügten Zeilen aller Diffs, ausgegeben werden nur Commit, Datei und Art.
g log --all -m -p -U0 --no-color --format='@@COMMIT %h' |
  awk '
  BEGIN {
    n = split("name x u user username geheim secret default public runneradmin usr home", ph, " ")
    for (i = 1; i <= n; i++) okname[tolower(ph[i])] = 1
    m = split("example.com example.org example.net example.invalid example.co.uk invalid localhost evil.example garmin.com acme.com b.co noreply.github.com users.noreply.github.com anthropic.com", dom, " ")
    for (i = 1; i <= m; i++) okdom[dom[i]] = 1
  }
  /^@@COMMIT / { commit = $2; next }
  /^diff --git / { file = $NF; sub(/^b\//, "", file); next }
  /^\+\+\+/ { next }
  /^\+/ {
    line = substr($0, 2)   # ohne das Diff-Zeichen, sonst gälte der Dekorator in Zeile 1 als Postfach
    # Benutzerpfade: C:\Users\NAME, /Users/NAME, /home/NAME
    rest = line
    while (match(rest, /([Uu]sers\\+|\/Users\/|\/home\/)[A-Za-z0-9._<>-]+/)) {
      hit = substr(rest, RSTART, RLENGTH)
      rest = substr(rest, RSTART + RLENGTH)
      name = hit; sub(/^([Uu]sers\\+|\/Users\/|\/home\/)/, "", name)
      if (!(tolower(name) in okname) && name !~ /^</) found["Benutzerpfad mit Namen " commit " " file] = 1
    }
    # E-Mail-Adressen
    rest = line
    while (match(rest, /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z][A-Za-z]+/)) {
      hit = substr(rest, RSTART, RLENGTH)
      rest = substr(rest, RSTART + RLENGTH)
      d = hit; sub(/^[^@]*@/, "", d); d = tolower(d)
      ok = 0
      for (k in okdom) { if (d == k || d ~ ("\\." k "$") || d ~ /(^|\.)example(\.|$)/) ok = 1 }
      if (d ~ /\.(png|jpg|svg|gif|ico)$/) ok = 1   # "name@2x.png" ist kein Postfach
      if (d ~ /\.(test|invalid|example|localhost)$/) ok = 1   # reservierte Endungen (RFC 2606)
      if (!ok) found["E-Mail-Adresse außerhalb reservierter Domains " commit " " file] = 1
    }
  }
  END { for (k in found) print k }' | sort >"$tmp/personal.txt.all"
# Geprüfte Altfunde herausnehmen: Zeilen der Form "Art ... Kurzhash Pfad" gegen die Liste.
: >"$tmp/hist-allow.txt"
if [ -f "$root/$hist_allow" ]; then
  grep -v '^[[:space:]]*#' "$root/$hist_allow" | awk 'NF >= 2 { print substr($1, 1, 7) " " $2 }' >"$tmp/hist-allow.txt"
elif [ -f "$hist_allow" ]; then
  grep -v '^[[:space:]]*#' "$hist_allow" | awk 'NF >= 2 { print substr($1, 1, 7) " " $2 }' >"$tmp/hist-allow.txt"
fi
awk 'FILENAME == ARGV[1] { ok[$0] = 1; next } { n = split($0, f, " "); key = substr(f[n-1], 1, 7) " " f[n]; if (!(key in ok)) print }' \
  "$tmp/hist-allow.txt" "$tmp/personal.txt.all" >"$tmp/personal.txt"
if [ -s "$tmp/personal.txt" ]; then
  finding "Personenbezug in der Geschichte ($(wc -l <"$tmp/personal.txt") Fundstellen, Art / Commit / Datei):"
  head -n 25 "$tmp/personal.txt" | sed 's/^/    /'
else
  echo "  keine Fundstellen"
fi

step "Metadaten der Commits (Adressen werden nur gezählt)"
total=$(g rev-list --all --count)
g log --all --format='%ae%n%ce' | sort | uniq -c | awk '
  { addr = $2; n = $1; sub(/^ */, "", addr); all += n; if (addr !~ /noreply/) real += n }
  END { printf "%d von %d Adressangaben (Autor und Committer) ohne \"noreply\"\n", real, all }' >"$tmp/meta.txt"
sed 's/^/  /' "$tmp/meta.txt"
real=$(awk '{ print $1 }' "$tmp/meta.txt")
echo "  Commits mit Sitzungsverweis (claude.ai/code/session_...): $(g log --all --format=%B | grep -c 'claude\.ai/code/session_')"
echo "  Verschiedene Autorennamen: $(g log --all --format=%an | sort -u | wc -l)"
if [ "$real" -gt 0 ] && [ "$metadaten_ok" -eq 0 ]; then
  finding "Commits tragen Adressen ohne noreply ($real Angaben); bewusste Annahme mit --metadaten-ok"
fi
echo "  (Gesamtzahl Commits: $total)"

printf '\n'
if [ "$failed" -ne 0 ]; then
  echo "Ergebnis: FUND. Nicht veröffentlichen, bevor jeder Fund eingeordnet ist."
  exit 1
fi
if [ "$skipped" -ne 0 ]; then
  echo "Ergebnis: ohne Fund, aber unvollständig (siehe ÜBERSPRUNGEN)."
  exit 3
fi
echo "Ergebnis: ohne Fund."
