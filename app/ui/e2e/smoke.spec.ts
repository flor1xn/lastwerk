import { expect, test, type Page } from '@playwright/test';
import { assertClean, installGuard, isAllowedUrl, type Guard } from './guard';
import { openHistoryTab, openSettingsTab, toEnglish } from './nav';

// Rauchtest (SPEC 12, M2): alle Ansichten und Reiter in Deutsch und Englisch, kein Konsolenfehler, keine
// Anfrage an einen fremden Host. Daten liefert die Attrappe (kein Tauri im Browser).

const TEXT = {
  de: {
    nav: {
      today: 'Heute',
      plan: 'Plan',
      history: 'Verlauf',
      settings: 'Einstellungen',
    },
    historyTabs: ['Kalender', 'Aktivitäten', 'Fortschritt', 'Leistung', 'Körper'],
    settingsTabs: ['Daten und Garmin', 'Coach', 'Profil und Schwellen', 'App'],
    dash: {
      instruments: [
        ['today-form', 'Form'],
        ['today-readiness', 'Erholung zu deinem Schnitt'],
        ['today-week-story', 'Diese Woche'],
      ],
      progress: [
        ['dash-form', 'Form-Verlauf'],
        ['dash-ramp', 'Rampe'],
        ['dash-distribution', 'Verteilung'],
      ],
      insights: 'Coach-Erkenntnisse',
      noPlan: 'Noch kein aktiver Plan',
      discuss: /^Mit Coach besprechen/,
      moved: [
        ['dash-efficiency', 'Effizienz'],
        ['dash-power', 'Leistungskurve'],
        ['dash-calendar', 'Belastungskalender'],
      ],
      readiness: 'Im Rahmen deines Schnitts',
    },
    notComputable: 'nicht berechenbar',
    estimated: 'geschätzt',
    duplicate: 'Duplikat',
    validation: 'Schema in Ordnung',
    source: 'Quelle:',
    ics: /^ICS gespeichert: \d+ Veranstaltungen\.$/,
    cancelled: 'Abgebrochen',
    restored: 'Wiederhergestellt',
    notMonday: 'Der Planbeginn muss ein Montag sein.',
  },
  en: {
    nav: {
      today: 'Today',
      plan: 'Plan',
      history: 'History',
      settings: 'Settings',
    },
    historyTabs: ['Calendar', 'Activities', 'Progress', 'Performance', 'Body'],
    settingsTabs: ['Data and Garmin', 'Coach', 'Profile and thresholds', 'App'],
    dash: {
      instruments: [
        ['today-form', 'Form'],
        ['today-readiness', 'Recovery against your average'],
        ['today-week-story', 'This week'],
      ],
      progress: [
        ['dash-form', 'Form history'],
        ['dash-ramp', 'Ramp'],
        ['dash-distribution', 'Distribution'],
      ],
      insights: 'Coach insights',
      noPlan: 'No active plan yet',
      discuss: /^Discuss with coach/,
      moved: [
        ['dash-efficiency', 'Efficiency'],
        ['dash-power', 'Power curve'],
        ['dash-calendar', 'Load calendar'],
      ],
      readiness: 'Within your average',
    },
    notComputable: 'not computable',
    estimated: 'estimated',
    duplicate: 'Duplicate',
    validation: 'Schema valid',
    source: 'Source:',
    ics: /^ICS saved: \d+ events\.$/,
    cancelled: 'Cancelled',
    restored: 'Restored',
    notMonday: 'The plan must start on a Monday.',
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
  test(`öffnet die Ansichten (${lang})`, async ({ page }) => {
    const text = TEXT[lang];
    await page.goto('/');
    if (lang === 'en') await toEnglish(page);
    await expect(page.locator('html')).toHaveAttribute('lang', lang);

    // Navigation (T42): genau drei Hauptpunkte und Einstellungen, in der Sprache; dazu der
    // Knopf für das Seitenfenster des Coachs (T43), geschlossen.
    await expect(page.getByRole('navigation').getByRole('button')).toHaveCount(5);
    await expect(page.getByTestId('nav-coach')).toHaveText(/^Coach/);
    await expect(page.getByTestId('nav-coach')).toHaveAttribute('aria-expanded', 'false');
    for (const [view, label] of Object.entries(text.nav)) {
      await expect(page.getByTestId(`nav-${view}`)).toHaveText(label);
    }
    await expect(page.getByTestId('header-language')).toHaveCount(0);
    await expect(page.getByTestId('header-theme')).toHaveCount(0);

    // 0. Heute (Startseite, T63): Entscheidungsstreifen als einzige erhobene Fläche mit genau
    // einer Primäraktion, drei Instrumente, höchstens drei Notizen.
    await expect(page.getByTestId('today-title')).toHaveText(text.dash.noPlan);
    const decision = page.getByTestId('today-decision');
    await expect(decision.locator('button.primary')).toHaveCount(1);
    await expect(decision.locator('button.primary')).toHaveText(text.dash.discuss);
    await expect(page.locator('.card')).toHaveCount(1);
    for (const [id, label] of text.dash.instruments) {
      await expect(page.getByTestId(id).locator('.inst-label')).toHaveText(label);
    }
    await expect(page.getByTestId('today-insights')).toHaveAttribute(
      'aria-label',
      text.dash.insights,
    );
    expect(await page.getByTestId('insight').count()).toBeLessThanOrEqual(3);
    await expect(page.getByTestId('form-state')).toBeVisible();
    await expect(page.getByTestId('form-value')).toBeVisible();
    await expect(page.getByTestId('form-scale')).toBeVisible();
    await expect(page.getByTestId('lead')).toHaveCount(0);
    await expect(page.getByTestId('readiness-state')).toHaveText(text.dash.readiness);
    await expect(page.getByTestId('vital-hrv')).toBeVisible();
    await expect(page.getByTestId('today-week-story').getByTestId('day')).toHaveCount(7);
    // Nach Verlauf umgezogen (T44, T63).
    for (const id of [
      'today-adherence',
      'today-efficiency',
      'today-power',
      'today-calendar',
      'dash-ramp',
      'dash-form',
    ]) {
      await expect(page.getByTestId(id)).toHaveCount(0);
    }
    // Die Legende der Bibliothek ("Datum: ...:·") erscheint nirgends.
    await expect(page.locator('.u-legend')).toHaveCount(0);
    await shot(page, `00-heute-${lang}`);

    // 1. Verlauf, Reiter Kalender (T35, T42): ganze Wochen, Wochensummen, Blättern ändert den
    // Monat. Reiterleiste mit Namen der Sprache, nur der aktive Reiter per Tab-Taste erreichbar.
    await page.getByTestId('nav-history').click();
    await expect(page.getByRole('heading', { level: 1 })).toHaveText(text.nav.history);
    await expect(page.getByTestId('tabs-history').getByRole('tab')).toHaveText([
      ...text.historyTabs,
    ]);
    await expect(page.getByTestId('tab-history-calendar')).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('[data-testid="tabs-history"] [tabindex="0"]')).toHaveCount(1);
    const days = await page.getByTestId('cal-day').count();
    expect(days % 7).toBe(0);
    expect(days).toBeGreaterThanOrEqual(28);
    await expect(page.getByTestId('cal-week-sum')).toHaveCount(days / 7);
    expect(await page.getByTestId('cal-activity').count()).toBeGreaterThan(0);
    const before = await page.getByTestId('cal-month').textContent();
    await page.getByTestId('cal-prev').click();
    await expect(page.getByTestId('cal-month')).not.toHaveText(before ?? '');
    await page.getByTestId('cal-today').click();
    await shot(page, `01-kalender-${lang}`);

    // 1a. Umschalter Woche: sieben Tage, Last-Balken, kein zweites PMC-Diagramm.
    await page.locator('[data-testid="calendar-mode"] button[data-value="week"]').click();
    await expect(page.getByTestId('day')).toHaveCount(7);
    await expect(page.getByTestId('week-range')).toContainText(lang === 'de' ? ' bis ' : ' to ');
    await expect(page.getByTestId('pmc-values')).toHaveCount(0);
    await shot(page, `01a-woche-${lang}`);
    // Pfeiltaste wechselt den Reiter und nimmt den Fokus mit.
    await page.getByTestId('tab-history-calendar').focus();
    await page.keyboard.press('ArrowRight');
    await expect(page.getByTestId('tab-history-activities')).toBeFocused();
    await expect(page.getByTestId('tab-history-activities')).toHaveAttribute(
      'aria-selected',
      'true',
    );

    // 1b. Fortschritt (T32): Jahresvergleich, 52 Wochen, 24 Monate, Umschalter der Größe.
    await page.getByTestId('tab-history-progress').click();
    await expect(page.getByTestId('progress-current')).toBeVisible();
    await expect(page.getByTestId('progress-week')).toHaveCount(52);
    await expect(page.getByTestId('progress-month')).toHaveCount(24);
    await expect(page.getByTestId('progress-year').locator('.uplot canvas').first()).toBeVisible();
    await page.locator('[data-testid="progress-metric"] button[data-value="distance"]').click();
    await expect(page.getByTestId('lead')).toContainText('km');
    // Form-Verlauf, Rampe und Verteilung seit T63 hier statt auf Heute.
    for (const [id, label] of text.dash.progress) {
      const section = page.getByTestId(id);
      await expect(section.locator('.section-label')).toHaveText(label);
      await expect(section.locator('.section-statement')).not.toBeEmpty();
    }
    await expect(page.getByTestId('dash-pmc').locator('.uplot canvas').first()).toBeVisible();
    await expect(page.getByTestId('dash-ramp').locator('.uplot canvas').first()).toBeVisible();
    await expect(page.getByTestId('share-week')).toHaveCount(8);
    // Belastungskalender seit T44 hier statt auf Heute.
    const calendar = page.getByTestId(text.dash.moved[2][0]);
    await expect(calendar.locator('.section-label')).toHaveText(text.dash.moved[2][1]);
    expect(await calendar.locator('[data-testid="calendar"] rect.cell').count()).toBe(365);
    await shot(page, `01b-fortschritt-${lang}`);

    // 1c. Leistung (T33): Kurve gezeichnet, Tabelle der Eckdauern, Zeitraum umschaltbar.
    await page.getByTestId('tab-history-performance').click();
    // Effizienz und Leistungskurve der letzten 90 Tage oben (seit T44).
    for (const [id, label] of text.dash.moved.slice(0, 2)) {
      await expect(page.getByTestId(id).locator('.section-label')).toHaveText(label);
      await expect(page.getByTestId(id).locator('.section-statement')).not.toBeEmpty();
    }
    await expect(
      page.getByTestId('dash-efficiency').locator('.uplot canvas').first(),
    ).toBeVisible();
    await expect(page.getByTestId('power-row')).toHaveCount(5);
    await expect(page.getByTestId('perf-curve').locator('.uplot canvas').first()).toBeVisible();
    await expect(page.getByTestId('perf-row')).toHaveCount(12);
    // T37: fünf Strecken von 1 km bis Marathon.
    await expect(page.getByTestId('run-row')).toHaveCount(5);
    // T36: Schwellen mit Aussage, FTP-Verlauf gezeichnet, Tabelle der gesetzten Werte.
    await expect(page.getByTestId('perf-thresholds').locator('.section-statement')).not.toBeEmpty();
    await expect(page.getByTestId('ftp-chart').locator('.uplot canvas').first()).toBeVisible();
    expect(await page.getByTestId('thr-row').count()).toBeGreaterThan(0);
    await expect(page.getByTestId('perf-thresholds')).not.toContainText('ui.');
    await page.locator('[data-testid="perf-range"] button[data-value="all"]').click();
    await expect(
      page.locator('[data-testid="perf-range"] button[aria-pressed="true"]'),
    ).toHaveAttribute('data-value', 'all');
    await shot(page, `01c-leistung-${lang}`);

    // 1d. Körper, Erholung (T34): Kennzahlen, Verläufe mit Basis, Tageslast, Zeitraum.
    await page.getByTestId('tab-history-body').click();
    await expect(page.getByTestId('rec-summary')).toBeVisible();
    for (const id of ['rec-hrv', 'rec-rhr', 'rec-sleep', 'rec-battery', 'rec-load']) {
      await expect(page.getByTestId(id).locator('.uplot canvas').first()).toBeVisible();
    }
    await page.locator('[data-testid="rec-range"] button[data-value="30"]').click();
    await expect(page.getByTestId('lead')).toContainText('30');
    await shot(page, `01d-erholung-${lang}`);

    // 1e. Körper, Gesundheit (T39): Tages- und Trainingswerte aus Garmin mit Mini-Verlauf.
    await page.locator('[data-testid="body-tab"] button[data-value="health"]').click();
    for (const id of ['health-daily', 'health-training']) {
      await expect(page.getByTestId(id).locator('.section-statement')).not.toBeEmpty();
      await expect(page.getByTestId(id).locator('svg.spark').first()).toBeVisible();
    }
    await expect(page.getByTestId('health-hydration')).toHaveCount(0);
    await page.locator('[data-testid="health-range"] button[data-value="365"]').click();
    await expect(page.getByTestId('health-vo2max_running')).toBeVisible();
    await shot(page, `01e-gesundheit-${lang}`);

    // 2. Aktivitätsliste: Zeilen, Duplikate sichtbar, nicht berechenbare Lasten als Text.
    await page.getByTestId('tab-history-activities').click();
    await expect(page.getByTestId('activity-row').first()).toBeVisible();
    expect(await page.getByTestId('activity-row').count()).toBeGreaterThan(10);
    await expect(page.getByTestId('duplicate-flag').first()).toHaveText(text.duplicate);
    const missing = page.locator('[data-testid="load"][data-status="missing_threshold"]').first();
    await expect(missing).toHaveText(text.notComputable);
    // Eine nicht berechenbare Last erscheint nie als 0.
    await expect(
      page.getByTestId('activity-table').locator('td', { hasText: /^0([.,]0)?$/ }),
    ).toHaveCount(0);
    await shot(page, `02-aktivitaeten-${lang}`);

    // 3. Detail an Ort und Stelle (T42): Kennzahlen, Zonen mit Kennzeichnung, Verlauf.
    await page
      .getByTestId('activity-row')
      .filter({ has: page.locator('[data-testid="load"][data-status="ok"]') })
      .filter({ hasText: lang === 'de' ? 'Rad' : 'Cycling' })
      .first()
      .click();
    await expect(page.getByTestId('detail-title')).toBeVisible();
    await expect(page.getByTestId('metrics')).toBeVisible();
    await expect(page.getByTestId('zone-scheme').first()).toBeVisible();
    await expect(page.locator('.uplot canvas').first()).toBeVisible();
    // T31: Gleichmäßigkeit mit Aussage aus dem Zustandscode, nie ein roher Schlüssel.
    await expect(page.getByTestId('decoupling-statement')).not.toBeEmpty();
    await expect(page.getByTestId('decoupling-statement')).not.toContainText('ui.detail');
    // Zonen nach Herzfrequenzreserve beruhen auf dem Alter und sind als geschätzt gekennzeichnet.
    const karvonen = page.locator('[data-testid="zone-scheme"][data-scheme="hr_karvonen"]');
    if ((await karvonen.count()) > 0) {
      await expect(karvonen.getByTestId('estimated').first()).toHaveText(text.estimated);
    }
    await shot(page, `03-detail-${lang}`);

    // Der Verlauf merkt sich das offene Detail; "Zurück" führt zur Liste.
    await page.getByTestId('nav-today').click();
    await page.getByTestId('nav-history').click();
    await expect(page.getByTestId('detail-title')).toBeVisible();
    await page.getByTestId('detail-back').click();
    await expect(page.getByTestId('activity-table')).toBeVisible();

    // 4. Einstellungen in vier Reitern: Daten und Garmin, Coach, Profil und Schwellen, App.
    await page.getByTestId('nav-settings').click();
    await expect(page.getByTestId('tabs-settings').getByRole('tab')).toHaveText([
      ...text.settingsTabs,
    ]);
    await expect(page.getByTestId('data-section')).toBeVisible();
    await expect(page.getByTestId('garmin-section')).toBeVisible();
    await expect(page.getByTestId('mcp-section')).toBeVisible();
    await page.getByTestId('tab-settings-coach').click();
    await expect(page.getByTestId('coach-setup-section')).toBeVisible();
    await page.getByTestId('tab-settings-profile').click();
    await expect(page.getByTestId('threshold-table')).toBeVisible();
    await expect(page.getByTestId('hr-max-estimate').getByTestId('estimated')).toHaveText(
      text.estimated,
    );
    await expect(page.getByTestId('athlete-section')).toBeVisible();
    await expect(page.getByTestId('pmc-section')).toBeVisible();
    await shot(page, `04-einstellungen-${lang}`);
    await page.getByTestId('tab-settings-app').click();
    await expect(page.getByTestId('appearance-section')).toBeVisible();
    await expect(page.getByTestId('about-section')).toBeVisible();
  });
}

