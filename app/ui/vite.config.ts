import { readFileSync } from 'node:fs';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { svelteTesting } from '@testing-library/svelte/vite';
import { defineConfig } from 'vitest/config';

// Die Content-Security-Policy steht an einer Stelle: in der Tauri-Konfiguration. Die
// Vorschau für den Playwright-Rauchtest sendet sie als Header, damit jede Verletzung (Inline-
// Skript, fremde Quelle) als Konsolenfehler auffällt und den Test fehlschlagen lässt.
const tauriConf = JSON.parse(
  readFileSync(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8'),
) as { app: { security: { csp: string } } };

export default defineConfig({
  plugins: [svelte(), svelteTesting()],
  // Feste Ports, weil Tauri (devUrl) und Playwright sie kennen müssen.
  clearScreen: false,
  server: { port: 5173, strictPort: true, host: '127.0.0.1' },
  preview: {
    port: 4173,
    strictPort: true,
    host: '127.0.0.1',
    headers: { 'Content-Security-Policy': tauriConf.app.security.csp },
  },
  build: { target: 'es2022', sourcemap: false },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.ts'],
    setupFiles: ['src/test-setup.ts'],
  },
});
