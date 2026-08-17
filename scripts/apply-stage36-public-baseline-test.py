from pathlib import Path
import re

path = Path("src-tauri/src/stage36_38_regression_tests.rs")
source = path.read_text(encoding="utf-8")
pattern = re.compile(
    r"#\[test\]\nfn schema_fourteen_and_all_three_repairs_remain_present_on_canonical_open\(\) \{[\s\S]*\Z"
)
replacement = '''#[test]
fn public_baseline_preserves_stage36_38_attribution_schema() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let schema_version: i64 = connection
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .expect("schema version should load");
            let schema_lineage: String = connection
                .query_row(
                    "SELECT value FROM schema_metadata WHERE key = 'schema_lineage'",
                    [],
                    |row| row.get(0),
                )
                .expect("public schema lineage should load");
            let public_baseline_present: i64 = connection
                .query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM schema_migrations
                         WHERE version = 1 AND name = 'public_baseline'
                     )",
                    [],
                    |row| row.get(0),
                )
                .expect("public baseline ledger entry should load");
            let migration_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get(0))
                .expect("migration count should load");
            let repair_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM data_repairs", [], |row| row.get(0))
                .expect("repair count should load");
            let attribution_tables: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master
                     WHERE type = 'table'
                       AND name IN (
                           'practice_session_card_snapshots',
                           'practice_session_card_tag_snapshots',
                           'review_event_attributions',
                           'review_event_tag_attributions'
                       )",
                    [],
                    |row| row.get(0),
                )
                .expect("attribution tables should load");
            assert_eq!(schema_version, LATEST_SCHEMA_VERSION);
            assert_eq!(schema_lineage, "ostudy-public-v1");
            assert_eq!(public_baseline_present, 1);
            assert_eq!(migration_count, 1);
            assert_eq!(repair_count, 0);
            assert_eq!(attribution_tables, 4);
            Ok(())
        })
        .expect("public baseline should retain Stage 3.6-3.8 attribution state");
}
'''
source, count = pattern.subn(replacement, source, count=1)
if count != 1:
    raise SystemExit(f"expected one stale Stage 3.6-3.8 schema test, found {count}")
path.write_text(source, encoding="utf-8")
