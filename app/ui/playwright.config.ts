import { defineConfig } from '@playwright/test';

// Rauchtest der Oberfläche gegen den gebauten Stand (vite preview, mit der CSP aus der
// Tauri-Konfiguration als Header). Browser: In der CI installiert `playwright install`
// Chromium, in der Cloud-Umgebung liegt er unter PLAYWRIGHT_BROWSERS_PATH. Mit der Variable
// LASTWERK_CHROMIUM lässt sich ein anderes Programm erzwingen.
const executablePath = process.env.LASTWERK_CHROMIUM || undefined;

export default defineConfig({
  testDir: 'e2e',
  // Ergebnisse und Screenshots: nicht einchecken (siehe .gitignore).
  outputDir: 'test-results',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [['list']],
  use: {
    baseURL: 'http://127.0.0.1:4173',
    viewport: { width: 1280, height: 800 },
    colorScheme: 'dark',
    locale: 'de-DE',
    timezoneId: 'Europe/Berlin',
    launchOptions: executablePath ? { executablePath } : {},
  },
  webServer: {
    command: 'pnpm run build && pnpm exec vite preview',
    url: 'http://127.0.0.1:4173',
    reuseExistingServer: !process.env.CI,
    timeout: 180_000,
  },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
});
