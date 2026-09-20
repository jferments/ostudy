use super::*;

use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
use rusqlite::params;

use crate::{
    document_canonical::sha256_hex,
    document_ingestion::contracts::DocumentCancellation,
    documents::image_sets::{
        amend::{self, AmendImageSetInput, MemberEdit},
        import::ImageSelectionInput,
        manifest::{self, InclusionCrop},
        repository as image_repository,
        test_support::Fixture,
    },
    knowledge::{
        contracts::{KnowledgeKind, KnowledgeUnitInput, SourceRecord},
        evidence::{EvidenceCatalog, Grounding, Importance, EVIDENCE_VERSION},
        repository,
    },
    paths::AppPaths,
    pdf_ocr_backend::regions::{OcrLine, PixelBounds},
};

struct Active;
impl DocumentCancellation for Active {
    fn check(&self) -> Result<(), CommandError> {
        Ok(())
    }
}

fn png(value: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&[value; 8 * 6 * 3], 8, 6, ExtendedColorType::Rgb8)
        .unwrap();
    bytes
}

fn text(value: u8) -> String {
    format!("Source {value}: keep 5 mg, not 50 mg.")
}

fn extraction(fixture: &Fixture, manifest: &ImageManifest) -> (AdapterExtraction, usize) {
    let path = fixture.paths.resolve_relative(
        &manifest::manifest_path(&manifest.document_id, &manifest.fingerprint().unwrap()).unwrap(),
    );
    let mut calls = 0;
    let output = extract_with(
        Some(&fixture.database),
        AdapterRequest {
            paths: &fixture.paths,
            document_id: &manifest.document_id,
            source_path: &path,
            format: DocumentFormat::ImageBundle,
            maximum_source_units: 128,
            cancellation: &Active,
        },
        &mut |_| Ok(()),
        Some("amendment-fixture-v1"),
        |png| {
            calls += 1;
            let pixels = image::load_from_memory(png).unwrap().into_rgb8();
            let text = text(pixels.get_pixel(0, 0).0[0]);
            Ok(RegionOcrResult {
                version: 1,
                text: text.clone(),
                confidence: 0.95,
                lines: vec![OcrLine {
                    text,
                    bounds: Some(PixelBounds {
                        left: 0,
                        top: 1,
                        right: pixels.width(),
                        bottom: pixels.height() - 1,
                    }),
                    group: "body".to_string(),
                    confidence: Some(0.95),
                }],
                duration_millis: 1,
                engine: "fixture".to_string(),
                language: "eng".to_string(),
                width: pixels.width(),
                height: pixels.height(),
            })
        },
    )
    .unwrap();
    (output, calls)
}

fn publish(fixture: &Fixture, manifest: &ImageManifest, output: &AdapterExtraction) {
    crate::document_store::mark_document_processing(&fixture.database, &manifest.document_id)
        .unwrap();
    crate::document_ingestion::persist_prepared_image_set(
        &fixture.database,
        &manifest.document_id,
        output,
    )
    .unwrap();
}

fn current(fixture: &Fixture, id: &str) -> ImageManifest {
    fixture
        .database
        .with_connection(|connection| image_repository::load(connection, id))
        .unwrap()
}

fn input(manifest: &ImageManifest, operation: &str) -> AmendImageSetInput {
    AmendImageSetInput {
        operation_id: operation.repeat(32),
        document_id: manifest.document_id.clone(),
        expected_revision: manifest.revision,
        expected_filter_revision: 1,
        members: manifest
            .members
            .iter()
            .map(|member| MemberEdit {
                member_id: member.id.clone(),
                quarter_turns: member.quarter_turns,
                inclusion: member.inclusion,
            })
            .collect(),
        append: Vec::new(),
    }
}

