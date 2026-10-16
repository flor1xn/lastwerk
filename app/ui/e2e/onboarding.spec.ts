import { expect, test, type Page } from '@playwright/test';
import { assertClean, installGuard, type Guard } from './guard';

// Rauchtest des Erststarts: `?erststart` startet die Attrappe als frische Installation. Der
// Assistent läuft in Deutsch und Englisch bis zum Abschluss, ohne Konsolenfehler und ohne
// Anfrage an einen fremden Host. Es gibt kein Passwortfeld.

const TEXT = {
  de: {
    title: 'Sprache und Erscheinungsbild',
    takes: 'Geburtsjahr',
    garmin: 'Garmin verbinden',
    connect: 'Mit Garmin verbinden',
    synced: 'Neue Aktivitäten: 3',
    today: 'Heute',
  },
  en: {
    title: 'Language and appearance',
    takes: 'birth year',
    garmin: 'Connect Garmin',
    connect: 'Connect with Garmin',
    synced: 'New activities: 3',
    today: 'Today',
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
  test(`Erststart läuft in ${lang} bis zum Abschluss`, async ({ page }) => {
    await page.goto('/?erststart');
    await expect(page.getByTestId('wizard')).toBeVisible();
    if (lang === 'en') {
      await page.getByTestId('language-select').selectOption('en');
    }
    await expect(page.getByTestId('wizard-title')).toHaveText(TEXT[lang].title);
    await shot(page, `erststart-${lang}-1`);

    // Seit T39 nur zwei Schritte: Sprache und Garmin. Schwellen kommen aus Garmin.
    await page.getByTestId('wizard-next').click();
    await expect(page.getByTestId('wizard-step-garmin')).toBeVisible();
    await expect(page.getByTestId('wizard-title')).toContainText(TEXT[lang].garmin);
    await expect(page.getByTestId('garmin-takes')).toContainText(TEXT[lang].takes);
    // Kein Kopierbefehl mehr, kein Feld für Zugangsdaten: Die Anmeldung läuft im Fenster von
    // Garmin (in der Attrappe gelingt sie sofort, ohne Netz).
    await expect(page.getByTestId('garmin-command')).toHaveCount(0);
    expect(await page.locator('input[type="password"]').count()).toBe(0);
    await expect(page.getByTestId('garmin-risk')).toBeVisible();
    await shot(page, `erststart-${lang}-2`);
    await expect(page.getByTestId('garmin-connect-button')).toHaveText(TEXT[lang].connect);
    await page.getByTestId('garmin-connect-button').click();
    await expect(page.getByTestId('garmin-sync-state')).toContainText(TEXT[lang].synced);
    await expect(page.getByTestId('garmin-connect-button')).toHaveCount(0);
    await shot(page, `erststart-${lang}-2-verbunden`);

    await page.getByTestId('wizard-finish').click();
    await expect(page.getByTestId('nav-today')).toHaveText(TEXT[lang].today);
    await expect(page.getByTestId('wizard')).toHaveCount(0);
    await expect(page.getByTestId('sidebar-sync')).toContainText(TEXT[lang].synced);
  });
}
