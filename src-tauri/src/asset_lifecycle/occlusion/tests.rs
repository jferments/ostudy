use std::sync::atomic::{AtomicU64, Ordering};

use image::{codecs::jpeg::JpegEncoder, DynamicImage, ExtendedColorType, ImageEncoder, Rgba, RgbaImage};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::*;

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Fixture {
    database: Database,
    paths: AppPaths,
    draft_id: String,
    _cleanup: Cleanup,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ostudy-occlusion-import-{}-{}-{}",
            std::process::id(), unix_time_millis().unwrap(), NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        let paths = AppPaths::new(root.clone()).unwrap();
        let database = Database::open(paths.database_path()).unwrap();
        let draft = database.with_connection(|connection| begin_draft_in_connection(connection, "card", None)).unwrap();
        Self { database, paths, draft_id: draft.id, _cleanup: Cleanup(root) }
    }

    fn run<T>(&self, work: impl FnOnce(&mut Connection) -> Result<T, CommandError>) -> T {
        self.database.with_connection(work).unwrap()
    }

    fn stage(&self, bytes: &[u8], kind: &str, extension: &str) -> String {
        self.run(|connection| {
            let id = generate_id(connection)?;
            let relative = AppPaths::staged_import_relative_path(&id, extension);
            fs::write(self.paths.resolve_relative(&relative), bytes).map_err(CommandError::io)?;
            let now = unix_time_millis()?;
            connection.execute(
                "INSERT INTO staged_external_imports (
                    id, operation_id, purpose, detected_kind, display_name, media_type,
                    byte_size, sha256, relative_path, state, created_at, updated_at, expires_at
                 ) VALUES (?1, ?1, 'image', ?2, 'private-answer-name', ?3,
                    ?4, ?5, ?6, 'staged', ?7, ?7, ?8)",
                params![id, kind, format!("image/{kind}"), bytes.len() as i64,
                    format!("{:x}", Sha256::digest(bytes)), relative, now, now + 86_400_000],
            ).map_err(CommandError::database)?;
            Ok(id)
        })
    }

    fn prepare(&self, id: &str) -> Result<PreparedOcclusionImage, CommandError> {
        preparation::prepare(&self.database, &self.paths, ImportImageInput {
            staged_file_id: id.to_string(), draft_id: self.draft_id.clone(),
        })
    }

    fn count_assets(&self) -> i64 {
        self.run(|connection| connection.query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))
            .map_err(CommandError::database))
    }

    fn preview(&self, image: &PreparedOcclusionImage, revealed: bool, draft_id: Option<String>) -> Result<Vec<u8>, CommandError> {
        preview(&self.database, &self.paths, PreviewOcclusionInput {
            definition_json: definition(&image.asset.id).to_string(),
            presentation_key: "4".repeat(32), asset_draft_id: draft_id, revealed, maximum_edge: 128,
        })
    }
}

fn png() -> Vec<u8> {
    let image = RgbaImage::from_pixel(12, 7, Rgba([180, 80, 30, 255]));
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(image.as_raw(), 12, 7, ExtendedColorType::Rgba8).unwrap();
    png
}

fn definition(asset_id: &str) -> Value {
    let rich = |text: &str| json!({"type":"doc","content":[{"type":"paragraph","content":[
        {"type":"text","text":text}]}]});
    json!({"schemaVersion":1,"question":{"kind":"image_occlusion","imageAssetId":asset_id,
        "masks":[{"id":"2".repeat(32),"rect":{"x":0.1,"y":0.1,"width":0.2,"height":0.2}}],
        "questions":[{"id":"4".repeat(32),"prompt":rich("Identify the region."),
            "answer":rich("Private answer"),"explanation":null,"maskIds":["2".repeat(32)],"mode":"hide_target"}]}})
}

