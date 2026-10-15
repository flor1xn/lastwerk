import { expect, test, type Page } from '@playwright/test';
import { assertClean, installGuard, type Guard } from './guard';

// Rauchtest der Einrichtung des lokalen Coachs (T29) mit der Attrappe, in Deutsch und Englisch:
// Ollama fehlt, "Jetzt einrichten", Modell laden mit Fortschritt, Abschnitt in den Einstellungen,
// Hinweis in der Planansicht. Kein Netz, kein fremder Host (Guard), kein Eingabefeld.

const TEXT = {
  de: {
    missing: 'Ollama ist nicht installiert',
    running: 'Ollama läuft',
    install: 'Jetzt einrichten',
    recommended: 'Empfohlen für deinen PC',
    slow: 'langsam auf diesem PC',
    tooBig: 'zu groß für diesen PC',
    noModel: 'Noch kein Modell geladen',
    pulled: 'qwen3:4b ist geladen',
    background: 'Du kannst weitermachen',
    openSettings: 'Zur Einrichtung in den Einstellungen',
    sectionTitle: 'Lokaler Coach',
    ready: 'Der Coach ist bereit.',
  },
  en: {
    missing: 'Ollama is not installed',
    running: 'Ollama is running',
    install: 'Set up now',
    recommended: 'Recommended for your PC',
    slow: 'slow on this PC',
    tooBig: 'too big for this PC',
    noModel: 'No model loaded yet',
    pulled: 'qwen3:4b is loaded',
    background: 'You can carry on',
    openSettings: 'Go to the setup in the settings',
    sectionTitle: 'Local coach',
    ready: 'The coach is ready.',
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
  test(`Coach einrichten in den Einstellungen (${lang})`, async ({ page }) => {
    await page.goto('/?ollama=fehlt');
    if (lang === 'en') await page.getByTestId('header-language').selectOption('en');
    await page.getByTestId('nav-settings').click();

    const section = page.getByTestId('coach-setup-section');
    await expect(section.locator('h3')).toHaveText(TEXT[lang].sectionTitle);
    await expect(section.getByTestId('coach-setup-state')).toHaveText(TEXT[lang].missing);
    await expect(section.getByTestId('coach-setup-install-button')).toHaveText(TEXT[lang].install);
    await expect(section.getByTestId('coach-model-qwen3:4b')).toContainText(TEXT[lang].recommended);
    await expect(section.getByTestId('coach-model-qwen3:8b')).toContainText(TEXT[lang].slow);
    await expect(section.getByTestId('coach-model-qwen3:14b')).toContainText(TEXT[lang].tooBig);
    expect(await section.locator('input').count()).toBe(0);
    await section.scrollIntoViewIfNeeded();
    await shot(page, `coach-setup-${lang}-1-fehlt`);

    await section.getByTestId('coach-setup-install-button').click();
    await expect(section.getByTestId('coach-setup-installing')).toBeVisible();
    await expect(section.getByTestId('coach-setup-state')).toHaveText(TEXT[lang].running);
    await expect(section.getByTestId('coach-setup-model')).toHaveText(TEXT[lang].noModel);
    await shot(page, `coach-setup-${lang}-2-laeuft`);

    // Das empfohlene Modell laden: Balken mit Zahlen und Hinweis auf den Hintergrund.
    await section.getByTestId('coach-load-qwen3:4b').click();
    const progress = section.getByTestId('coach-pull-progress');
    await expect(progress).toBeVisible();
    await expect(progress).toContainText(TEXT[lang].background);
    await expect(progress.getByRole('progressbar')).toBeVisible();
    await shot(page, `coach-setup-${lang}-3-laedt`);
    await expect(section.getByTestId('coach-setup-pulled')).toContainText(TEXT[lang].pulled, {
      timeout: 20_000,
    });
    await shot(page, `coach-setup-${lang}-4-fertig`);

    // Im Plan antwortet der Coach jetzt: kein Hinweis zur Einrichtung mehr.
    await page.getByTestId('nav-plan').click();
    await expect(page.getByTestId('coach-open-setup')).toHaveCount(0);
  });

  test(`Hinweis in der Planansicht führt zur Einrichtung (${lang})`, async ({ page }) => {
    await page.goto('/?ollama=fehlt');
    if (lang === 'en') await page.getByTestId('header-language').selectOption('en');
    await page.getByTestId('nav-plan').click();
    const link = page.getByTestId('coach-open-setup');
    await expect(link).toHaveText(TEXT[lang].openSettings);
    await link.click();
    await expect(page.getByTestId('coach-setup-section')).toBeVisible();
    await expect(page.getByTestId('coach-setup-install-button')).toHaveText(TEXT[lang].install);
  });

  test(`Zustand "lädt gerade" zeigt den Fortschritt (${lang})`, async ({ page }) => {
    await page.goto('/?ollama=laedt');
    if (lang === 'en') await page.getByTestId('header-language').selectOption('en');
    await page.getByTestId('nav-settings').click();
    const progress = page.getByTestId('coach-pull-progress');
    await expect(progress).toBeVisible();
    await expect(progress).toContainText(TEXT[lang].background);
    await expect(page.getByTestId('coach-pull-cancel')).toBeVisible();
    await shot(page, `coach-setup-${lang}-laedt-variante`);
  });

  test(`Ollama läuft schon (${lang})`, async ({ page }) => {
    await page.goto('/?ollama=laeuft');
    if (lang === 'en') await page.getByTestId('header-language').selectOption('en');
    await page.getByTestId('nav-settings').click();
    await expect(page.getByTestId('coach-setup-state')).toHaveText(TEXT[lang].running);
    await expect(page.getByTestId('coach-setup-ready')).toHaveText(TEXT[lang].ready);
    await expect(page.getByTestId('coach-setup-install-button')).toHaveCount(0);
  });
}
