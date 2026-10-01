//! Commit admission must include changes made by startup reconciliation.
//! Only a newly created synthetic library is used here.

use super::*;
use sha2::{Digest, Sha256};

#[test]
fn invalid_post_reconciliation_state_cannot_commit_the_upgrade() {
    let root = std::env::temp_dir().join(format!(
        "ostudy-upgrade-commit-{}-{}",
        std::process::id(),
        crate::util::unix_time_millis().unwrap(),
    ));
    let paths = AppPaths::new(root).unwrap();
    let connection = Connection::open(paths.database_path()).unwrap();
    let sql = include_str!("../../schema/0001_release_baseline.sql");
    connection.execute_batch(sql).unwrap();
    connection
        .execute(
            "INSERT INTO schema_metadata(key, value) VALUES ('schema_lineage', ?1)",
            [crate::database::CURRENT_SCHEMA_LINEAGE],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO schema_migrations(version, name, checksum, applied_at)
         VALUES (1, 'release_baseline', ?1, 0)",
            [format!("{:x}", Sha256::digest(sql.as_bytes()))],
        )
        .unwrap();
    connection.pragma_update(None, "user_version", 1).unwrap();
    drop(connection);
    let original = fs::read(paths.database_path()).unwrap();
    let mut guard = begin_upgrade(
        &paths,
        "io.github.jferments.ostudy.dev",
        (crate::database::CURRENT_SCHEMA_LINEAGE, 1),
        &mut |_| {},
    )
    .unwrap();
    let transaction = guard.transaction();
    let active = Connection::open(paths.database_path()).unwrap();
    active.pragma_update(None, "user_version", 999).unwrap();
    drop(active);

    assert!(commit_upgrade(&paths, &mut guard).is_err());
    assert!(!transaction.join("committed").exists());
    drop(guard);
    assert_eq!(fs::read(paths.database_path()).unwrap(), original);
    let restored = read_database(paths.database_path()).unwrap();
    let identity = validate_database(&restored).unwrap();
    assert_eq!(identity.1, 1);
    drop(restored);
    verify_checkpoint(&transaction).unwrap();
    assert!(transaction.join("failed/library.sqlite3").is_file());
    assert!(admit_startup(&paths).is_err());
    fs::remove_dir_all(paths.app_data_dir()).unwrap();
}
