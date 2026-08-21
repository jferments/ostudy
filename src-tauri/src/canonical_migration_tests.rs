use rusqlite::{params, Connection};
use serde_json::Value;

use crate::{
    database::{Database, LATEST_SCHEMA_VERSION},
    document_domain::{SourceLocator, SourceUnitKind},
    error::CommandError,
    util::unix_time_millis,
};

const PUBLIC_BASELINE_CHECKSUM: &str =
    "2d978f8be026ecb820e71ba77a8585e1de51cb08fc2820714526a33284167f85";
const DOCUMENT_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CANONICAL_SEMANTIC_HASH_REPAIR_CHECKSUM: &str =
    "779d93574a75f1e694fc4476bbf278ce7a540155cc4c35cf113f9ddb03d2a46f";
const CANONICAL_BLOCK_HASH_REPAIR_CHECKSUM: &str =
    "02a07faa3635627246ef361160e94fe80a4634f7d36c97dbc3d6cb0819846085";
const EXPECTED_CANONICAL_CHUNK_HASH: &str =
    "1ddbbf54830254d96e8eaf5c04a21564d65f02984061b9d7b3c49931223bad3d";
const EXPECTED_CANONICAL_BLOCK_HASH: &str =
    "d4d9309e42a634acaabb18a6805f2da962eb887bb4518672e0e35fcdac70a6fe";

#[test]
fn public_v1_pdf_rows_migrate_to_the_canonical_source_ledger() {
    let root = std::env::temp_dir().join(format!(
        "ostudy-canonical-migration-{}-{}",
        std::process::id(),
        unix_time_millis().unwrap_or(0)
    ));
    std::fs::create_dir_all(&root).expect("migration fixture directory should exist");
    let database_path = root.join("library.sqlite3");

    seed_public_v1_pdf(&database_path);

    let database = Database::open(&database_path).expect("version-1 database should migrate");
    database
        .with_connection(assert_canonical_pdf_backfill)
        .expect("canonical PDF backfill should be complete");
    drop(database);

    std::fs::remove_dir_all(root).expect("migration fixture directory should be removed");
}

fn seed_public_v1_pdf(path: &std::path::Path) {
    let connection = Connection::open(path).expect("version-1 fixture database should open");
    connection
        .execute_batch(include_str!("../migrations/0001_public_baseline.sql"))
        .expect("public baseline should load");
    connection
        .execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at)
             VALUES (1, 'public_baseline', ?1, 1)",
            [PUBLIC_BASELINE_CHECKSUM],
        )
        .expect("public migration ledger should be seeded");
    connection
        .pragma_update(None, "user_version", 1_i64)
        .expect("public user_version should be seeded");

    connection
        .execute(
            "INSERT INTO documents (
                 id, title, original_filename, mime_type, byte_size, sha256,
                 source_relative_path, lifecycle_state, page_count,
                 extracted_page_count, rendered_page_count, created_at,
                 updated_at, completed_at, extraction_backend, render_backend,
                 semantic_element_count, semantic_chunk_count, source_format,
                 source_unit_kind, extraction_mode
             ) VALUES (
                 ?1, 'Cardiovascular lesson', 'cardio.pdf', 'application/pdf',
                 100, ?2, ?3, 'ready', 1, 1, 1, 10, 20, 20,
                 'oxidize-pdf', 'pdfium', 1, 1, 'pdf', 'page', 'native'
             )",
            params![
                DOCUMENT_ID,
                "1".repeat(64),
                format!("documents/{DOCUMENT_ID}/source.pdf"),
            ],
        )
        .expect("version-1 document should be seeded");
    connection
        .execute(
            "INSERT INTO document_pages (
                 document_id, page_number, plain_text, text_sha256,
                 render_relative_path, render_mime_type, render_byte_size,
                 render_sha256, created_at, updated_at
             ) VALUES (?1, 1, ?2, ?3, ?4, 'image/png', 80, ?5, 10, 20)",
            params![
                DOCUMENT_ID,
                "Cardiac output equals heart rate multiplied by stroke volume.",
                "2".repeat(64),
                format!("documents/{DOCUMENT_ID}/pages/page-000001.png"),
                "3".repeat(64),
            ],
        )
        .expect("version-1 page should be seeded");
    connection
        .execute(
            "INSERT INTO document_semantic_elements (
                 document_id, element_index, page_number, element_type, text,
                 parent_heading, heading_path_json, bbox_x, bbox_y, bbox_width,
                 bbox_height, confidence, font_name, font_size, is_bold,
                 is_italic, text_sha256, created_at
             ) VALUES (
                 ?1, 0, 1, 'table', ?2, 'Hemodynamics',
                 '[\"Hemodynamics\"]', 0.1, 0.2, 0.7, 0.1, 0.99,
                 'Example Sans', 12.0, 0, 0, ?3, 10
             )",
            params![
                DOCUMENT_ID,
                "Cardiac output equals heart rate multiplied by stroke volume.",
                "4".repeat(64),
            ],
        )
        .expect("version-1 semantic element should be seeded");
    connection
        .execute(
            "INSERT INTO document_semantic_chunks (
                 document_id, chunk_index, text, full_text, page_numbers_json,
                 bounding_boxes_json, element_types_json, heading_context,
                 token_estimate, is_oversized, text_sha256, created_at
             ) VALUES (
                 ?1, 0, 'Cardiac output', ?2, '[1]',
                 '[{\"pageNumber\":1}]', '[\"table\"]',
                 'Hemodynamics', 18, 0, ?3, 10
             )",
            params![
                DOCUMENT_ID,
                "Cardiac output equals heart rate multiplied by stroke volume.",
                "5".repeat(64),
            ],
        )
        .expect("version-1 semantic chunk should be seeded");
}

