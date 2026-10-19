use std::fs;

use crate::{
    paths::AppPaths,
    pdf_backend,
    pdf_render_backend::{self, PdfRenderRuntime},
    util::unix_time_millis,
};

fn temporary_paths(label: &str) -> AppPaths {
    let root = std::env::temp_dir().join(format!(
        "ostudy-document-pdf-{label}-{}-{}",
        std::process::id(),
        unix_time_millis().expect("clock should be available")
    ));
    AppPaths::new(root).expect("temporary document paths should initialize")
}

#[test]
fn structured_extraction_and_pdfium_rendering_process_a_real_fixture() {
    let paths = temporary_paths("extract-render");
    let source = paths.app_data_dir().join("simple-text.pdf");
    fs::write(
        &source,
        include_bytes!("../tests/fixtures/document-generation/simple-text.pdf"),
    )
    .expect("fixture should be copied to a filesystem path");

    let extracted = pdf_backend::extract_document(&source)
        .expect("oxidize-pdf should structurally extract the fixture");
    assert_eq!(extracted.page_count, 1);
    assert_eq!(extracted.pages.len(), 1);
    let normalized = extracted.pages[0]
        .text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(!normalized.is_empty());
    assert!(
        normalized
            .chars()
            .filter(|character| character.is_alphabetic())
            .count()
            >= 10,
        "the real PDF fixture should yield meaningful extracted text"
    );
    assert!(!extracted.elements.is_empty());
    assert!(!extracted.chunks.is_empty());
    assert!(extracted.chunks[0].page_numbers.contains(&1));
    assert_eq!(
        pdf_backend::active_backend_id(),
        "oxidize-pdf-5.5.0-rag-native-first-ocr-v9"
    );

    let runtime = PdfRenderRuntime::new(None).expect("provisioned PDFium should initialize");
    let pdfium_text = pdf_render_backend::extract_native_text_pages(&runtime, &source, &[1])
        .expect("PDFium should expose native text before OCR is considered");
    assert_eq!(pdfium_text.len(), 1);
    assert_eq!(pdfium_text[0].0, 1);
    assert!(
        pdfium_text[0]
            .1
            .chars()
            .filter(|character| character.is_alphabetic())
            .count()
            >= 10
    );

    let mut rendered = 0_usize;
    let page_count = pdf_render_backend::render_pages(&runtime, &source, |page_number, png| {
        rendered += 1;
        assert_eq!(page_number, 1);
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]));
        assert!(png.len() > 100);
        Ok::<(), ()>(())
    })
    .expect("the provisioned PDFium renderer should render the fixture");
    assert_eq!(page_count, 1);
    assert_eq!(rendered, 1);
    assert_eq!(
        pdf_render_backend::active_backend_id(),
        "pdfium-render-0.9.3/pdfium-api-7881-external-v2-120dpi"
    );

    fs::remove_dir_all(paths.app_data_dir()).expect("temporary fixture root should clean up");
}

fn words(text: &str) -> usize {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .count()
}

fn extract_fixture(label: &str, name: &str, bytes: &[u8]) -> pdf_backend::ExtractedDocument {
    let paths = temporary_paths(label);
    let source = paths.app_data_dir().join(name);
    fs::write(&source, bytes).expect("fixture should be copied to a filesystem path");
    pdf_backend::extract_document(&source).expect("oxidize-pdf should extract the fixture")
}

#[test]
fn two_text_columns_are_not_read_as_a_table_or_sent_twice() {
    let extracted = extract_fixture(
        "two-column",
        "textbook-two-column.pdf",
        include_bytes!("../tests/fixtures/document-generation/textbook-two-column.pdf"),
    );
    assert!(
        extracted
            .elements
            .iter()
            .all(|element| element.element_type != "table"),
        "a two-column page must not produce a table element"
    );
    let page_words = words(&extracted.pages[0].text);
    let chunk_words = extracted
        .chunks
        .iter()
        .map(|chunk| words(&chunk.text))
        .sum::<usize>();
    assert!(
        chunk_words * 10 <= page_words * 11,
        "chunks carry {chunk_words} words for a {page_words}-word page"
    );
}

#[test]
fn a_ruled_table_between_paragraphs_stays_a_table() {
    let extracted = extract_fixture(
        "ruled-table",
        "table-handout.pdf",
        include_bytes!("../tests/fixtures/document-generation/table-handout.pdf"),
    );
    let tables = extracted
        .elements
        .iter()
        .filter(|element| element.element_type == "table")
        .collect::<Vec<_>>();
    assert_eq!(tables.len(), 1);
    assert!(tables[0]
        .text
        .contains("Digoxin | Cardiac glycoside | Inhibits Na+/K+ ATPase | 36-48"));
}

#[test]
fn wrapped_rows_of_drawn_tables_keep_their_values_together() {
    let extracted = extract_fixture(
        "ruled-tables-wrapped",
        "ruled-tables-wrapped.pdf",
        include_bytes!("../tests/fixtures/document-generation/ruled-tables-wrapped.pdf"),
    );
    let tables = extracted
        .elements
        .iter()
        .filter(|element| element.element_type == "table")
        .map(|element| element.text.as_str())
        .collect::<Vec<_>>();
    assert_eq!(tables.len(), 2, "{tables:?}");
    assert!(tables[0].contains(
        "Vitamin C | Citrus fruit, peppers, berries, tomatoes | 75-90 mg | Collagen synthesis and wound healing | Bleeding gums, slow healing"
    ));
    assert!(tables[1].contains(
        "Zinc | Shellfish, seeds, nuts, dairy | 8-11 mg | Immunity and growth | Poor wound healing, hair loss"
    ));
    assert!(extracted
        .elements
        .iter()
        .any(|element| element.element_type == "paragraph"
            && element.text.contains("Table 2 Example minerals")));
}
