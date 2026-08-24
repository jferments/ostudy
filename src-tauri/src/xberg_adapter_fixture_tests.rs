use std::fs;

use crate::{
    document_canonical::CanonicalBlockRole,
    document_domain::{DocumentFormat, SourceUnitKind},
    paths::AppPaths,
    util::unix_time_millis,
    xberg_document_adapter,
};

const DOCUMENT_ID: &str = "0123456789abcdef0123456789abcdef";
const MAX_TEST_SOURCE_UNITS: usize = 5_000;

struct FixtureCase {
    label: &'static str,
    filename: &'static str,
    format: DocumentFormat,
    required_text: &'static str,
    required_role: Option<CanonicalBlockRole>,
    bytes: &'static [u8],
}

fn fixture_cases() -> [FixtureCase; 7] {
    [
        FixtureCase {
            label: "word-docx",
            filename: "handmade-tables.docx",
            format: DocumentFormat::Docx,
            required_text: "Head A",
            required_role: Some(CanonicalBlockRole::Table),
            bytes: include_bytes!("../tests/fixtures/anydoc-v0.2.3/handmade-tables.docx"),
        },
        FixtureCase {
            label: "powerpoint-pptx",
            filename: "handmade-altpath.pptx",
            format: DocumentFormat::Pptx,
            required_text: "Relocated deck title",
            required_role: None,
            bytes: include_bytes!("../tests/fixtures/anydoc-v0.2.3/handmade-altpath.pptx"),
        },
        FixtureCase {
            label: "opendocument-text",
            filename: "handmade-lists.odt",
            format: DocumentFormat::Odt,
            required_text: "twelve continues",
            required_role: Some(CanonicalBlockRole::ListItem),
            bytes: include_bytes!("../tests/fixtures/anydoc-v0.2.3/handmade-lists.odt"),
        },
        FixtureCase {
            label: "epub",
            filename: "handmade-math.epub",
            format: DocumentFormat::Epub,
            required_text: "Euler",
            required_role: Some(CanonicalBlockRole::Equation),
            bytes: include_bytes!("../tests/fixtures/anydoc-v0.2.3/handmade-math.epub"),
        },
        FixtureCase {
            label: "rtf",
            filename: "handmade-blockstyle.rtf",
            format: DocumentFormat::Rtf,
            required_text: "Body before.",
            required_role: None,
            bytes: include_bytes!("../tests/fixtures/anydoc-v0.2.3/handmade-blockstyle.rtf"),
        },
        FixtureCase {
            label: "spreadsheet-xlsx",
            filename: "handmade-merged.xlsx",
            format: DocumentFormat::Excel,
            required_text: "Merged across",
            required_role: Some(CanonicalBlockRole::Table),
            bytes: include_bytes!("../tests/fixtures/anydoc-v0.2.3/handmade-merged.xlsx"),
        },
        FixtureCase {
            label: "csv",
            filename: "handmade-semicolon.csv",
            format: DocumentFormat::Csv,
            required_text: "1,5",
            required_role: Some(CanonicalBlockRole::Table),
            bytes: include_bytes!("../tests/fixtures/anydoc-v0.2.3/handmade-semicolon.csv"),
        },
    ]
}

fn temporary_paths(label: &str) -> AppPaths {
    let root = std::env::temp_dir().join(format!(
        "ostudy-xberg-fixture-{label}-{}-{}",
        std::process::id(),
        unix_time_millis().expect("clock should be available")
    ));
    AppPaths::new(root).expect("temporary Xberg fixture paths should initialize")
}

fn managed_source_path(
    paths: &AppPaths,
    format: DocumentFormat,
    filename: &str,
    bytes: &[u8],
) -> std::path::PathBuf {
    let extension = std::path::Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_else(|| format.default_extension());
    let relative = AppPaths::document_source_relative_path_with_extension(DOCUMENT_ID, extension);
    let source = paths.resolve_relative(&relative);
    fs::create_dir_all(source.parent().expect("managed source parent should exist"))
        .expect("managed source directory should initialize");
    fs::write(&source, bytes).expect("fixture should write into managed source storage");
    source
}

