use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
use rusqlite::params;

use crate::{
    database::Database,
    document_canonical::sha256_hex,
    error::CommandError,
    paths::AppPaths,
};

use super::{
    files::{self, InitialSource},
    import::{self, ImageSelectionInput, ImportImageSetInput},
    manifest::{self, ImageManifest, InclusionCrop},
    repository,
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    paths: AppPaths,
    database: Database,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ostudy-image-set-retention-{}-{}-{}",
            std::process::id(),
            crate::util::unix_time_millis().unwrap(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        let paths = AppPaths::new(root.clone()).unwrap();
        let database = Database::open_in_memory().unwrap();
        Self { root, paths, database }
    }

    fn stage(&self, number: u64, data: &[u8]) -> String {
        let id = format!("{number:032x}");
        let relative = AppPaths::staged_import_relative_path(&id, "png");
        fs::write(self.paths.resolve_relative(&relative), data).unwrap();
        self.database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO staged_external_imports (
                     id, operation_id, purpose, detected_kind, display_name, media_type,
                     byte_size, sha256, relative_path, state, result_json, created_at, updated_at, expires_at
                 ) VALUES (?1, ?1, 'image', 'png', 'same.png', 'image/png', ?2, ?3, ?4,
                           'staged', '', 1, 1, 9223372036854775807)",
                params![id, data.len() as i64, sha256_hex(data), relative],
            ).map_err(CommandError::database)?;
            Ok(())
        }).unwrap();
        id
    }

    fn request(&self) -> ImportImageSetInput {
        let bytes = png();
        let a = self.stage(1, &bytes);
        let b = self.stage(2, &bytes);
        ImportImageSetInput {
            operation_id: "d".repeat(32),
            title: "Ordered screenshots".to_string(),
            members: vec![
                ImageSelectionInput { staged_file_id: b, quarter_turns: 1, inclusion: None },
                ImageSelectionInput {
                    staged_file_id: a,
                    quarter_turns: 0,
                    inclusion: Some(InclusionCrop { left: 0.0, top: 0.0, right: 0.5, bottom: 1.0 }),
                },
            ],
        }
    }

    fn load(&self, id: &str) -> ImageManifest {
        self.database.with_connection(|connection| repository::load(connection, id)).unwrap()
    }

    fn count(&self, table: &str) -> i64 {
        self.database.with_connection(|connection| {
            connection.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
                .map_err(CommandError::database)
        }).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    let pixels = [55_u8; 4 * 4 * 4];
    PngEncoder::new(&mut bytes).write_image(&pixels, 4, 4, ExtendedColorType::Rgba8).unwrap();
    bytes
}

#[test]
fn image_set_import_is_one_atomic_owned_document_and_retry_is_read_only() {
    let fixture = Fixture::new();
    let request = fixture.request();
    let result = import::import(&fixture.database, &fixture.paths, &request).unwrap();
    assert_eq!(result.revision, 1);
    assert_eq!(fixture.count("documents"), 1);
    assert_eq!(fixture.count("document_image_sets"), 1);
    assert_eq!(fixture.count("document_image_members"), 2);
    assert_eq!(fixture.count("document_source_assets"), 0, "originals must not leak into model-visible assets");
    let manifest = fixture.load(&result.document_id);
    assert_ne!(manifest.members[0].id, manifest.members[1].id);
    assert_ne!(manifest.members[0].relative_path, manifest.members[1].relative_path);
    assert_eq!(manifest.members[0].sha256, manifest.members[1].sha256);
    assert_eq!(manifest.members[0].quarter_turns, 1);
    assert_eq!(manifest.members[1].inclusion, request.members[1].inclusion);
    for member in &manifest.members {
        assert_eq!(fs::read(fixture.paths.resolve_relative(&member.relative_path)).unwrap(), png());
    }
    let manifest_path = manifest::manifest_path(&manifest.document_id, &manifest.fingerprint().unwrap()).unwrap();
    assert_eq!(fs::read(fixture.paths.resolve_relative(&manifest_path)).unwrap(), manifest.bytes().unwrap());
    let before = fixture.database.with_connection(|connection| {
        repository::validate(connection)?;
        for member in &request.members {
            let state: String = connection.query_row(
                "SELECT state FROM staged_external_imports WHERE id = ?1", [&member.staged_file_id], |row| row.get(0),
            ).map_err(CommandError::database)?;
            assert_eq!(state, "consumed");
        }
        Ok(connection.total_changes())
    }).unwrap();
    // The operation receipt, not the continued existence of the staging files,
    // is authoritative when a command acknowledgement was lost.
    for selection in &request.members {
        fs::remove_file(fixture.paths.resolve_relative(&AppPaths::staged_import_relative_path(&selection.staged_file_id, "png"))).unwrap();
    }
    assert_eq!(import::import(&fixture.database, &fixture.paths, &request).unwrap(), result);
    let after = fixture.database.with_connection(|connection| Ok(connection.total_changes())).unwrap();
    assert_eq!(before, after);
    assert_eq!(fixture.count("documents"), 1);
}

