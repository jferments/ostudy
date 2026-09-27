use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn legacy_current_fixture() -> Connection {
    let mut connection = super::super::public_baseline_connection().unwrap();
    for migration in &super::super::MIGRATIONS[1..] {
        super::super::apply_transactional_migration(&mut connection, migration).unwrap();
    }
    super::super::apply_required_repairs(
        &mut connection,
        super::super::RepairPhase::AfterMigration(LEGACY_PUBLIC_SCHEMA_VERSION),
    )
    .unwrap();
    connection
}

fn temporary_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "ostudy-h09-{label}-{}-{}",
        std::process::id(),
        TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    root
}

fn copy_connection(source: &Connection, destination: &std::path::Path) {
    let mut destination = Connection::open(destination).unwrap();
    let backup = rusqlite::backup::Backup::new(source, &mut destination).unwrap();
    backup
        .run_to_completion(128, std::time::Duration::from_millis(1), None)
        .unwrap();
}

#[cfg(target_os = "linux")]
fn peak_rss_kib() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            let value = line.strip_prefix("VmHWM:")?.trim();
            value.split_whitespace().next()?.parse().ok()
        })
}

#[cfg(not(target_os = "linux"))]
fn peak_rss_kib() -> Option<u64> {
    None
}

#[test]
fn low_space_staging_failure_preserves_the_live_legacy_library() {
    let mut connection = legacy_current_fixture();
    let before = preservation_receipt(&connection).unwrap();
    let error = convert_legacy_current_with(&mut connection, |source| {
        staging::Candidate::copy_from_with_page_limit(source, 1)
    })
    .unwrap_err();

    assert!(error.contains("free disk space"));
    assert_eq!(preservation_receipt(&connection).unwrap(), before);
    assert_eq!(
        entry::user_version(&connection).unwrap(),
        LEGACY_PUBLIC_SCHEMA_VERSION
    );
    let lineage: String = connection
        .query_row(
            "SELECT value FROM schema_metadata WHERE key='schema_lineage'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(lineage, super::super::LEGACY_PUBLIC_SCHEMA_LINEAGE);
}

#[test]
fn abandoned_disk_candidate_never_changes_live_lineage() {
    let connection = legacy_current_fixture();
    let before = preservation_receipt(&connection).unwrap();
    let mut candidate = staging::Candidate::copy_from(&connection).unwrap();
    let candidate_path = candidate.path().to_path_buf();

    {
        let transaction = candidate
            .connection_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        write_release_ledger(&transaction).unwrap();
        transaction.commit().unwrap();
    }
    assert_eq!(
        validate_supported_input(candidate.connection()).unwrap(),
        BASELINE_VERSION
    );

    drop(candidate);
    assert!(!candidate_path.exists());
    assert_eq!(preservation_receipt(&connection).unwrap(), before);
    assert_eq!(
        entry::user_version(&connection).unwrap(),
        LEGACY_PUBLIC_SCHEMA_VERSION
    );
}

#[test]
fn disk_conversion_preserves_sibling_managed_files_and_cleans_candidate() {
    let root = temporary_root("managed-files");
    let path = root.join("library.sqlite3");
    let legacy = legacy_current_fixture();
    copy_connection(&legacy, &path);
    drop(legacy);

    let managed = root.join("documents");
    std::fs::create_dir(&managed).unwrap();
    let sentinel = managed.join("preserve.bin");
    std::fs::write(&sentinel, b"managed learner bytes").unwrap();

    let mut live = Connection::open(&path).unwrap();
    convert_legacy_current(&mut live).unwrap();
    validate_input(&live).unwrap();

    assert_eq!(std::fs::read(&sentinel).unwrap(), b"managed learner bytes");
    let leaked = std::fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".ostudy-release-conversion-")
        });
    assert!(!leaked);

    drop(live);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn representative_h09_startup_and_conversion_measurement() {
    let root = temporary_root("measurement");
    let path = root.join("library.sqlite3");
    let legacy = legacy_current_fixture();
    let payload = serde_json::to_string(&"x".repeat(1_000_000)).unwrap();
    legacy
        .execute(
            "INSERT INTO app_settings(key, value_json, updated_at)
             VALUES('h09-measurement-payload', ?1, 1)",
            [payload],
        )
        .unwrap();
    copy_connection(&legacy, &path);
    drop(legacy);

    let source_bytes = std::fs::metadata(&path).unwrap().len();
    let peak_before = peak_rss_kib();

    let conversion_started = std::time::Instant::now();
    let database = super::super::Database::open(&path).unwrap();
    let conversion_ms = conversion_started.elapsed().as_millis();
    drop(database);
    let peak_after_conversion = peak_rss_kib();

    let reopen_started = std::time::Instant::now();
    let database = super::super::Database::open(&path).unwrap();
    let reopen_ms = reopen_started.elapsed().as_millis();
    drop(database);
    let peak_after_reopen = peak_rss_kib();

    eprintln!(
        "OSTUDY_H09_MEASUREMENT conversion_ms={conversion_ms} current_reopen_ms={reopen_ms} source_bytes={source_bytes} peak_rss_before_kib={} peak_rss_after_conversion_kib={} peak_rss_after_reopen_kib={}",
        peak_before.map_or_else(|| "unavailable".to_string(), |value| value.to_string()),
        peak_after_conversion
            .map_or_else(|| "unavailable".to_string(), |value| value.to_string()),
        peak_after_reopen
            .map_or_else(|| "unavailable".to_string(), |value| value.to_string()),
    );

    std::fs::remove_dir_all(root).unwrap();
}
