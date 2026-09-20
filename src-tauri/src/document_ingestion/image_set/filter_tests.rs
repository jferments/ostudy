use super::*;

use crate::{
    document_ingestion::contracts::{CompatibilityProjection, DocumentCancellation},
    documents::image_sets::{
        amend::{self, AmendImageSetInput, MemberEdit},
        filter::RegionOverride,
        filter_commands::{self, FilterChange, SetImageFiltersInput},
        manifest, repository as image_repository, storage,
        test_support::Fixture,
        view::{self, ImagePreviewInput},
    },
    knowledge::{
        contracts::{KnowledgeKind, KnowledgeUnitInput, SourceRecord},
        evidence::{EvidenceCatalog, Grounding, Importance, EVIDENCE_VERSION},
        repository,
    },
    pdf_ocr_backend::regions::{OcrLine, PixelBounds},
};

const BODY: &str = "Use 5 mg; never replace it with 50 mg.";
struct NotCanceled;
impl DocumentCancellation for NotCanceled {
    fn check(&self) -> Result<(), CommandError> {
        Ok(())
    }
}

fn recognition() -> RegionOcrResult {
    let lines = [
        ("Accept all cookies", 0, 1),
        (BODY, 2, 4),
        ("© Example author", 5, 6),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (text, top, bottom))| OcrLine {
        text: text.to_string(),
        bounds: Some(PixelBounds {
            left: 0,
            top,
            right: 8,
            bottom,
        }),
        group: index.to_string(),
        confidence: Some(0.95),
    })
    .collect::<Vec<_>>();
    RegionOcrResult {
        version: 1,
        text: lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        confidence: 0.95,
        lines,
        duration_millis: 1,
        engine: "fixture".to_string(),
        language: "eng".to_string(),
        width: 8,
        height: 6,
    }
}

fn rotated_recognition() -> RegionOcrResult {
    let lines = [
        ("Accept all cookies", 0, 1),
        (BODY, 2, 4),
        ("© Example author", 6, 7),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (text, top, bottom))| OcrLine {
        text: text.to_string(),
        bounds: Some(PixelBounds {
            left: 0,
            top,
            right: 6,
            bottom,
        }),
        group: index.to_string(),
        confidence: Some(0.95),
    })
    .collect::<Vec<_>>();
    RegionOcrResult {
        version: 1,
        text: lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        confidence: 0.95,
        lines,
        duration_millis: 1,
        engine: "fixture".to_string(),
        language: "eng".to_string(),
        width: 6,
        height: 8,
    }
}

fn extract(fixture: &Fixture, manifest: &ImageManifest, fresh: bool) -> AdapterExtraction {
    let path = fixture.paths.resolve_relative(
        &manifest::manifest_path(&manifest.document_id, &manifest.fingerprint().unwrap()).unwrap(),
    );
    extract_with(
        Some(&fixture.database),
        AdapterRequest {
            paths: &fixture.paths,
            document_id: &manifest.document_id,
            source_path: &path,
            format: DocumentFormat::ImageBundle,
            maximum_source_units: 128,
            cancellation: &NotCanceled,
        },
        &mut |_| Ok(()),
        Some("filter-fixture-v1"),
        |_| {
            assert!(fresh, "compatible saved OCR must not run again");
            Ok(recognition())
        },
    )
    .unwrap()
}

fn persist(fixture: &Fixture, manifest: &ImageManifest, output: &AdapterExtraction) {
    crate::document_store::mark_document_processing(&fixture.database, &manifest.document_id)
        .unwrap();
    crate::document_ingestion::persist_ingested_document(
        &fixture.database,
        &manifest.document_id,
        output,
    )
    .unwrap();
}

fn source(fixture: &Fixture, text: &str) -> SourceRecord {
    fixture
        .database
        .with_connection(|connection| {
            let json: String = connection.query_row(
                "SELECT record_json FROM knowledge_sources WHERE json_extract(record_json, '$.text') = ?1", [text], |row| row.get(0),
            ).map_err(CommandError::database)?;
            serde_json::from_str(&json).map_err(CommandError::serialization)
        })
        .unwrap()
}