#[test]
fn image_set_operation_reuse_cannot_change_inputs_or_resurrect_a_deleted_source() {
    let fixture = Fixture::new();
    let mut request = fixture.request();
    let result = import::import(&fixture.database, &fixture.paths, &request).unwrap();
    request.title = "Different".to_string();
    assert_eq!(import::import(&fixture.database, &fixture.paths, &request).unwrap_err().code, "image_set_operation_conflict");
    request.title = "Ordered screenshots".to_string();
    fixture.database.with_connection(|connection| {
        connection.execute("DELETE FROM documents WHERE id = ?1", [&result.document_id]).map_err(CommandError::database)?;
        Ok(())
    }).unwrap();
    assert_eq!(fixture.count("document_image_members"), 0);
    assert_eq!(fixture.count("document_image_imports"), 1);
    assert_eq!(import::import(&fixture.database, &fixture.paths, &request).unwrap_err().code, "image_set_import_removed");
    assert_eq!(fixture.count("documents"), 0);
}

#[test]
fn image_set_failed_member_and_sql_failure_do_not_publish_a_shortened_set() {
    let fixture = Fixture::new();
    let mut request = fixture.request();
    request.members[1].staged_file_id = fixture.stage(3, b"not a PNG");
    assert_eq!(import::import(&fixture.database, &fixture.paths, &request).unwrap_err().code, "image_set_selection_failed");
    assert_eq!(fixture.count("documents"), 0);
    request.members[1].staged_file_id = format!("{:032x}", 1);
    fixture.database.with_connection(|connection| {
        connection.execute_batch(
            "CREATE TRIGGER reject_second_image BEFORE INSERT ON document_image_members
             WHEN NEW.position = 1 BEGIN SELECT RAISE(ABORT, 'fixture interrupted insert'); END;",
        ).map_err(CommandError::database)
    }).unwrap();
    assert!(import::import(&fixture.database, &fixture.paths, &request).is_err());
    assert_eq!(fixture.count("documents"), 0);
    assert_eq!(fixture.count("document_image_members"), 0);
    assert_eq!(fixture.count("document_image_imports"), 0);
    assert_eq!(fs::read_dir(fixture.paths.resolve_relative("documents")).unwrap().count(), 0);
    fixture.database.with_connection(|connection| {
        let consumed: i64 = connection.query_row("SELECT COUNT(*) FROM staged_external_imports WHERE state = 'consumed'", [], |row| row.get(0)).map_err(CommandError::database)?;
        assert_eq!(consumed, 0);
        connection.execute_batch("DROP TRIGGER reject_second_image").map_err(CommandError::database)
    }).unwrap();
    import::import(&fixture.database, &fixture.paths, &request).unwrap();
    assert_eq!(fixture.count("documents"), 1);
}

