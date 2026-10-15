export const APP_NAME = 'Lastwerk';

/** Fenstertitel, optional mit Abschnitt davor. */
export function windowTitle(section?: string): string {
  return section ? `${section} · ${APP_NAME}` : APP_NAME;
}
