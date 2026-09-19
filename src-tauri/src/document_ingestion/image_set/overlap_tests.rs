use super::*;

use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder, Rgba, RgbaImage};

use crate::{
    document_ingestion::{contracts::DocumentCancellation, persist_ingested_document},
    documents::image_sets::{manifest, test_support::Fixture},
    pdf_ocr_backend::regions::{OcrLine, PixelBounds},
};

const A: &str = "A distinct introduction belongs only to the first image.";
const B: &str = "The limit is 5 mg, not 50 mg; preserve every qualification.";
const C: &str = "Use x <= 10 and preserve the negative sign and its context.";
const D: &str = "A new conclusion belongs only to the second image.";

struct NotCanceled;

impl DocumentCancellation for NotCanceled {
    fn check(&self) -> Result<(), CommandError> {
        Ok(())
    }
}

fn png(bands: &[(u32, u8)], altered: bool) -> Vec<u8> {
    let mut pixels = RgbaImage::from_pixel(32, 80, Rgba([255, 255, 255, 255]));
    for &(top, value) in bands {
        for y in top..top + 6 {
            for x in 0..32 {
                pixels.put_pixel(x, y, Rgba([value, x as u8, (y - top) as u8, 255]));
            }
        }
    }
    if altered {
        pixels.put_pixel(7, 10, Rgba([1, 2, 3, 255]));
    }
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(pixels.as_raw(), 32, 80, ExtendedColorType::Rgba8)
        .unwrap();
    bytes
}

fn inputs(altered: bool) -> Vec<Vec<u8>> {
    vec![
        png(&[(10, 10), (40, 20), (60, 30)], false),
        png(&[(10, 20), (30, 30), (50, 40)], altered),
    ]
}

fn scan(texts: &[&str], tops: &[u32]) -> RegionOcrResult {
    RegionOcrResult {
        version: 1,
        text: texts.join("\n"),
        confidence: 0.95,
        lines: texts
            .iter()
            .zip(tops)
            .enumerate()
            .map(|(index, (text, top))| OcrLine {
                text: text.to_string(),
                bounds: Some(PixelBounds {
                    left: 4,
                    top: *top,
                    right: 28,
                    bottom: top + 6,
                }),
                group: format!("paragraph-{index}"),
                confidence: Some(0.95),
            })
            .collect(),
        duration_millis: 1,
        engine: "synthetic-local-regions".to_string(),
        language: "eng".to_string(),
        width: 32,
        height: 80,
    }
}

fn extract(
    fixture: &Fixture,
    manifest: &ImageManifest,
    readings: Vec<RegionOcrResult>,
) -> AdapterExtraction {
    let path = fixture.paths.resolve_relative(
        &manifest::manifest_path(&manifest.document_id, &manifest.fingerprint().unwrap()).unwrap(),
    );
    let request = AdapterRequest {
        paths: &fixture.paths,
        document_id: &manifest.document_id,
        source_path: &path,
        format: DocumentFormat::ImageBundle,
        maximum_source_units: 128,
        cancellation: &NotCanceled,
    };
    let mut readings = readings.into_iter();
    let result = extract_with(
        Some(&fixture.database),
        request,
        &mut |_| Ok(()),
        Some("synthetic-overlap-v1"),
        |_| Ok(readings.next().expect("unexpected OCR call")),
    )
    .unwrap();
    assert!(
        readings.next().is_none(),
        "expected local OCR was not exercised"
    );
    result
}

#[test]
fn image_overlap_projects_unique_passages_and_retains_exact_owned_occurrences() {
    let fixture = Fixture::new();
    let originals = inputs(false);
    let manifest = fixture.import_images(&originals, 0, None);
    let output = extract(
        &fixture,
        &manifest,
        vec![
            scan(&[A, B, C], &[10, 40, 60]),
            scan(&[B, C, D], &[10, 30, 50]),
        ],
    );
    let canonical = &output.canonical;
    assert_eq!(canonical.semantic_chunks.len(), 6);
    for text in [B, C] {
        let passages = canonical
            .semantic_chunks
            .iter()
            .filter(|chunk| chunk.full_text == text)
            .collect::<Vec<_>>();
        assert_eq!(passages.len(), 1);
        assert_eq!(passages[0].source_locator.unit_numbers, vec![1, 2]);
        assert!(canonical
            .document
            .units
            .iter()
            .all(|unit| unit.plain_text.contains(text)));
    }
    assert_eq!(
        canonical.document.metadata.attributes["verifiedImageOverlapRegions"],
        "2"
    );
    assert_eq!(
        canonical.document.metadata.attributes["ambiguousImageOverlaps"],
        "0"
    );
    let linked = canonical
        .document
        .blocks
        .iter()
        .filter(|block| block.attributes.contains_key("equivalentToRegionId"))
        .count();
    assert_eq!(linked, 2);
    let asset = &canonical.document.assets[1];
    let rendered = std::fs::read(fixture.paths.resolve_relative(&asset.relative_path)).unwrap();
    let pixels = image::load_from_memory(&rendered).unwrap().into_rgba8();
    for top in [10, 30] {
        for y in top..top + 6 {
            for x in 0..32 {
                assert_eq!(*pixels.get_pixel(x, y), Rgba([255, 255, 255, 255]));
            }
        }
    }
    assert_ne!(*pixels.get_pixel(7, 50), Rgba([255, 255, 255, 255]));
    for (member, bytes) in manifest.members.iter().zip(&originals) {
        let retained =
            std::fs::read(fixture.paths.resolve_relative(&member.relative_path)).unwrap();
        assert_eq!(&retained, bytes);
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
        canonical.semantic_chunks
    );
    assert_eq!(
        repeated.canonical.document.metadata.attributes["reusedImageExtractions"],
        "2"
    );
}