/** Aufrufe der Exporte, die die Attrappe im Browser protokolliert (kein Dateidialog). */
async function mockCalls(
  page: Page,
): Promise<{ command: string; args: Record<string, unknown> }[]> {
  return page.evaluate(
    () =>
      (
        window as unknown as {
          __lastwerkMockCalls: { command: string; args: Record<string, unknown> }[];
        }
      ).__lastwerkMockCalls,
  );
}

/** Tag `days` nach einem Kalendertag, in UTC gerechnet (wie die Oberfläche). */
function plusDays(iso: string, days: number): string {
  const d = new Date(`${iso}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() + days);
  return d.toISOString().slice(0, 10);
}

for (const lang of ['de', 'en'] as const) {
  test(`Planansicht erzeugt, zeigt und exportiert einen Plan (${lang})`, async ({ page }) => {
    const text = TEXT[lang];
    await page.goto('/');
    if (lang === 'en') await toEnglish(page);
    await page.getByTestId('nav-plan').click();
    // Die Panels der Feineinstellungen liegen hinter einer eingeklappten Auswahl (T26).
    await expect(page.getByTestId('planner')).toBeVisible();
    await expect(page.getByTestId('generate-section')).toBeHidden();
    await page.getByTestId('fine-tuning').locator('summary').click();
    await expect(page.getByTestId('generate-section')).toBeVisible();
    await expect(page.getByTestId('goal-section')).toBeVisible();
    await expect(page.getByTestId('availability-section')).toBeVisible();
    await expect(page.getByTestId('plan-list-empty')).toBeVisible();
    // Ohne Plan sind ICS und JSON gesperrt.
    await expect(page.getByTestId('export-ics')).toBeDisabled();

    // Ein Beginn, der kein Montag ist, wird abgewiesen.
    const start = await page.getByTestId('gen-week-start').inputValue();
    await page.getByTestId('gen-week-start').fill(plusDays(start, 1));
    await page.getByTestId('gen-run').click();
    await expect(page.getByTestId('gen-error')).toHaveText(text.notMonday);
    await page.getByTestId('gen-week-start').fill(start);

    // Ziel: Termin am Mittwoch der vierten Planwoche.
    await page.getByTestId('goal-date').fill(plusDays(start, 23));
    await page.getByTestId('goal-label').fill('Testrennen');
    await page.getByTestId('goal-save').click();
    await expect(page.getByTestId('goal-saved')).toBeVisible();
    await expect(page.getByTestId('goal-active')).toContainText('Testrennen');

    // Verfügbarkeit: Sonntag fester Ruhetag.
    await page.getByTestId('avail-rest-6').check();
    await page.getByTestId('avail-save').click();
    await expect(page.getByTestId('avail-saved')).toBeVisible();

    await page.getByTestId('gen-sport-cycling').check();
    await page.getByTestId('gen-sport-running').check();
    await page.getByTestId('gen-run').click();
    await expect(page.getByTestId('plan-display')).toBeVisible();
    await expect(page.getByTestId('gen-done')).toBeVisible();

    // Wochen mit Phase, sieben Tage je Woche, Einheiten mit Namen der Vorlage.
    await expect(page.getByTestId('plan-week')).toHaveCount(4);
    await expect(page.locator('[data-testid="plan-week"][data-phase="race"]')).toHaveCount(1);
    await expect(page.getByTestId('plan-week').first().getByTestId('plan-day')).toHaveCount(7);
    expect(await page.getByTestId('plan-session').count()).toBeGreaterThan(10);
    await expect(page.getByTestId('plan-display')).not.toContainText('bike.endurance.steady');
    await expect(page.getByTestId('week-target').first()).toBeVisible();
    // Der feste Ruhetag bleibt frei.
    await expect(
      page
        .getByTestId('plan-week')
        .first()
        .locator('[data-testid="plan-day"]')
        .nth(6)
        .getByTestId('plan-session'),
    ).toHaveCount(0);

    // Prüfung, Warnungen mit Code, Text und Quelle, verlauf als Diagramm.
    await expect(page.getByTestId('plan-validation')).toContainText(text.validation);
    await expect(page.getByTestId('plan-warning').first()).toBeVisible();
    await expect(page.getByTestId('plan-warning-text').first()).toContainText(text.source);
    await expect(
      page.getByTestId('plan-projection').locator('.uplot canvas').first(),
    ).toBeVisible();
    await shot(page, `06-plan-${lang}`);

    // Plan in der Liste, aktivieren.
    await expect(page.getByTestId('plan-row')).toHaveCount(1);
    await page.getByTestId('plan-activate').click();
    await expect(page.locator('[data-testid="plan-row"][data-status="active"]')).toHaveCount(1);
    await expect(page.getByTestId('plan-display')).toHaveAttribute('data-status', 'active');

    // Export: Die Attrappe protokolliert die Aufrufe, es öffnet sich kein Dateidialog.
    const id = (await page.getByTestId('plan-display').getAttribute('data-plan-id')) as string;
    await page.getByTestId('export-ics').click();
    await expect(page.getByTestId('export-result')).toHaveText(text.ics);
    await page.getByTestId('export-json').click();
    await expect(page.getByTestId('export-result')).toBeVisible();
    await page.getByTestId('export-backup').click();
    await expect(page.getByTestId('export-result')).toContainText('Backup');
    expect(await mockCalls(page)).toEqual([
      { command: 'export_plan_ics', args: { id } },
      { command: 'export_plan_json', args: { id } },
      { command: 'export_backup', args: {} },
    ]);

    // Wiederherstellung: erst Bestätigung, Abbrechen ruft nichts auf.
    await page.getByTestId('export-restore').click();
    await expect(page.getByTestId('restore-confirm')).toBeVisible();
    await page.getByTestId('restore-no').click();
    await expect(page.getByTestId('restore-confirm')).toHaveCount(0);
    expect((await mockCalls(page)).length).toBe(3);
    await page.getByTestId('export-restore').click();
    await page.getByTestId('restore-yes').click();
    await expect(page.getByTestId('export-result')).toContainText(text.restored);
    await expect(page.getByTestId('restore-hint')).toContainText('lastwerk rebuild');
    expect((await mockCalls(page)).at(-1)).toEqual({
      command: 'restore_backup',
      args: { confirmed: true },
    });
    await shot(page, `07-export-${lang}`);
  });
}

for (const lang of ['de', 'en'] as const) {
  test(`Planer mit drei Fragen und Gespräch mit dem Coach (${lang})`, async ({ page }) => {
    const t =
      lang === 'de'
        ? {
            submit: 'Plan erstellen',
            done: 'Der Plan ist erstellt und aktiv.',
            thinking: 'Der Coach denkt nach',
            accept: 'Übernehmen',
            applied: 'Übernommen',
            rejected: 'wurde verworfen',
            seen: 'Was sieht der Coach?',
            setup: 'Der Coach braucht ein lokales Modell',
            activeLabel: 'Aktiv',
          }
        : {
            submit: 'Create plan',
            done: 'The plan is created and active.',
            thinking: 'The coach is thinking',
            accept: 'Apply',
            applied: 'Applied',
            rejected: 'was discarded',
            seen: 'What does the coach see?',
            setup: 'The coach needs a local model',
            activeLabel: 'Active',
          };
    await page.goto('/');
    if (lang === 'en') await toEnglish(page);
    await page.getByTestId('nav-plan').click();
    await expect(page.getByTestId('planner')).toBeVisible();
    await expect(page.getByRole('heading', { level: 1 })).toHaveText('Plan');

    // Drei Fragen, eine Primäraktion. Das Gespräch liegt im Seitenfenster (T43) und ist ohne
    // Plan gesperrt.
    await expect(page.getByTestId('planner-submit')).toHaveText(t.submit);
    await expect(page.getByTestId('planner').locator('button.primary')).toHaveCount(1);
    await expect(page.getByTestId('coach-drawer')).toBeHidden();
    await page.getByTestId('nav-coach').click();
    await expect(page.getByTestId('nav-coach')).toHaveAttribute('aria-expanded', 'true');
    await expect(page.getByTestId('coach-consent')).toBeVisible();
    await expect(page.getByTestId('coach-input')).toHaveCount(0);
    await shot(page, `08-planer-leer-${lang}`);
    await page.getByTestId('coach-consent-accept').click();
    await expect(page.getByTestId('coach-input')).toBeDisabled();
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('coach-drawer')).toBeHidden();

    // Antworten: 8 Stunden, Mo bis Fr.
    await page.getByTestId('planner-hours').fill('8');
    await expect(page.getByTestId('planner-hours-value')).toContainText('8');
    for (const i of [0, 2, 4]) await page.getByTestId(`planner-day-${i}`).click();
    for (const i of [6]) await page.getByTestId(`planner-day-${i}`).click();
    await page.getByTestId('planner-submit').click();
    await expect(page.getByTestId('planner-done')).toHaveText(t.done);
    await expect(page.getByTestId('plan-display')).toHaveAttribute('data-status', 'active');
    await expect(page.getByTestId('plan-status')).toHaveText(t.activeLabel);

    // Der Coach öffnet sich von Heute aus, mit der Ansicht in der Kontextzeile.
    await page.getByTestId('nav-today').click();
    await page.getByTestId('nav-coach').click();
    await expect(page.getByTestId('coach-context')).toHaveText(
      lang === 'de' ? 'Ich sehe: Heute' : 'I see: Today',
    );

    // Gespräch: Antwort als Text, Vorschlag mit Vorher und Nachher, Übernehmen.
    await expect(page.getByTestId('coach-input')).toBeEnabled();
    await expect(page.getByTestId('coach-input')).toBeFocused();
    await page.getByTestId('coach-input').fill('Hallo <b>Coach</b>');
    await page.getByTestId('coach-send').click();
    await expect(page.getByTestId('coach-thinking')).toContainText(t.thinking);
    await expect(page.getByTestId('coach-message')).toHaveCount(2);
    await expect(page.getByTestId('coach-text').first()).toHaveText('Hallo <b>Coach</b>');
    await expect(page.locator('[data-testid="coach-list"] b')).toHaveCount(0);

    // Das Gespräch übersteht Schließen und Wechsel der Ansicht.
    await page.getByTestId('coach-close').click();
    await page.getByTestId('nav-plan').click();
    await page.getByTestId('plan-talk-coach').click();
    await expect(page.getByTestId('coach-message')).toHaveCount(2);

    await page.getByTestId('coach-input').fill('Weniger');
    await page.getByTestId('coach-send').click();
    const card = page.getByTestId('proposal');
    await expect(card).toBeVisible();
    await expect(card.getByTestId('proposal-compare')).toContainText('→');
    await page.getByTestId('coach-seen').click();
    await expect(page.getByTestId('coach-preview-message').first()).toBeVisible();
    await expect(page.getByTestId('coach-preview')).toContainText('127.0.0.1');
    await expect(page.getByTestId('coach-preview')).toContainText('"focus":{"view":"plan"}');
    await shot(page, `09-coach-vorschlag-${lang}`);
    await page.getByTestId('coach-seen').click();
    await card.getByTestId('proposal-accept').click();
    await expect(card.getByTestId('proposal-applied')).toContainText(t.applied);
    await expect(page.getByTestId('plan-display')).toContainText(
      lang === 'de' ? 'Sprachmodell' : 'Language model',
    );

    // Neue Zeiten, dann ein Vorschlag, den die Regeln verwerfen.
    await page.getByTestId('coach-input').fill('Dienstag nicht');
    await page.getByTestId('coach-send').click();
    await expect(
      page.locator('[data-testid="proposal"][data-kind="planning_change"]'),
    ).toBeVisible();
    await page.getByTestId('coach-input').fill('Zwei harte Tage');
    await page.getByTestId('coach-send').click();
    await expect(page.getByTestId('coach-rejected')).toContainText(t.rejected);
    await shot(page, `10-coach-verworfen-${lang}`);
  });

  test(`Gespräch ohne Modell erklärt die Einrichtung, der Planer geht (${lang})`, async ({
    page,
  }) => {
    await page.goto('/?coach=off');
    if (lang === 'en') await toEnglish(page);
    await page.getByTestId('nav-plan').click();
    await page.getByTestId('nav-coach').click();
    await expect(page.getByTestId('coach-setup')).toContainText(
      lang === 'de' ? 'Der Coach braucht ein lokales Modell' : 'The coach needs a local model',
    );
    await expect(page.getByTestId('coach-setup')).toContainText('ollama pull');
    await expect(page.getByTestId('coach-input')).toHaveCount(0);
    await shot(page, `11-coach-ohne-modell-${lang}`);
    await page.getByTestId('coach-close').click();
    await page.getByTestId('planner-submit').click();
    await expect(page.getByTestId('plan-display')).toHaveAttribute('data-status', 'active');
  });
}

for (const lang of ['de', 'en'] as const) {
  test(`Strg+J öffnet den Coach im Verlauf mit Kontextzeile (${lang})`, async ({ page }) => {
    await page.goto('/');
    if (lang === 'en') await toEnglish(page);
    await openHistoryTab(page, 'performance');
    await page.keyboard.press('Control+j');
    const drawer = page.getByTestId('coach-drawer');
    await expect(drawer).toBeVisible();
    await expect(page.getByTestId('coach-context')).toHaveText(
      lang === 'de' ? 'Ich sehe: Verlauf · Leistung' : 'I see: History · Performance',
    );
    // Nicht modal: Der Reiter wechselt bei offenem Fenster, die Zeile folgt.
    await page.getByTestId('tab-history-progress').click();
    await expect(page.getByTestId('coach-context')).toHaveText(
      lang === 'de' ? 'Ich sehe: Verlauf · Fortschritt' : 'I see: History · Progress',
    );
    await shot(page, `12-coach-verlauf-${lang}`);
    await page.keyboard.press('Control+j');
    await expect(drawer).toBeHidden();
    await expect(page.getByTestId('nav-coach')).toHaveAttribute('aria-expanded', 'false');
  });
}

for (const lang of ['de', 'en'] as const) {
  test(`Strg+K öffnet die Befehlspalette, Eingabe springt oder fragt den Coach (${lang})`, async ({
    page,
  }) => {
    await page.goto('/');
    if (lang === 'en') await toEnglish(page);
    await page.keyboard.press('Control+k');
    const input = page.getByTestId('palette-input');
    await expect(input).toBeFocused();
    await input.fill(lang === 'de' ? 'leistung' : 'performance');
    await page.keyboard.press('Enter');
    await expect(page.getByTestId('palette')).toHaveCount(0);
    await expect(page.getByTestId('tab-history-performance')).toHaveAttribute(
      'aria-selected',
      'true',
    );

    await page.keyboard.press('Control+k');
    await input.fill('xqzw warum so muede');
    await expect(page.getByTestId('palette-option')).toHaveCount(1);
    await page.keyboard.press('Enter');
    await expect(page.getByTestId('coach-drawer')).toHaveAttribute('data-open', 'true');
    await expect(page.getByTestId('coach-message')).toHaveCount(0);

    // Escape schließt die Palette, ohne etwas auszuführen.
    await page.getByTestId('coach-close').click();
    await page.keyboard.press('Control+k');
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('palette')).toHaveCount(0);
    // G dann P springt zum Plan.
    await page.keyboard.press('g');
    await page.keyboard.press('p');
    await expect(page.getByTestId('nav-plan')).toHaveAttribute('aria-current', 'page');
  });
}

test('Schmal (360 px): keine Ansicht scrollt waagerecht, die Leiste liegt unten', async ({
  page,
}) => {
  await page.setViewportSize({ width: 360, height: 780 });
  await page.goto('/?plan=laufend');
  for (const v of ['today', 'plan', 'history', 'settings'] as const) {
    await page.getByTestId(`nav-${v}`).click();
    await page.waitForTimeout(300);
    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    );
    expect(overflow, v).toBeLessThanOrEqual(0);
  }
  const box = await page.getByTestId('nav-today').boundingBox();
  expect(box!.y).toBeGreaterThan(700);
});

for (const lang of ['de', 'en'] as const) {
  test(`Tagesentscheidung und Erkenntnisse öffnen den Coach, ohne zu senden (${lang})`, async ({
    page,
  }) => {
    await page.goto('/?dashboard=low&decision=lighten');
    if (lang === 'en') await toEnglish(page);
    const decision = page.getByTestId('today-decision');
    await expect(decision).toHaveAttribute('data-state', 'lighten');
    await expect(page.getByTestId('decision-rules')).toContainText('R12');
    // Alternative zeigt Einzelheiten, ändert nichts und öffnet nichts.
    await page.getByTestId('decision-alt-planned').click();
    await expect(page.getByTestId('decision-alt-detail')).toBeVisible();
    await expect(page.getByTestId('coach-drawer')).toHaveAttribute('data-open', 'false');

    // Die Erholung steht im Instrument, nicht noch einmal als Notiz (T63).
    await expect(page.locator('[data-testid="insight"][data-code="readiness.low"]')).toHaveCount(0);
    await expect(page.getByTestId('readiness-state')).toBeVisible();
    // Kürzel B öffnet den Coach wie die Primäraktion, ohne zu senden.
    await page.keyboard.press('b');
    await expect(page.getByTestId('coach-drawer')).toHaveAttribute('data-open', 'true');
    await expect(page.getByTestId('coach-context')).toContainText(
      lang === 'de' ? 'Heute' : 'Today',
    );
    await expect(page.getByTestId('coach-message')).toHaveCount(0);
    await page.getByTestId('coach-close').click();
    await decision.locator('button.primary').click();
    await expect(page.getByTestId('coach-drawer')).toHaveAttribute('data-open', 'true');
    await expect(page.getByTestId('coach-message')).toHaveCount(0);
    await shot(page, `13-heute-entscheidung-coach-${lang}`);
  });
}

test('Erholungswerte für den MCP-Server: Standard aus, Freigabe nur mit Bestätigung', async ({
  page,
}) => {
  await page.goto('/');
  await page.getByTestId('nav-settings').click();
  const toggle = page.getByTestId('mcp-recovery-toggle');
  await expect(toggle).not.toBeChecked();
  await expect(page.getByTestId('mcp-recovery-state')).toContainText('Gesperrt');
  await expect(page.getByTestId('mcp-section')).toContainText('HRV, Ruhepuls, Schlafdauer');

  // Anklicken fragt nur nach, gespeichert wird erst mit "Freigeben".
  await toggle.click();
  await expect(page.getByTestId('mcp-recovery-confirm')).toBeVisible();
  await expect(toggle).not.toBeChecked();
  await page.getByTestId('mcp-recovery-no').click();
  await expect(page.getByTestId('mcp-recovery-confirm')).toHaveCount(0);
  await expect(page.getByTestId('mcp-recovery-state')).toContainText('Gesperrt');

  await toggle.click();
  await page.getByTestId('mcp-recovery-yes').click();
  await expect(toggle).toBeChecked();
  await expect(page.getByTestId('mcp-recovery-state')).toContainText('Freigegeben');
  await shot(page, '08-mcp-freigabe');

  // Ausschalten sperrt sofort.
  await toggle.click();
  await expect(toggle).not.toBeChecked();
  await expect(page.getByTestId('mcp-recovery-state')).toContainText('Gesperrt');
});

test('Neu einlesen zeigt Fortschritt und Ergebnis, ohne Ordnerwahl', async ({ page }) => {
  await page.goto('/');
  await page.getByTestId('nav-settings').click();
  // Der Ablageort steht nur zur Anzeige da: kein Eingabefeld, kein Knopf zur Ordnerwahl.
  await expect(page.getByTestId('fit-root')).toBeVisible();
  await expect(page.getByTestId('data-section').locator('input')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Ordner wählen' })).toHaveCount(0);
  await page.getByRole('button', { name: 'Neu einlesen' }).click();
  await expect(page.getByTestId('import-summary')).toBeVisible();
  await expect(page.getByTestId('import-summary')).toContainText('Prüfsumme stimmt nicht');
  // Dateipfade gehen nicht an die Oberfläche: nur Fehlercodes mit Anzahl.
  await expect(page.getByTestId('import-summary')).not.toContainText('.fit');
});

test('FIT-Dateien hinzufügen meldet nur Zahlen', async ({ page }) => {
  await page.goto('/');
  await page.getByTestId('nav-settings').click();
  await page.getByRole('button', { name: 'FIT-Dateien hinzufügen' }).click();
  await expect(page.getByTestId('add-result')).toContainText('2 gewählt: 2 neu abgelegt');
  await expect(page.getByTestId('import-summary')).toBeVisible();
  await expect(page.getByTestId('add-result')).not.toContainText('.fit');
  const calls = await mockCalls(page);
  expect(calls.filter((c) => c.command === 'add_fit_files')).toHaveLength(1);
});

test('die Übernahme des früheren Ordners wird einmal gemeldet und lässt sich ausblenden', async ({
  page,
}) => {
  await page.goto('/?uebernahme=fertig');
  const notice = page.getByTestId('migration-notice');
  await expect(notice).toContainText('12 neu, 3 waren schon da');
  await expect(notice).toContainText('Unverändert geblieben');
  await expect(notice).not.toContainText('/');
  await page.getByTestId('migration-dismiss').click();
  await expect(notice).toHaveCount(0);
  await page.goto('/?uebernahme=fehlt');
  await expect(page.getByTestId('migration-notice')).toContainText('nicht gefunden');
});

test('eine neue Schwelle macht Läufe berechenbar, geschätzt bleibt gekennzeichnet', async ({
  page,
}) => {
  await page.goto('/');
  await openHistoryTab(page, 'activities');
  await page.locator('select').filter({ hasText: 'Alle Sportarten' }).selectOption('running');
  const rows = page.getByTestId('activity-row');
  await expect(rows.first()).toBeVisible();
  // Ohne Laufschwelle ist jede Last nicht berechenbar.
  await expect(page.locator('[data-testid="load"][data-status="ok"]')).toHaveCount(0);

  await openSettingsTab(page, 'profile');
  await page.getByTestId('threshold-kind').selectOption('run_speed_mps');
  await page.getByTestId('threshold-value').fill('3,3');
  await page.getByTestId('threshold-from').fill('2020-01-01');
  await page.getByTestId('threshold-form').getByRole('button', { name: 'Speichern' }).click();
  await expect(page.getByTestId('threshold-result')).toBeVisible();
  await expect(page.getByTestId('threshold-table')).toContainText('3,30 m/s');

  await openHistoryTab(page, 'activities');
  await page.locator('select').filter({ hasText: 'Alle Sportarten' }).selectOption('running');
  await expect(rows.first()).toBeVisible();
  await expect(page.locator('[data-testid="load"][data-status="ok"]').first()).toBeVisible();
  // rTSS ist eine Faustregel und trägt die Kennzeichnung.
  await expect(rows.first().getByTestId('estimated')).toHaveText('geschätzt');
});

test('ungültige Eingabe zeigt einen Fehlertext aus der Sprachdatei', async ({ page }) => {
  await page.goto('/');
  await openSettingsTab(page, 'profile');
  await page.getByTestId('threshold-kind').selectOption('ftp_w');
  await page.getByTestId('threshold-value').fill('9999');
  await page.getByTestId('threshold-form').getByRole('button', { name: 'Speichern' }).click();
  await expect(page.getByTestId('threshold-error')).toHaveText('Ungültiger Wert.');
});

test('Thema wechselt und die Oberfläche bleibt dunkel zuerst', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  const dark = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  await openSettingsTab(page, 'app');
  await page.getByTestId('theme-select').selectOption('light');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  const light = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  expect(light).not.toBe(dark);
  await shot(page, '05-hell');
});

test('Erkennung fremder Hosts', () => {
  expect(isAllowedUrl('http://127.0.0.1:4173/assets/index.js')).toBe(true);
  expect(isAllowedUrl('http://localhost:4173/')).toBe(true);
  expect(isAllowedUrl('data:image/png;base64,AAAA')).toBe(true);
  expect(isAllowedUrl('https://example.com/x.js')).toBe(false);
  expect(isAllowedUrl('http://127.0.0.1.evil.example/')).toBe(false);
  expect(isAllowedUrl('https://fonts.googleapis.com/css')).toBe(false);
  expect(isAllowedUrl('ws://example.com/')).toBe(false);
  expect(isAllowedUrl('kein url')).toBe(false);
});
