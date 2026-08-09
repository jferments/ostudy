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
fn structured_extraction_and_pure_rust_rendering_process_a_real_fixture() {
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
        "oxidize-pdf-4.2.2-rag-quality-ocr-v1"
    );

    let runtime = PdfRenderRuntime::new(None);
    let mut rendered = 0_usize;
    let page_count = pdf_render_backend::render_pages(&runtime, &source, |page_number, png| {
        rendered += 1;
        assert_eq!(page_number, 1);
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]));
        assert!(png.len() > 100);
        Ok::<(), ()>(())
    })
    .expect("the bounded parallel pure-Rust justpdf renderer should render the fixture");
    assert_eq!(page_count, 1);
    assert_eq!(rendered, 1);
    assert_eq!(
        pdf_render_backend::active_backend_id(),
        "justpdf-render-0.1.2/pure-rust-parallel-v2-120dpi"
    );

    fs::remove_dir_all(paths.app_data_dir()).expect("temporary fixture root should clean up");
}