#[test]
fn image_overlap_protected_characters_reject_equivalence_without_losing_either_reading() {
    for (original, changed) in [
        (
            B,
            "The limit is 50 mg, not 5 mg; preserve every qualification.",
        ),
        (
            B,
            "The limit is 5 g, not 50 mg; preserve every qualification.",
        ),
        (
            B,
            "The limit is 5 mg, or 50 mg; preserve every qualification.",
        ),
        (
            "Keep x <= 10 with the stated prerequisite.",
            "Keep x >= 10 with the stated prerequisite.",
        ),
        (
            "The coefficient is -5 for this particular case.",
            "The coefficient is +5 for this particular case.",
        ),
        (
            "The area is 5 m² under the stated conditions.",
            "The area is 5 m³ under the stated conditions.",
        ),
        (
            "Use if (x != 10) { return false; } here.",
            "Use if (x == 10) { return false; } here.",
        ),
    ] {
        let fixture = Fixture::new();
        let manifest = fixture.import_images(&inputs(false), 0, None);
        let output = extract(
            &fixture,
            &manifest,
            vec![
                scan(&[A, original, C], &[10, 40, 60]),
                scan(&[changed, C, D], &[10, 30, 50]),
            ],
        );
        assert_eq!(
            output.canonical.document.metadata.attributes["verifiedImageOverlapRegions"],
            "0"
        );
        assert_eq!(
            output.canonical.document.metadata.attributes["ambiguousImageOverlaps"],
            "1"
        );
        for text in [original, changed] {
            assert!(output
                .canonical
                .semantic_chunks
                .iter()
                .any(|chunk| chunk.full_text == text));
        }
    }
}

#[test]
fn image_overlap_identical_ocr_does_not_hide_different_source_pixels() {
    let fixture = Fixture::new();
    let manifest = fixture.import_images(&inputs(true), 0, None);
    let output = extract(
        &fixture,
        &manifest,
        vec![
            scan(&[A, B, C], &[10, 40, 60]),
            scan(&[B, C, D], &[10, 30, 50]),
        ],
    );
    assert_eq!(
        output.canonical.document.metadata.attributes["verifiedImageOverlapRegions"],
        "0"
    );
    let asset = &output.canonical.document.assets[1];
    let bytes = std::fs::read(fixture.paths.resolve_relative(&asset.relative_path)).unwrap();
    let pixels = image::load_from_memory(&bytes).unwrap().into_rgba8();
    assert_eq!(*pixels.get_pixel(7, 10), Rgba([1, 2, 3, 255]));
}

#[test]
fn image_overlap_repeated_headers_tables_and_unlocated_text_stay_unmerged() {
    let mut unlocated = scan(&[B, C, D], &[10, 30, 50]);
    unlocated.lines[0].bounds = None;
    for right in [
        scan(&[B, B, C], &[10, 30, 50]),
        scan(&[B, C, D], &[10, 10, 50]),
        unlocated,
    ] {
        let fixture = Fixture::new();
        let manifest = fixture.import_images(&inputs(false), 0, None);
        let output = extract(
            &fixture,
            &manifest,
            vec![scan(&[A, B, C], &[10, 40, 60]), right.clone()],
        );
        assert_eq!(
            output.canonical.document.metadata.attributes["verifiedImageOverlapRegions"],
            "0"
        );
        assert_eq!(output.canonical.document.units[1].plain_text, right.text);
    }
}

#[test]
fn image_overlap_pixel_work_is_bounded_without_truncating_dense_ocr() {
    let fixture = Fixture::new();
    let manifest = fixture.import_images(&inputs(false), 0, None);
    let member = &manifest.members[0];
    let image = raster::Raster::read(&fixture.paths, member).unwrap();
    let mut reading = scan(&[B], &[0]);
    reading.lines[0].bounds = Some(PixelBounds {
        left: 0,
        top: 0,
        right: 32,
        bottom: 80,
    });
    reading.lines = (0..128)
        .map(|index| {
            let mut line = reading.lines[0].clone();
            line.group = format!("paragraph-{index}");
            line
        })
        .collect();
    reading.text = vec![B; 128].join("\n");
    let frame = regions::Frame::build(
        member,
        &image,
        raster::inclusion(member).unwrap(),
        Some(&reading),
    )
    .unwrap();
    assert_eq!(frame.regions.len(), 128);
    assert_eq!(
        frame
            .regions
            .iter()
            .filter(|region| region.signature.is_some())
            .count(),
        2
    );
}
