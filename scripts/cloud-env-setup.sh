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

exit 0