fn stage(fixture: &Fixture, value: u8) -> ImageSelectionInput {
    let id = "e".repeat(32);
    let bytes = png(value);
    let path = AppPaths::staged_import_relative_path(&id, "png");
    std::fs::write(fixture.paths.resolve_relative(&path), &bytes).unwrap();
    fixture
        .database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO staged_external_imports (id, operation_id, purpose, detected_kind,
             display_name, media_type, byte_size, sha256, relative_path, state, result_json,
             created_at, updated_at, expires_at)
             VALUES (?1, ?1, 'image', 'png', 'appended.png', 'image/png', ?2, ?3, ?4,
                     'staged', '', 1, 1, 9223372036854775807)",
                    params![id, bytes.len() as i64, sha256_hex(&bytes), path],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    ImageSelectionInput {
        staged_file_id: id,
        quarter_turns: 0,
        inclusion: None,
    }
}

fn source(fixture: &Fixture, value: u8) -> SourceRecord {
    fixture.database.with_connection(|connection| {
        let json: String = connection.query_row(
            "SELECT record_json FROM knowledge_sources WHERE json_extract(record_json, '$.text') = ?1",
            [text(value)], |row| row.get(0),
        ).map_err(CommandError::database)?;
        serde_json::from_str(&json).map_err(CommandError::serialization)
    }).unwrap()
}

fn teach(fixture: &Fixture, source: &SourceRecord) -> String {
    let catalog = EvidenceCatalog::build(std::slice::from_ref(source), false).unwrap();
    repository::publish_units(
        &fixture.database,
        source,
        &"1".repeat(64),
        vec![KnowledgeUnitInput {
            title: "Stable image knowledge".to_string(),
            kind: KnowledgeKind::Fact,
            statement: source.text.clone(),
            qualifications: Vec::new(),
            topic: String::new(),
            supporting_excerpt: source.text.clone(),
            details: None,
            grounding: Some(Grounding {
                version: EVIDENCE_VERSION,
                importance: Importance::Core,
                evidence: catalog.entries,
            }),
        }],
    )
    .unwrap()[0]
        .id
        .clone()
}

fn assert_retained(fixture: &Fixture, source: &SourceRecord, unit: &str) {
    fixture
        .database
        .with_connection(|connection| {
            let current = repository::load_source(connection, &source.id)?.unwrap();
            assert_eq!(current.content_hash, source.content_hash);
            let units = repository::load_units(connection, &source.id)?;
            assert_eq!(units.len(), 1);
            assert_eq!(units[0].id, unit);
            crate::database::validate_backup_lineage(connection)
        })
        .unwrap();
}

