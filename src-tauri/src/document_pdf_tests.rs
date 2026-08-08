use std::fs;

use crate::{paths::AppPaths, pdf_backend, util::unix_time_millis};

fn temporary_paths(label: &str) -> AppPaths {
    let root = std::env::temp_dir().join(format!(
        "ostudy-document-pdf-{label}-{}-{}",
        std::process::id(),
        unix_time_millis().expect("clock should be available")
    ));
    AppPaths::new(root).expect("temporary document paths should initialize")
}

#[test]
fn active_pdf_backend_extracts_text_and_renders_a_real_fixture() {
    let paths = temporary_paths("extract-render");
    let source = paths.app_data_dir().join("simple-text.pdf");
    fs::write(
        &source,
        include_bytes!("../tests/fixtures/document-generation/simple-text.pdf"),
    )
    .expect("fixture should be copied to a filesystem path");

    let mut visited = 0_usize;
    let page_count = pdf_backend::for_each_page(&source, 72.0, |page_number, text, png| {
        visited += 1;
        assert_eq!(page_number, 1);
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(!normalized.is_empty());
        assert!(
            normalized
                .chars()
                .filter(|character| character.is_alphabetic())
                .count()
                >= 10,
            "the real PDF fixture should yield meaningful extracted text"
        );
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]));
        assert!(png.len() > 100);
        Ok::<(), ()>(())
    })
    .expect("active PDF backend should process the fixture");
    assert_eq!(page_count, 1);
    assert_eq!(visited, 1);
    assert_eq!(pdf_backend::active_backend_id(), "justpdf-0.1");

    fs::remove_dir_all(paths.app_data_dir()).expect("temporary fixture root should clean up");
}
