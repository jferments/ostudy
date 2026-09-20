use super::*;
use crate::{
    documents::image_sets::{cache, test_support::Fixture},
    pdf_ocr_backend::regions::RegionOcrResult,
};

#[test]
fn document_image_schema21_preserves_owned_extractions_and_originals_on_upgrade() {
    let mut connection = public_baseline_connection().unwrap();
    for migration in &MIGRATIONS[1..20] {
        apply_transactional_migration(&mut connection, migration).unwrap();
    }
    let mut fixture = Fixture::new();
    fixture.database = Database {
        connection: Arc::new(Mutex::new(connection)),
    };
    let manifest = fixture.import(2, 0, None);
    for member in &manifest.members {
        cache::get_or_extract(
            Some(&fixture.database),
            member,
            Some("upgrade-fixture-v1"),
            || {
                Ok(RegionOcrResult {
                    version: 1,
                    text: String::new(),
                    confidence: 0.0,
                    lines: Vec::new(),
                    duration_millis: 1,
                    engine: "fixture".to_string(),
                    language: "eng".to_string(),
                    width: 8,
                    height: 6,
                })
            },
        )
        .unwrap();
    }
    let originals = manifest
        .members
        .iter()
        .map(|member| std::fs::read(fixture.paths.resolve_relative(&member.relative_path)).unwrap())
        .collect::<Vec<_>>();
    fixture
        .database
        .with_connection(|connection| {
            let before =
                crate::documents::image_sets::repository::load(connection, &manifest.document_id)?;
            initialize_connection(connection)
                .map_err(|message| CommandError::new("fixture_migration", message))?;
            assert_eq!(
                before,
                crate::documents::image_sets::repository::load(connection, &manifest.document_id)?
            );
            let version: i64 = connection
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(version, 21);
            cache::validate(connection)?;
            let before = connection.total_changes();
            initialize_connection(connection)
                .map_err(|message| CommandError::new("fixture_reopen", message))?;
            assert_eq!(before, connection.total_changes());
            Ok(())
        })
        .unwrap();
    assert_eq!(fixture.count("document_image_extractions"), 1);
    assert_eq!(fixture.count("document_image_extraction_owners"), 2);
    for (member, original) in manifest.members.iter().zip(originals) {
        assert_eq!(
            std::fs::read(fixture.paths.resolve_relative(&member.relative_path)).unwrap(),
            original
        );
    }
}

#[test]
fn document_image_entry_refuses_surviving_projection_guards_and_parked_positions() {
    let fixture = Fixture::new();
    let manifest = fixture.import(2, 0, None);
    fixture
        .database
        .with_connection(|connection| {
            for mutation in [
                "INSERT INTO document_image_projection_writes SELECT document_id FROM document_image_sets",
                "UPDATE document_image_members SET position = position + 128",
            ] {
                let transaction = connection.transaction().map_err(CommandError::database)?;
                transaction
                    .execute_batch(mutation)
                    .map_err(CommandError::database)?;
                let changes = transaction.total_changes();
                assert!(validate_backup_lineage(&transaction).is_err());
                assert_eq!(transaction.total_changes(), changes);
                transaction.rollback().map_err(CommandError::database)?;
            }
            assert_eq!(
                manifest,
                crate::documents::image_sets::repository::load(connection, &manifest.document_id)?
            );
            validate_backup_lineage(connection)
        })
        .unwrap();
}
