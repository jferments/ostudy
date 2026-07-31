//! Version 2 SQLite deck packages.
//!
//! `.studydeck` files remain ordinary SQLite databases. Version 2 adds tags,
//! safe field templates, generated-card directions, and image layout metadata.
//! Packages are opened read-only with `trusted_schema=OFF`, integrity checked,
//! size limited, and every document/template/identifier is revalidated before
//! entering the user's library. Version 1 packages continue through the legacy
//! importer exposed by `transfer.rs`.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

use crate::{
    assets::{detect_image_bytes, validate_original_filename},
    cards::{collect_asset_ids, validate_content},
    database::Database,
    decks::{get_deck_from_connection, validate_description, validate_name, DeckDto},
    error::CommandError,
    paths::AppPaths,
    phase2::{
        compute_duplicate_key, refresh_note_search, render_imported_template_card,
        validate_imported_fill_blank,
        validate_imported_template, validate_imported_template_metadata,
        validate_imported_template_values, TemplateCardSpec,
    },
    scheduler::insert_new_schedule,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const PACKAGE_FORMAT: &str = "studydeck-sqlite";
const PACKAGE_FORMAT_VERSION: i64 = 2;
const MAX_PACKAGE_FILE_BYTES: u64 = 640 * 1024 * 1024;
const MAX_PACKAGE_ASSET_BYTES: usize = 512 * 1024 * 1024;
const MAX_PACKAGE_SINGLE_ASSET_BYTES: usize = 25 * 1024 * 1024;
const MAX_PACKAGE_NOTES: usize = 100_000;
const MAX_PACKAGE_TEMPLATES: usize = 1_000;
const MAX_PACKAGE_TAGS: usize = 20_000;
const MAX_PACKAGE_ASSETS: usize = 200_000;
const MAX_PACKAGE_RELATION_ROWS: usize = 2_000_000;

const PACKAGE_SCHEMA: &str = r#"
PRAGMA foreign_keys = ON;
CREATE TABLE package_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;
CREATE TABLE deck (
    source_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    revision INTEGER NOT NULL
) STRICT;
CREATE TABLE templates (
    source_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    fields_json TEXT NOT NULL,
    cards_json TEXT NOT NULL,
    is_builtin INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    revision INTEGER NOT NULL
) STRICT;
CREATE TABLE notes (
    source_id TEXT PRIMARY KEY,
    source_deck_id TEXT NOT NULL,
    note_type TEXT NOT NULL,
    content_format TEXT NOT NULL,
    content_schema_version INTEGER NOT NULL,
    front_doc TEXT NOT NULL,
    back_doc TEXT NOT NULL,
    front_text TEXT NOT NULL,
    back_text TEXT NOT NULL,
    template_id TEXT,
    fields_json TEXT NOT NULL,
    duplicate_key TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    revision INTEGER NOT NULL
) STRICT;
CREATE TABLE cards (
    source_id TEXT PRIMARY KEY,
    source_note_id TEXT NOT NULL,
    direction TEXT NOT NULL,
    template_ordinal INTEGER NOT NULL,
    display_name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;
CREATE TABLE tags (
    source_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL
) STRICT;
CREATE TABLE note_tags (
    source_note_id TEXT NOT NULL,
    source_tag_id TEXT NOT NULL,
    PRIMARY KEY(source_note_id, source_tag_id)
) STRICT;
CREATE TABLE assets (
    source_id TEXT PRIMARY KEY,
    original_filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    byte_size INTEGER NOT NULL,
    data BLOB NOT NULL
) STRICT;
CREATE TABLE note_assets (
    source_note_id TEXT NOT NULL,
    source_asset_id TEXT NOT NULL,
    side TEXT NOT NULL,
    PRIMARY KEY(source_note_id, source_asset_id, side)
) STRICT;
"#;


#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpectedPackageColumn {
    name: &'static str,
    declared_type: &'static str,
    not_null: i64,
    primary_key_ordinal: i64,
}

#[derive(Clone, Copy)]
struct ExpectedPackageTable {
    name: &'static str,
    columns: &'static [ExpectedPackageColumn],
}

#[derive(Debug, Eq, PartialEq)]
struct ActualPackageColumn {
    name: String,
    declared_type: String,
    not_null: i64,
    primary_key_ordinal: i64,
}

const fn package_column(
    name: &'static str,
    declared_type: &'static str,
    not_null: i64,
    primary_key_ordinal: i64,
) -> ExpectedPackageColumn {
    ExpectedPackageColumn {
        name,
        declared_type,
        not_null,
        primary_key_ordinal,
    }
}

const PACKAGE_TABLES: &[ExpectedPackageTable] = &[
    ExpectedPackageTable {
        name: "package_metadata",
        columns: &[
            package_column("key", "TEXT", 1, 1),
            package_column("value", "TEXT", 1, 0),
        ],
    },
    ExpectedPackageTable {
        name: "deck",
        columns: &[
            package_column("source_id", "TEXT", 1, 1),
            package_column("name", "TEXT", 1, 0),
            package_column("description", "TEXT", 1, 0),
            package_column("created_at", "INTEGER", 1, 0),
            package_column("updated_at", "INTEGER", 1, 0),
            package_column("revision", "INTEGER", 1, 0),
        ],
    },
    ExpectedPackageTable {
        name: "templates",
        columns: &[
            package_column("source_id", "TEXT", 1, 1),
            package_column("name", "TEXT", 1, 0),
            package_column("description", "TEXT", 1, 0),
            package_column("fields_json", "TEXT", 1, 0),
            package_column("cards_json", "TEXT", 1, 0),
            package_column("is_builtin", "INTEGER", 1, 0),
            package_column("created_at", "INTEGER", 1, 0),
            package_column("updated_at", "INTEGER", 1, 0),
            package_column("revision", "INTEGER", 1, 0),
        ],
    },
    ExpectedPackageTable {
        name: "notes",
        columns: &[
            package_column("source_id", "TEXT", 1, 1),
            package_column("source_deck_id", "TEXT", 1, 0),
            package_column("note_type", "TEXT", 1, 0),
            package_column("content_format", "TEXT", 1, 0),
            package_column("content_schema_version", "INTEGER", 1, 0),
            package_column("front_doc", "TEXT", 1, 0),
            package_column("back_doc", "TEXT", 1, 0),
            package_column("front_text", "TEXT", 1, 0),
            package_column("back_text", "TEXT", 1, 0),
            package_column("template_id", "TEXT", 0, 0),
            package_column("fields_json", "TEXT", 1, 0),
            package_column("duplicate_key", "TEXT", 1, 0),
            package_column("created_at", "INTEGER", 1, 0),
            package_column("updated_at", "INTEGER", 1, 0),
            package_column("revision", "INTEGER", 1, 0),
        ],
    },
    ExpectedPackageTable {
        name: "cards",
        columns: &[
            package_column("source_id", "TEXT", 1, 1),
            package_column("source_note_id", "TEXT", 1, 0),
            package_column("direction", "TEXT", 1, 0),
            package_column("template_ordinal", "INTEGER", 1, 0),
            package_column("display_name", "TEXT", 1, 0),
            package_column("created_at", "INTEGER", 1, 0),
            package_column("updated_at", "INTEGER", 1, 0),
        ],
    },
    ExpectedPackageTable {
        name: "tags",
        columns: &[
            package_column("source_id", "TEXT", 1, 1),
            package_column("name", "TEXT", 1, 0),
            package_column("normalized_name", "TEXT", 1, 0),
        ],
    },
    ExpectedPackageTable {
        name: "note_tags",
        columns: &[
            package_column("source_note_id", "TEXT", 1, 1),
            package_column("source_tag_id", "TEXT", 1, 2),
        ],
    },
    ExpectedPackageTable {
        name: "assets",
        columns: &[
            package_column("source_id", "TEXT", 1, 1),
            package_column("original_filename", "TEXT", 1, 0),
            package_column("mime_type", "TEXT", 1, 0),
            package_column("byte_size", "INTEGER", 1, 0),
            package_column("data", "BLOB", 1, 0),
        ],
    },
    ExpectedPackageTable {
        name: "note_assets",
        columns: &[
            package_column("source_note_id", "TEXT", 1, 1),
            package_column("source_asset_id", "TEXT", 1, 2),
            package_column("side", "TEXT", 1, 3),
        ],
    },
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDeckV2Input {
    pub deck_id: String,
    pub destination_path: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDeckV2ResultDto {
    pub destination_path: String,
    pub card_count: i64,
    pub asset_count: i64,
    pub template_count: i64,
    pub tag_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportDeckV2ResultDto {
    pub deck: DeckDto,
    pub card_count: i64,
    pub asset_count: i64,
    pub template_count: i64,
    pub tag_count: i64,
}

#[derive(Clone)]
struct PackageDeck {
    source_id: String,
    name: String,
    description: String,
    created_at: i64,
    updated_at: i64,
    revision: i64,
}

#[derive(Clone)]
struct PackageTemplate {
    source_id: String,
    name: String,
    description: String,
    fields: Vec<String>,
    cards: Vec<TemplateCardSpec>,
    created_at: i64,
    updated_at: i64,
    revision: i64,
}

#[derive(Clone)]
struct PackageNote {
    source_id: String,
    note_type: String,
    content_format: String,
    content_schema_version: i64,
    front_doc: Value,
    back_doc: Value,
    front_text: String,
    back_text: String,
    template_id: Option<String>,
    fields: BTreeMap<String, String>,
    created_at: i64,
    updated_at: i64,
    revision: i64,
}

#[derive(Clone)]
struct PackageCard {
    source_id: String,
    source_note_id: String,
    direction: String,
    template_ordinal: i64,
    display_name: String,
    created_at: i64,
    updated_at: i64,
}

#[derive(Clone)]
struct PackageTag {
    source_id: String,
    name: String,
    normalized_name: String,
}

struct PackageAsset {
    source_id: String,
    original_filename: String,
    mime_type: String,
    byte_size: i64,
    data: Vec<u8>,
}

struct PackageData {
    deck: PackageDeck,
    templates: Vec<PackageTemplate>,
    notes: Vec<PackageNote>,
    cards: Vec<PackageCard>,
    tags: Vec<PackageTag>,
    note_tags: Vec<(String, String)>,
    assets: Vec<PackageAsset>,
    note_assets: Vec<(String, String, String)>,
}

struct TemporaryFileGuard {
    path: PathBuf,
}

impl Drop for TemporaryFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[tauri::command]
pub fn export_deck_v2(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ExportDeckV2Input,
) -> Result<ExportDeckV2ResultDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    let destination = normalize_export_path(&input.destination_path)?;
    database.with_connection(|connection| {
        export_from_connection(connection, &paths, &input.deck_id, &destination)
    })
}

#[tauri::command]
pub fn import_deck_v2(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    source_path: String,
) -> Result<ImportDeckV2ResultDto, CommandError> {
    let source = fs::canonicalize(source_path).map_err(CommandError::io)?;
    if !source.is_file() {
        return Err(CommandError::invalid_package(
            "Select a .studydeck SQLite file to import.",
        ));
    }
    let length = fs::metadata(&source).map_err(CommandError::io)?.len();
    if length == 0 || length > MAX_PACKAGE_FILE_BYTES {
        return Err(CommandError::invalid_package(
            "The selected deck is empty or exceeds the 640 MB import limit.",
        ));
    }
    let package = read_package(&source)?;
    database.with_connection(|connection| import_package(connection, &paths, package))
}

fn export_from_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    deck_id: &str,
    destination: &Path,
) -> Result<ExportDeckV2ResultDto, CommandError> {
    let deck = get_deck_from_connection(connection, deck_id)?;
    let parent = destination.parent().ok_or_else(|| {
        CommandError::new("invalid_export_path", "Choose a valid export folder.")
    })?;
    if !parent.exists() {
        return Err(CommandError::new(
            "invalid_export_path",
            "The selected export folder no longer exists.",
        ));
    }
    let temporary = temporary_export_path(destination);
    let _ = fs::remove_file(&temporary);
    let _guard = TemporaryFileGuard {
        path: temporary.clone(),
    };
    let mut package = Connection::open(&temporary).map_err(CommandError::database)?;
    package
        .execute_batch(
            "PRAGMA foreign_keys = ON;\n\
             PRAGMA journal_mode = DELETE;\n\
             PRAGMA synchronous = FULL;\n\
             PRAGMA trusted_schema = OFF;",
        )
        .map_err(CommandError::database)?;
    package
        .execute_batch(PACKAGE_SCHEMA)
        .map_err(CommandError::database)?;
    let transaction = package
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let exported_at = unix_time_millis()?;
    for (key, value) in [
        ("format", PACKAGE_FORMAT.to_string()),
        ("formatVersion", PACKAGE_FORMAT_VERSION.to_string()),
        ("exportedAt", exported_at.to_string()),
        ("applicationVersion", env!("CARGO_PKG_VERSION").to_string()),
    ] {
        transaction
            .execute(
                "INSERT INTO package_metadata(key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .map_err(CommandError::database)?;
    }
    transaction
        .execute(
            "INSERT INTO deck(source_id, name, description, created_at, updated_at, revision)\n\
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                deck.id,
                deck.name,
                deck.description,
                deck.created_at,
                deck.updated_at,
                deck.revision,
            ],
        )
        .map_err(CommandError::database)?;

    let mut template_statement = connection
        .prepare(
            "SELECT DISTINCT t.id, t.name, t.description, t.fields_json, t.cards_json,\n\
                    t.is_builtin, t.created_at, t.updated_at, t.revision\n\
             FROM card_templates t\n\
             JOIN notes n ON n.template_id = t.id\n\
             WHERE n.deck_id = ?1 AND n.deleted_at IS NULL AND t.deleted_at IS NULL\n\
             ORDER BY t.id",
        )
        .map_err(CommandError::database)?;
    let templates = template_statement
        .query_map([deck_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(template_statement);
    for row in &templates {
        transaction
            .execute(
                "INSERT INTO templates(source_id, name, description, fields_json, cards_json,\n\
                                        is_builtin, created_at, updated_at, revision)\n\
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8],
            )
            .map_err(CommandError::database)?;
    }

    let mut note_statement = connection
        .prepare(
            "SELECT id, deck_id, note_type, content_format, content_schema_version,\n\
                    front_doc, back_doc, front_text, back_text, template_id, fields_json,\n\
                    duplicate_key, created_at, updated_at, revision\n\
             FROM notes WHERE deck_id = ?1 AND deleted_at IS NULL\n\
             ORDER BY created_at, id",
        )
        .map_err(CommandError::database)?;
    let notes = note_statement
        .query_map([deck_id], |row| {
            Ok((
                row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
                row.get::<_, String>(3)?, row.get::<_, i64>(4)?, row.get::<_, String>(5)?,
                row.get::<_, String>(6)?, row.get::<_, String>(7)?, row.get::<_, String>(8)?,
                row.get::<_, Option<String>>(9)?, row.get::<_, String>(10)?,
                row.get::<_, String>(11)?, row.get::<_, i64>(12)?, row.get::<_, i64>(13)?,
                row.get::<_, i64>(14)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(note_statement);
    for row in &notes {
        transaction
            .execute(
                "INSERT INTO notes(\n\
                     source_id, source_deck_id, note_type, content_format, content_schema_version,\n\
                     front_doc, back_doc, front_text, back_text, template_id, fields_json,\n\
                     duplicate_key, created_at, updated_at, revision\n\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8, row.9,
                    row.10, row.11, row.12, row.13, row.14,
                ],
            )
            .map_err(CommandError::database)?;
    }

    let mut card_statement = connection
        .prepare(
            "SELECT c.id, c.note_id, c.direction, c.template_ordinal, c.display_name,\n\
                    c.created_at, c.updated_at\n\
             FROM cards c JOIN notes n ON n.id = c.note_id\n\
             WHERE c.deck_id = ?1 AND c.deleted_at IS NULL AND n.deleted_at IS NULL\n\
             ORDER BY c.created_at, c.id",
        )
        .map_err(CommandError::database)?;
    let cards = card_statement
        .query_map([deck_id], |row| {
            Ok((
                row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?, row.get::<_, String>(4)?, row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(card_statement);
    for row in &cards {
        transaction
            .execute(
                "INSERT INTO cards(source_id, source_note_id, direction, template_ordinal,\n\
                                   display_name, created_at, updated_at)\n\
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![row.0, row.1, row.2, row.3, row.4, row.5, row.6],
            )
            .map_err(CommandError::database)?;
    }

    let mut tag_statement = connection
        .prepare(
            "SELECT DISTINCT t.id, t.name, t.normalized_name\n\
             FROM tags t JOIN note_tags nt ON nt.tag_id = t.id\n\
             JOIN notes n ON n.id = nt.note_id\n\
             WHERE n.deck_id = ?1 AND n.deleted_at IS NULL\n\
             ORDER BY t.id",
        )
        .map_err(CommandError::database)?;
    let tags = tag_statement
        .query_map([deck_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(tag_statement);
    for row in &tags {
        transaction
            .execute(
                "INSERT INTO tags(source_id, name, normalized_name) VALUES (?1, ?2, ?3)",
                params![row.0, row.1, row.2],
            )
            .map_err(CommandError::database)?;
    }

    let mut note_tag_statement = connection
        .prepare(
            "SELECT nt.note_id, nt.tag_id FROM note_tags nt\n\
             JOIN notes n ON n.id = nt.note_id\n\
             WHERE n.deck_id = ?1 AND n.deleted_at IS NULL\n\
             ORDER BY nt.note_id, nt.tag_id",
        )
        .map_err(CommandError::database)?;
    let note_tags = note_tag_statement
        .query_map([deck_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(note_tag_statement);
    for row in &note_tags {
        transaction
            .execute(
                "INSERT INTO note_tags(source_note_id, source_tag_id) VALUES (?1, ?2)",
                params![row.0, row.1],
            )
            .map_err(CommandError::database)?;
    }

    let mut link_statement = connection
        .prepare(
            "SELECT na.note_id, na.asset_id, na.side FROM note_assets na\n\
             JOIN notes n ON n.id = na.note_id\n\
             WHERE n.deck_id = ?1 AND n.deleted_at IS NULL\n\
             ORDER BY na.note_id, na.asset_id, na.side",
        )
        .map_err(CommandError::database)?;
    let links = link_statement
        .query_map([deck_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(link_statement);
    for row in &links {
        transaction
            .execute(
                "INSERT INTO note_assets(source_note_id, source_asset_id, side)\n\
                 VALUES (?1, ?2, ?3)",
                params![row.0, row.1, row.2],
            )
            .map_err(CommandError::database)?;
    }

    let mut asset_statement = connection
        .prepare(
            "SELECT DISTINCT a.id, a.original_filename, a.mime_type, a.byte_size, a.relative_path\n\
             FROM assets a JOIN note_assets na ON na.asset_id = a.id\n\
             JOIN notes n ON n.id = na.note_id\n\
             WHERE n.deck_id = ?1 AND n.deleted_at IS NULL AND a.deleted_at IS NULL\n\
             ORDER BY a.id",
        )
        .map_err(CommandError::database)?;
    let assets = asset_statement
        .query_map([deck_id], |row| {
            Ok((
                row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?, row.get::<_, String>(4)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(asset_statement);
    let mut total_bytes = 0_usize;
    for row in &assets {
        let data = fs::read(paths.resolve_relative(&row.4)).map_err(CommandError::io)?;
        validate_export_asset(&row.1, &row.2, row.3, &data, &mut total_bytes)?;
        transaction
            .execute(
                "INSERT INTO assets(source_id, original_filename, mime_type, byte_size, data)\n\
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![row.0, row.1, row.2, row.3, data],
            )
            .map_err(CommandError::database)?;
    }

    transaction.commit().map_err(CommandError::database)?;
    let integrity: String = package
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(CommandError::database)?;
    if integrity != "ok" {
        return Err(CommandError::new(
            "export_validation_failed",
            "The exported SQLite deck did not pass its integrity check.",
        ));
    }
    drop(package);
    OpenOptions::new()
        .read(true)
        .open(&temporary)
        .and_then(|file| file.sync_all())
        .map_err(CommandError::io)?;
    replace_export_file(&temporary, destination)?;
    Ok(ExportDeckV2ResultDto {
        destination_path: destination.to_string_lossy().into_owned(),
        card_count: i64::try_from(cards.len()).unwrap_or(i64::MAX),
        asset_count: i64::try_from(assets.len()).unwrap_or(i64::MAX),
        template_count: i64::try_from(templates.len()).unwrap_or(i64::MAX),
        tag_count: i64::try_from(tags.len()).unwrap_or(i64::MAX),
    })
}

fn read_package(source: &Path) -> Result<PackageData, CommandError> {
    let package = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| CommandError::invalid_package(format!("Could not open the deck: {error}")))?;
    package
        .execute_batch(
            "PRAGMA trusted_schema = OFF;\n\
             PRAGMA foreign_keys = ON;\n\
             PRAGMA query_only = ON;",
        )
        .map_err(|error| {
            CommandError::invalid_package(format!("Could not secure the deck reader: {error}"))
        })?;
    let integrity: String = package
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| CommandError::invalid_package(format!("Integrity check failed: {error}")))?;
    if integrity != "ok" {
        return Err(CommandError::invalid_package(
            "The selected SQLite deck is damaged.",
        ));
    }
    validate_package_schema(&package)?;
    let metadata = validate_package_metadata(&package)?;
    let format = metadata.get("format").expect("metadata key was validated");
    let version = metadata
        .get("formatVersion")
        .expect("metadata key was validated");
    if format.as_str() != PACKAGE_FORMAT {
        return Err(CommandError::invalid_package(
            "This file is not an ostudy SQLite deck package.",
        ));
    }
    if version.as_str() == "1" {
        return Err(CommandError::new(
            "legacy_deck_package",
            "This is a version 1 deck package and must be opened with the compatibility importer.",
        ));
    }
    if version.as_str() != PACKAGE_FORMAT_VERSION.to_string() {
        return Err(CommandError::invalid_package(
            "This deck was created by an unsupported ostudy package version.",
        ));
    }

    enforce_package_row_limit(
        &package,
        "templates",
        MAX_PACKAGE_TEMPLATES,
        "The deck contains too many card templates.",
    )?;
    enforce_package_row_limit(
        &package,
        "notes",
        MAX_PACKAGE_NOTES,
        "This deck contains more than 100,000 notes.",
    )?;
    enforce_package_row_limit(
        &package,
        "cards",
        MAX_PACKAGE_NOTES.saturating_mul(64),
        "The deck contains too many generated cards.",
    )?;
    enforce_package_row_limit(
        &package,
        "tags",
        MAX_PACKAGE_TAGS,
        "The deck contains too many tags.",
    )?;
    enforce_package_row_limit(
        &package,
        "assets",
        MAX_PACKAGE_ASSETS,
        "The deck contains too many image records.",
    )?;
    enforce_package_row_limit(
        &package,
        "note_tags",
        MAX_PACKAGE_RELATION_ROWS,
        "The deck contains too many note-tag relationships.",
    )?;
    enforce_package_row_limit(
        &package,
        "note_assets",
        MAX_PACKAGE_RELATION_ROWS,
        "The deck contains too many note-image relationships.",
    )?;

    let deck_count: i64 = package
        .query_row("SELECT COUNT(*) FROM deck", [], |row| row.get(0))
        .map_err(CommandError::database)?;
    if deck_count != 1 {
        return Err(CommandError::invalid_package(
            "A deck package must contain exactly one deck.",
        ));
    }
    let deck = package
        .query_row(
            "SELECT source_id, name, description, created_at, updated_at, revision FROM deck",
            [],
            |row| {
                Ok(PackageDeck {
                    source_id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    revision: row.get(5)?,
                })
            },
        )
        .map_err(CommandError::database)?;
    validate_entity_id(&deck.source_id, "exported deck")?;
    validate_name(deck.name.clone())
        .map_err(|error| CommandError::invalid_package(error.message))?;
    validate_description(deck.description.clone())
        .map_err(|error| CommandError::invalid_package(error.message))?;
    validate_timestamps(deck.created_at, deck.updated_at, "deck")?;
    if deck.revision < 1 {
        return Err(CommandError::invalid_package(
            "The exported deck has an invalid revision.",
        ));
    }

    let templates = read_templates(&package)?;
    let template_map = templates
        .iter()
        .map(|template| (template.source_id.as_str(), template))
        .collect::<HashMap<_, _>>();
    let notes = read_notes(&package, &deck, &template_map)?;
    let cards = read_cards(&package, &notes, &template_map)?;
    let tags = read_tags(&package)?;
    let note_tags = read_note_tags(&package, &notes, &tags)?;
    let assets = read_assets(&package)?;
    let note_assets = read_note_assets(&package, &notes, &assets)?;
    validate_document_asset_links(&notes, &assets, &note_assets)?;
    Ok(PackageData {
        deck,
        templates,
        notes,
        cards,
        tags,
        note_tags,
        assets,
        note_assets,
    })
}

fn read_templates(package: &Connection) -> Result<Vec<PackageTemplate>, CommandError> {
    let mut statement = package
        .prepare(
            "SELECT source_id, name, description, fields_json, cards_json, is_builtin,\n\
                    created_at, updated_at, revision FROM templates ORDER BY source_id",
        )
        .map_err(CommandError::database)?;
    let raw = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
                row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?, row.get::<_, i64>(7)?, row.get::<_, i64>(8)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if raw.len() > MAX_PACKAGE_TEMPLATES {
        return Err(CommandError::invalid_package(
            "The deck contains too many card templates.",
        ));
    }
    raw.into_iter()
        .map(|row| {
            validate_entity_id(&row.0, "exported card template")?;
            validate_timestamps(row.6, row.7, "card template")?;
            if row.8 < 1 || !matches!(row.5, 0 | 1) {
                return Err(CommandError::invalid_package(
                    "The deck contains an invalid card template record.",
                ));
            }
            let fields: Vec<String> = serde_json::from_str(&row.3).map_err(|error| {
                CommandError::invalid_package(format!("Invalid template fields: {error}"))
            })?;
            let cards: Vec<TemplateCardSpec> = serde_json::from_str(&row.4).map_err(|error| {
                CommandError::invalid_package(format!("Invalid template cards: {error}"))
            })?;
            let (name, description) = validate_imported_template_metadata(&row.1, &row.2)
                .map_err(|error| CommandError::invalid_package(error.message))?;
            let (fields, cards) = validate_imported_template(fields, cards)
                .map_err(|error| CommandError::invalid_package(error.message))?;
            Ok(PackageTemplate {
                source_id: row.0,
                name,
                description,
                fields,
                cards,
                created_at: row.6,
                updated_at: row.7,
                revision: row.8,
            })
        })
        .collect()
}

fn read_notes(
    package: &Connection,
    deck: &PackageDeck,
    templates: &HashMap<&str, &PackageTemplate>,
) -> Result<Vec<PackageNote>, CommandError> {
    let mut statement = package
        .prepare(
            "SELECT source_id, source_deck_id, note_type, content_format,\n\
                    content_schema_version, front_doc, back_doc, front_text, back_text,\n\
                    template_id, fields_json, duplicate_key, created_at, updated_at, revision\n\
             FROM notes ORDER BY created_at, source_id",
        )
        .map_err(CommandError::database)?;
    let raw = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
                row.get::<_, String>(3)?, row.get::<_, i64>(4)?, row.get::<_, String>(5)?,
                row.get::<_, String>(6)?, row.get::<_, String>(7)?, row.get::<_, String>(8)?,
                row.get::<_, Option<String>>(9)?, row.get::<_, String>(10)?,
                row.get::<_, String>(11)?, row.get::<_, i64>(12)?, row.get::<_, i64>(13)?,
                row.get::<_, i64>(14)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if raw.len() > MAX_PACKAGE_NOTES {
        return Err(CommandError::invalid_package(
            "This deck contains more than 100,000 notes.",
        ));
    }
    raw.into_iter()
        .map(|row| {
            validate_entity_id(&row.0, "exported note")?;
            if row.1 != deck.source_id
                || !matches!(row.2.as_str(), "basic" | "cloze" | "image_occlusion")
                || row.3 != "tiptap-json"
                || row.4 != 1
            {
                return Err(CommandError::invalid_package(
                    "The deck contains an unsupported note format.",
                ));
            }
            let front_doc: Value = serde_json::from_str(&row.5).map_err(|error| {
                CommandError::invalid_package(format!("Invalid question document: {error}"))
            })?;
            let back_doc: Value = serde_json::from_str(&row.6).map_err(|error| {
                CommandError::invalid_package(format!("Invalid answer document: {error}"))
            })?;
            let validated = validate_content(
                front_doc.clone(),
                back_doc.clone(),
                row.7.clone(),
                row.8.clone(),
            )
            .map_err(|error| CommandError::invalid_package(error.message))?;
            let mut fields: BTreeMap<String, String> = serde_json::from_str(&row.10).map_err(|error| {
                CommandError::invalid_package(format!("Invalid note fields: {error}"))
            })?;
            if let Some(template_id) = row.9.as_deref() {
                if row.2 != "basic" {
                    return Err(CommandError::invalid_package(
                        "Template notes must use the basic note format.",
                    ));
                }
                validate_entity_id(template_id, "exported card template")?;
                let template = templates.get(template_id).ok_or_else(|| {
                    CommandError::invalid_package(
                        "A note refers to a card template not included in the package.",
                    )
                })?;
                fields = validate_imported_template_values(&template.fields, &fields)
                    .map_err(|error| CommandError::invalid_package(error.message))?;
                let first = template.cards.first().ok_or_else(|| {
                    CommandError::invalid_package(
                        "An imported template does not generate any cards.",
                    )
                })?;
                let (expected_front, expected_back) = render_imported_template_card(first, &fields)
                    .map_err(|error| CommandError::invalid_package(error.message))?;
                if validated.front_text != expected_front || validated.back_text != expected_back {
                    return Err(CommandError::invalid_package(
                        "A template note does not match its validated template fields.",
                    ));
                }
            } else if row.2 == "cloze" {
                let source = fields.get("fill in the blank").ok_or_else(|| {
                    CommandError::invalid_package(
                        "A fill-in-the-blank note is missing its source text.",
                    )
                })?;
                let answers = validate_imported_fill_blank(source)
                    .map_err(|error| CommandError::invalid_package(error.message))?;
                if fields.len() != 1
                    || source != &validated.front_text
                    || answers.join(" · ") != validated.back_text
                {
                    return Err(CommandError::invalid_package(
                        "A fill-in-the-blank note contains inconsistent source text.",
                    ));
                }
            } else if !fields.is_empty() {
                return Err(CommandError::invalid_package(
                    "A non-template note contains unexpected template fields.",
                ));
            }
            validate_timestamps(row.12, row.13, "note")?;
            if row.14 < 1 || row.11.len() > 128 || row.11.contains('\0') {
                return Err(CommandError::invalid_package(
                    "A note contains invalid revision or duplicate metadata.",
                ));
            }
            Ok(PackageNote {
                source_id: row.0,
                note_type: row.2,
                content_format: row.3,
                content_schema_version: row.4,
                front_doc,
                back_doc,
                front_text: validated.front_text,
                back_text: validated.back_text,
                template_id: row.9,
                fields,
                created_at: row.12,
                updated_at: row.13,
                revision: row.14,
            })
        })
        .collect()
}

fn read_cards(
    package: &Connection,
    notes: &[PackageNote],
    templates: &HashMap<&str, &PackageTemplate>,
) -> Result<Vec<PackageCard>, CommandError> {
    let note_map = notes
        .iter()
        .map(|note| (note.source_id.as_str(), note))
        .collect::<HashMap<_, _>>();
    let mut statement = package
        .prepare(
            "SELECT source_id, source_note_id, direction, template_ordinal, display_name,\n\
                    created_at, updated_at FROM cards ORDER BY created_at, source_id",
        )
        .map_err(CommandError::database)?;
    let cards = statement
        .query_map([], |row| {
            Ok(PackageCard {
                source_id: row.get(0)?,
                source_note_id: row.get(1)?,
                direction: row.get(2)?,
                template_ordinal: row.get(3)?,
                display_name: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if cards.len() > MAX_PACKAGE_NOTES.saturating_mul(64) {
        return Err(CommandError::invalid_package(
            "The deck contains too many generated cards.",
        ));
    }

    let mut keys = BTreeSet::new();
    let mut by_note: HashMap<&str, Vec<&PackageCard>> = HashMap::new();
    for card in &cards {
        validate_entity_id(&card.source_id, "exported card")?;
        validate_entity_id(&card.source_note_id, "exported note")?;
        let normalized_name = card.display_name.trim();
        if !note_map.contains_key(card.source_note_id.as_str())
            || !matches!(
                card.direction.as_str(),
                "forward" | "reverse" | "cloze" | "image_occlusion"
            )
            || !(0..=10_000).contains(&card.template_ordinal)
            || normalized_name.is_empty()
            || normalized_name.chars().count() > 200
            || normalized_name.chars().any(char::is_control)
            || normalized_name != card.display_name
            || !keys.insert((
                card.source_note_id.clone(),
                card.direction.clone(),
                card.template_ordinal,
            ))
        {
            return Err(CommandError::invalid_package(
                "The deck contains invalid generated-card relationships.",
            ));
        }
        validate_timestamps(card.created_at, card.updated_at, "generated card")?;
        by_note
            .entry(card.source_note_id.as_str())
            .or_default()
            .push(card);
    }

    for note in notes {
        let generated = by_note.get(note.source_id.as_str()).ok_or_else(|| {
            CommandError::invalid_package("Every note must generate at least one card.")
        })?;
        if let Some(template_id) = note.template_id.as_deref() {
            let template = templates.get(template_id).ok_or_else(|| {
                CommandError::invalid_package(
                    "A generated card refers to a template not included in the package.",
                )
            })?;
            if generated.len() != template.cards.len() {
                return Err(CommandError::invalid_package(
                    "A template note has the wrong number of generated cards.",
                ));
            }
            for (ordinal, spec) in template.cards.iter().enumerate() {
                let ordinal = i64::try_from(ordinal).unwrap_or(i64::MAX);
                let card = generated
                    .iter()
                    .find(|card| card.direction == "forward" && card.template_ordinal == ordinal)
                    .ok_or_else(|| {
                        CommandError::invalid_package(
                            "A template note has an invalid generated-card ordinal.",
                        )
                    })?;
                if card.display_name != spec.name {
                    return Err(CommandError::invalid_package(
                        "A template card name does not match its validated template.",
                    ));
                }
            }
        } else if note.note_type == "cloze" {
            let source = note.fields.get("fill in the blank").ok_or_else(|| {
                CommandError::invalid_package(
                    "A fill-in-the-blank note is missing its source text.",
                )
            })?;
            let blank_count = validate_imported_fill_blank(source)
                .map_err(|error| CommandError::invalid_package(error.message))?
                .len();
            if generated.len() != blank_count {
                return Err(CommandError::invalid_package(
                    "A fill-in-the-blank note has the wrong number of generated cards.",
                ));
            }
            for ordinal in 0..blank_count {
                let ordinal_i64 = i64::try_from(ordinal).unwrap_or(i64::MAX);
                let expected_name = format!("Blank {}", ordinal + 1);
                let card = generated
                    .iter()
                    .find(|card| card.direction == "cloze" && card.template_ordinal == ordinal_i64)
                    .ok_or_else(|| {
                        CommandError::invalid_package(
                            "A fill-in-the-blank note has an invalid generated-card ordinal.",
                        )
                    })?;
                if card.display_name != expected_name {
                    return Err(CommandError::invalid_package(
                        "A fill-in-the-blank card has an invalid display name.",
                    ));
                }
            }
        } else if note.note_type == "image_occlusion" {
            if generated.iter().any(|card| card.direction != "image_occlusion") {
                return Err(CommandError::invalid_package(
                    "An image-occlusion note contains an invalid generated card.",
                ));
            }
            for ordinal in 0..generated.len() {
                let ordinal = i64::try_from(ordinal).unwrap_or(i64::MAX);
                if !generated
                    .iter()
                    .any(|card| card.direction == "image_occlusion" && card.template_ordinal == ordinal)
                {
                    return Err(CommandError::invalid_package(
                        "An image-occlusion note has a missing generated-card ordinal.",
                    ));
                }
            }
        } else {
            let forward = generated
                .iter()
                .filter(|card| card.direction == "forward" && card.template_ordinal == 0)
                .count();
            let reverse = generated
                .iter()
                .filter(|card| card.direction == "reverse" && card.template_ordinal == 0)
                .count();
            if forward != 1
                || reverse > 1
                || generated.len() != forward + reverse
                || generated
                    .iter()
                    .any(|card| match card.direction.as_str() {
                        "forward" => card.display_name != "Forward",
                        "reverse" => card.display_name != "Reverse",
                        _ => true,
                    })
            {
                return Err(CommandError::invalid_package(
                    "A basic note contains invalid forward or reverse cards.",
                ));
            }
        }
    }
    Ok(cards)
}

fn read_tags(package: &Connection) -> Result<Vec<PackageTag>, CommandError> {
    let mut statement = package
        .prepare("SELECT source_id, name, normalized_name FROM tags ORDER BY source_id")
        .map_err(CommandError::database)?;
    let tags = statement
        .query_map([], |row| {
            Ok(PackageTag {
                source_id: row.get(0)?,
                name: row.get(1)?,
                normalized_name: row.get(2)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if tags.len() > MAX_PACKAGE_TAGS {
        return Err(CommandError::invalid_package(
            "The deck contains too many tags.",
        ));
    }
    let mut normalized = BTreeSet::new();
    for tag in &tags {
        validate_entity_id(&tag.source_id, "exported tag")?;
        let expected = normalize_tag(&tag.name);
        if expected.is_empty()
            || expected.chars().count() > 80
            || tag.normalized_name != expected
            || !normalized.insert(expected)
        {
            return Err(CommandError::invalid_package(
                "The deck contains an invalid tag.",
            ));
        }
    }
    Ok(tags)
}

fn read_note_tags(
    package: &Connection,
    notes: &[PackageNote],
    tags: &[PackageTag],
) -> Result<Vec<(String, String)>, CommandError> {
    let note_ids = notes.iter().map(|note| note.source_id.as_str()).collect::<BTreeSet<_>>();
    let tag_ids = tags.iter().map(|tag| tag.source_id.as_str()).collect::<BTreeSet<_>>();
    let mut statement = package
        .prepare("SELECT source_note_id, source_tag_id FROM note_tags ORDER BY source_note_id, source_tag_id")
        .map_err(CommandError::database)?;
    let links = statement
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if links.iter().any(|link| {
        !note_ids.contains(link.0.as_str()) || !tag_ids.contains(link.1.as_str())
    }) {
        return Err(CommandError::invalid_package(
            "The deck contains an invalid note-tag relationship.",
        ));
    }
    Ok(links)
}

fn read_assets(package: &Connection) -> Result<Vec<PackageAsset>, CommandError> {
    let mut statement = package
        .prepare(
            "SELECT source_id, original_filename, mime_type, byte_size, data\n\
             FROM assets ORDER BY source_id",
        )
        .map_err(CommandError::database)?;
    let assets = statement
        .query_map([], |row| {
            Ok(PackageAsset {
                source_id: row.get(0)?,
                original_filename: row.get(1)?,
                mime_type: row.get(2)?,
                byte_size: row.get(3)?,
                data: row.get(4)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut total = 0_usize;
    for asset in &assets {
        validate_entity_id(&asset.source_id, "exported asset")?;
        validate_original_filename(&asset.original_filename)
            .map_err(|error| CommandError::invalid_package(error.message))?;
        validate_export_asset(
            &asset.original_filename,
            &asset.mime_type,
            asset.byte_size,
            &asset.data,
            &mut total,
        )
        .map_err(|error| CommandError::invalid_package(error.message))?;
    }
    Ok(assets)
}

fn read_note_assets(
    package: &Connection,
    notes: &[PackageNote],
    assets: &[PackageAsset],
) -> Result<Vec<(String, String, String)>, CommandError> {
    let note_ids = notes.iter().map(|note| note.source_id.as_str()).collect::<BTreeSet<_>>();
    let asset_ids = assets.iter().map(|asset| asset.source_id.as_str()).collect::<BTreeSet<_>>();
    let mut statement = package
        .prepare(
            "SELECT source_note_id, source_asset_id, side FROM note_assets\n\
             ORDER BY source_note_id, source_asset_id, side",
        )
        .map_err(CommandError::database)?;
    let links = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if links.iter().any(|link| {
        !note_ids.contains(link.0.as_str())
            || !asset_ids.contains(link.1.as_str())
            || !matches!(link.2.as_str(), "front" | "back")
    }) {
        return Err(CommandError::invalid_package(
            "The deck contains an invalid note-image relationship.",
        ));
    }
    Ok(links)
}

fn validate_document_asset_links(
    notes: &[PackageNote],
    assets: &[PackageAsset],
    links: &[(String, String, String)],
) -> Result<(), CommandError> {
    let asset_ids = assets.iter().map(|asset| asset.source_id.as_str()).collect::<BTreeSet<_>>();
    let actual = links.iter().cloned().collect::<BTreeSet<_>>();
    let mut expected = BTreeSet::new();
    for note in notes {
        for asset_id in collect_asset_ids(&note.front_doc)? {
            if !asset_ids.contains(asset_id.as_str()) {
                return Err(CommandError::invalid_package(
                    "A card refers to an image not included in the package.",
                ));
            }
            expected.insert((note.source_id.clone(), asset_id, "front".to_string()));
        }
        for asset_id in collect_asset_ids(&note.back_doc)? {
            if !asset_ids.contains(asset_id.as_str()) {
                return Err(CommandError::invalid_package(
                    "A card refers to an image not included in the package.",
                ));
            }
            expected.insert((note.source_id.clone(), asset_id, "back".to_string()));
        }
    }
    if actual != expected {
        return Err(CommandError::invalid_package(
            "The package image index does not match the validated card documents.",
        ));
    }
    Ok(())
}

fn import_package(
    connection: &mut Connection,
    paths: &AppPaths,
    package: PackageData,
) -> Result<ImportDeckV2ResultDto, CommandError> {
    let mut created_files = Vec::new();
    let result = (|| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let deck_id = generate_id(&transaction)?;
        let sort_order: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM decks WHERE deleted_at IS NULL",
                [],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "INSERT INTO decks(id, name, description, sort_order, created_at, updated_at, revision)\n\
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    deck_id,
                    package.deck.name,
                    package.deck.description,
                    sort_order,
                    package.deck.created_at,
                    package.deck.updated_at,
                    package.deck.revision,
                ],
            )
            .map_err(CommandError::database)?;

        let mut template_map = HashMap::new();
        for template in &package.templates {
            let fields_json =
                serde_json::to_string(&template.fields).map_err(CommandError::serialization)?;
            let cards_json =
                serde_json::to_string(&template.cards).map_err(CommandError::serialization)?;
            let existing: Option<(String, String, String)> = transaction
                .query_row(
                    "SELECT id, fields_json, cards_json FROM card_templates
                     WHERE id = ?1 AND deleted_at IS NULL",
                    [template.source_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .map_err(CommandError::database)?;
            let template_id = if let Some((id, _, _)) = existing.as_ref().filter(
                |(_, stored_fields, stored_cards)| {
                    stored_fields == &fields_json && stored_cards == &cards_json
                },
            ) {
                id.clone()
            } else {
                let id = generate_id(&transaction)?;
                let mut name = template.name.clone();
                if template_name_exists(&transaction, &name)? {
                    name = unique_imported_template_name(&transaction, &name)?;
                }
                transaction
                    .execute(
                        "INSERT INTO card_templates(\n\
                             id, name, description, fields_json, cards_json, is_builtin,\n\
                             created_at, updated_at, revision\n\
                         ) VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8)",
                        params![
                            id,
                            name,
                            template.description,
                            fields_json,
                            cards_json,
                            template.created_at,
                            template.updated_at,
                            template.revision,
                        ],
                    )
                    .map_err(CommandError::database)?;
                id
            };
            template_map.insert(template.source_id.clone(), template_id);
        }

        let mut asset_map = HashMap::new();
        for asset in &package.assets {
            let id = generate_id(&transaction)?;
            let (mime, extension) = detect_image_bytes(&asset.data)?;
            if mime != asset.mime_type {
                return Err(CommandError::invalid_package(
                    "An imported image does not match its declared file type.",
                ));
            }
            let relative_path = AppPaths::image_relative_path(&id, extension);
            let final_path = paths.resolve_relative(&relative_path);
            let temporary_path = final_path.with_extension(format!("{extension}.tmp"));
            if let Some(parent) = final_path.parent() {
                fs::create_dir_all(parent).map_err(CommandError::io)?;
            }
            let _ = fs::remove_file(&temporary_path);
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary_path)
                .map_err(CommandError::io)?;
            file.write_all(&asset.data).map_err(CommandError::io)?;
            file.sync_all().map_err(CommandError::io)?;
            drop(file);
            fs::rename(&temporary_path, &final_path).map_err(|error| {
                let _ = fs::remove_file(&temporary_path);
                CommandError::io(error)
            })?;
            created_files.push(final_path);
            transaction
                .execute(
                    "INSERT INTO assets(id, original_filename, mime_type, byte_size, relative_path, created_at)\n\
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        id,
                        asset.original_filename,
                        asset.mime_type,
                        asset.byte_size,
                        relative_path,
                        unix_time_millis()?,
                    ],
                )
                .map_err(CommandError::database)?;
            asset_map.insert(asset.source_id.clone(), id);
        }

        let reverse_note_ids = package
            .cards
            .iter()
            .filter(|card| card.direction == "reverse")
            .map(|card| card.source_note_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut note_map = HashMap::new();
        for note in &package.notes {
            let note_id = generate_id(&transaction)?;
            let mut front_doc = note.front_doc.clone();
            let mut back_doc = note.back_doc.clone();
            rewrite_asset_ids(&mut front_doc, &asset_map)?;
            rewrite_asset_ids(&mut back_doc, &asset_map)?;
            let validated = validate_content(
                front_doc,
                back_doc,
                note.front_text.clone(),
                note.back_text.clone(),
            )?;
            let mapped_template = note
                .template_id
                .as_ref()
                .map(|id| {
                    template_map.get(id).cloned().ok_or_else(|| {
                        CommandError::invalid_package(
                            "An imported note refers to a missing template.",
                        )
                    })
                })
                .transpose()?;
            let fields_json =
                serde_json::to_string(&note.fields).map_err(CommandError::serialization)?;
            let mode = if mapped_template.is_some() {
                "template"
            } else if note.note_type == "cloze" {
                "fill_blank"
            } else if reverse_note_ids.contains(note.source_id.as_str()) {
                "reverse"
            } else if note.note_type == "image_occlusion" {
                "image_occlusion"
            } else {
                "basic"
            };
            let duplicate_key = compute_duplicate_key(
                mode,
                &validated.front_text,
                &validated.back_text,
                &fields_json,
            );
            transaction
                .execute(
                    "INSERT INTO notes(\n\
                         id, deck_id, note_type, content_format, content_schema_version,\n\
                         front_doc, back_doc, front_text, back_text, template_id, fields_json,\n\
                         duplicate_key, created_at, updated_at, revision\n\
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                    params![
                        note_id,
                        deck_id,
                        note.note_type,
                        note.content_format,
                        note.content_schema_version,
                        validated.front_doc_json,
                        validated.back_doc_json,
                        validated.front_text,
                        validated.back_text,
                        mapped_template,
                        fields_json,
                        duplicate_key,
                        note.created_at,
                        note.updated_at,
                        note.revision,
                    ],
                )
                .map_err(CommandError::database)?;
            note_map.insert(note.source_id.clone(), note_id);
        }

        let imported_at = unix_time_millis()?;
        for card in &package.cards {
            let note_id = note_map.get(&card.source_note_id).ok_or_else(|| {
                CommandError::invalid_package("A generated card refers to a missing note.")
            })?;
            let card_id = generate_id(&transaction)?;
            transaction
                .execute(
                    "INSERT INTO cards(\n\
                         id, note_id, deck_id, direction, template_ordinal, created_at, updated_at,\n\
                         display_name\n\
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        card_id,
                        note_id,
                        deck_id,
                        card.direction,
                        card.template_ordinal,
                        card.created_at,
                        card.updated_at,
                        card.display_name,
                    ],
                )
                .map_err(CommandError::database)?;
            insert_new_schedule(&transaction, &card_id, imported_at)?;
        }

        let mut tag_map = HashMap::new();
        for tag in &package.tags {
            let existing: Option<String> = transaction
                .query_row(
                    "SELECT id FROM tags WHERE normalized_name = ?1",
                    [tag.normalized_name.as_str()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(CommandError::database)?;
            let tag_id = if let Some(id) = existing {
                id
            } else {
                let id = generate_id(&transaction)?;
                transaction
                    .execute(
                        "INSERT INTO tags(id, name, normalized_name, created_at, updated_at)\n\
                         VALUES (?1, ?2, ?3, ?4, ?4)",
                        params![id, tag.name, tag.normalized_name, imported_at],
                    )
                    .map_err(CommandError::database)?;
                id
            };
            tag_map.insert(tag.source_id.clone(), tag_id);
        }
        for (source_note, source_tag) in &package.note_tags {
            let note_id = note_map.get(source_note).ok_or_else(|| {
                CommandError::invalid_package("An imported tag refers to a missing note.")
            })?;
            let tag_id = tag_map.get(source_tag).ok_or_else(|| {
                CommandError::invalid_package("An imported note refers to a missing tag.")
            })?;
            transaction
                .execute(
                    "INSERT INTO note_tags(note_id, tag_id) VALUES (?1, ?2)",
                    params![note_id, tag_id],
                )
                .map_err(CommandError::database)?;
        }
        for (source_note, source_asset, side) in &package.note_assets {
            let note_id = note_map.get(source_note).ok_or_else(|| {
                CommandError::invalid_package("An imported image refers to a missing note.")
            })?;
            let asset_id = asset_map.get(source_asset).ok_or_else(|| {
                CommandError::invalid_package("An imported card refers to a missing image.")
            })?;
            transaction
                .execute(
                    "INSERT INTO note_assets(note_id, asset_id, side) VALUES (?1, ?2, ?3)",
                    params![note_id, asset_id, side],
                )
                .map_err(CommandError::database)?;
        }
        for note_id in note_map.values() {
            refresh_note_search(&transaction, note_id)?;
        }

        transaction.commit().map_err(CommandError::database)?;
        let deck = get_deck_from_connection(connection, &deck_id)?;
        Ok(ImportDeckV2ResultDto {
            deck,
            card_count: i64::try_from(package.cards.len()).unwrap_or(i64::MAX),
            asset_count: i64::try_from(package.assets.len()).unwrap_or(i64::MAX),
            template_count: i64::try_from(package.templates.len()).unwrap_or(i64::MAX),
            tag_count: i64::try_from(package.tags.len()).unwrap_or(i64::MAX),
        })
    })();
    if result.is_err() {
        for path in created_files {
            let _ = fs::remove_file(path);
        }
    }
    result
}

fn rewrite_asset_ids(
    value: &mut Value,
    asset_map: &HashMap<String, String>,
) -> Result<(), CommandError> {
    match value {
        Value::Array(values) => {
            for nested in values {
                rewrite_asset_ids(nested, asset_map)?;
            }
        }
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some("image") {
                let attrs = object
                    .get_mut("attrs")
                    .and_then(Value::as_object_mut)
                    .ok_or_else(|| {
                        CommandError::invalid_package("An imported image node has no attributes.")
                    })?;
                let source_id = attrs
                    .get("assetId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        CommandError::invalid_package(
                            "An imported image is missing its asset identifier.",
                        )
                    })?;
                let mapped = asset_map.get(source_id).ok_or_else(|| {
                    CommandError::invalid_package(
                        "An imported card refers to an image not included in the package.",
                    )
                })?;
                attrs.insert("assetId".to_string(), Value::String(mapped.clone()));
                attrs.insert(
                    "src".to_string(),
                    Value::String(format!("study-asset://{mapped}")),
                );
            }
            for nested in object.values_mut() {
                rewrite_asset_ids(nested, asset_map)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_export_asset(
    filename: &str,
    mime_type: &str,
    byte_size: i64,
    data: &[u8],
    total: &mut usize,
) -> Result<(), CommandError> {
    if data.is_empty() || data.len() > MAX_PACKAGE_SINGLE_ASSET_BYTES {
        return Err(CommandError::new(
            "invalid_asset",
            format!("The image “{filename}” is empty or exceeds 25 MB."),
        ));
    }
    *total = total.checked_add(data.len()).ok_or_else(|| {
        CommandError::new("deck_too_large", "The deck contains too much image data.")
    })?;
    if *total > MAX_PACKAGE_ASSET_BYTES {
        return Err(CommandError::new(
            "deck_too_large",
            "The deck contains more than 512 MB of images.",
        ));
    }
    if i64::try_from(data.len()).unwrap_or(i64::MAX) != byte_size {
        return Err(CommandError::new(
            "asset_size_mismatch",
            format!("The image “{filename}” has an invalid size record."),
        ));
    }
    let (detected, _) = detect_image_bytes(data)?;
    if detected != mime_type {
        return Err(CommandError::new(
            "asset_type_mismatch",
            format!("The image “{filename}” does not match its declared type."),
        ));
    }
    Ok(())
}

fn validate_package_metadata(
    connection: &Connection,
) -> Result<BTreeMap<String, String>, CommandError> {
    let expected = BTreeSet::from([
        "applicationVersion",
        "exportedAt",
        "format",
        "formatVersion",
    ]);
    let mut statement = connection
        .prepare("SELECT key, value FROM package_metadata ORDER BY key")
        .map_err(package_schema_error)?;
    let rows = statement
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(package_schema_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(package_schema_error)?;
    if rows.len() != expected.len() {
        return Err(CommandError::invalid_package(
            "The deck package metadata is incomplete or contains unsupported fields.",
        ));
    }

    let mut metadata = BTreeMap::new();
    for (key, value) in rows {
        if !expected.contains(key.as_str()) || value.len() > 256 {
            return Err(CommandError::invalid_package(
                "The deck package metadata is invalid.",
            ));
        }
        metadata.insert(key, value);
    }
    if metadata.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected {
        return Err(CommandError::invalid_package(
            "The deck package metadata is incomplete or contains unsupported fields.",
        ));
    }

    let exported_at = metadata
        .get("exportedAt")
        .expect("metadata key was validated")
        .parse::<i64>()
        .map_err(|_| CommandError::invalid_package("The deck export time is invalid."))?;
    validate_timestamp(exported_at, exported_at, "package metadata")?;
    let application_version = metadata
        .get("applicationVersion")
        .expect("metadata key was validated")
        .trim();
    if application_version.is_empty() || application_version.len() > 100 {
        return Err(CommandError::invalid_package(
            "The deck application version is invalid.",
        ));
    }
    Ok(metadata)
}

fn validate_package_schema(connection: &Connection) -> Result<(), CommandError> {
    let expected = PACKAGE_TABLES
        .iter()
        .map(|table| table.name)
        .collect::<BTreeSet<_>>();
    let mut statement = connection
        .prepare(
            "SELECT type, name FROM sqlite_schema\n\
             WHERE name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .map_err(package_schema_error)?;
    let objects = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(package_schema_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(package_schema_error)?;
    let mut tables = BTreeSet::new();
    for (object_type, name) in objects {
        if object_type != "table" || !expected.contains(name.as_str()) {
            return Err(CommandError::invalid_package(
                "The deck contains unsupported SQLite objects.",
            ));
        }
        tables.insert(name);
    }
    if tables != expected {
        return Err(CommandError::invalid_package(
            "The deck package schema is incomplete.",
        ));
    }

    for table in PACKAGE_TABLES {
        validate_package_table(connection, *table)?;
    }
    Ok(())
}

fn validate_package_table(
    connection: &Connection,
    expected: ExpectedPackageTable,
) -> Result<(), CommandError> {
    let (object_type, create_sql): (String, String) = connection
        .query_row(
            "SELECT type, sql FROM sqlite_schema WHERE name = ?1",
            [expected.name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(package_schema_error)?;
    if object_type != "table" || !create_sql.trim_end().to_ascii_uppercase().ends_with("STRICT") {
        return Err(CommandError::invalid_package(format!(
            "The {0} table is not the expected strict SQLite table.",
            expected.name
        )));
    }

    // Table names come only from the compile-time allowlist above. They are
    // never derived from package content or user input.
    let pragma = format!("PRAGMA table_xinfo({})", expected.name);
    let mut statement = connection.prepare(&pragma).map_err(package_schema_error)?;
    let columns = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                ActualPackageColumn {
                    name: row.get(1)?,
                    declared_type: row.get(2)?,
                    not_null: row.get(3)?,
                    primary_key_ordinal: row.get(5)?,
                },
                row.get::<_, i64>(6)?,
            ))
        })
        .map_err(package_schema_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(package_schema_error)?;

    if columns.len() != expected.columns.len() {
        return Err(CommandError::invalid_package(format!(
            "The {} table has unexpected columns.",
            expected.name
        )));
    }
    for (index, (column_id, actual, hidden)) in columns.iter().enumerate() {
        let expected_column = expected.columns[index];
        if *column_id != i64::try_from(index).unwrap_or(i64::MAX)
            || *hidden != 0
            || actual.name != expected_column.name
            || actual.declared_type.to_ascii_uppercase() != expected_column.declared_type
            || actual.not_null != expected_column.not_null
            || actual.primary_key_ordinal != expected_column.primary_key_ordinal
        {
            return Err(CommandError::invalid_package(format!(
                "The {} table does not match the supported deck format.",
                expected.name
            )));
        }
    }
    Ok(())
}

fn package_schema_error(error: rusqlite::Error) -> CommandError {
    CommandError::invalid_package(format!("Could not inspect the deck schema: {error}"))
}

fn enforce_package_row_limit(
    connection: &Connection,
    table: &str,
    maximum: usize,
    message: &str,
) -> Result<(), CommandError> {
    let query = match table {
        "templates" => "SELECT COUNT(*) FROM templates",
        "notes" => "SELECT COUNT(*) FROM notes",
        "cards" => "SELECT COUNT(*) FROM cards",
        "tags" => "SELECT COUNT(*) FROM tags",
        "assets" => "SELECT COUNT(*) FROM assets",
        "note_tags" => "SELECT COUNT(*) FROM note_tags",
        "note_assets" => "SELECT COUNT(*) FROM note_assets",
        _ => {
            return Err(CommandError::invalid_package(
                "The package validator requested an unknown table.",
            ))
        }
    };
    let count: i64 = connection.query_row(query, [], |row| row.get(0)).map_err(|error| {
        CommandError::invalid_package(format!("The deck package schema is invalid: {error}"))
    })?;
    if count < 0 || usize::try_from(count).unwrap_or(usize::MAX) > maximum {
        return Err(CommandError::invalid_package(message));
    }
    Ok(())
}

fn validate_timestamps(created_at: i64, updated_at: i64, label: &str) -> Result<(), CommandError> {
    const MAX_TIMESTAMP: i64 = 253_402_300_799_999;
    if created_at < 0 || updated_at < created_at || updated_at > MAX_TIMESTAMP {
        return Err(CommandError::invalid_package(format!(
            "The imported {label} has invalid timestamps."
        )));
    }
    Ok(())
}

fn normalize_tag(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn template_name_exists(connection: &Connection, name: &str) -> Result<bool, CommandError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM card_templates\n\
             WHERE lower(name) = lower(?1) AND deleted_at IS NULL)",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(CommandError::database)
}

fn unique_imported_template_name(
    connection: &Connection,
    base: &str,
) -> Result<String, CommandError> {
    for suffix in 2..=10_000 {
        let ending = format!(" (imported {suffix})");
        let available = 120_usize.saturating_sub(ending.chars().count());
        let shortened = base.chars().take(available).collect::<String>();
        let candidate = format!("{shortened}{ending}");
        if !template_name_exists(connection, &candidate)? {
            return Ok(candidate);
        }
    }
    Err(CommandError::new(
        "template_name_conflict",
        "Could not choose a unique name for an imported card template.",
    ))
}

fn normalize_export_path(value: &str) -> Result<PathBuf, CommandError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return Err(CommandError::new(
            "invalid_export_path",
            "Choose a valid destination for the deck export.",
        ));
    }
    let mut path = PathBuf::from(trimmed);
    if path.extension().and_then(|value| value.to_str()) != Some("studydeck") {
        path.set_extension("studydeck");
    }
    Ok(path)
}

fn temporary_export_path(destination: &Path) -> PathBuf {
    let mut name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("deck.studydeck")
        .to_string();
    name.push_str(".tmp");
    destination.with_file_name(name)
}

fn replace_export_file(temporary: &Path, destination: &Path) -> Result<(), CommandError> {
    if destination.exists() {
        fs::remove_file(destination).map_err(CommandError::io)?;
    }
    fs::rename(temporary, destination).map_err(CommandError::io)?;
    if let Some(parent) = destination.parent() {
        OpenOptions::new()
            .read(true)
            .open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(CommandError::io)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    const BUILTIN_PHARMACOLOGY_ID: &str = "fffffffffffffffffffffffffffffff1";
    const TEST_DECK_ID: &str = "10000000000000000000000000000001";
    const TEST_NOTE_ID: &str = "20000000000000000000000000000001";
    const TEST_CARD_ID: &str = "30000000000000000000000000000001";

    #[test]
    fn package_schema_is_sqlite_and_versioned() {
        let connection = Connection::open_in_memory().expect("SQLite should open");
        connection
            .execute_batch(PACKAGE_SCHEMA)
            .expect("package schema should apply");
        let tables: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'templates'",
                [],
                |row| row.get(0),
            )
            .expect("schema should be readable");
        assert_eq!(tables, 1);
        validate_package_schema(&connection).expect("canonical schema should validate exactly");
        assert_eq!(PACKAGE_FORMAT_VERSION, 2);
    }

    #[test]
    fn package_metadata_has_an_exact_bounded_shape() {
        let connection = Connection::open_in_memory().expect("SQLite should open");
        connection
            .execute_batch(PACKAGE_SCHEMA)
            .expect("package schema should apply");
        for (key, value) in [
            ("format", PACKAGE_FORMAT),
            ("formatVersion", "2"),
            ("exportedAt", "1"),
            ("applicationVersion", "0.1.0"),
        ] {
            connection
                .execute(
                    "INSERT INTO package_metadata(key, value) VALUES (?1, ?2)",
                    params![key, value],
                )
                .expect("metadata should insert");
        }
        validate_package_metadata(&connection).expect("canonical metadata should validate");
        connection
            .execute(
                "INSERT INTO package_metadata(key, value) VALUES ('surprise', '1')",
                [],
            )
            .expect("untrusted metadata can contain an extra row before validation");
        assert!(validate_package_metadata(&connection).is_err());
    }

    #[test]
    fn package_schema_with_an_unexpected_column_is_rejected() {
        let connection = Connection::open_in_memory().expect("SQLite should open");
        let altered = PACKAGE_SCHEMA.replacen(
            "    value TEXT NOT NULL\n) STRICT;",
            "    value TEXT NOT NULL,\n    surprise TEXT\n) STRICT;",
            1,
        );
        connection
            .execute_batch(&altered)
            .expect("altered schema should be valid SQLite");
        let error = validate_package_schema(&connection)
            .expect_err("package columns must match the allowlist exactly");
        assert_eq!(error.code, "invalid_deck_package");
        assert!(error.message.contains("unexpected columns"));
    }

    #[test]
    fn package_schema_with_a_wrong_declared_type_is_rejected() {
        let connection = Connection::open_in_memory().expect("SQLite should open");
        let altered = PACKAGE_SCHEMA.replacen(
            "    revision INTEGER NOT NULL\n) STRICT;",
            "    revision TEXT NOT NULL\n) STRICT;",
            1,
        );
        connection
            .execute_batch(&altered)
            .expect("altered schema should be valid SQLite");
        let error = validate_package_schema(&connection)
            .expect_err("declared types must match the package format");
        assert_eq!(error.code, "invalid_deck_package");
        assert!(error.message.contains("supported deck format"));
    }

    #[test]
    fn malformed_or_extended_package_schema_is_rejected() {
        let connection = Connection::open_in_memory().expect("SQLite should open");
        connection
            .execute_batch(PACKAGE_SCHEMA)
            .expect("package schema should apply");
        connection
            .execute_batch("CREATE VIEW executable_surprise AS SELECT 1;")
            .expect("test view should be created");

        let error = validate_package_schema(&connection)
            .expect_err("unsupported schema objects must fail closed");
        assert_eq!(error.code, "invalid_deck_package");
        assert!(error.message.contains("unsupported SQLite objects"));
    }

    #[test]
    fn generated_cards_cannot_reference_missing_notes() {
        let connection = Connection::open_in_memory().expect("SQLite should open");
        connection
            .execute_batch(PACKAGE_SCHEMA)
            .expect("package schema should apply");
        connection
            .execute(
                "INSERT INTO cards(
                     source_id, source_note_id, direction, template_ordinal,
                     display_name, created_at, updated_at
                 ) VALUES (?1, ?2, 'forward', 0, 'Forward', 1, 1)",
                params![TEST_CARD_ID, "20000000000000000000000000009999"],
            )
            .expect("malicious relationship should fit the untrusted package schema");

        let note = basic_package_note();
        let error = match read_cards(&connection, &[note], &HashMap::new()) {
            Ok(_) => panic!("missing-note relationships must be rejected before import"),
            Err(error) => error,
        };
        assert_eq!(error.code, "invalid_deck_package");
        assert!(error.message.contains("generated-card relationships"));
    }

    #[test]
    fn conflicting_template_ids_never_replace_a_local_template() {
        let database = Database::open_in_memory().expect("database should open");
        let root = temporary_test_directory("template-conflict");
        let paths = AppPaths::new(root.clone()).expect("test paths should initialize");
        let fields = BTreeMap::from([("topic".to_string(), "Medication safety".to_string())]);
        let template = PackageTemplate {
            source_id: BUILTIN_PHARMACOLOGY_ID.to_string(),
            name: "Pharmacology".to_string(),
            description: "A deliberately conflicting imported template.".to_string(),
            fields: vec!["topic".to_string()],
            cards: vec![TemplateCardSpec {
                name: "Definition".to_string(),
                front: "What is {{topic}}?".to_string(),
                back: "{{topic}}".to_string(),
            }],
            created_at: 1,
            updated_at: 1,
            revision: 1,
        };
        let package = PackageData {
            deck: PackageDeck {
                source_id: TEST_DECK_ID.to_string(),
                name: "Imported security test".to_string(),
                description: String::new(),
                created_at: 1,
                updated_at: 1,
                revision: 1,
            },
            templates: vec![template],
            notes: vec![PackageNote {
                source_id: TEST_NOTE_ID.to_string(),
                note_type: "basic".to_string(),
                content_format: "tiptap-json".to_string(),
                content_schema_version: 1,
                front_doc: plain_document("What is medication safety?"),
                back_doc: plain_document("Medication safety"),
                front_text: "What is medication safety?".to_string(),
                back_text: "Medication safety".to_string(),
                template_id: Some(BUILTIN_PHARMACOLOGY_ID.to_string()),
                fields,
                created_at: 1,
                updated_at: 1,
                revision: 1,
            }],
            cards: vec![PackageCard {
                source_id: TEST_CARD_ID.to_string(),
                source_note_id: TEST_NOTE_ID.to_string(),
                direction: "forward".to_string(),
                template_ordinal: 0,
                display_name: "Definition".to_string(),
                created_at: 1,
                updated_at: 1,
            }],
            tags: Vec::new(),
            note_tags: Vec::new(),
            assets: Vec::new(),
            note_assets: Vec::new(),
        };

        database
            .with_connection(|connection| {
                import_package(connection, &paths, package)?;
                let original: (String, String) = connection
                    .query_row(
                        "SELECT name, fields_json FROM card_templates WHERE id = ?1",
                        [BUILTIN_PHARMACOLOGY_ID],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(original.0, "Pharmacology");
                assert!(original.1.contains("medication name"));

                let imported: (String, String) = connection
                    .query_row(
                        "SELECT id, name FROM card_templates
                         WHERE id <> ?1 AND name LIKE 'Pharmacology (imported %)'
                         ORDER BY name LIMIT 1",
                        [BUILTIN_PHARMACOLOGY_ID],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_ne!(imported.0, BUILTIN_PHARMACOLOGY_ID);
                assert_eq!(imported.1, "Pharmacology (imported 2)");

                let mapped: String = connection
                    .query_row(
                        "SELECT n.template_id FROM notes n
                         JOIN decks d ON d.id = n.deck_id
                         WHERE d.name = 'Imported security test'",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(mapped, imported.0);
                Ok(())
            })
            .expect("conflicting template should import under a fresh local identity");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn version_two_sqlite_package_round_trips_without_review_history() {
        let source_database = Database::open_in_memory().expect("source database should open");
        let source_root = temporary_test_directory("roundtrip-source");
        let source_paths = AppPaths::new(source_root.clone()).expect("source paths should initialize");
        let package_path = source_root.join("roundtrip.studydeck");

        let source_deck = source_database
            .with_connection(|connection| {
                let deck = crate::decks::create_deck_in_connection(
                    connection,
                    crate::decks::CreateDeckInput {
                        name: "Round-trip deck".to_string(),
                        description: Some("SQLite package version 2".to_string()),
                    },
                )?;
                crate::cards::create_card_in_connection(
                    connection,
                    &source_paths,
                    &deck.id,
                    crate::cards::validate_content(
                        plain_document("What is ATP?"),
                        plain_document("Adenosine triphosphate"),
                        "What is ATP?".to_string(),
                        "Adenosine triphosphate".to_string(),
                    )?,
                )?;
                export_from_connection(connection, &source_paths, &deck.id, &package_path)?;
                Ok(deck)
            })
            .expect("source deck should export");

        let package = read_package(&package_path).expect("exported package should validate");
        assert_eq!(package.deck.source_id, source_deck.id);
        assert_eq!(package.notes.len(), 1);
        assert_eq!(package.cards.len(), 1);

        let destination_database = Database::open_in_memory().expect("destination should open");
        let destination_root = temporary_test_directory("roundtrip-destination");
        let destination_paths =
            AppPaths::new(destination_root.clone()).expect("destination paths should initialize");
        destination_database
            .with_connection(|connection| {
                let imported = import_package(connection, &destination_paths, package)?;
                assert_eq!(imported.deck.name, "Round-trip deck");
                assert_eq!(imported.card_count, 1);
                let review_count: i64 = connection
                    .query_row("SELECT COUNT(*) FROM review_events", [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                let schedule_count: i64 = connection
                    .query_row("SELECT COUNT(*) FROM card_schedules", [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                assert_eq!(review_count, 0);
                assert_eq!(schedule_count, 1);
                Ok(())
            })
            .expect("package should import with a fresh local schedule");

        let _ = fs::remove_dir_all(source_root);
        let _ = fs::remove_dir_all(destination_root);
    }

    #[test]
    fn imported_rich_text_cannot_smuggle_javascript_links() {
        let connection = Connection::open_in_memory().expect("SQLite should open");
        connection
            .execute_batch(PACKAGE_SCHEMA)
            .expect("package schema should apply");
        let unsafe_document = serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{
                    "type": "text",
                    "text": "click",
                    "marks": [{"type": "link", "attrs": {"href": "javascript:alert(1)"}}]
                }]
            }]
        });
        connection
            .execute(
                "INSERT INTO notes(
                     source_id, source_deck_id, note_type, content_format,
                     content_schema_version, front_doc, back_doc, front_text, back_text,
                     template_id, fields_json, duplicate_key, created_at, updated_at, revision
                 ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, 'click', 'answer',
                           NULL, '{}', '', 1, 1, 1)",
                params![
                    TEST_NOTE_ID,
                    TEST_DECK_ID,
                    unsafe_document.to_string(),
                    plain_document("answer").to_string(),
                ],
            )
            .expect("untrusted document should fit the transport schema");
        let deck = PackageDeck {
            source_id: TEST_DECK_ID.to_string(),
            name: "Unsafe".to_string(),
            description: String::new(),
            created_at: 1,
            updated_at: 1,
            revision: 1,
        };
        let error = read_notes(&connection, &deck, &HashMap::new())
            .expect_err("unsafe rich content must be revalidated before import");
        assert!(matches!(error.code.as_str(), "invalid_link" | "invalid_deck_package"));
    }

    #[test]
    fn export_extension_stays_a_single_sqlite_deck_file() {
        assert_eq!(
            normalize_export_path("/tmp/nursing").unwrap(),
            PathBuf::from("/tmp/nursing.studydeck")
        );
        assert_eq!(
            normalize_export_path("/tmp/nursing.studydeck").unwrap(),
            PathBuf::from("/tmp/nursing.studydeck")
        );
    }

    fn basic_package_note() -> PackageNote {
        PackageNote {
            source_id: TEST_NOTE_ID.to_string(),
            note_type: "basic".to_string(),
            content_format: "tiptap-json".to_string(),
            content_schema_version: 1,
            front_doc: plain_document("Question"),
            back_doc: plain_document("Answer"),
            front_text: "Question".to_string(),
            back_text: "Answer".to_string(),
            template_id: None,
            fields: BTreeMap::new(),
            created_at: 1,
            updated_at: 1,
            revision: 1,
        }
    }

    fn plain_document(text: &str) -> Value {
        serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": text}]
            }]
        })
    }

    fn temporary_test_directory(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "ostudy-transfer-v2-{label}-{}-{nonce}",
            std::process::id()
        ))
    }
}