#[test]
fn native_preparation_stages_one_canonical_png_and_replays_after_source_removal() {
    let fixture = Fixture::new();
    let staged = fixture.stage(&png(), "png", "png");
    let first = fixture.prepare(&staged).unwrap();
    assert_eq!((first.width, first.height), (12, 7));
    assert_eq!(first.asset.mime_type, "image/png");
    assert_eq!(first.asset.original_filename, "occlusion-source.png");
    assert_eq!(first.pixel_fingerprint.len(), 64);
    fs::remove_file(fixture.paths.resolve_relative(&AppPaths::staged_import_relative_path(&staged, "png"))).unwrap();
    let replay = fixture.prepare(&staged).unwrap();
    assert_eq!(serde_json::to_value(&first).unwrap(), serde_json::to_value(replay).unwrap());
    assert_eq!(fixture.count_assets(), 1);
    assert_eq!(fs::read_dir(fixture.paths.asset_staging_dir()).unwrap().count(), 1);
    let reopened = Database::open(fixture.paths.database_path()).unwrap();
    let replay = preparation::prepare(&reopened, &fixture.paths, ImportImageInput {
        staged_file_id: staged, draft_id: fixture.draft_id.clone(),
    }).unwrap();
    assert_eq!(first.asset.id, replay.asset.id);
}

#[test]
fn jpeg_orientation_is_normalized_before_any_editor_can_place_masks() {
    let fixture = Fixture::new();
    let rgb = DynamicImage::ImageRgba8(RgbaImage::from_pixel(12, 7, Rgba([40, 90, 170, 255]))).into_rgb8();
    let mut jpeg = Vec::new();
    JpegEncoder::new(&mut jpeg).write_image(rgb.as_raw(), 12, 7, ExtendedColorType::Rgb8).unwrap();
    let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
    let mut oriented = jpeg[..2].to_vec();
    oriented.extend_from_slice(b"\xff\xe1");
    oriented.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    oriented.extend_from_slice(exif);
    oriented.extend_from_slice(&jpeg[2..]);
    let staged = fixture.stage(&oriented, "jpeg", "jpg");
    let result = fixture.prepare(&staged).unwrap();
    assert_eq!((result.width, result.height), (7, 12));
    let file = fixture.run(|connection| open_managed_image(connection, &fixture.paths, &result.asset.id, Some(&fixture.draft_id)));
    let bytes = read_image(file).unwrap();
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(CanonicalImage::decode(&bytes).unwrap().dimensions(), (7, 12));
}

#[test]
fn a_consumption_failure_rolls_back_the_receipt_asset_and_staged_png() {
    let fixture = Fixture::new();
    let staged = fixture.stage(&png(), "png", "png");
    fixture.run(|connection| connection.execute_batch(
        "CREATE TRIGGER reject_occlusion_import BEFORE UPDATE ON staged_external_imports
         BEGIN SELECT RAISE(ABORT, 'injected consume failure'); END;"
    ).map_err(CommandError::database));
    assert!(fixture.prepare(&staged).is_err());
    assert_eq!(fixture.count_assets(), 0);
    assert_eq!(fs::read_dir(fixture.paths.asset_staging_dir()).unwrap().count(), 0);
    fixture.run(|connection| connection.execute_batch("DROP TRIGGER reject_occlusion_import;").map_err(CommandError::database));
    fixture.prepare(&staged).unwrap();
    assert_eq!(fixture.count_assets(), 1);
}

#[test]
fn malformed_image_and_canceled_draft_cannot_leave_a_normalized_asset() {
    let fixture = Fixture::new();
    let bad = fixture.stage(b"\x89PNG\r\n\x1a\nnot-a-png", "png", "png");
    assert!(fixture.prepare(&bad).is_err());
    let staged = fixture.stage(&png(), "png", "png");
    fixture.run(|connection| {
        connection.execute("UPDATE asset_drafts SET canceled_at = 1 WHERE id = ?1", [&fixture.draft_id])
            .map_err(CommandError::database)?;
        Ok(())
    });
    assert_eq!(fixture.prepare(&staged).unwrap_err().code, "asset_draft_canceled");
    assert_eq!(fixture.count_assets(), 0);
    assert_eq!(fs::read_dir(fixture.paths.asset_staging_dir()).unwrap().count(), 0);
}

