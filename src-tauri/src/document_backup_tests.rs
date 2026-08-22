use std::fs;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

use crate::{
    document_backup::{export_document_files, validate_document_files},
    paths::AppPaths,
    util::unix_time_millis,
};

const DOCUMENT_ID: &str = "0123456789abcdef0123456789abcdef";
const ONE_PIXEL_PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9ZJXkAAAAASUVORK5CYII=";

fn temporary_paths(label: &str) -> AppPaths {
    let root = std::env::temp_dir().join(format!(
        "ostudy-document-backup-{label}-{}-{}",
        std::process::id(),
        unix_time_millis().expect("clock should be available")
    ));
    AppPaths::new(root).expect("temporary document paths should initialize")
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn create_package() -> Connection {
    let package = Connection::open_in_memory().expect("package should open");
    package
        .execute_batch(
            "CREATE TABLE document_files (
                 file_key TEXT PRIMARY KEY,
                 document_id TEXT NOT NULL,
                 page_number INTEGER,
                 file_kind TEXT NOT NULL,
                 relative_path TEXT NOT NULL UNIQUE,
                 mime_type TEXT NOT NULL,
                 byte_size INTEGER NOT NULL,
                 sha256 TEXT NOT NULL,
                 data BLOB NOT NULL
             );",
        )
        .expect("package schema should initialize");
    package
}

#[test]
fn document_backup_round_trip_preserves_pdf_source_and_page_files() {
    let paths = temporary_paths("pdf-round-trip");
    let pdf = include_bytes!("../tests/fixtures/document-generation/simple-text.pdf").to_vec();
    let png = STANDARD
        .decode(ONE_PIXEL_PNG)
        .expect("test PNG should decode");
    let source_relative = AppPaths::document_source_relative_path(DOCUMENT_ID);
    let page_relative = AppPaths::document_page_relative_path(DOCUMENT_ID, 1);
    let source_path = paths.resolve_relative(&source_relative);
    let page_path = paths.resolve_relative(&page_relative);
    fs::create_dir_all(source_path.parent().expect("source parent"))
        .expect("source directory should exist");
    fs::create_dir_all(page_path.parent().expect("page parent"))
        .expect("page directory should exist");
    fs::write(&source_path, &pdf).expect("source fixture should write");
    fs::write(&page_path, &png).expect("page fixture should write");

    let snapshot = Connection::open_in_memory().expect("snapshot should open");
    snapshot
        .execute_batch(
            "CREATE TABLE documents (
                 id TEXT PRIMARY KEY,
                 source_relative_path TEXT NOT NULL,
                 source_format TEXT NOT NULL,
                 source_media_type TEXT NOT NULL,
                 byte_size INTEGER NOT NULL,
                 sha256 TEXT NOT NULL
             );
             CREATE TABLE document_pages (
                 document_id TEXT NOT NULL,
                 page_number INTEGER NOT NULL,
                 render_relative_path TEXT NOT NULL,
                 render_mime_type TEXT NOT NULL,
                 render_byte_size INTEGER NOT NULL,
                 render_sha256 TEXT NOT NULL
             );
             CREATE TABLE document_source_assets (
                 document_id TEXT NOT NULL,
                 asset_id TEXT NOT NULL,
                 role TEXT NOT NULL,
                 media_type TEXT NOT NULL,
                 relative_path TEXT NOT NULL,
                 byte_size INTEGER NOT NULL,
                 sha256 TEXT NOT NULL
             );",
        )
        .expect("snapshot schema should initialize");
    snapshot
        .execute(
            "INSERT INTO documents (
                 id, source_relative_path, source_format, source_media_type,
                 byte_size, sha256
             ) VALUES (?1, ?2, 'pdf', 'application/pdf', ?3, ?4)",
            params![
                DOCUMENT_ID,
                source_relative,
                i64::try_from(pdf.len()).unwrap(),
                sha256(&pdf),
            ],
        )
        .expect("source ledger should insert");
    snapshot
        .execute(
            "INSERT INTO document_pages (
                 document_id, page_number, render_relative_path,
                 render_mime_type, render_byte_size, render_sha256
             ) VALUES (?1, 1, ?2, 'image/png', ?3, ?4)",
            params![
                DOCUMENT_ID,
                page_relative,
                i64::try_from(png.len()).unwrap(),
                sha256(&png),
            ],
        )
        .expect("page ledger should insert");

    let mut package = create_package();
    let transaction = package
        .transaction()
        .expect("package transaction should begin");
    assert_eq!(
        export_document_files(&snapshot, &paths, &transaction)
            .expect("document files should export"),
        2
    );
    transaction.commit().expect("package should commit");
    assert_eq!(
        validate_document_files(&package, &snapshot, None)
            .expect("packaged document files should validate"),
        2
    );

    let restore_root = paths.app_data_dir().join("restore-extraction");
    fs::create_dir_all(&restore_root).expect("restore root should exist");
    assert_eq!(
        validate_document_files(&package, &snapshot, Some(&restore_root))
            .expect("document files should extract"),
        2
    );
    assert_eq!(fs::read(restore_root.join(source_relative)).unwrap(), pdf);
    assert_eq!(fs::read(restore_root.join(page_relative)).unwrap(), png);

    fs::remove_dir_all(paths.app_data_dir()).expect("temporary backup root should clean up");
}