#[test]
fn document_image_amendments_preserve_ocr_knowledge_and_originals_selectively() {
    let fixture = Fixture::new();
    let original = fixture.import_images(&[png(17), png(47)], 0, None);
    let (output, calls) = extraction(&fixture, &original);
    assert_eq!(calls, 2);
    publish(&fixture, &original, &output);
    let learned = source(&fixture, 17);
    let unit = teach(&fixture, &learned);
    let mut reorder = input(&original, "a");
    reorder.members.reverse();
    let saved = amend::apply(&fixture.database, &fixture.paths, &reorder).unwrap();
    assert_eq!(saved.revision, 2);
    fixture
        .database
        .with_connection(|connection| {
            let before = connection.total_changes();
            assert!(!storage::ready(connection, &original.document_id)?);
            assert!(repository::load_source(connection, &learned.id)?.is_none());
            crate::database::validate_backup_lineage(connection)?;
            assert_eq!(before, connection.total_changes());
            Ok(())
        })
        .unwrap();
    assert!(
        crate::document_ingestion::persist_prepared_image_set(
            &fixture.database,
            &original.document_id,
            &output,
        )
        .is_err(),
        "a stale extraction must not publish against an amended source"
    );
    let reordered = current(&fixture, &original.document_id);
    let (output, calls) = extraction(&fixture, &reordered);
    assert_eq!(calls, 0);
    publish(&fixture, &reordered, &output);
    assert_retained(&fixture, &learned, &unit);
    assert_ne!(source(&fixture, 17).locator, learned.locator);
    let mut append = input(&reordered, "b");
    append.append.push(stage(&fixture, 79));
    amend::apply(&fixture.database, &fixture.paths, &append).unwrap();
    let appended = current(&fixture, &original.document_id);
    assert_eq!(appended.members.len(), 3);
    let (output, calls) = extraction(&fixture, &appended);
    assert_eq!(calls, 1, "only the novel source requires OCR");
    publish(&fixture, &appended, &output);
    assert_retained(&fixture, &learned, &unit);
    let mut rotate = input(&appended, "c");
    rotate.members[0].quarter_turns = 1;
    amend::apply(&fixture.database, &fixture.paths, &rotate).unwrap();
    let rotated = current(&fixture, &original.document_id);
    let (output, calls) = extraction(&fixture, &rotated);
    assert_eq!(calls, 1, "only the rotated member requires new OCR");
    publish(&fixture, &rotated, &output);
    assert_retained(&fixture, &learned, &unit);
    let mut crop = input(&rotated, "d");
    crop.members[0].inclusion = Some(InclusionCrop {
        left: 0.0,
        top: 0.0,
        right: 0.5,
        bottom: 1.0,
    });
    amend::apply(&fixture.database, &fixture.paths, &crop).unwrap();
    let cropped = current(&fixture, &original.document_id);
    let (output, calls) = extraction(&fixture, &cropped);
    assert_eq!(calls, 0, "an inclusion decision does not change source OCR");
    assert!(output.canonical.document.units[0].plain_text.is_empty());
    publish(&fixture, &cropped, &output);
    assert_retained(&fixture, &learned, &unit);
    assert_eq!(
        amend::apply(&fixture.database, &fixture.paths, &reorder).unwrap(),
        saved
    );
    assert_eq!(current(&fixture, &original.document_id), cropped);
    for (member, bytes) in original.members.iter().zip([png(17), png(47)]) {
        assert_eq!(
            std::fs::read(fixture.paths.resolve_relative(&member.relative_path)).unwrap(),
            bytes
        );
    }
}

#[test]
fn document_image_amendment_rejection_and_sql_failure_do_not_publish_partial_sources() {
    let fixture = Fixture::new();
    let original = fixture.import_images(&[png(17), png(47)], 0, None);
    let mut edit = input(&original, "a");
    edit.members[0].member_id = "f".repeat(32);
    assert!(amend::apply(&fixture.database, &fixture.paths, &edit).is_err());
    edit = input(&original, "a");
    edit.members.reverse();
    edit.append.push(stage(&fixture, 79));
    fixture
        .database
        .with_connection(|connection| {
            connection
                .execute_batch(
                    "CREATE TRIGGER fixture_amendment_failure BEFORE UPDATE ON document_image_sets
             BEGIN SELECT RAISE(ABORT, 'fixture amendment failure'); END;",
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    assert!(amend::apply(&fixture.database, &fixture.paths, &edit).is_err());
    assert_eq!(current(&fixture, &original.document_id), original);
    assert_eq!(fixture.count("document_image_edit_receipts"), 0);
    assert_eq!(fixture.count("document_image_members"), 2);
    let originals = fixture
        .paths
        .resolve_relative(&format!("documents/{}/originals", original.document_id));
    assert_eq!(std::fs::read_dir(originals).unwrap().count(), 2);
    fixture
        .database
        .with_connection(|connection| {
            let state: String = connection
                .query_row(
                    "SELECT state FROM staged_external_imports WHERE id = ?1",
                    ["e".repeat(32)],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(state, "staged");
            connection
                .execute_batch("DROP TRIGGER fixture_amendment_failure")
                .map_err(CommandError::database)?;
            crate::database::validate_backup_lineage(connection)
        })
        .unwrap();
    let saved = amend::apply(&fixture.database, &fixture.paths, &edit).unwrap();
    let mut stale = edit.clone();
    stale.operation_id = "b".repeat(32);
    assert!(amend::apply(&fixture.database, &fixture.paths, &stale).is_err());
    assert_eq!(
        amend::apply(&fixture.database, &fixture.paths, &edit).unwrap(),
        saved
    );
}
