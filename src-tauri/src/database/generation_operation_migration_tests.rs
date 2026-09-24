use super::*;

#[test]
fn document_operation_outcome_migration_preserves_legacy_rows_without_inventing_causes() {
    let mut connection = public_baseline_connection().unwrap();
    for migration in &MIGRATIONS[1..29] {
        apply_transactional_migration(&mut connection, migration).unwrap();
    }
    connection
        .execute(
            "INSERT INTO generation_operations(id,purpose,started_at,finished_at,state)
             VALUES (?1,'document_generation',10,20,'failed')",
            ["a".repeat(32)],
        )
        .unwrap();
    apply_transactional_migration(&mut connection, &MIGRATIONS[29]).unwrap();
    let row: (String, i64, i64, Option<String>) = connection
        .query_row(
            "SELECT state,started_at,finished_at,outcome_json FROM generation_operations",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(row, ("failed".into(), 10, 20, None));
    let oversized = format!("{{\"value\":\"{}\"}}", "x".repeat(1024));
    for invalid in ["not json", "[]", "null", oversized.as_str()] {
        assert!(connection
            .execute("UPDATE generation_operations SET outcome_json=?1", [invalid])
            .is_err());
    }
    let outcome: Option<String> = connection
        .query_row("SELECT outcome_json FROM generation_operations", [], |row| row.get(0))
        .unwrap();
    assert!(outcome.is_none());
    initialize_connection(&mut connection).unwrap();
    let before = connection.total_changes();
    initialize_connection(&mut connection).unwrap();
    assert_eq!(connection.total_changes(), before);
    assert_eq!(entry::user_version(&connection).unwrap(), LATEST_SCHEMA_VERSION);
}
