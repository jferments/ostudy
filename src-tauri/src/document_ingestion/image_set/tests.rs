use super::*;

use crate::{
    document_ingestion::contracts::DocumentCancellation,
    documents::image_sets::{manifest::{self, InclusionCrop}, test_support::Fixture},
    pdf_ocr_backend::regions::{OcrLine, PixelBounds},
};

struct NotCanceled;
impl DocumentCancellation for NotCanceled {
    fn check(&self) -> Result<(), CommandError> { Ok(()) }
}

fn recognition(png: &[u8]) -> Result<RegionOcrResult, CommandError> {
    let image = image::load_from_memory(png).unwrap();
    Ok(RegionOcrResult {
        version: 1, text: "5 mg < 10; not 50 mg".to_string(), confidence: 0.95,
        lines: vec![OcrLine {
            text: "5 mg < 10; not 50 mg".to_string(),
            bounds: Some(PixelBounds { left: 0, top: 0, right: 4, bottom: 3 }),
            group: "paragraph-1".to_string(), confidence: Some(0.95),
        }],
        duration_millis: 1, engine: "synthetic-native".to_string(), language: "eng".to_string(),
        width: image.width(), height: image.height(),
    })
}

#[test]
fn image_set_adapter_reuses_ocr_and_publishes_the_existing_canonical_contract() {
    let fixture = Fixture::new();
    let manifest = fixture.import(2, 0, None);
    let path = fixture.paths.resolve_relative(&manifest::manifest_path(&manifest.document_id, &manifest.fingerprint().unwrap()).unwrap());
    let request = AdapterRequest {
        paths: &fixture.paths, document_id: &manifest.document_id, source_path: &path,
        format: DocumentFormat::ImageBundle, maximum_source_units: 128, cancellation: &NotCanceled,
    };
    let mut calls = 0;
    let output = extract_with(Some(&fixture.database), request, &mut |_| Ok(()), Some("fixture-v1"), |png| {
        calls += 1;
        recognition(png)
    }).unwrap();
    assert_eq!(calls, 1);
    assert_eq!(output.canonical.document.units.len(), 2);
    assert_eq!(output.canonical.semantic_chunks.len(), 2);
    assert_eq!(output.canonical.document.metadata.attributes["reusedImageExtractions"], "1");
    assert_eq!(output.canonical.document.units[0].plain_text, "5 mg < 10; not 50 mg");
    assert_ne!(output.canonical.document.assets[0].asset_id, output.canonical.document.assets[1].asset_id);
    crate::document_ingestion::persist_ingested_document(&fixture.database, &manifest.document_id, &output).unwrap();
    fixture.database.with_connection(|connection| {
        crate::database::validate_backup_lineage(connection)?;
        crate::document_canonical_store::evidence::validate(connection)?;
        let count: i64 = connection.query_row("SELECT COUNT(*) FROM document_semantic_chunk_sources WHERE document_id = ?1", [&manifest.document_id], |row| row.get(0)).map_err(CommandError::database)?;
        assert_eq!(count, 2);
        Ok(())
    }).unwrap();
    let repeated = extract_with(Some(&fixture.database), request, &mut |_| Ok(()), Some("fixture-v1"), |_| panic!("re-OCR on compatible retry")).unwrap();
    assert_eq!(repeated.canonical.document.metadata.attributes["reusedImageExtractions"], "2");
    for (left, right) in output.canonical.document.assets.iter().zip(&repeated.canonical.document.assets) {
        assert_eq!(left.sha256, right.sha256);
        assert_eq!(left.relative_path, right.relative_path);
    }
}

#[test]
fn image_set_crop_removes_excluded_text_from_chunks_and_pixels_but_retains_ocr() {
    let fixture = Fixture::new();
    let manifest = fixture.import(1, 0, Some(InclusionCrop { left: 0.5, top: 0.0, right: 1.0, bottom: 1.0 }));
    let path = fixture.paths.resolve_relative(&manifest::manifest_path(&manifest.document_id, &manifest.fingerprint().unwrap()).unwrap());
    let original = std::fs::read(fixture.paths.resolve_relative(&manifest.members[0].relative_path)).unwrap();
    let request = AdapterRequest {
        paths: &fixture.paths, document_id: &manifest.document_id, source_path: &path,
        format: DocumentFormat::ImageBundle, maximum_source_units: 128, cancellation: &NotCanceled,
    };
    let output = extract_with(Some(&fixture.database), request, &mut |_| Ok(()), Some("fixture-v1"), recognition).unwrap();
    assert!(output.canonical.document.units[0].plain_text.is_empty());
    assert!(output.canonical.semantic_chunks.iter().all(|chunk| !chunk.full_text.contains("50 mg")));
    let asset = &output.canonical.document.assets[0];
    let png = std::fs::read(fixture.paths.resolve_relative(&asset.relative_path)).unwrap();
    let pixels = image::load_from_memory(&png).unwrap().into_rgba8();
    for y in 0..6 { for x in 0..4 { assert_eq!(*pixels.get_pixel(x, y), image::Rgba([255, 255, 255, 255])); } }
    assert_eq!(original, std::fs::read(fixture.paths.resolve_relative(&manifest.members[0].relative_path)).unwrap());
    let cached = cache::get_or_extract(Some(&fixture.database), &manifest.members[0], Some("fixture-v1"), || panic!("crop erased source extraction")).unwrap();
    assert_eq!(cached.result.text, "5 mg < 10; not 50 mg");
}