fn teach(fixture: &Fixture, source: &SourceRecord) -> String {
    let catalog = EvidenceCatalog::build(std::slice::from_ref(source), false).unwrap();
    let units = repository::publish_units(
        &fixture.database,
        source,
        &"1".repeat(64),
        vec![KnowledgeUnitInput {
            title: "Retained fixture interpretation".to_string(),
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
    .unwrap();
    units[0].id.clone()
}

#[test]
fn document_image_filters_restore_exact_evidence_without_reocr_or_sibling_knowledge_loss() {
    let fixture = Fixture::new();
    let manifest = fixture.import(1, 0, None);
    let original = std::fs::read(
        fixture
            .paths
            .resolve_relative(&manifest.members[0].relative_path),
    )
    .unwrap();
    let output = extract(&fixture, &manifest, true);
    assert_eq!(output.canonical.document.units[0].plain_text, BODY);
    assert!(output
        .canonical
        .semantic_chunks
        .iter()
        .all(|chunk| !chunk.full_text.contains("cookies")));
    persist(&fixture, &manifest, &output);
    let saved_body = source(&fixture, BODY);
    let unit_id = teach(&fixture, &saved_body);
    let cookie = fixture
        .database
        .with_connection(|connection| {
            let receipt = storage::load(connection, &manifest.document_id)?.unwrap();
            assert_eq!(receipt.members[0].regions[2].text, "© Example author");
            Ok(receipt.members[0].regions[0].id.clone())
        })
        .unwrap();
    let input = SetImageFiltersInput {
        operation_id: "a".repeat(32),
        document_id: manifest.document_id.clone(),
        expected_revision: 1,
        expected_filter_revision: 1,
        include_interface: false,
        overrides: vec![FilterChange {
            region_id: cookie.clone(),
            action: Some(RegionOverride::Include),
        }],
    };
    let saved = filter_commands::apply(&fixture.database, &input).unwrap();
    assert_eq!(saved.filter_revision, 2);
    assert_eq!(
        filter_commands::apply(&fixture.database, &input).unwrap(),
        saved
    );
    fixture
        .database
        .with_connection(|connection| {
            assert!(!storage::ready(connection, &manifest.document_id)?);
            assert!(repository::load_source(connection, &saved_body.id)?.is_none());
            assert_eq!(
                repository::load_retained_source(connection, &saved_body.id)?
                    .unwrap()
                    .text,
                BODY
            );
            crate::database::validate_backup_lineage(connection)
        })
        .unwrap();
    assert_eq!(fixture.count("knowledge_units"), 1);
    let restored = extract(&fixture, &manifest, false);
    assert!(restored.canonical.document.units[0]
        .plain_text
        .contains("Accept all cookies"));
    persist(&fixture, &manifest, &restored);
    let current_body = source(&fixture, BODY);
    assert_eq!(saved_body.id, current_body.id);
    assert_eq!(saved_body.content_hash, current_body.content_hash);
    fixture
        .database
        .with_connection(|connection| {
            assert_eq!(
                repository::load_units(connection, &current_body.id)?[0].id,
                unit_id
            );
            crate::database::validate_backup_lineage(connection)
        })
        .unwrap();
    teach(&fixture, &source(&fixture, "Accept all cookies"));
    let reset = SetImageFiltersInput {
        operation_id: "b".repeat(32),
        expected_filter_revision: 2,
        overrides: vec![FilterChange {
            region_id: cookie,
            action: None,
        }],
        ..input
    };
    filter_commands::apply(&fixture.database, &reset).unwrap();
    let reset_output = extract(&fixture, &manifest, false);
    persist(&fixture, &manifest, &reset_output);
    assert_eq!(
        fixture.count("knowledge_units"),
        1,
        "only the excluded region's dependents are invalidated"
    );
    assert_eq!(fixture.count("document_image_extractions"), 1);
    assert_eq!(
        std::fs::read(
            fixture
                .paths
                .resolve_relative(&manifest.members[0].relative_path)
        )
        .unwrap(),
        original
    );
}

#[test]
fn document_image_filter_pixels_and_original_inspection_remain_separate() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let fixture = Fixture::new();
    let manifest = fixture.import(1, 0, None);
    let output = extract(&fixture, &manifest, true);
    let asset = &output.canonical.document.assets[0];
    let png = std::fs::read(fixture.paths.resolve_relative(&asset.relative_path)).unwrap();
    let filtered = image::load_from_memory(&png).unwrap().into_rgba8();
    assert_eq!(*filtered.get_pixel(0, 0), image::Rgba([255, 255, 255, 255]));
    assert_eq!(*filtered.get_pixel(0, 5), image::Rgba([255, 255, 255, 255]));
    let inspected = view::preview(
        &fixture.database,
        &fixture.paths,
        &ImagePreviewInput {
            document_id: manifest.document_id.clone(),
            member_id: manifest.members[0].id.clone(),
            expected_revision: 1,
            viewport: None,
        },
    )
    .unwrap();
    let bytes = STANDARD
        .decode(
            inspected
                .data_url
                .strip_prefix("data:image/png;base64,")
                .unwrap(),
        )
        .unwrap();
    let original = image::load_from_memory(&bytes).unwrap().into_rgba8();
    assert_ne!(original.get_pixel(0, 0), filtered.get_pixel(0, 0));
    assert_eq!(fixture.count("document_image_extractions"), 1);
    assert_eq!(
        fixture.count("document_image_settings"),
        0,
        "display viewport inspection is read-only"
    );
}

#[test]
fn document_image_filter_rejects_stale_results_forged_regions_and_failed_publication_atomically() {
    let fixture = Fixture::new();
    let manifest = fixture.import(1, 0, None);
    let output = extract(&fixture, &manifest, true);
    persist(&fixture, &manifest, &output);
    let mut input = SetImageFiltersInput {
        operation_id: "e".repeat(32),
        document_id: manifest.document_id.clone(),
        expected_revision: 1,
        expected_filter_revision: 1,
        include_interface: true,
        overrides: vec![FilterChange {
            region_id: "f".repeat(64),
            action: Some(RegionOverride::Include),
        }],
    };
    assert!(filter_commands::apply(&fixture.database, &input).is_err());
    assert_eq!(fixture.count("document_image_edit_receipts"), 0);
    input.overrides.clear();
    filter_commands::apply(&fixture.database, &input).unwrap();
    assert!(crate::document_ingestion::persist_ingested_document(
        &fixture.database,
        &manifest.document_id,
        &output
    )
    .is_err());
    let current = extract(&fixture, &manifest, false);
    fixture.database.with_connection(|connection| {
        connection.execute_batch("CREATE TRIGGER fixture_image_receipt_failure BEFORE UPDATE ON document_image_receipts BEGIN SELECT RAISE(ABORT, 'fixture publication failure'); END;")
            .map_err(CommandError::database)
    }).unwrap();
    crate::document_store::mark_document_processing(&fixture.database, &manifest.document_id)
        .unwrap();
    assert!(crate::document_ingestion::persist_ingested_document(
        &fixture.database,
        &manifest.document_id,
        &current
    )
    .is_err());
    fixture
        .database
        .with_connection(|connection| {
            assert_eq!(
                storage::load(connection, &manifest.document_id)?
                    .unwrap()
                    .filter_revision,
                1
            );
            let text: String = connection
                .query_row(
                    "SELECT plain_text FROM document_source_units WHERE document_id = ?1",
                    [&manifest.document_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(text, BODY);
            connection
                .execute_batch("DROP TRIGGER fixture_image_receipt_failure")
                .map_err(CommandError::database)?;
            crate::database::validate_backup_lineage(connection)
        })
        .unwrap();
    assert_eq!(fixture.count("document_image_projection_writes"), 0);
    persist(&fixture, &manifest, &current);
}

#[test]
fn document_image_filter_unknown_geometry_never_leaks_through_an_unfiltered_attachment() {
    let fixture = Fixture::new();
    let manifest = fixture.import(1, 0, None);
    let relative =
        manifest::manifest_path(&manifest.document_id, &manifest.fingerprint().unwrap()).unwrap();
    let path = fixture.paths.resolve_relative(&relative);
    let request = AdapterRequest {
        paths: &fixture.paths,
        document_id: &manifest.document_id,
        source_path: &path,
        format: DocumentFormat::ImageBundle,
        maximum_source_units: 128,
        cancellation: &NotCanceled,
    };
    let output = extract_with(
        Some(&fixture.database),
        request,
        &mut |_| Ok(()),
        Some("unlocated-filter-v1"),
        |_| {
            let mut result = recognition();
            result.lines[0].bounds = None;
            Ok(result)
        },
    )
    .unwrap();
    persist(&fixture, &manifest, &output);
    let Some(CompatibilityProjection::ImageSet(receipt)) = &output.compatibility else {
        panic!("missing image receipt");
    };
    filter_commands::apply(
        &fixture.database,
        &SetImageFiltersInput {
            operation_id: "d".repeat(32),
            document_id: manifest.document_id.clone(),
            expected_revision: 1,
            expected_filter_revision: 1,
            include_interface: false,
            overrides: vec![FilterChange {
                region_id: receipt.members[0].regions[0].id.clone(),
                action: Some(RegionOverride::Exclude),
            }],
        },
    )
    .unwrap();
    let filtered = extract_with(
        Some(&fixture.database),
        request,
        &mut |_| Ok(()),
        Some("unlocated-filter-v1"),
        |_| panic!("unexpected OCR"),
    )
    .unwrap();
    assert!(filtered.canonical.document.assets.is_empty());
    assert!(filtered
        .canonical
        .semantic_chunks
        .iter()
        .all(|chunk| !chunk.full_text.contains("cookies")));
    let Some(CompatibilityProjection::ImageSet(receipt)) = &filtered.compatibility else {
        panic!("missing image receipt");
    };
    assert!(receipt.members[0].visual_withheld);
    assert_eq!(receipt.members[0].regions[0].text, "Accept all cookies");
    persist(&fixture, &manifest, &filtered);
}

#[test]
fn document_image_manual_filter_rebinds_to_one_exact_region_after_rotation() {
    let fixture = Fixture::new();
    let manifest = fixture.import(1, 0, None);
    let initial = extract(&fixture, &manifest, true);
    persist(&fixture, &manifest, &initial);
    let old_region = fixture
        .database
        .with_connection(|connection| {
            let receipt = storage::load(connection, &manifest.document_id)?.unwrap();
            Ok(receipt.members[0]
                .regions
                .iter()
                .find(|region| region.text == BODY)
                .unwrap()
                .id
                .clone())
        })
        .unwrap();
    filter_commands::apply(
        &fixture.database,
        &SetImageFiltersInput {
            operation_id: "1".repeat(32),
            document_id: manifest.document_id.clone(),
            expected_revision: manifest.revision,
            expected_filter_revision: 1,
            include_interface: false,
            overrides: vec![FilterChange {
                region_id: old_region.clone(),
                action: Some(RegionOverride::Exclude),
            }],
        },
    )
    .unwrap();
    let filtered = extract(&fixture, &manifest, false);
    assert!(!filtered.canonical.document.units[0]
        .plain_text
        .contains(BODY));
    persist(&fixture, &manifest, &filtered);

    amend::apply(
        &fixture.database,
        &fixture.paths,
        &AmendImageSetInput {
            operation_id: "2".repeat(32),
            document_id: manifest.document_id.clone(),
            expected_revision: manifest.revision,
            expected_filter_revision: 2,
            members: vec![MemberEdit {
                member_id: manifest.members[0].id.clone(),
                quarter_turns: 1,
                inclusion: None,
            }],
            append: Vec::new(),
        },
    )
    .unwrap();
    let rotated = fixture
        .database
        .with_connection(|connection| image_repository::load(connection, &manifest.document_id))
        .unwrap();
    let source = fixture.paths.resolve_relative(
        &manifest::manifest_path(&rotated.document_id, &rotated.fingerprint().unwrap()).unwrap(),
    );
    let mut calls = 0;
    let output = extract_with(
        Some(&fixture.database),
        AdapterRequest {
            paths: &fixture.paths,
            document_id: &rotated.document_id,
            source_path: &source,
            format: DocumentFormat::ImageBundle,
            maximum_source_units: 128,
            cancellation: &NotCanceled,
        },
        &mut |_| Ok(()),
        Some("filter-fixture-v1"),
        |_| {
            calls += 1;
            Ok(rotated_recognition())
        },
    )
    .unwrap();
    assert_eq!(calls, 1);
    assert!(!output.canonical.document.units[0].plain_text.contains(BODY));
    let Some(CompatibilityProjection::ImageSet(receipt)) = &output.compatibility else {
        panic!("missing rotated image receipt");
    };
    let new_region = receipt.members[0]
        .regions
        .iter()
        .find(|region| region.text == BODY)
        .unwrap();
    assert_eq!(new_region.override_action, Some(RegionOverride::Exclude));
    assert_ne!(new_region.id, old_region);
    persist(&fixture, &rotated, &output);
    fixture
        .database
        .with_connection(|connection| {
            let settings = storage::settings(connection, &rotated.document_id)?;
            assert_eq!(settings.revision, 2);
            assert_eq!(
                settings.overrides.get(&new_region.id),
                Some(&RegionOverride::Exclude)
            );
            assert!(!settings.overrides.contains_key(&old_region));
            crate::database::validate_backup_lineage(connection)
        })
        .unwrap();
}