#[test]
fn image_set_manifest_separates_member_identity_from_order_and_rejects_unsupported_data() {
    let fixture = Fixture::new();
    let result = import::import(&fixture.database, &fixture.paths, &fixture.request()).unwrap();
    let saved = fixture.load(&result.document_id);
    assert_eq!(ImageManifest::decode(&saved.bytes().unwrap()).unwrap(), saved);
    let mut reordered = saved.clone();
    reordered.members.reverse();
    assert_ne!(saved.fingerprint().unwrap(), reordered.fingerprint().unwrap());
    assert_eq!(reordered.members[0], saved.members[1]);
    assert_eq!(reordered.members[1], saved.members[0]);
    let mut duplicate = saved.clone();
    duplicate.members[1] = duplicate.members[0].clone();
    assert!(duplicate.validate().is_err());
    let mut future = saved.clone();
    future.version = 2;
    assert!(future.validate().is_err());
    let mut path = saved.clone();
    path.members[0].relative_path = "../source.png".to_string();
    assert!(path.validate().is_err());
    let mut missing = saved.clone();
    missing.members.clear();
    assert!(missing.validate().is_err());
    let mut value = serde_json::to_value(&saved).unwrap();
    value["guessedSourceUrl"] = serde_json::json!("https://invalid.example");
    assert!(ImageManifest::decode(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn image_set_inclusion_crops_fail_closed_without_clamping_or_rounding() {
    let full = InclusionCrop { left: 0.0, top: 0.0, right: 1.0, bottom: 1.0 };
    full.validate().unwrap();
    for crop in [
        InclusionCrop { left: -0.1, ..full },
        InclusionCrop { top: f64::NAN, ..full },
        InclusionCrop { right: f64::INFINITY, ..full },
        InclusionCrop { bottom: 1.1, ..full },
        InclusionCrop { left: 1.0, ..full },
        InclusionCrop { top: 1.0, ..full },
    ] {
        assert!(crop.validate().is_err());
    }
}

#[test]
fn image_set_entry_and_restore_admission_reject_forged_manifests_without_writes() {
    let fixture = Fixture::new();
    import::import(&fixture.database, &fixture.paths, &fixture.request()).unwrap();
    fixture.database.with_connection(|connection| {
        for mutation in [
            "UPDATE document_image_members SET inclusion_json = '{\"left\":0,\"top\":0,\"right\":2,\"bottom\":1}'",
            "UPDATE document_image_members SET pixel_contract = 'future'",
            "UPDATE document_image_sets SET manifest_sha256 = printf('%064d', 0)",
            "UPDATE documents SET source_relative_path = 'documents/other/source.imageset'",
        ] {
            let transaction = connection.transaction().map_err(CommandError::database)?;
            transaction.execute_batch(mutation).map_err(CommandError::database)?;
            let changes = transaction.total_changes();
            assert!(crate::database::validate_backup_lineage(&transaction).is_err());
            assert_eq!(transaction.total_changes(), changes);
            transaction.rollback().map_err(CommandError::database)?;
        }
        Ok(())
    }).unwrap();
}

#[test]
fn image_set_crash_recovery_removes_only_native_uncommitted_candidates() {
    let fixture = Fixture::new();
    let result = import::import(&fixture.database, &fixture.paths, &fixture.request()).unwrap();
    let saved = fixture.load(&result.document_id);
    let mut abandoned = saved.clone();
    abandoned.document_id = "c".repeat(32);
    for member in &mut abandoned.members {
        member.relative_path = manifest::original_path(&abandoned.document_id, &member.id, &member.media_type).unwrap();
    }
    let guard = InitialSource::begin(&fixture.paths, &abandoned, &"e".repeat(32)).unwrap();
    guard.write(&fixture.paths, &abandoned.members[0].relative_path, &png()).unwrap();
    assert!(guard.write(&fixture.paths, &format!("documents/{}/../escaped.png", abandoned.document_id), b"escape").is_err());
    std::mem::forget(guard); // A process interruption does not run Rust destructors.
    let unknown = fixture.paths.resolve_relative(&format!("documents/{}/keep.txt", "f".repeat(32)));
    fs::create_dir_all(unknown.parent().unwrap()).unwrap();
    fs::write(&unknown, b"unrelated useful data").unwrap();
    let marker = fixture.paths.resolve_relative(&format!("documents/{}/image-set.pending", saved.document_id));
    fs::write(&marker, serde_json::to_vec(&serde_json::json!({
        "version": 1, "documentId": saved.document_id, "operationId": "d".repeat(32)
    })).unwrap()).unwrap();
    files::reconcile_pending(&fixture.database, &fixture.paths).unwrap();
    assert!(!fixture.paths.resolve_relative(&format!("documents/{}", abandoned.document_id)).exists());
    assert!(!marker.exists());
    assert_eq!(fs::read(&unknown).unwrap(), b"unrelated useful data");
    for member in saved.members {
        assert_eq!(fs::read(fixture.paths.resolve_relative(&member.relative_path)).unwrap(), png());
    }
}

#[test]
fn image_set_missing_original_is_explicit_and_does_not_erase_other_members() {
    let fixture = Fixture::new();
    let result = import::import(&fixture.database, &fixture.paths, &fixture.request()).unwrap();
    let saved = fixture.load(&result.document_id);
    fs::remove_file(fixture.paths.resolve_relative(&saved.members[0].relative_path)).unwrap();
    files::reconcile_missing_originals(&fixture.database, &fixture.paths, 30).unwrap();
    files::reconcile_missing_originals(&fixture.database, &fixture.paths, 40).unwrap();
    fixture.database.with_connection(|connection| {
        let (state, error, revision): (String, String, i64) = connection.query_row(
            "SELECT lifecycle_state, safe_error_code, revision FROM documents WHERE id = ?1", [&result.document_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).map_err(CommandError::database)?;
        assert_eq!(state, "failed");
        assert_eq!(error, "image_set_original_missing");
        assert_eq!(revision, 2, "repeat detection must not create new source revisions");
        Ok(())
    }).unwrap();
    assert_eq!(fixture.load(&result.document_id), saved);
    assert_eq!(fs::read(fixture.paths.resolve_relative(&saved.members[1].relative_path)).unwrap(), png());
}
