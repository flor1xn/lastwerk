//! Gezielte Lesezugriffe für die Oberfläche (T06) gegen die synthetischen Fixtures. Jede
//! gezielte Abfrage muss dasselbe liefern wie die Gesamtlesung, gefiltert.

mod common;

use common::{add_thresholds, date, import, work_dir, write_fixtures};
use lastwerk_store::Store;

fn populated(name: &str) -> Store {
    let dir = work_dir(name);
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    add_thresholds(&mut store);
    import(&mut store, &fix);
    store
}

#[test]
fn activities_between_equals_the_filtered_full_read() {
    let store = populated("read-between");
    let all = store.activities().unwrap();
    assert!(all.len() >= 10, "die Fixtures liefern mehrere Aktivitäten");
    let (from, to) = (date(2026, 5, 1), date(2026, 5, 31));
    let got = store.activities_between(from, to).unwrap();
    let mut expected: Vec<_> = all
        .into_iter()
        .filter(|a| a.row.local_date >= from && a.row.local_date <= to)
        .collect();
    expected.sort_by_key(|a| (a.row.start_utc, a.id));
    assert_eq!(got, expected);
    // Ein Zeitraum ohne Aktivität ist leer, nicht fehlerhaft.
    assert!(
        store
            .activities_between(date(2000, 1, 1), date(2000, 1, 7))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn single_activity_reads_match_the_full_reads() {
    let store = populated("read-single");
    let loads = store.activity_loads().unwrap();
    let zones = store.zone_times().unwrap();
    let powers = store.best_powers().unwrap();
    for a in store.activities().unwrap() {
        assert_eq!(store.activity(a.id).unwrap().as_ref(), Some(&a));
        let expected_loads: Vec<_> = loads
            .iter()
            .filter(|l| l.activity_id == a.id)
            .cloned()
            .collect();
        assert_eq!(store.activity_loads_of(a.id).unwrap(), expected_loads);
        let expected_zones: Vec<_> = zones
            .iter()
            .filter(|z| z.activity_id == a.id)
            .cloned()
            .collect();
        assert_eq!(store.zone_times_of(a.id).unwrap(), expected_zones);
        let expected_powers: Vec<_> = powers
            .iter()
            .filter(|p| p.activity_id == a.id)
            .copied()
            .collect();
        assert_eq!(store.best_powers_of(a.id).unwrap(), expected_powers);
        let file = store.source_file(a.row.source_file_id).unwrap().unwrap();
        assert_eq!(file.id, a.row.source_file_id);
    }
    assert_eq!(store.activity(-1).unwrap(), None);
    assert_eq!(store.source_file(-1).unwrap(), None);
}

#[test]
fn loads_between_covers_exactly_the_activities_of_the_range() {
    let store = populated("read-loads");
    let (from, to) = (date(2026, 5, 1), date(2026, 5, 31));
    let ids: Vec<i64> = store
        .activities_between(from, to)
        .unwrap()
        .iter()
        .map(|a| a.id)
        .collect();
    let loads = store.loads_between(from, to).unwrap();
    assert!(!loads.is_empty());
    assert!(loads.iter().all(|l| ids.contains(&l.activity_id)));
    let expected = store
        .activity_loads()
        .unwrap()
        .into_iter()
        .filter(|l| ids.contains(&l.activity_id))
        .count();
    assert_eq!(loads.len(), expected);
}
