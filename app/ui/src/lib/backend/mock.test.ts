import { describe, expect, it } from 'vitest';
import { catalogs } from '../i18n';
import { computePmc, MockBackend, mulberry32 } from './mock';
import { BackendError } from './types';
import { toBackendError } from './tauri';

const TODAY = '2026-10-13';

describe('computePmc (SPEC 8.2)', () => {
  // Handrechnung aus SPEC 8.2: Last 100 an zwei Tagen, 1 - exp(-1/42) = 0,023528,
  // 1 - exp(-1/7) = 0,133122. Tag 1: CTL 2,3528, ATL 13,3122, TSB 0.
  // Tag 2: CTL 4,6503, ATL 24,8523, TSB = 2,3528 - 13,3122 = -10,9594.
  it('stimmt mit der Handrechnung überein', () => {
    const daily = new Map([
      ['2026-01-01', 100],
      ['2026-01-02', 100],
    ]);
    const p = computePmc(daily, '2026-01-01', '2026-01-02', {
      tau_ctl_d: 42,
      tau_atl_d: 7,
      ctl0: 0,
      atl0: 0,
    });
    expect(p[0].ctl).toBeCloseTo(2.3528, 3);
    expect(p[0].atl).toBeCloseTo(13.3122, 3);
    expect(p[0].tsb).toBeCloseTo(0, 6);
    expect(p[1].ctl).toBeCloseTo(4.6503, 3);
    expect(p[1].atl).toBeCloseTo(24.8523, 3);
    expect(p[1].tsb).toBeCloseTo(-10.9594, 3);
  });

  it('zählt Tage ohne Last als 0 und liefert nie NaN bei Zeitkonstante 0', () => {
    const p = computePmc(new Map(), '2026-01-01', '2026-01-03', {
      tau_ctl_d: 0,
      tau_atl_d: 7,
      ctl0: 5,
      atl0: 5,
    });
    expect(p).toHaveLength(3);
    expect(p.every((x) => Number.isFinite(x.ctl) && Number.isFinite(x.tsb))).toBe(true);
  });
});

describe('mulberry32', () => {
  it('ist deterministisch', () => {
    const a = mulberry32(42);
    const b = mulberry32(42);
    expect([a(), a(), a()]).toEqual([b(), b(), b()]);
  });
});