#[test]
fn image_set_unreliable_geometry_is_not_guessed_to_allow_a_crop() {
    let fixture = Fixture::new();
    let manifest = fixture.import(1, 0, Some(InclusionCrop { left: 0.0, top: 0.0, right: 0.5, bottom: 1.0 }));
    let path = fixture.paths.resolve_relative(&manifest::manifest_path(&manifest.document_id, &manifest.fingerprint().unwrap()).unwrap());
    let request = AdapterRequest {
        paths: &fixture.paths, document_id: &manifest.document_id, source_path: &path,
        format: DocumentFormat::ImageBundle, maximum_source_units: 128, cancellation: &NotCanceled,
    };
    let output = extract_with(Some(&fixture.database), request, &mut |_| Ok(()), Some("unlocated-v1"), |png| {
        let mut result = recognition(png)?;
        result.lines[0].bounds = None;
        Ok(result)
    }).unwrap();
    assert!(output.canonical.document.units[0].plain_text.is_empty());
    assert!(output.canonical.document.extraction.semantic_analysis_degraded);
    assert_eq!(output.canonical.document.metadata.attributes["unresolvedImageRegions"], "1");
}

#[test]
fn image_set_ocr_failure_retains_successful_siblings_and_explicit_visual_evidence() {
    let fixture = Fixture::new();
    let manifest = fixture.import(2, 0, None);
    let path = fixture.paths.resolve_relative(&manifest::manifest_path(&manifest.document_id, &manifest.fingerprint().unwrap()).unwrap());
    let request = AdapterRequest {
        paths: &fixture.paths, document_id: &manifest.document_id, source_path: &path,
        format: DocumentFormat::ImageBundle, maximum_source_units: 128, cancellation: &NotCanceled,
    };
    let mut calls = 0;
    let output = extract_with(Some(&fixture.database), request, &mut |_| Ok(()), None, |png| {
        calls += 1;
        if calls == 2 { return Err(CommandError::new("image_ocr_unavailable", "Synthetic unreadable image")); }
        recognition(png)
    }).unwrap();
    assert_eq!(calls, 2);
    assert_eq!(output.canonical.document.units[0].plain_text, "5 mg < 10; not 50 mg");
    assert!(output.canonical.document.units[1].plain_text.is_empty());
    assert_eq!(output.canonical.document.assets.len(), 2);
    assert!(output.canonical.document.extraction.semantic_analysis_degraded);
    assert_eq!(output.canonical.semantic_chunks[1].element_types, vec!["paragraph", "image"]);
}

#[test]
fn image_set_stale_source_cannot_replace_published_canonical_evidence() {
    let fixture = Fixture::new();
    let manifest = fixture.import(1, 0, None);
    let path = fixture.paths.resolve_relative(&manifest::manifest_path(&manifest.document_id, &manifest.fingerprint().unwrap()).unwrap());
    let request = AdapterRequest {
        paths: &fixture.paths, document_id: &manifest.document_id, source_path: &path,
        format: DocumentFormat::ImageBundle, maximum_source_units: 128, cancellation: &NotCanceled,
    };
    let output = extract_with(Some(&fixture.database), request, &mut |_| Ok(()), Some("fixture-v1"), recognition).unwrap();
    fixture.database.with_connection(|connection| {
        connection.execute("UPDATE documents SET sha256 = ?1 WHERE id = ?2", rusqlite::params!["0".repeat(64), manifest.document_id]).map_err(CommandError::database)?;
        Ok(())
    }).unwrap();
    let error = crate::document_ingestion::persist_ingested_document(&fixture.database, &manifest.document_id, &output).unwrap_err();
    assert_eq!(error.code, "document_source_changed");
    assert_eq!(fixture.count("document_source_blocks"), 0);
    assert_eq!(fixture.count("document_image_extractions"), 1);
}
