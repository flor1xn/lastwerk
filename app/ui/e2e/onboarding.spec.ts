import { expect, test, type Page } from '@playwright/test';
import { assertClean, installGuard, type Guard } from './guard';

// Rauchtest der Einrichtung (T41): `?erststart` startet die Attrappe als frische Installation.
// Die Sprache kommt vom System. Der Assistent läuft in Deutsch und Englisch von Garmin über
// Freigaben und Coach bis zum fertigen Plan, ohne Konsolenfehler und ohne Anfrage an einen
// fremden Host. Es gibt kein Passwortfeld.

const TEXT = {
  de: {
    browser: 'de-DE',
    garmin: 'Mit Garmin verbinden',
    takes: 'Geburtsjahr',
    synced: 'Neue Aktivitäten: 3',
    extras: 'Während Garmin lädt',
    plan: 'Dein Plan',
    history: '12 Wochen',
    today: 'Heute',
  },
  en: {
    browser: 'en-US',
    garmin: 'Connect with Garmin',
    takes: 'birth year',
    synced: 'New activities: 3',
    extras: 'While Garmin loads',
    plan: 'Your plan',
    history: '12 weeks',
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
  test.describe(`Einrichtung in ${lang}`, () => {
    test.use({ locale: TEXT[lang].browser });

    test('läuft von Garmin bis zum fertigen Plan', async ({ page }) => {
      await page.goto('/?erststart');
      await expect(page.getByTestId('wizard')).toBeVisible();
      await expect(page.getByTestId('wizard-title')).toHaveText(TEXT[lang].garmin);
      await expect(page.getByTestId('garmin-takes')).toContainText(TEXT[lang].takes);
      await expect(page.getByTestId('garmin-command')).toHaveCount(0);
      expect(await page.locator('input[type="password"]').count()).toBe(0);
      await expect(page.getByTestId('garmin-risk')).toBeVisible();
      await shot(page, `einrichtung-${lang}-1`);
      await page.getByTestId('garmin-connect-button').click();
      await expect(page.getByTestId('garmin-sync-state')).toContainText(TEXT[lang].synced);
      await shot(page, `einrichtung-${lang}-1-verbunden`);

      await page.getByTestId('wizard-next').click();
      await expect(page.getByTestId('wizard-title')).toHaveText(TEXT[lang].extras);
      await expect(page.getByTestId('garmin-share-body')).not.toBeChecked();
      await shot(page, `einrichtung-${lang}-2`);

      await page.getByTestId('wizard-next').click();
      await expect(page.getByTestId('wizard-title')).toHaveText(TEXT[lang].plan);
      await expect(page.getByTestId('wizard-plan-history')).toContainText(TEXT[lang].history);
      await shot(page, `einrichtung-${lang}-3`);
      await page.getByTestId('planner-submit').click();

      await expect(page.getByTestId('nav-today')).toHaveText(TEXT[lang].today);
      await expect(page.getByTestId('wizard')).toHaveCount(0);
      await expect(page.getByTestId('today-decision')).toBeVisible();
      await shot(page, `einrichtung-${lang}-heute`);
    });
  });
}
