import { expect, type Page } from '@playwright/test';

/** Nur diese Hosts sind erlaubt: der lokale Server der Vorschau (SPEC 10.1, M2). */
const ALLOWED_HOSTS = new Set(['127.0.0.1', 'localhost']);

/** Erlaubt sind Anfragen an den lokalen Server und Adressen ohne Netzwerk (data, blob). */
export function isAllowedUrl(raw: string): boolean {
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return false;
  }
  if (url.protocol === 'data:' || url.protocol === 'blob:' || url.protocol === 'about:') {
    return true;
  }
  return (url.protocol === 'http:' || url.protocol === 'https:') && ALLOWED_HOSTS.has(url.hostname);
}

/** Sammelt Konsolenfehler, Seitenfehler und verbotene Anfragen einer Seite. */
export interface Guard {
  consoleErrors: string[];
  pageErrors: string[];
  externalRequests: string[];
}

/**
 * Blockiert jede Anfrage an einen fremden Host und merkt sie sich. Der Test schlägt danach in
 * `assertClean` fehl. Zusätzlich zählt jeder Konsolenfehler (auch Verstöße gegen die CSP).
 */
export async function installGuard(page: Page): Promise<Guard> {
  const guard: Guard = { consoleErrors: [], pageErrors: [], externalRequests: [] };
  page.on('console', (msg) => {
    if (msg.type() === 'error') guard.consoleErrors.push(msg.text());
  });
  page.on('pageerror', (err) => guard.pageErrors.push(String(err)));
  page.on('request', (req) => {
    if (!isAllowedUrl(req.url())) guard.externalRequests.push(req.url());
  });
  await page.route('**/*', (route) => {
    if (isAllowedUrl(route.request().url())) return route.continue();
    guard.externalRequests.push(route.request().url());
    return route.abort();
  });
  return guard;
}

export function assertClean(guard: Guard): void {
  expect(guard.externalRequests, 'Anfragen an fremde Hosts').toEqual([]);
  expect(guard.consoleErrors, 'Konsolenfehler').toEqual([]);
  expect(guard.pageErrors, 'Fehler im Seitenskript').toEqual([]);
}
