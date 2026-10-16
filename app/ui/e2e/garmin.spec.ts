import { expect, test, type Page } from '@playwright/test';
import { assertClean, installGuard, type Guard } from './guard';
import { toEnglish } from './nav';

// Rauchtest des Abschnitts Garmin in den Einstellungen (T23) mit der Attrappe, in Deutsch und
// Englisch: verbinden, abgleichen, Zustand in der Seitenleiste, Trennen mit Bestätigung. Kein
// Netz, kein fremder Host (Guard), kein Feld für Zugangsdaten.

const TEXT = {
  de: {
    settings: 'Einstellungen',
    notConnected: 'Nicht verbunden',
    connected: 'Verbunden',
    synced: 'Neue Aktivitäten: 3',
    lastSync: 'Letzter Abgleich',
    disconnected: 'Getrennt',
  },
  en: {
    settings: 'Settings',
    notConnected: 'Not connected',
    connected: 'Connected',
    synced: 'New activities: 3',
    lastSync: 'Last sync',
    disconnected: 'Disconnected',
  },
} as const;

let guard: Guard;

test.beforeEach(async ({ page }) => {
  guard = await installGuard(page);
});

test.afterEach(() => {
  assertClean(guard);
});

async function shot(page: Page, name: string) {
  await page.screenshot({ path: `test-results/screens/${name}.png`, fullPage: true });
}

for (const lang of ['de', 'en'] as const) {
  test(`Garmin in den Einstellungen (${lang})`, async ({ page }) => {
    await page.goto('/');
    if (lang === 'en') await toEnglish(page);
    await page.getByTestId('nav-settings').click();
    await expect(page.getByTestId('nav-settings')).toHaveText(TEXT[lang].settings);
    // Garmin steht im ersten Reiter der Einstellungen (T42).
    await expect(page.getByTestId('tab-settings-data')).toHaveAttribute('aria-selected', 'true');

    const section = page.getByTestId('garmin-section');
    await expect(section.getByTestId('garmin-state')).toHaveText(TEXT[lang].notConnected);
    await expect(section.getByTestId('garmin-risk')).toBeVisible();
    expect(await page.locator('input[type="password"]').count()).toBe(0);
    await section.scrollIntoViewIfNeeded();
    await shot(page, `garmin-${lang}-1`);

    await section.getByTestId('garmin-connect-button').click();
    await expect(section.getByTestId('garmin-state')).toHaveText(TEXT[lang].connected);
    await expect(section.getByTestId('garmin-sync-state')).toContainText(TEXT[lang].synced);
    await expect(page.getByTestId('sidebar-sync')).toContainText(TEXT[lang].synced);
    await expect(section.getByTestId('garmin-last-sync')).toContainText(TEXT[lang].lastSync);
    await shot(page, `garmin-${lang}-2-verbunden`);

    // "Jetzt abgleichen" läuft erneut, ohne neue Aktivitäten.
    await section.getByTestId('garmin-sync-now').click();
    await expect(section.getByTestId('garmin-sync-state')).toContainText(': 0');

    // Erweitert: Status des Zusatzprogramms erst nach dem Aufklappen.
    await section.getByTestId('garmin-advanced').locator('summary').click();
    await expect(section.getByTestId('bridge-status')).toBeVisible();

    await section.getByTestId('garmin-disconnect').click();
    await expect(section.getByTestId('garmin-disconnect-confirm')).toBeVisible();
    await section.getByTestId('garmin-disconnect-yes').click();
    await expect(section.getByTestId('garmin-disconnected')).toContainText(TEXT[lang].disconnected);
    await expect(section.getByTestId('garmin-state')).toHaveText(TEXT[lang].notConnected);
  });
}