#[test]
fn pinned_document_family_fixtures_produce_deterministic_xberg_canonical_ingestion() {
    for case in fixture_cases() {
        let detected = xberg_document_adapter::detected_document_format(case.bytes, case.filename)
            .unwrap_or_else(|error| {
                panic!(
                    "{} fixture format detection failed: {} ({})",
                    case.label, error.message, error.code
                )
            });
        assert_eq!(
            detected, case.format,
            "unexpected detected format for {}",
            case.label
        );

        let paths = temporary_paths(case.label);
        let source = managed_source_path(&paths, case.format, case.filename, case.bytes);
        let mut progress = Vec::new();
        let first = xberg_document_adapter::extract_for_test(
            &paths,
            DOCUMENT_ID,
            &source,
            case.format,
            MAX_TEST_SOURCE_UNITS,
            |completed, total| {
                progress.push((completed, total));
                Ok(())
            },
        )
        .unwrap_or_else(|error| panic!("{} fixture failed: {}", case.label, error.message));
        first.validate().unwrap_or_else(|error| {
            panic!("{} canonical output invalid: {}", case.label, error.message)
        });

        assert_eq!(first.document.source.format, case.format, "{}", case.label);
        assert_eq!(
            first.document.source.unit_kind,
            SourceUnitKind::Section,
            "{} must not change durable source-unit provenance in the parser cutover",
            case.label
        );
        assert!(!first.document.units.is_empty(), "{}", case.label);
        assert!(!first.document.blocks.is_empty(), "{}", case.label);
        assert!(!first.semantic_chunks.is_empty(), "{}", case.label);
        assert!(
            first
                .document
                .blocks
                .iter()
                .any(|block| block.text.contains(case.required_text)),
            "{} must preserve required fixture text: {}",
            case.label,
            case.required_text
        );
        if let Some(role) = case.required_role {
            assert!(
                first.document.blocks.iter().any(|block| block.role == role),
                "{} must preserve the required semantic role {:?}",
                case.label,
                role
            );
        }
        assert_eq!(
            progress.last().copied(),
            Some((first.document.units.len(), first.document.units.len())),
            "{} must finish bounded source-unit progress",
            case.label
        );

        let second = xberg_document_adapter::extract_for_test(
            &paths,
            DOCUMENT_ID,
            &source,
            case.format,
            MAX_TEST_SOURCE_UNITS,
            |_, _| Ok(()),
        )
        .unwrap_or_else(|error| {
            panic!("{} second extraction failed: {}", case.label, error.message)
        });
        assert_eq!(
            first, second,
            "{} canonical extraction must be deterministic",
            case.label
        );

        fs::remove_dir_all(paths.app_data_dir()).expect("fixture temp directory should clean up");
    }
}

#[test]
fn pinned_encrypted_fixture_fails_before_ai_processing() {
    let bytes = include_bytes!("../tests/fixtures/anydoc-v0.2.3/encrypted--errors.odt");
    assert_eq!(
        xberg_document_adapter::detected_document_format(bytes, "encrypted--errors.odt")
            .expect("encrypted fixture package type should still be detectable"),
        DocumentFormat::Odt
    );

    let paths = temporary_paths("encrypted-odt");
    let source = managed_source_path(&paths, DocumentFormat::Odt, "encrypted--errors.odt", bytes);
    let error = xberg_document_adapter::extract_for_test(
        &paths,
        DOCUMENT_ID,
        &source,
        DocumentFormat::Odt,
        MAX_TEST_SOURCE_UNITS,
        |_, _| Ok(()),
    )
    .expect_err("encrypted fixture must fail locally");
    assert_eq!(error.code, "document_encrypted");

    fs::remove_dir_all(paths.app_data_dir()).expect("fixture temp directory should clean up");
}
