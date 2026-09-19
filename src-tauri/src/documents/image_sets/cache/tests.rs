use super::*;
use crate::{
    documents::image_sets::{manifest::InclusionCrop, test_support::Fixture},
    pdf_ocr_backend::regions::{OcrLine, PixelBounds},
};

fn result(member: &ImageMember) -> RegionOcrResult {
    let (width, height) = if member.quarter_turns % 2 == 0 {
        (member.width, member.height)
    } else {
        (member.height, member.width)
    };
    RegionOcrResult {
        version: 1, text: "Do not replace 5 mg with 50 mg.".to_string(), confidence: 0.9,
        lines: vec![OcrLine {
            text: "Do not replace 5 mg with 50 mg.".to_string(),
            bounds: Some(PixelBounds { left: 0, top: 0, right: width, bottom: height }),
            group: "paragraph-1".to_string(), confidence: Some(0.9),
        }],
        duration_millis: 1, engine: "synthetic-native-ocr".to_string(), language: "eng".to_string(), width, height,
    }
}

#[test]
fn image_extraction_reuses_pixels_across_occurrences_without_transferring_ownership() {
    let fixture = Fixture::new();
    let first = fixture.import(2, 0, None);
    let second = fixture.import(1, 0, Some(InclusionCrop { left: 0.0, top: 0.0, right: 0.5, bottom: 1.0 }));
    let mut calls = 0;
    for member in first.members.iter().chain(&second.members) {
        let saved = get_or_extract(Some(&fixture.database), member, Some("fixture-ocr-v1"), || {
            calls += 1;
            // Taking the same connection inside the callback proves OCR is not
            // executed under its lock. This is a real repository call, not a mock.
            fixture.database.with_connection(|connection| {
                connection.query_row("SELECT 1", [], |row| row.get::<_, i64>(0)).map_err(CommandError::database)?;
                Ok(())
            })?;
            Ok(result(member))
        }).unwrap();
        assert_eq!(saved.reused, calls == 1 && member.id != first.members[0].id);
        assert_eq!(saved.result.text, "Do not replace 5 mg with 50 mg.");
    }
    assert_eq!(calls, 1);
    assert_eq!(fixture.count("document_image_extractions"), 1);
    assert_eq!(fixture.count("document_image_extraction_owners"), 3);
    fixture.database.with_connection(|connection| {
        validate(connection)?;
        connection.execute("DELETE FROM documents WHERE id = ?1", [&first.document_id]).map_err(CommandError::database)?;
        validate(connection)
    }).unwrap();
    assert_eq!(fixture.count("document_image_extractions"), 1);
    assert_eq!(fixture.count("document_image_extraction_owners"), 1);
    fixture.database.with_connection(|connection| {
        connection.execute("DELETE FROM documents WHERE id = ?1", [&second.document_id]).map_err(CommandError::database)?;
        validate(connection)
    }).unwrap();
    assert_eq!(fixture.count("document_image_extractions"), 0);
}

#[test]
fn image_extraction_survives_database_reopen_and_compatible_retry_is_read_only() {
    let fixture = Fixture::new();
    let manifest = fixture.import(1, 0, None);
    let member = &manifest.members[0];
    get_or_extract(Some(&fixture.database), member, Some("fixture-v1"), || Ok(result(member))).unwrap();
    let path = fixture.root.join("reopen.sqlite3");
    fixture.database.with_connection(|connection| {
        connection.execute("VACUUM INTO ?1", [path.to_str().unwrap()]).map_err(CommandError::database)?;
        Ok(())
    }).unwrap();
    let reopened = Database::open(&path).unwrap();
    let before = reopened.with_connection(|connection| Ok(connection.total_changes())).unwrap();
    let cached = get_or_extract(Some(&reopened), member, Some("fixture-v1"), || panic!("must reuse retained OCR")).unwrap();
    assert!(cached.reused);
    assert_eq!(cached.result, result(member));
    let after = reopened.with_connection(|connection| Ok(connection.total_changes())).unwrap();
    assert_eq!(before, after);
}