#[test]
fn a_consumed_handle_cannot_be_reused_for_a_different_draft_or_import_kind() {
    let fixture = Fixture::new();
    let staged = fixture.stage(&png(), "png", "png");
    fixture.prepare(&staged).unwrap();
    let other = fixture.run(|connection| begin_draft_in_connection(connection, "card", None));
    let error = preparation::prepare(&fixture.database, &fixture.paths, ImportImageInput {
        staged_file_id: staged, draft_id: other.id,
    }).unwrap_err();
    assert_eq!(error.code, "occlusion_import_conflict");
    let normal = fixture.stage(&png(), "png", "png");
    stage_image(&fixture.database, &fixture.paths, ImportImageInput {
        staged_file_id: normal.clone(), draft_id: fixture.draft_id.clone(),
    }).unwrap();
    assert_eq!(fixture.prepare(&normal).unwrap_err().code, "occlusion_import_conflict");
    assert_eq!(fixture.count_assets(), 2);
}

#[test]
fn authoring_preview_masks_pixels_and_requires_staged_image_ownership() {
    let fixture = Fixture::new();
    let staged = fixture.stage(&png(), "png", "png");
    let prepared = fixture.prepare(&staged).unwrap();
    let hidden = fixture.preview(&prepared, false, Some(fixture.draft_id.clone())).unwrap();
    let revealed = fixture.preview(&prepared, true, Some(fixture.draft_id.clone())).unwrap();
    assert_ne!(hidden, revealed);
    assert!(!hidden.windows(7).any(|bytes| bytes == b"Private"));
    assert_eq!(CanonicalImage::decode(&hidden).unwrap().dimensions(), (12, 7));
    assert!(fixture.preview(&prepared, false, None).is_err());
    assert!(fixture.preview(&prepared, false, Some("a".repeat(32))).is_err());
    assert_eq!(fixture.count_assets(), 1);
    assert_eq!(fs::read_dir(fixture.paths.asset_staging_dir()).unwrap().count(), 1);
}

#[test]
fn preview_never_falls_back_to_the_original_when_a_target_or_file_is_missing() {
    let fixture = Fixture::new();
    let staged = fixture.stage(&png(), "png", "png");
    let prepared = fixture.prepare(&staged).unwrap();
    let error = preview(&fixture.database, &fixture.paths, PreviewOcclusionInput {
        definition_json: definition(&prepared.asset.id).to_string(),
        presentation_key: "0".repeat(32), asset_draft_id: Some(fixture.draft_id.clone()),
        revealed: false, maximum_edge: 128,
    }).unwrap_err();
    assert_eq!(error.code, "invalid_question");
    fs::remove_file(fixture.paths.resolve_relative(&AppPaths::staged_asset_relative_path(
        &fixture.draft_id, &prepared.asset.id, "png",
    ))).unwrap();
    assert!(fixture.preview(&prepared, false, Some(fixture.draft_id.clone())).is_err());
}

#[test]
fn reading_a_tampered_ledger_path_or_changed_file_size_is_rejected() {
    let fixture = Fixture::new();
    let staged = fixture.stage(&png(), "png", "png");
    let prepared = fixture.prepare(&staged).unwrap();
    fixture.run(|connection| {
        connection.execute("UPDATE assets SET staged_relative_path = '../outside.png' WHERE id = ?1", [&prepared.asset.id])
            .map_err(CommandError::database)?;
        Ok(())
    });
    assert!(fixture.preview(&prepared, false, Some(fixture.draft_id.clone())).is_err());
    fixture.run(|connection| {
        connection.execute("UPDATE assets SET staged_relative_path = ?1, byte_size = byte_size + 1 WHERE id = ?2",
            params![AppPaths::staged_asset_relative_path(&fixture.draft_id, &prepared.asset.id, "png"), prepared.asset.id])
            .map_err(CommandError::database)?;
        Ok(())
    });
    assert!(fixture.preview(&prepared, false, Some(fixture.draft_id.clone())).is_err());
}
