import type { Page } from '@playwright/test';

// Wege durch die Navigation seit T42: drei Hauptpunkte, alles Weitere als Reiter. Sprache und
// Thema stehen nur noch in den Einstellungen, Reiter App.

/** Stellt auf Englisch um und kehrt zu Heute zurück; der Reiter der Einstellungen steht danach
 *  wieder auf Daten. */
export async function toEnglish(page: Page): Promise<void> {
  await page.getByTestId('nav-settings').click();
  await page.getByTestId('tab-settings-app').click();
  await page.getByTestId('language-select').selectOption('en');
  await page.getByTestId('tab-settings-data').click();
  await page.getByTestId('nav-today').click();
}

/** Öffnet einen Reiter des Verlaufs. */
export async function openHistoryTab(
  page: Page,
  tab: 'calendar' | 'activities' | 'progress' | 'performance' | 'body',
): Promise<void> {
  await page.getByTestId('nav-history').click();
  await page.getByTestId(`tab-history-${tab}`).click();
}

/** Öffnet einen Reiter der Einstellungen. */
export async function openSettingsTab(
  page: Page,
  tab: 'data' | 'coach' | 'profile' | 'app',
): Promise<void> {
  await page.getByTestId('nav-settings').click();
  await page.getByTestId(`tab-settings-${tab}`).click();
}