#[test]
fn image_extraction_contract_and_orientation_changes_do_not_reuse_incompatible_ocr() {
    let fixture = Fixture::new();
    let first = fixture.import(1, 0, None);
    let rotated = fixture.import(1, 1, None);
    let mut calls = 0;
    for (member, contract) in [(&first.members[0], "v1"), (&first.members[0], "v2"), (&rotated.members[0], "v1")] {
        let saved = get_or_extract(Some(&fixture.database), member, Some(contract), || {
            calls += 1;
            Ok(result(member))
        }).unwrap();
        assert!(!saved.reused);
    }
    assert_eq!(calls, 3);
    assert_eq!(fixture.count("document_image_extractions"), 3);
    fixture.database.with_connection(|connection| validate(connection)).unwrap();
}

#[test]
fn image_extraction_failure_preserves_successful_siblings_and_does_not_store_failed_results() {
    let fixture = Fixture::new();
    let manifest = fixture.import(2, 0, None);
    get_or_extract(Some(&fixture.database), &manifest.members[0], Some("v1"), || Ok(result(&manifest.members[0]))).unwrap();
    let error = get_or_extract(Some(&fixture.database), &manifest.members[1], Some("v2"), || {
        Err(CommandError::new("fixture_ocr_failure", "Synthetic OCR failure"))
    }).err().unwrap();
    assert_eq!(error.code, "fixture_ocr_failure");
    assert_eq!(fixture.count("document_image_extractions"), 1);
    assert!(get_or_extract(Some(&fixture.database), &manifest.members[0], Some("v1"), || panic!("sibling was lost")).unwrap().reused);
}

#[test]
fn image_extraction_rejects_geometry_drift_and_a_source_deleted_during_ocr() {
    let fixture = Fixture::new();
    let manifest = fixture.import(1, 0, None);
    let member = &manifest.members[0];
    let mut wrong = result(member);
    wrong.width += 1;
    assert!(get_or_extract(Some(&fixture.database), member, Some("v1"), || Ok(wrong)).is_err());
    let error = get_or_extract(Some(&fixture.database), member, Some("v1"), || {
        fixture.database.with_connection(|connection| {
            connection.execute("DELETE FROM documents WHERE id = ?1", [&manifest.document_id]).map_err(CommandError::database)?;
            Ok(())
        })?;
        Ok(result(member))
    }).err().unwrap();
    assert_eq!(error.code, "image_set_changed");
    assert_eq!(fixture.count("document_image_extractions"), 0);
}

#[test]
fn image_extraction_entry_refuses_altered_payloads_before_any_write() {
    let fixture = Fixture::new();
    let manifest = fixture.import(1, 0, None);
    get_or_extract(Some(&fixture.database), &manifest.members[0], Some("v1"), || Ok(result(&manifest.members[0]))).unwrap();
    fixture.database.with_connection(|connection| {
        connection.execute("UPDATE document_image_extractions SET result_json = '{}'", []).map_err(CommandError::database)?;
        let changes = connection.total_changes();
        assert!(crate::database::validate_backup_lineage(connection).is_err());
        assert_eq!(changes, connection.total_changes());
        Ok(())
    }).unwrap();
}

#[test]
fn image_extraction_without_a_verified_runtime_contract_never_claims_reuse() {
    let fixture = Fixture::new();
    let manifest = fixture.import(1, 0, None);
    let mut calls = 0;
    for _ in 0..2 {
        let value = get_or_extract(Some(&fixture.database), &manifest.members[0], None, || {
            calls += 1;
            Ok(result(&manifest.members[0]))
        }).unwrap();
        assert!(!value.reused);
    }
    assert_eq!(calls, 2);
    assert_eq!(fixture.count("document_image_extractions"), 0);
}
