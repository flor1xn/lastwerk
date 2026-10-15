#!/usr/bin/env node
// Die Tauri-CLI sucht `tauri.conf.json` nur im aktuellen Ordner und darunter. In Lastwerk
// liegt die Oberfläche in app/ui und die Hülle in app/src-tauri (SPEC 5.2), die Abnahme ruft
// aber `pnpm --dir app/ui exec tauri ...` auf. Dieses Skript trägt die beiden Pfade ein, die
// die CLI dafür kennt (TAURI_APP_PATH: Ordner mit src-tauri, TAURI_FRONTEND_PATH: Ordner der
// Oberfläche, dort laufen auch beforeDevCommand und beforeBuildCommand), und startet dann die
// unveränderte CLI. Bereits gesetzte Werte bleiben. Gesucht wird vom aktuellen Ordner aus.

import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';

function findAppDir(start) {
  for (let dir = start; ; dir = path.dirname(dir)) {
    if (fs.existsSync(path.join(dir, 'src-tauri', 'tauri.conf.json'))) return dir;
    // Aus app/ui heraus liegt app/src-tauri als Nachbar.
    const sibling = path.join(dir, '..', 'src-tauri', 'tauri.conf.json');
    if (path.basename(dir) === 'ui' && fs.existsSync(sibling)) return path.dirname(dir);
    if (path.dirname(dir) === dir) return null;
  }
}

const appDir = findAppDir(process.cwd());
if (appDir) {
  process.env.TAURI_APP_PATH ??= appDir;
  process.env.TAURI_FRONTEND_PATH ??= path.join(appDir, 'ui');
}

const require = createRequire(import.meta.url);
const { run } = require('@tauri-apps/cli/main');
run(process.argv.slice(2), 'tauri').catch((error) => {
  console.error(error);
  process.exit(1);
});
