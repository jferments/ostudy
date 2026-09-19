use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
use rusqlite::params;

use super::*;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    paths: AppPaths,
    database: Database,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ostudy-image-set-admission-{}-{}-{}",
            std::process::id(),
            crate::util::unix_time_millis().unwrap(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        let paths = AppPaths::new(root.clone()).unwrap();
        let database = Database::open_in_memory().unwrap();
        Self {
            root,
            paths,
            database,
        }
    }

    fn stage(&self, number: u64, bytes: &[u8]) -> String {
        let id = format!("{number:032x}");
        let relative = AppPaths::staged_import_relative_path(&id, "png");
        fs::write(self.paths.resolve_relative(&relative), bytes).unwrap();
        self.database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO staged_external_imports (
                             id, operation_id, purpose, detected_kind, display_name, media_type,
                             byte_size, sha256, relative_path, state, result_json,
                             created_at, updated_at, expires_at
                         ) VALUES (?1, ?1, 'image', 'png', 'same.png', 'image/png',
                                   ?2, ?3, ?4, 'staged', '', 1, 1, 9223372036854775807)",
                        params![id, bytes.len() as i64, sha256_hex(bytes), relative],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();
        id
    }

    fn changes(&self) -> u64 {
        self.database
            .with_connection(|connection| Ok(connection.total_changes()))
            .unwrap()
    }

    fn inspect(&self, ids: &[String]) -> ImageSetInspection {
        inspect(&self.database, &self.paths, ids).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn png(value: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&[value, 19, 38, 255], 1, 1, ExtendedColorType::Rgba8)
        .unwrap();
    bytes
}

#[test]
fn image_set_inspection_preserves_order_ownership_bytes_and_database() {
    let fixture = Fixture::new();
    let original = png(51);
    let a = fixture.stage(1, &original);
    let b = fixture.stage(2, &original);
    let c = fixture.stage(3, &png(72));
    let order = vec![b.clone(), c.clone(), a.clone()];
    let before = fixture.changes();
    let result = fixture.inspect(&order);
    assert!(result.can_import);
    assert_eq!(
        result
            .members
            .iter()
            .map(|member| &member.id)
            .collect::<Vec<_>>(),
        order.iter().collect::<Vec<_>>()
    );
    assert_eq!(result.decoded_images, 2);
    assert_eq!(result.reused_images, 1);
    assert_eq!(
        result.members[0].image.as_ref().unwrap().pixel_sha256,
        result.members[2].image.as_ref().unwrap().pixel_sha256
    );
    assert_ne!(result.members[0].id, result.members[2].id);
    assert_eq!(
        fixture.changes(),
        before,
        "inspection must not consume or rewrite leases"
    );
    let original_path = fixture
        .paths
        .resolve_relative(&AppPaths::staged_import_relative_path(&a, "png"));
    assert_eq!(fs::read(original_path).unwrap(), original);
    fixture
        .database
        .with_connection(|connection| {
            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(count, 0, "read-only preview must not import a document");
            Ok(())
        })
        .unwrap();
}

#[test]
fn image_set_bad_selection_does_not_hide_successful_siblings() {
    let fixture = Fixture::new();
    let a = fixture.stage(1, &png(51));
    let bad = fixture.stage(2, b"not an image");
    let b = fixture.stage(3, &png(80));
    let result = fixture.inspect(&[a, bad.clone(), b]);
    assert!(!result.can_import);
    assert_eq!(result.members.len(), 3);
    assert!(result.members[0].image.is_some());
    assert!(result.members[1].image.is_none());
    assert_eq!(result.members[1].id, bad);
    assert!(result.members[1].error.is_some());
    assert!(result.members[2].image.is_some());
}

