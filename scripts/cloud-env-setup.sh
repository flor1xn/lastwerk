#!/bin/bash
# Lastwerk: Setup-Skript für die Cloud-Umgebung (Claude Code im Web).
#
# Verwendung: Inhalt dieser Datei einmal in das Feld "Setup script" der Cloud-Umgebung
# auf claude.ai/code einfügen (Anleitung: docs/remote-sessions.md). Das Skript läuft als
# root auf Ubuntu 24.04 und wird von Anthropic zwischengespeichert, wenn es unter etwa
# fünf Minuten bleibt. Es muss mit Exit-Code 0 enden, sonst startet die Session nicht.
# Deshalb steht hinter jedem nicht kritischen Schritt "|| true".

export DEBIAN_FRONTEND=noninteractive

# 1. Systembibliotheken für die Tauri-Shell (ab Task T06) und Linker.
(
  apt-get update -qq &&
    apt-get install -y -qq --no-install-recommends \
      build-essential pkg-config libssl-dev xdg-utils libdbus-1-dev \
      libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
      libayatana-appindicator3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev
) || echo "Hinweis: apt-Installation fehlgeschlagen, Tauri-Build wird in dieser Umgebung nicht gehen"

# 2. Rust: stabile Toolchain mit rustfmt und clippy (SPEC 11.6 verlangt beide).
if command -v rustup >/dev/null 2>&1; then
  rustup toolchain install stable --profile minimal -c rustfmt -c clippy || true
  rustup default stable || true
else
  echo "Hinweis: rustup nicht gefunden, rustfmt und clippy nicht nachinstalliert"
fi

# 3. Python 3.12 für das Zusatzprogramm in bridge/ (SPEC 1.1, O21).
if command -v uv >/dev/null 2>&1; then
  uv python install 3.12 || true
fi

# 4. Prüfwerkzeuge der CI, damit scripts/check-all.sh hier vollständig läuft (Exit 0, nicht 3).
# Bis T46 fehlten sie in der Cloud; Sessions pushten nach einem Teillauf, gitleaks und die
# Sicherheitshinweise fielen erst in der CI auf. Versionen wie in .github/workflows, fertige
# Programme statt Übersetzung (Zeitbudget des Skripts), Prüfsummen aus den Releases der
# Projekte, am 18. Oktober 2026 nachgerechnet.
install_tool() { # Name, URL, SHA-256, Pfad im Archiv
  tmp=$(mktemp -d) &&
    curl -fsSL "$2" -o "$tmp/a.tar.gz" &&
    echo "$3  $tmp/a.tar.gz" | sha256sum -c --quiet &&
    tar -xzf "$tmp/a.tar.gz" -C "$tmp" "$4" &&
    install -m 0755 "$tmp/$4" "/usr/local/bin/$1"
  rc=$?
  rm -rf "$tmp"
  [ "$rc" = 0 ] || echo "Hinweis: $1 nicht installiert, check-all.sh überspringt den Schritt"
}
install_tool cargo-deny \
  https://github.com/EmbarkStudios/cargo-deny/releases/download/0.20.2/cargo-deny-0.20.2-x86_64-unknown-linux-musl.tar.gz \
  9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f \
  cargo-deny-0.20.2-x86_64-unknown-linux-musl/cargo-deny
install_tool actionlint \
  https://github.com/rhysd/actionlint/releases/download/v1.7.7/actionlint_1.7.7_linux_amd64.tar.gz \
  023070a287cd8cccd71515fedc843f1985bf96c436b7effaecce67290e7e0757 \
  actionlint
install_tool gitleaks \
  https://github.com/gitleaks/gitleaks/releases/download/v8.18.4/gitleaks_8.18.4_linux_x64.tar.gz \
  ba6dbb656933921c775ee5a2d1c13a91046e7952e9d919f9bac4cec61d628e7d \
  gitleaks
if command -v uv >/dev/null 2>&1; then
  UV_TOOL_BIN_DIR=/usr/local/bin uv tool install zizmor==1.31.0 >/dev/null 2>&1 ||
    echo "Hinweis: zizmor nicht installiert"
fi

exit 0
