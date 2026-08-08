use std::fs;

use justpdf::Document;

use crate::{paths::AppPaths, util::unix_time_millis};

fn temporary_paths(label: &str) -> AppPaths {
    let root = std::env::temp_dir().join(format!(
        "ostudy-document-pdf-{label}-{}-{}",
        std::process::id(),
        unix_time_millis().expect("clock should be available")
    ));
    AppPaths::new(root).expect("temporary document paths should initialize")
}

#[test]
fn pinned_pdf_engine_extracts_text_and_renders_a_real_fixture() {
    let paths = temporary_paths("extract-render");
    let source = paths.app_data_dir().join("simple-text.pdf");
    fs::write(
        &source,
        include_bytes!("../tests/fixtures/document-generation/simple-text.pdf"),
    )
    .expect("fixture should be copied to a filesystem path");

    let document = Document::open(&source).expect("fixture PDF should open");
    assert_eq!(document.page_count(), 1);
    let page = document.page(0).expect("fixture page should load");
    let text = page.text().expect("fixture text should extract");
    assert!(text.to_ascii_lowercase().contains("cardiac output"));
    assert!(!text.trim().is_empty());
    let png = page.render_png(72.0).expect("fixture page should render");
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]));
    assert!(png.len() > 100);

    fs::remove_dir_all(paths.app_data_dir()).expect("temporary fixture root should clean up");
}
