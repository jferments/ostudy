use super::*;

use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder, Rgba, RgbaImage};

use crate::{
    document_ingestion::{contracts::DocumentCancellation, persist_ingested_document},
    documents::image_sets::{manifest, test_support::Fixture},
    pdf_ocr_backend::regions::{OcrLine, PixelBounds},
};

const TEXT: [&str; 6] = [
    "The first introduction contains distinct educational material.",
    "The limit is 5 mg, not 50 mg; preserve every qualification.",
    "Use x <= 10 and preserve the negative sign and its context.",
    "The prerequisite must hold before the next process step.",
    "The final conclusion retains its stated uncertainty and units.",
    "An unrelated screenshot belongs to a separate source passage.",
];

struct NotCanceled;
impl DocumentCancellation for NotCanceled {
    fn check(&self) -> Result<(), CommandError> {
        Ok(())
    }
}

fn image_and_scan(passages: &[usize]) -> (Vec<u8>, RegionOcrResult) {
    let mut pixels = RgbaImage::from_pixel(32, 80, Rgba([255, 255, 255, 255]));
    let mut lines = Vec::new();
    for (index, &passage) in passages.iter().enumerate() {
        let top = 10 + index as u32 * 20;
        for y in top..top + 6 {
            for x in 0..32 {
                pixels.put_pixel(x, y, Rgba([passage as u8, x as u8, (y - top) as u8, 255]));
            }
        }
        lines.push(OcrLine {
            text: TEXT[passage].to_string(),
            bounds: Some(PixelBounds {
                left: 4,
                top,
                right: 28,
                bottom: top + 6,
            }),
            group: format!("paragraph-{index}"),
            confidence: Some(0.95),
        });
    }
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(pixels.as_raw(), 32, 80, ExtendedColorType::Rgba8)
        .unwrap();
    (
        png,
        RegionOcrResult {
            version: 1,
            text: passages
                .iter()
                .map(|&index| TEXT[index])
                .collect::<Vec<_>>()
                .join("\n"),
            confidence: 0.95,
            lines,
            duration_millis: 1,
            engine: "synthetic-local-regions".to_string(),
            language: "eng".to_string(),
            width: 32,
            height: 80,
        },
    )
}

fn extract(
    fixture: &Fixture,
    manifest: &ImageManifest,
    readings: Vec<RegionOcrResult>,
) -> AdapterExtraction {
    let path = fixture.paths.resolve_relative(
        &manifest::manifest_path(&manifest.document_id, &manifest.fingerprint().unwrap()).unwrap(),
    );
    let mut readings = readings.into_iter();
    let output = extract_with(
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
        Some("synthetic-collection-v1"),
        |_| Ok(readings.next().expect("unexpected OCR call")),
    )
    .unwrap();
    assert!(readings.next().is_none());
    output
}

#[test]
fn image_collection_all_upload_orders_retain_the_same_unique_evidence() {
    let sources = [vec![0, 1, 2], vec![1, 2, 3], vec![2, 3, 4]];
    // Includes the bridge arriving last: the first two screenshots share only
    // one anchor and cannot be collapsed until the middle screenshot arrives.
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let fixture = Fixture::new();
        let pairs = order.map(|index| image_and_scan(&sources[index]));
        let originals = pairs.iter().map(|(png, _)| png.clone()).collect::<Vec<_>>();
        let manifest = fixture.import_images(&originals, 0, None);
        let output = extract(
            &fixture,
            &manifest,
            pairs.into_iter().map(|(_, scan)| scan).collect(),
        );
        assert_eq!(output.canonical.semantic_chunks.len(), 8);
        for (passage, &text) in TEXT.iter().take(5).enumerate() {
            let chunks = output
                .canonical
                .semantic_chunks
                .iter()
                .filter(|chunk| chunk.full_text == text)
                .collect::<Vec<_>>();
            assert_eq!(chunks.len(), 1, "order={order:?} passage={passage}");
            let expected = order
                .iter()
                .enumerate()
                .filter_map(|(index, source)| {
                    sources[*source]
                        .contains(&passage)
                        .then_some(index as i64 + 1)
                })
                .collect::<Vec<_>>();
            assert_eq!(chunks[0].source_locator.unit_numbers, expected);
        }
        assert_eq!(
            output.canonical.document.metadata.attributes["verifiedImageOverlapRegions"],
            "4"
        );
        for (member, original) in manifest.members.iter().zip(&originals) {
            assert_eq!(
                std::fs::read(fixture.paths.resolve_relative(&member.relative_path)).unwrap(),
                *original
            );
        }
        persist_ingested_document(&fixture.database, &manifest.document_id, &output).unwrap();
        fixture
            .database
            .with_connection(|connection| {
                crate::database::validate_backup_lineage(connection)?;
                crate::document_canonical_store::evidence::validate(connection)
            })
            .unwrap();
        let repeated = extract(&fixture, &manifest, Vec::new());
        assert_eq!(
            repeated.canonical.semantic_chunks,
            output.canonical.semantic_chunks
        );
        assert_eq!(
            repeated.canonical.document.metadata.attributes["reusedImageExtractions"],
            "3"
        );
    }
}