#[test]
fn image_set_released_consumed_wrong_purpose_and_forged_paths_are_not_admitted() {
    let fixture = Fixture::new();
    let ids = (1..=5)
        .map(|index| fixture.stage(index, &png(51)))
        .collect::<Vec<_>>();
    fixture
        .database
        .with_connection(|connection| {
            for (index, mutation) in [
                "state = 'released'",
                "state = 'consumed'",
                "purpose = 'study_content'",
                "relative_path = 'transfers/imports/wrong.png'",
            ]
            .iter()
            .enumerate()
            {
                connection
                    .execute(
                        &format!("UPDATE staged_external_imports SET {mutation} WHERE id = ?1"),
                        [&ids[index]],
                    )
                    .map_err(CommandError::database)?;
            }
            Ok(())
        })
        .unwrap();
    let before = fixture.changes();
    let result = fixture.inspect(&ids);
    assert!(!result.can_import);
    for member in &result.members[..4] {
        assert!(member.image.is_none());
        assert!(member.error.is_some());
    }
    assert!(result.members[4].image.is_some());
    assert_eq!(fixture.changes(), before);
}

#[test]
fn image_set_changed_staged_bytes_and_mislabeled_containers_fail_closed() {
    let fixture = Fixture::new();
    let changed = fixture.stage(1, &png(51));
    let path = fixture
        .paths
        .resolve_relative(&AppPaths::staged_import_relative_path(&changed, "png"));
    fs::write(path, png(52)).unwrap();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
        .encode(&[10, 20, 30], 1, 1, ExtendedColorType::Rgb8)
        .unwrap();
    let mislabeled = fixture.stage(2, &jpeg);
    let result = fixture.inspect(&[changed, mislabeled]);
    assert!(!result.can_import);
    assert!(result
        .members
        .iter()
        .all(|member| member.error.is_some() && member.image.is_none()));
    assert_eq!(result.decoded_images, 0);
}

#[test]
fn image_set_selection_validation_refuses_ambiguity_and_unbounded_work() {
    let id = "a".repeat(32);
    assert!(validate_selection(&[]).is_err());
    assert!(validate_selection(&["not-an-id".to_string()]).is_err());
    assert!(validate_selection(&[id.clone(), id]).is_err());
    let maximum = (0..MAX_MEMBERS)
        .map(|value| format!("{value:032x}"))
        .collect::<Vec<_>>();
    validate_selection(&maximum).unwrap();
    let mut excessive = maximum;
    excessive.push(format!("{:032x}", MAX_MEMBERS));
    assert!(validate_selection(&excessive).is_err());
}

#[test]
fn image_set_budget_has_exact_limits_and_rejects_overflow_without_mutation() {
    let mut budget = Budget::default();
    budget
        .include(MAX_SET_BYTES - 1, MAX_SET_PIXELS - 1)
        .unwrap();
    assert!(budget.include(2, 1).is_err());
    assert_eq!(budget.bytes, MAX_SET_BYTES - 1);
    assert_eq!(budget.pixels, MAX_SET_PIXELS - 1);
    budget.include(1, 1).unwrap();
    assert!(budget.include(1, 0).is_err());
    assert!(budget.include(0, 1).is_err());
    assert!(budget.include(u64::MAX, u64::MAX).is_err());
    assert_eq!(budget.bytes, MAX_SET_BYTES);
    assert_eq!(budget.pixels, MAX_SET_PIXELS);
}

#[test]
fn image_set_oversized_ledger_and_missing_files_fail_before_decoding() {
    let fixture = Fixture::new();
    let a = fixture.stage(1, &png(51));
    let b = fixture.stage(2, &png(51));
    fixture
        .database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE staged_external_imports SET byte_size = ?1 WHERE id = ?2",
                    params![MAX_IMAGE_BYTES as i64 + 1, a],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    let missing_path = fixture
        .paths
        .resolve_relative(&AppPaths::staged_import_relative_path(&b, "png"));
    fs::remove_file(missing_path).unwrap();
    let result = fixture.inspect(&[a, b]);
    assert!(!result.can_import);
    assert_eq!(result.decoded_images, 0);
    assert!(result.members.iter().all(|member| member.error.is_some()));
}