fn assert_canonical_pdf_backfill(connection: &mut Connection) -> Result<(), CommandError> {
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(CommandError::database)?;
    assert_eq!(version, LATEST_SCHEMA_VERSION);

    let migration_rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .map_err(CommandError::database)?;
    assert_eq!(migration_rows, 2);

    let counts: (i64, i64, i64, i64, String) = connection
        .query_row(
            "SELECT source_file_count, source_unit_count,
                    extracted_source_unit_count, source_asset_count,
                    adapter_backend
             FROM documents WHERE id = ?1",
            [DOCUMENT_ID],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .map_err(CommandError::database)?;
    assert_eq!(counts, (1, 1, 1, 1, "legacy-pdf-adapter-v0".to_string()));

    let source_file: (String, String, String, i64, String) = connection
        .query_row(
            "SELECT original_filename, media_type, relative_path, byte_size, sha256
             FROM document_source_files
             WHERE document_id = ?1 AND file_ordinal = 1",
            [DOCUMENT_ID],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .map_err(CommandError::database)?;
    assert_eq!(source_file.0, "cardio.pdf");
    assert_eq!(source_file.1, "application/pdf");
    assert_eq!(source_file.2, format!("documents/{DOCUMENT_ID}/source.pdf"));
    assert_eq!(source_file.3, 100);
    assert_eq!(source_file.4, "1".repeat(64));

    let unit: (String, String, String, String) = connection
        .query_row(
            "SELECT unit_kind, label, plain_text, locator_json
             FROM document_source_units
             WHERE document_id = ?1 AND unit_number = 1",
            [DOCUMENT_ID],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(CommandError::database)?;
    assert_eq!(unit.0, "page");
    assert_eq!(unit.1, "Page 1");
    assert!(unit.2.contains("stroke volume"));
    let locator: SourceLocator =
        serde_json::from_str(&unit.3).map_err(CommandError::serialization)?;
    assert_eq!(locator.unit_kind, SourceUnitKind::Page);
    assert_eq!(locator.unit_numbers, vec![1]);
    assert_eq!(locator.asset_ids, vec!["unit-render:000001"]);

    let asset: (String, String, i64, String) = connection
        .query_row(
            "SELECT asset_id, role, source_unit_number, relative_path
             FROM document_source_assets
             WHERE document_id = ?1",
            [DOCUMENT_ID],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(CommandError::database)?;
    assert_eq!(asset.0, "unit-render:000001");
    assert_eq!(asset.1, "unit_render");
    assert_eq!(asset.2, 1);
    assert!(asset.3.ends_with("page-000001.png"));

    let block: (String, String, i64, String, String) = connection
        .query_row(
            "SELECT role, text, source_unit_number, asset_ids_json, content_sha256
             FROM document_source_blocks
             WHERE document_id = ?1 AND block_index = 0",
            [DOCUMENT_ID],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .map_err(CommandError::database)?;
    assert_eq!(block.0, "table");
    assert!(block.1.contains("Cardiac output"));
    assert_eq!(block.2, 1);
    assert_eq!(
        serde_json::from_str::<Value>(&block.3).map_err(CommandError::serialization)?,
        serde_json::json!(["unit-render:000001"])
    );
    assert_ne!(block.4, "4".repeat(64));
    assert_eq!(block.4, EXPECTED_CANONICAL_BLOCK_HASH);

    let chunk: (String, String, String) = connection
        .query_row(
            "SELECT source_locator_json, visual_asset_ids_json, content_sha256
             FROM document_semantic_chunk_sources
             WHERE document_id = ?1 AND chunk_index = 0",
            [DOCUMENT_ID],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(CommandError::database)?;
    let chunk_locator: SourceLocator =
        serde_json::from_str(&chunk.0).map_err(CommandError::serialization)?;
    assert_eq!(chunk_locator.unit_numbers, vec![1]);
    assert_eq!(chunk_locator.asset_ids, vec!["unit-render:000001"]);
    assert_eq!(
        serde_json::from_str::<Value>(&chunk.1).map_err(CommandError::serialization)?,
        serde_json::json!(["unit-render:000001"])
    );
    assert_ne!(chunk.2, "5".repeat(64));
    assert_eq!(chunk.2, EXPECTED_CANONICAL_CHUNK_HASH);

    let semantic_repair: (String, String, i64) = connection
        .query_row(
            "SELECT name, checksum, changed_rows FROM data_repairs WHERE version = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(CommandError::database)?;
    assert_eq!(semantic_repair.0, "canonical_semantic_chunk_hashes");
    assert_eq!(semantic_repair.1, CANONICAL_SEMANTIC_HASH_REPAIR_CHECKSUM);
    assert_eq!(semantic_repair.2, 1);

    let block_repair: (String, String, i64) = connection
        .query_row(
            "SELECT name, checksum, changed_rows FROM data_repairs WHERE version = 2",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(CommandError::database)?;
    assert_eq!(block_repair.0, "canonical_block_hashes");
    assert_eq!(block_repair.1, CANONICAL_BLOCK_HASH_REPAIR_CHECKSUM);
    assert_eq!(block_repair.2, 1);

    let legacy_page_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM document_pages WHERE document_id = ?1",
            [DOCUMENT_ID],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    assert_eq!(
        legacy_page_count, 1,
        "compatibility projection must remain intact"
    );

    let foreign_key_violations: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .map_err(CommandError::database)?;
    assert_eq!(foreign_key_violations, 0);
    Ok(())
}