#[test]
fn image_collection_nonadjacent_identical_images_share_ocr_without_sharing_ownership() {
    let fixture = Fixture::new();
    let (first, first_scan) = image_and_scan(&[0, 1, 2]);
    let (other, other_scan) = image_and_scan(&[5]);
    let originals = vec![first.clone(), other, first];
    let manifest = fixture.import_images(&originals, 0, None);
    let output = extract(&fixture, &manifest, vec![first_scan, other_scan]);
    assert_eq!(output.canonical.document.assets.len(), 3);
    assert_eq!(output.canonical.semantic_chunks.len(), 6);
    for text in TEXT.iter().take(3) {
        let chunks = output
            .canonical
            .semantic_chunks
            .iter()
            .filter(|chunk| &chunk.full_text == text)
            .collect::<Vec<_>>();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].source_locator.unit_numbers, vec![1, 3]);
    }
    assert_eq!(
        output.canonical.document.metadata.attributes["reusedImageExtractions"],
        "1"
    );
    assert_ne!(manifest.members[0].id, manifest.members[2].id);
    persist_ingested_document(&fixture.database, &manifest.document_id, &output).unwrap();
    fixture
        .database
        .with_connection(|connection| {
            crate::database::validate_backup_lineage(connection)?;
            crate::document_canonical_store::evidence::validate(connection)
        })
        .unwrap();
}

#[test]
fn image_collection_changed_literal_or_pixels_never_become_an_equivalence() {
    for change_pixels in [false, true] {
        let fixture = Fixture::new();
        let (first, first_scan) = image_and_scan(&[0, 1, 2]);
        let (other, other_scan) = image_and_scan(&[5]);
        let (mut last, mut last_scan) = image_and_scan(&[1, 2, 3]);
        if change_pixels {
            let mut pixels = image::load_from_memory(&last).unwrap().into_rgba8();
            pixels.put_pixel(7, 10, Rgba([17, 18, 19, 255]));
            last.clear();
            PngEncoder::new(&mut last)
                .write_image(pixels.as_raw(), 32, 80, ExtendedColorType::Rgba8)
                .unwrap();
        } else {
            last_scan.lines[0].text =
                "The limit is 50 g, not 5 g; preserve every qualification.".to_string();
            last_scan.text = last_scan
                .lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
        }
        let manifest = fixture.import_images(&[last, other, first], 0, None);
        let output = extract(&fixture, &manifest, vec![last_scan, other_scan, first_scan]);
        assert_eq!(
            output.canonical.document.metadata.attributes["verifiedImageOverlapRegions"],
            "0"
        );
        assert_eq!(
            output
                .canonical
                .semantic_chunks
                .iter()
                .filter(|chunk| chunk.full_text == TEXT[2])
                .count(),
            2
        );
        assert_eq!(output.canonical.semantic_chunks.len(), 10);
    }
}