describe('MockBackend', () => {
  it('zeigt ohne Schwelle keine Last und nie 0', async () => {
    const b = new MockBackend({ today: TODAY });
    const list = await b.listActivities({ from: '2026-07-01', to: TODAY, sport: 'running' });
    expect(list.length).toBeGreaterThan(5);
    for (const a of list) {
      expect(a.load.status).toBe('missing_threshold');
      expect(a.load.value).toBeNull();
    }
    const strength = await b.listActivities({ from: '2026-07-01', to: TODAY, sport: 'strength' });
    expect(strength.every((a) => a.load.status === 'missing_channel')).toBe(true);
  });

  it('rechnet nach einer Schwelle und kennzeichnet rTSS als geschätzt', async () => {
    const b = new MockBackend({ today: TODAY });
    const set = await b.setThreshold({
      sport: 'running',
      kind: 'run_speed_mps',
      value: 3.3,
      valid_from: '2020-01-01',
      note: null,
    });
    expect(set.recompute.skipped).toBe(false);
    expect(set.recompute.recomputed).toBeGreaterThan(5);
    const list = await b.listActivities({ from: '2026-07-01', to: TODAY, sport: 'running' });
    expect(list.every((a) => a.load.status === 'ok' && (a.load.value ?? 0) > 0)).toBe(true);
    expect(list.every((a) => a.load.method === 'r_tss' && a.load.estimated)).toBe(true);
  });

  it('gilt eine Schwelle erst ab ihrem Datum', async () => {
    const b = new MockBackend({ today: TODAY });
    await b.setThreshold({
      sport: 'running',
      kind: 'run_speed_mps',
      value: 3.3,
      valid_from: '2026-09-01',
      note: null,
    });
    const list = await b.listActivities({ from: '2026-07-01', to: TODAY, sport: 'running' });
    for (const a of list) {
      expect(a.load.status).toBe(a.local_date >= '2026-09-01' ? 'ok' : 'missing_threshold');
    }
  });

  it('lehnt ungültige Schwellen mit einem Fehlercode ab', async () => {
    const b = new MockBackend({ today: TODAY });
    const base = {
      sport: 'cycling',
      kind: 'ftp_w',
      value: 250,
      valid_from: '2026-01-01',
      note: null,
    } as const;
    const code = async (patch: Partial<Parameters<MockBackend['setThreshold']>[0]>) => {
      try {
        await b.setThreshold({ ...base, ...patch });
        return 'ok';
      } catch (e) {
        return e instanceof BackendError ? e.code : 'other';
      }
    };
    expect(await code({ value: 0 })).toBe('invalid_value');
    expect(await code({ value: 2600 })).toBe('invalid_value');
    expect(await code({ sport: 'running' })).toBe('invalid_sport');
    expect(await code({ valid_from: 'gestern' })).toBe('invalid_date');
    expect(await code({ kind: 'lthr_bpm', sport: null, value: 20 })).toBe('invalid_value');
    expect(await code({})).toBe('ok');
  });

  it('zählt Duplikate nicht in die Wochenlast und weist sie in der Liste aus', async () => {
    const b = new MockBackend({ today: TODAY });
    const all = await b.listActivities({ from: '2026-05-01', to: TODAY, sport: null });
    const dup = all.find((a) => !a.is_primary);
    expect(dup).toBeDefined();
    expect(dup?.duplicate_of).not.toBeNull();
    const { mondayOf } = await import('../dates');
    const week = await b.getWeekOverview(mondayOf(dup!.local_date), 4);
    const primaryLoads = all
      .filter(
        (a) => a.is_primary && a.local_date >= week.week_start && a.local_date <= week.week_end,
      )
      .reduce((s, a) => s + (a.load.value ?? 0), 0);
    const shown = week.days.flatMap((d) => d.by_sport).reduce((s, x) => s + x.load, 0);
    expect(shown).toBeCloseTo(primaryLoads, 6);
  });

  it('liefert TSB als CTL minus ATL des Vortags', async () => {
    const b = new MockBackend({ today: TODAY });
    const week = await b.getWeekOverview('2026-10-05', 12);
    expect(week.pmc.length).toBe(12 * 7);
    for (let i = 1; i < week.pmc.length; i++) {
      expect(week.pmc[i].tsb).toBeCloseTo(week.pmc[i - 1].ctl - week.pmc[i - 1].atl, 6);
    }
    // Eine Woche in der Zukunft hat keinen Verlauf.
    expect((await b.getWeekOverview('2026-11-30', 1)).pmc).toEqual([]);
  });

  it('kennzeichnet die Schätzung der maximalen Herzfrequenz und lässt sie bei Messwert weg', async () => {
    const b = new MockBackend({ today: TODAY });
    const list = await b.listThresholds();
    // Geburtsjahr 1988, Alter 38: 208 - 0,7 * 38 = 181,4 (Q7).
    expect(list.hr_max_estimate).toEqual({ bpm: 181.4, estimated: true });
    await b.setThreshold({
      sport: null,
      kind: 'hr_max_bpm',
      value: 190,
      valid_from: '2026-01-01',
      note: null,
    });
    expect((await b.listThresholds()).hr_max_estimate).toBeNull();
  });

  it('weist Zonen nach Reserve auf geschätzter HR_max als geschätzt aus', async () => {
    const b = new MockBackend({ today: TODAY });
    const all = await b.listActivities({ from: '2026-09-01', to: TODAY, sport: 'cycling' });
    const withHr = all.find((a) => a.avg_hr !== null)!;
    const d = await b.getActivity(withHr.id);
    const karvonen = d.zones.find((z) => z.id === 'hr_karvonen');
    expect(karvonen?.estimated).toBe(true);
    // Leistungszonen beruhen auf einem gesetzten FTP und sind nicht geschätzt.
    const power7 = d.zones.find((z) => z.id === 'power7');
    if (power7) expect(power7.estimated).toBe(false);
  });

  it('liefert Entkopplung, Effizienz und Runden nach den Regeln des Rust-Teils (T31)', async () => {
    const b = new MockBackend({ today: TODAY });
    const all = await b.listActivities({ from: '2026-01-01', to: TODAY, sport: null });
    for (const item of all) {
      const d = await b.getActivity(item.id);
      expect(d.series.speed_mps.length).toBe(d.series.t_s.length);
      if (d.efficiency_factor !== null) {
        expect(d.item.sport).toBe('cycling');
        expect(d.item.duration_s).toBeGreaterThanOrEqual(45 * 60);
      }
      expect(d.decoupling.percent === null).toBe(d.decoupling.state === 'decoupling.no_data');
      expect(d.laps.length === 0 || d.laps.length >= 2).toBe(true);
    }
    expect(
      (await Promise.all(all.map((a) => b.getActivity(a.id)))).some((d) => d.laps.length > 1),
    ).toBe(true);
  });
});

describe('Fehler', () => {
  it('wandelt das Fehlerobjekt des Rust-Teils um', () => {
    const e = toBackendError({ code: 'not_found', message_key: 'ui.error.not_found' });
    expect(e).toBeInstanceOf(BackendError);
    expect(e.code).toBe('not_found');
    expect(e.messageKey).toBe('ui.error.not_found');
    expect(toBackendError('Absturz in /home/x').messageKey).toBe('ui.error.internal');
    expect(toBackendError(null).code).toBe('internal');
  });

  it('hat zu jedem Fehlercode der Attrappe einen Text in beiden Sprachen', () => {
    const codes = [
      'invalid_date',
      'invalid_value',
      'invalid_sport',
      'invalid_year',
      'invalid_setting',
      'not_found',
      'internal',
    ];
    for (const c of codes) {
      expect(catalogs.de[`ui.error.${c}`], c).toBeTruthy();
      expect(catalogs.en[`ui.error.${c}`], c).toBeTruthy();
    }
  });
});