#[test]
fn document_backup_round_trip_preserves_non_pdf_source_and_canonical_images() {
    let paths = temporary_paths("anydoc-round-trip");
    let csv = b"term,definition\nheart,pump\n".to_vec();
    let png = STANDARD
        .decode(ONE_PIXEL_PNG)
        .expect("test PNG should decode");
    let source_relative =
        AppPaths::document_source_relative_path_with_extension(DOCUMENT_ID, "csv");
    let asset_relative = AppPaths::document_asset_relative_path(DOCUMENT_ID, 1, "png");
    let source_path = paths.resolve_relative(&source_relative);
    let asset_path = paths.resolve_relative(&asset_relative);
    fs::create_dir_all(source_path.parent().expect("source parent"))
        .expect("source directory should exist");
    fs::create_dir_all(asset_path.parent().expect("asset parent"))
        .expect("asset directory should exist");
    fs::write(&source_path, &csv).expect("CSV source fixture should write");
    fs::write(&asset_path, &png).expect("canonical image fixture should write");

    let snapshot = Connection::open_in_memory().expect("snapshot should open");
    snapshot
        .execute_batch(
            "CREATE TABLE documents (
                 id TEXT PRIMARY KEY,
                 source_relative_path TEXT NOT NULL,
                 source_format TEXT NOT NULL,
                 source_media_type TEXT NOT NULL,
                 byte_size INTEGER NOT NULL,
                 sha256 TEXT NOT NULL
             );
             CREATE TABLE document_source_assets (
                 document_id TEXT NOT NULL,
                 asset_id TEXT NOT NULL,
                 role TEXT NOT NULL,
                 media_type TEXT NOT NULL,
                 relative_path TEXT NOT NULL,
                 byte_size INTEGER NOT NULL,
                 sha256 TEXT NOT NULL
             );",
        )
        .expect("snapshot schema should initialize");
    snapshot
        .execute(
            "INSERT INTO documents (
                 id, source_relative_path, source_format, source_media_type,
                 byte_size, sha256
             ) VALUES (?1, ?2, 'csv', 'text/csv', ?3, ?4)",
            params![
                DOCUMENT_ID,
                source_relative,
                i64::try_from(csv.len()).unwrap(),
                sha256(&csv),
            ],
        )
        .expect("CSV source ledger should insert");
    snapshot
        .execute(
            "INSERT INTO document_source_assets (
                 document_id, asset_id, role, media_type, relative_path,
                 byte_size, sha256
             ) VALUES (?1, 'embedded:1', 'embedded_image', 'image/png', ?2, ?3, ?4)",
            params![
                DOCUMENT_ID,
                asset_relative,
                i64::try_from(png.len()).unwrap(),
                sha256(&png),
            ],
        )
        .expect("canonical image ledger should insert");

    let mut package = create_package();
    let transaction = package
        .transaction()
        .expect("package transaction should begin");
    assert_eq!(
        export_document_files(&snapshot, &paths, &transaction)
            .expect("generic document files should export"),
        2
    );
    transaction.commit().expect("package should commit");

    let kinds = {
        let mut statement = package
            .prepare("SELECT file_kind FROM document_files ORDER BY file_kind")
            .expect("file-kind query should prepare");
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("file-kind query should run")
            .collect::<Result<Vec<_>, _>>()
            .expect("file kinds should collect")
    };
    assert_eq!(kinds, vec!["source_asset", "source_document"]);
    assert_eq!(
        validate_document_files(&package, &snapshot, None)
            .expect("generic packaged document files should validate"),
        2
    );

    let restore_root = paths.app_data_dir().join("restore-extraction");
    fs::create_dir_all(&restore_root).expect("restore root should exist");
    validate_document_files(&package, &snapshot, Some(&restore_root))
        .expect("generic document files should extract");
    assert_eq!(fs::read(restore_root.join(source_relative)).unwrap(), csv);
    assert_eq!(fs::read(restore_root.join(asset_relative)).unwrap(), png);

    fs::remove_dir_all(paths.app_data_dir()).expect("temporary backup root should clean up");
}
