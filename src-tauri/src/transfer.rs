use std::{
    collections::{BTreeSet, HashMap},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
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
    authoring::{compute_duplicate_key, refresh_note_search},
    scheduler::insert_new_schedule,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const PACKAGE_FORMAT: &str = "studydeck-sqlite";
const PACKAGE_FORMAT_VERSION: i64 = 1;
const MAX_PACKAGE_FILE_BYTES: u64 = 640 * 1024 * 1024;
const MAX_PACKAGE_ASSET_BYTES: usize = 512 * 1024 * 1024;
const MAX_PACKAGE_SINGLE_ASSET_BYTES: usize = 25 * 1024 * 1024;
const MAX_PACKAGE_NOTES: usize = 100_000;
const MAX_PACKAGE_ASSETS: usize = 200_000;
const MAX_PACKAGE_RELATION_ROWS: usize = 2_000_000;

struct TemporaryExportGuard {
    path: PathBuf,
}

impl TemporaryExportGuard {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TemporaryExportGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

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
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    revision INTEGER NOT NULL
) STRICT;
CREATE TABLE cards (
    source_id TEXT PRIMARY KEY,
    source_note_id TEXT NOT NULL,
    direction TEXT NOT NULL,
    template_ordinal INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDeckInput {
    pub deck_id: String,
    pub destination_path: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResultDto {
    pub destination_path: String,
    pub card_count: i64,
    pub asset_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResultDto {
    pub deck: DeckDto,
    pub card_count: i64,
    pub asset_count: i64,
}

#[derive(Clone)]
struct ImportedDeck {
    source_id: String,
    name: String,
    description: String,
    created_at: i64,
    updated_at: i64,
    revision: i64,
}

#[derive(Clone)]
struct ImportedNote {
    source_id: String,
    source_deck_id: String,
    note_type: String,
    content_format: String,
    content_schema_version: i64,
    front_doc: Value,
    back_doc: Value,
    front_text: String,
    back_text: String,
    created_at: i64,
    updated_at: i64,
    revision: i64,
}

#[derive(Clone)]
struct ImportedCard {
    source_id: String,
    source_note_id: String,
    direction: String,
    template_ordinal: i64,
    created_at: i64,
    updated_at: i64,
}

struct ImportedAsset {
    source_id: String,
    original_filename: String,
    mime_type: String,
    byte_size: i64,
    data: Vec<u8>,
}

struct ImportedPackage {
    deck: ImportedDeck,
    notes: Vec<ImportedNote>,
    cards: Vec<ImportedCard>,
    assets: Vec<ImportedAsset>,
}

#[tauri::command]
pub fn export_deck(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ExportDeckInput,
) -> Result<ExportResultDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    let destination = normalize_export_path(&input.destination_path)?;
    database.with_connection(|connection| {
        export_deck_from_connection(connection, &paths, &input.deck_id, &destination)
    })
}

#[tauri::command]
pub fn import_deck(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    source_path: String,
) -> Result<ImportResultDto, CommandError> {
    let source = std::fs::canonicalize(&source_path).map_err(CommandError::io)?;
    if !source.is_file() {
        return Err(CommandError::invalid_package(
            "Select a .studydeck file to import.",
        ));
    }
    let metadata = fs::metadata(&source).map_err(CommandError::io)?;
    if metadata.len() == 0 || metadata.len() > MAX_PACKAGE_FILE_BYTES {
        return Err(CommandError::invalid_package(
            "The selected deck file is empty or exceeds the 640 MB import limit.",
        ));
    }
    let package = read_package(&source)?;
    database.with_connection(|connection| import_package(connection, &paths, package))
}

fn export_deck_from_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    deck_id: &str,
    destination: &Path,
) -> Result<ExportResultDto, CommandError> {
    let deck = get_deck_from_connection(connection, deck_id)?;
    let parent = destination
        .parent()
        .ok_or_else(|| CommandError::new("invalid_export_path", "Choose a valid export folder."))?;
    if !parent.exists() {
        return Err(CommandError::new(
            "invalid_export_path",
            "The selected export folder no longer exists.",
        ));
    }

    let temporary = temporary_export_path(destination);
    let _ = fs::remove_file(&temporary);
    let _temporary_guard = TemporaryExportGuard::new(temporary.clone());
    let mut package = Connection::open(&temporary).map_err(CommandError::database)?;
    package
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = DELETE;
             PRAGMA synchronous = FULL;
             PRAGMA trusted_schema = OFF;",
        )
        .map_err(CommandError::database)?;
    package
        .execute_batch(PACKAGE_SCHEMA)
        .map_err(CommandError::database)?;
    let transaction = package.transaction().map_err(CommandError::database)?;
    let exported_at = unix_time_millis()?;

    for (key, value) in [
        ("format", PACKAGE_FORMAT.to_string()),
        ("formatVersion", PACKAGE_FORMAT_VERSION.to_string()),
        ("exportedAt", exported_at.to_string()),
        ("applicationVersion", env!("CARGO_PKG_VERSION").to_string()),
    ] {
        transaction
            .execute(
                "INSERT INTO package_metadata (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .map_err(CommandError::database)?;
    }

    transaction
        .execute(
            "INSERT INTO deck (\n\
                 source_id, name, description, created_at, updated_at, revision\n\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                deck.id,
                deck.name,
                deck.description,
                deck.created_at,
                deck.updated_at,
                deck.revision
            ],
        )
        .map_err(CommandError::database)?;

    let mut note_statement = connection
        .prepare(
            "SELECT id, deck_id, note_type, content_format, content_schema_version,\n\
                    front_doc, back_doc, front_text, back_text, created_at, updated_at, revision\n\
             FROM notes\n\
             WHERE deck_id = ?1 AND deleted_at IS NULL\n\
             ORDER BY created_at ASC, id ASC",
        )
        .map_err(CommandError::database)?;
    let notes = note_statement
        .query_map([deck_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, i64>(11)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;

    for note in &notes {
        transaction
            .execute(
                "INSERT INTO notes (\n\
                     source_id, source_deck_id, note_type, content_format, content_schema_version,\n\
                     front_doc, back_doc, front_text, back_text, created_at, updated_at, revision\n\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    note.0, note.1, note.2, note.3, note.4, note.5, note.6, note.7, note.8,
                    note.9, note.10, note.11
                ],
            )
            .map_err(CommandError::database)?;
    }

    let mut card_statement = connection
        .prepare(
            "SELECT c.id, c.note_id, c.direction, c.template_ordinal, c.created_at, c.updated_at\n\
             FROM cards c\n\
             JOIN notes n ON n.id = c.note_id\n\
             WHERE c.deck_id = ?1\n\
               AND c.deleted_at IS NULL\n\
               AND n.deleted_at IS NULL\n\
             ORDER BY c.created_at ASC, c.id ASC",
        )
        .map_err(CommandError::database)?;
    let cards = card_statement
        .query_map([deck_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    for card in &cards {
        transaction
            .execute(
                "INSERT INTO cards (\n\
                     source_id, source_note_id, direction, template_ordinal, created_at, updated_at\n\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![card.0, card.1, card.2, card.3, card.4, card.5],
            )
            .map_err(CommandError::database)?;
    }

    let mut link_statement = connection
        .prepare(
            "SELECT na.note_id, na.asset_id, na.side\n\
             FROM note_assets na\n\
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
    for link in &links {
        transaction
            .execute(
                "INSERT INTO note_assets (source_note_id, source_asset_id, side)\n\
                 VALUES (?1, ?2, ?3)",
                params![link.0, link.1, link.2],
            )
            .map_err(CommandError::database)?;
    }

    let mut asset_statement = connection
        .prepare(
            "SELECT DISTINCT a.id, a.original_filename, a.mime_type, a.byte_size, a.relative_path\n\
             FROM assets a\n\
             JOIN note_assets na ON na.asset_id = a.id\n\
             JOIN notes n ON n.id = na.note_id\n\
             WHERE n.deck_id = ?1\n\
               AND n.deleted_at IS NULL\n\
               AND a.deleted_at IS NULL\n\
             ORDER BY a.id",
        )
        .map_err(CommandError::database)?;
    let assets = asset_statement
        .query_map([deck_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;

    let mut total_asset_bytes = 0_usize;
    for asset in &assets {
        let file_path = paths.resolve_relative(&asset.4);
        let data = fs::read(&file_path).map_err(|error| {
            eprintln!(
                "Could not read exported asset {}: {error}",
                file_path.display()
            );
            CommandError::new(
                "missing_asset",
                format!("The image '{}' is missing from the local library.", asset.1),
            )
        })?;
        if data.len() > MAX_PACKAGE_SINGLE_ASSET_BYTES {
            return Err(CommandError::new(
                "asset_too_large",
                format!(
                    "The image '{}' exceeds the 25 MB deck-export limit.",
                    asset.1
                ),
            ));
        }
        total_asset_bytes = total_asset_bytes.checked_add(data.len()).ok_or_else(|| {
            CommandError::new("deck_too_large", "The deck contains too much image data.")
        })?;
        if total_asset_bytes > MAX_PACKAGE_ASSET_BYTES {
            return Err(CommandError::new(
                "deck_too_large",
                "This deck contains more than 512 MB of images and cannot be exported as one file.",
            ));
        }
        if i64::try_from(data.len()).unwrap_or(i64::MAX) != asset.3 {
            return Err(CommandError::new(
                "asset_size_mismatch",
                format!(
                    "The image '{}' does not match its database size record.",
                    asset.1
                ),
            ));
        }
        let (detected_mime, _) = detect_image_bytes(&data)?;
        if detected_mime != asset.2 {
            return Err(CommandError::new(
                "asset_type_mismatch",
                format!(
                    "The image '{}' does not match its database file type.",
                    asset.1
                ),
            ));
        }
        transaction
            .execute(
                "INSERT INTO assets (\n\
                     source_id, original_filename, mime_type, byte_size, data\n\
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![asset.0, asset.1, asset.2, asset.3, data],
            )
            .map_err(CommandError::database)?;
    }

    transaction.commit().map_err(CommandError::database)?;
    let integrity: String = package
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(CommandError::database)?;
    if integrity != "ok" {
        let _ = fs::remove_file(&temporary);
        return Err(CommandError::new(
            "export_validation_failed",
            "The exported deck did not pass its integrity check.",
        ));
    }
    drop(package);
    OpenOptions::new()
        .read(true)
        .open(&temporary)
        .and_then(|file| file.sync_all())
        .map_err(CommandError::io)?;
    replace_export_file(&temporary, destination)?;

    Ok(ExportResultDto {
        destination_path: destination.to_string_lossy().into_owned(),
        card_count: i64::try_from(cards.len()).unwrap_or(i64::MAX),
        asset_count: i64::try_from(assets.len()).unwrap_or(i64::MAX),
    })
}

fn read_package(source: &Path) -> Result<ImportedPackage, CommandError> {
    let package =
        Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|error| {
            CommandError::invalid_package(format!("Could not open the deck: {error}"))
        })?;
    package
        .execute_batch(
            "PRAGMA trusted_schema = OFF;
             PRAGMA foreign_keys = ON;
             PRAGMA query_only = ON;",
        )
        .map_err(|error| {
            CommandError::invalid_package(format!("Could not secure the deck reader: {error}"))
        })?;

    let integrity: String = package
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| {
            CommandError::invalid_package(format!("Integrity check failed: {error}"))
        })?;
    if integrity != "ok" {
        return Err(CommandError::invalid_package(
            "The selected deck file is damaged.",
        ));
    }
    validate_package_schema(&package)?;

    let format = metadata_value(&package, "format")?;
    let version = metadata_value(&package, "formatVersion")?;
    if format != PACKAGE_FORMAT || version != PACKAGE_FORMAT_VERSION.to_string() {
        return Err(CommandError::invalid_package(
            "This file is not a supported ostudy deck package.",
        ));
    }
    enforce_package_row_limit(
        &package,
        "notes",
        MAX_PACKAGE_NOTES,
        "This deck contains more than 100,000 cards and cannot be imported at once.",
    )?;
    enforce_package_row_limit(
        &package,
        "cards",
        MAX_PACKAGE_NOTES,
        "This deck contains too many generated cards.",
    )?;
    enforce_package_row_limit(
        &package,
        "assets",
        MAX_PACKAGE_ASSETS,
        "This deck contains too many image records.",
    )?;
    enforce_package_row_limit(
        &package,
        "note_assets",
        MAX_PACKAGE_RELATION_ROWS,
        "This deck contains too many note-image relationships.",
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
                Ok(ImportedDeck {
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
            "The imported deck has an invalid revision number.",
        ));
    }

    let mut note_statement = package
        .prepare(
            "SELECT source_id, source_deck_id, note_type, content_format,\n\
                    content_schema_version, front_doc, back_doc, front_text, back_text,\n\
                    created_at, updated_at, revision\n\
             FROM notes ORDER BY created_at, source_id",
        )
        .map_err(CommandError::database)?;
    let notes = note_statement
        .query_map([], |row| {
            let front_json: String = row.get(5)?;
            let back_json: String = row.get(6)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                front_json,
                back_json,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, i64>(11)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if notes.len() > MAX_PACKAGE_NOTES {
        return Err(CommandError::invalid_package(
            "This deck contains more than 100,000 cards and cannot be imported at once.",
        ));
    }
    let notes = notes
        .into_iter()
        .map(|row| {
            validate_entity_id(&row.0, "exported note")?;
            if row.1 != deck.source_id || row.2 != "basic" || row.3 != "tiptap-json" || row.4 != 1 {
                return Err(CommandError::invalid_package(
                    "The deck contains an unsupported card format.",
                ));
            }
            let front_doc = serde_json::from_str(&row.5).map_err(|error| {
                CommandError::invalid_package(format!("Invalid question document: {error}"))
            })?;
            let back_doc = serde_json::from_str(&row.6).map_err(|error| {
                CommandError::invalid_package(format!("Invalid answer document: {error}"))
            })?;
            validate_timestamps(row.9, row.10, "card")?;
            if row.11 < 1 {
                return Err(CommandError::invalid_package(
                    "The deck contains a card with an invalid revision number.",
                ));
            }
            Ok(ImportedNote {
                source_id: row.0,
                source_deck_id: row.1,
                note_type: row.2,
                content_format: row.3,
                content_schema_version: row.4,
                front_doc,
                back_doc,
                front_text: row.7,
                back_text: row.8,
                created_at: row.9,
                updated_at: row.10,
                revision: row.11.max(1),
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;

    let mut card_statement = package
        .prepare(
            "SELECT source_id, source_note_id, direction, template_ordinal, created_at, updated_at\n\
             FROM cards ORDER BY created_at, source_id",
        )
        .map_err(CommandError::database)?;
    let cards = card_statement
        .query_map([], |row| {
            Ok(ImportedCard {
                source_id: row.get(0)?,
                source_note_id: row.get(1)?,
                direction: row.get(2)?,
                template_ordinal: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    for card in &cards {
        validate_entity_id(&card.source_id, "exported card")?;
        validate_entity_id(&card.source_note_id, "exported note")?;
        if card.direction != "forward" || card.template_ordinal != 0 {
            return Err(CommandError::invalid_package(
                "This deck contains an unsupported generated-card format.",
            ));
        }
        validate_timestamps(card.created_at, card.updated_at, "generated card")?;
    }
    validate_card_relationships(&notes, &cards)?;

    let mut asset_statement = package
        .prepare(
            "SELECT source_id, original_filename, mime_type, byte_size, data\n\
             FROM assets ORDER BY source_id",
        )
        .map_err(CommandError::database)?;
    let assets = asset_statement
        .query_map([], |row| {
            Ok(ImportedAsset {
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

    let mut total_asset_bytes = 0_usize;
    for asset in &assets {
        validate_entity_id(&asset.source_id, "exported asset")?;
        validate_original_filename(&asset.original_filename)
            .map_err(|error| CommandError::invalid_package(error.message))?;
        if asset.data.is_empty() || asset.data.len() > MAX_PACKAGE_SINGLE_ASSET_BYTES {
            return Err(CommandError::invalid_package(
                "An image in the deck is empty or exceeds the 25 MB per-image limit.",
            ));
        }
        total_asset_bytes = total_asset_bytes
            .checked_add(asset.data.len())
            .ok_or_else(|| CommandError::invalid_package("The deck's images are too large."))?;
        if total_asset_bytes > MAX_PACKAGE_ASSET_BYTES {
            return Err(CommandError::invalid_package(
                "This deck contains more than 512 MB of images.",
            ));
        }
        if i64::try_from(asset.data.len()).unwrap_or(i64::MAX) != asset.byte_size {
            return Err(CommandError::invalid_package(
                "An image in the deck has an invalid size record.",
            ));
        }
        let (detected_mime, _) = detect_image_bytes(&asset.data)?;
        if detected_mime != asset.mime_type {
            return Err(CommandError::invalid_package(
                "An image in the deck does not match its declared file type.",
            ));
        }
    }

    validate_package_asset_links(&package, &notes, &assets)?;

    Ok(ImportedPackage {
        deck,
        notes,
        cards,
        assets,
    })
}

fn import_package(
    connection: &mut Connection,
    paths: &AppPaths,
    package: ImportedPackage,
) -> Result<ImportResultDto, CommandError> {
    let mut created_files = Vec::new();
    let result = (|| {
        let transaction = connection.transaction().map_err(CommandError::database)?;
        let deck_id = choose_id(&transaction, EntityTable::Deck, &package.deck.source_id)?;
        // Display names are user content, not identifiers. Internal ID collisions
        // are remapped independently, so importing a duplicate must not silently
        // rewrite the title with an implementation-oriented suffix.
        let deck_name = validate_name(package.deck.name.clone())
            .map_err(|error| CommandError::invalid_package(error.message))?;
        let deck_description = validate_description(package.deck.description.clone())
            .map_err(|error| CommandError::invalid_package(error.message))?;
        let sort_order: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM decks WHERE deleted_at IS NULL",
                [],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "INSERT INTO decks (\n\
                     id, name, description, sort_order, created_at, updated_at, revision\n\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    deck_id,
                    deck_name,
                    deck_description,
                    sort_order,
                    package.deck.created_at,
                    package.deck.updated_at,
                    package.deck.revision.max(1)
                ],
            )
            .map_err(CommandError::database)?;

        let mut asset_map = HashMap::new();
        for asset in &package.assets {
            let mut new_id = choose_id(&transaction, EntityTable::Asset, &asset.source_id)?;
            let (mime, extension) = detect_image_bytes(&asset.data)?;
            if mime != asset.mime_type {
                return Err(CommandError::invalid_package(
                    "An imported image has an invalid file type.",
                ));
            }
            loop {
                let relative_path = AppPaths::image_relative_path(&new_id, extension);
                let final_path = paths.resolve_relative(&relative_path);
                if !final_path.exists() {
                    let temporary_path = final_path.with_extension(format!("{extension}.tmp"));
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
                    created_files.push(final_path.clone());

                    transaction
                        .execute(
                            "INSERT INTO assets (\n\
                                 id, original_filename, mime_type, byte_size, relative_path, created_at\n\
                             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                            params![
                                new_id,
                                asset.original_filename,
                                asset.mime_type,
                                asset.byte_size,
                                relative_path,
                                unix_time_millis()?
                            ],
                        )
                        .map_err(CommandError::database)?;
                    asset_map.insert(asset.source_id.clone(), new_id.clone());
                    break;
                }
                new_id = generate_id(&transaction)?;
            }
        }

        let reverse_note_ids = package
            .cards
            .iter()
            .filter(|card| card.direction == "reverse")
            .map(|card| card.source_note_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut note_map = HashMap::new();
        for note in &package.notes {
            if note.source_deck_id != package.deck.source_id
                || note.note_type != "basic"
                || note.content_format != "tiptap-json"
                || note.content_schema_version != 1
            {
                return Err(CommandError::invalid_package(
                    "The imported deck contains an unsupported note format.",
                ));
            }
            let note_id = choose_id(&transaction, EntityTable::Note, &note.source_id)?;
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
            let duplicate_key = compute_duplicate_key(
                if reverse_note_ids.contains(note.source_id.as_str()) {
                    "reverse"
                } else {
                    "basic"
                },
                &validated.front_text,
                &validated.back_text,
                "{}",
            );

            transaction
                .execute(
                    "INSERT INTO notes (\n\
                         id, deck_id, note_type, content_format, content_schema_version,\n\
                         front_doc, back_doc, front_text, back_text, duplicate_key,\n\
                         created_at, updated_at, revision\n\
                     ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        note_id,
                        deck_id,
                        validated.front_doc_json,
                        validated.back_doc_json,
                        validated.front_text,
                        validated.back_text,
                        duplicate_key,
                        note.created_at,
                        note.updated_at,
                        note.revision.max(1)
                    ],
                )
                .map_err(CommandError::database)?;

            for asset_id in &validated.front_asset_ids {
                transaction
                    .execute(
                        "INSERT INTO note_assets (note_id, asset_id, side) VALUES (?1, ?2, 'front')",
                        params![note_id, asset_id],
                    )
                    .map_err(CommandError::database)?;
            }
            for asset_id in &validated.back_asset_ids {
                transaction
                    .execute(
                        "INSERT INTO note_assets (note_id, asset_id, side) VALUES (?1, ?2, 'back')",
                        params![note_id, asset_id],
                    )
                    .map_err(CommandError::database)?;
            }
            refresh_note_search(&transaction, &note_id)?;
            note_map.insert(note.source_id.clone(), note_id);
        }

        let imported_at = unix_time_millis()?;
        for card in &package.cards {
            let note_id = note_map.get(&card.source_note_id).ok_or_else(|| {
                CommandError::invalid_package("A generated card refers to a missing note.")
            })?;
            let card_id = choose_id(&transaction, EntityTable::Card, &card.source_id)?;
            transaction
                .execute(
                    "INSERT INTO cards (\n\
                         id, note_id, deck_id, direction, template_ordinal, created_at, updated_at\n\
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        card_id,
                        note_id,
                        deck_id,
                        card.direction,
                        card.template_ordinal,
                        card.created_at,
                        card.updated_at
                    ],
                )
                .map_err(CommandError::database)?;
            insert_new_schedule(&transaction, &card_id, imported_at)?;
        }

        transaction.commit().map_err(CommandError::database)?;
        let deck = get_deck_from_connection(connection, &deck_id)?;
        Ok(ImportResultDto {
            deck,
            card_count: i64::try_from(package.cards.len()).unwrap_or(i64::MAX),
            asset_count: i64::try_from(package.assets.len()).unwrap_or(i64::MAX),
        })
    })();

    if result.is_err() {
        for path in created_files {
            let _ = fs::remove_file(path);
        }
    }
    result
}

#[derive(Clone, Copy)]
enum EntityTable {
    Deck,
    Note,
    Card,
    Asset,
}

fn choose_id(
    connection: &Connection,
    table: EntityTable,
    preferred: &str,
) -> Result<String, CommandError> {
    validate_entity_id(preferred, "package entity")?;
    let query = match table {
        EntityTable::Deck => "SELECT EXISTS(SELECT 1 FROM decks WHERE id = ?1)",
        EntityTable::Note => "SELECT EXISTS(SELECT 1 FROM notes WHERE id = ?1)",
        EntityTable::Card => "SELECT EXISTS(SELECT 1 FROM cards WHERE id = ?1)",
        EntityTable::Asset => "SELECT EXISTS(SELECT 1 FROM assets WHERE id = ?1)",
    };
    let exists: i64 = connection
        .query_row(query, [preferred], |row| row.get(0))
        .map_err(CommandError::database)?;
    if exists == 0 {
        Ok(preferred.to_string())
    } else {
        generate_id(connection)
    }
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
                        "An imported card refers to an image not contained in the package.",
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

fn validate_card_relationships(
    notes: &[ImportedNote],
    cards: &[ImportedCard],
) -> Result<(), CommandError> {
    if cards.len() != notes.len() {
        return Err(CommandError::invalid_package(
            "The deck does not contain exactly one basic card for every note.",
        ));
    }
    let note_ids = notes
        .iter()
        .map(|note| note.source_id.as_str())
        .collect::<BTreeSet<_>>();
    let card_note_ids = cards
        .iter()
        .map(|card| card.source_note_id.as_str())
        .collect::<BTreeSet<_>>();
    if card_note_ids.len() != cards.len() || card_note_ids != note_ids {
        return Err(CommandError::invalid_package(
            "Generated-card relationships in the deck are inconsistent.",
        ));
    }
    Ok(())
}

fn validate_package_asset_links(
    package: &Connection,
    notes: &[ImportedNote],
    assets: &[ImportedAsset],
) -> Result<(), CommandError> {
    let mut expected = BTreeSet::new();
    for note in notes {
        for asset_id in collect_asset_ids(&note.front_doc)? {
            expected.insert((note.source_id.clone(), asset_id, "front".to_string()));
        }
        for asset_id in collect_asset_ids(&note.back_doc)? {
            expected.insert((note.source_id.clone(), asset_id, "back".to_string()));
        }
    }

    let mut statement = package
        .prepare(
            "SELECT source_note_id, source_asset_id, side\n\
             FROM note_assets\n\
             ORDER BY source_note_id, source_asset_id, side",
        )
        .map_err(|error| {
            CommandError::invalid_package(format!("Could not read image links: {error}"))
        })?;
    let actual = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(CommandError::database)?;

    for (note_id, asset_id, side) in &actual {
        validate_entity_id(note_id, "exported note")?;
        validate_entity_id(asset_id, "exported asset")?;
        if !matches!(side.as_str(), "front" | "back") {
            return Err(CommandError::invalid_package(
                "The deck contains an invalid image-side relationship.",
            ));
        }
    }
    if actual != expected {
        return Err(CommandError::invalid_package(
            "The deck's image references do not match its card content.",
        ));
    }

    let packaged_assets = assets
        .iter()
        .map(|asset| asset.source_id.as_str())
        .collect::<BTreeSet<_>>();
    let referenced_assets = expected
        .iter()
        .map(|(_, asset_id, _)| asset_id.as_str())
        .collect::<BTreeSet<_>>();
    if packaged_assets != referenced_assets {
        return Err(CommandError::invalid_package(
            "The deck contains missing or unreferenced image data.",
        ));
    }
    Ok(())
}

fn validate_timestamps(created_at: i64, updated_at: i64, label: &str) -> Result<(), CommandError> {
    if created_at < 0 || updated_at < created_at {
        return Err(CommandError::invalid_package(format!(
            "The imported {label} has invalid timestamps."
        )));
    }
    Ok(())
}

fn replace_export_file(temporary: &Path, destination: &Path) -> Result<(), CommandError> {
    let backup = backup_export_path(destination);
    let had_destination = destination.exists();
    if backup.exists() {
        fs::remove_file(&backup).map_err(CommandError::io)?;
    }
    if had_destination {
        fs::rename(destination, &backup).map_err(CommandError::io)?;
    }

    if let Err(error) = fs::rename(temporary, destination) {
        if had_destination {
            let _ = fs::rename(&backup, destination);
        }
        let _ = fs::remove_file(temporary);
        return Err(CommandError::io(error));
    }

    if had_destination {
        if let Err(error) = fs::remove_file(&backup) {
            eprintln!(
                "Could not remove old export backup {}: {error}",
                backup.display()
            );
        }
    }
    Ok(())
}

fn backup_export_path(destination: &Path) -> PathBuf {
    let filename = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("deck.studydeck");
    destination.with_file_name(format!(".{filename}.backup-{}", std::process::id()))
}

fn metadata_value(connection: &Connection, key: &str) -> Result<String, CommandError> {
    connection
        .query_row(
            "SELECT value FROM package_metadata WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::invalid_package(format!("The deck is missing metadata: {key}."))
        })
}

fn validate_package_schema(connection: &Connection) -> Result<(), CommandError> {
    let expected = BTreeSet::from([
        "package_metadata",
        "deck",
        "notes",
        "cards",
        "assets",
        "note_assets",
    ]);
    let mut statement = connection
        .prepare(
            "SELECT type, name FROM sqlite_schema\n\
             WHERE name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .map_err(|error| {
            CommandError::invalid_package(format!("Could not inspect the deck schema: {error}"))
        })?;
    let objects = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| {
            CommandError::invalid_package(format!("Could not inspect the deck schema: {error}"))
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            CommandError::invalid_package(format!("Could not inspect the deck schema: {error}"))
        })?;
    let mut tables = BTreeSet::new();
    for (object_type, name) in objects {
        if object_type != "table" || !expected.contains(name.as_str()) {
            return Err(CommandError::invalid_package(
                "The deck contains unsupported SQLite objects.",
            ));
        }
        tables.insert(name);
    }
    if tables.len() != expected.len() || expected.iter().any(|name| !tables.contains(*name)) {
        return Err(CommandError::invalid_package(
            "The deck package schema is incomplete.",
        ));
    }
    Ok(())
}

fn enforce_package_row_limit(
    connection: &Connection,
    table: &str,
    maximum: usize,
    message: &str,
) -> Result<(), CommandError> {
    let query = match table {
        "notes" => "SELECT COUNT(*) FROM notes",
        "cards" => "SELECT COUNT(*) FROM cards",
        "assets" => "SELECT COUNT(*) FROM assets",
        "note_assets" => "SELECT COUNT(*) FROM note_assets",
        _ => {
            return Err(CommandError::invalid_package(
                "The package validator requested an unknown table.",
            ))
        }
    };
    let count: i64 = connection
        .query_row(query, [], |row| row.get(0))
        .map_err(|error| {
            CommandError::invalid_package(format!("The deck package schema is invalid: {error}"))
        })?;
    if count < 0 || usize::try_from(count).unwrap_or(usize::MAX) > maximum {
        return Err(CommandError::invalid_package(message));
    }
    Ok(())
}

fn normalize_export_path(value: &str) -> Result<PathBuf, CommandError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CommandError::new(
            "invalid_export_path",
            "Choose where to save the deck.",
        ));
    }
    let mut path = PathBuf::from(trimmed);
    if path.extension().and_then(|value| value.to_str()) != Some("studydeck") {
        path.set_extension("studydeck");
    }
    Ok(path)
}

fn temporary_export_path(destination: &Path) -> PathBuf {
    let filename = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("deck.studydeck");
    destination.with_file_name(format!(".{filename}.tmp-{}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn paragraph(text: &str) -> Value {
        serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": text}]
            }]
        })
    }

    fn temporary_test_directory() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos();
        std::env::temp_dir().join(format!("ostudy-transfer-test-{suffix}"))
    }

    #[test]
    fn adds_the_studydeck_extension() {
        assert_eq!(
            normalize_export_path("/tmp/pharmacology")
                .expect("path should normalize")
                .extension()
                .and_then(|value| value.to_str()),
            Some("studydeck")
        );
    }

    #[test]
    fn exports_and_imports_a_complete_deck_round_trip() {
        let root = temporary_test_directory();
        std::fs::create_dir_all(&root).expect("test directory should exist");
        let paths = AppPaths::new(root.join("app-data")).expect("paths should initialize");
        let database = Database::open_in_memory().expect("database should open");

        let deck = database
            .with_connection(|connection| {
                crate::decks::create_deck_in_connection(
                    connection,
                    crate::decks::CreateDeckInput {
                        name: "NCLEX Pharmacology".to_string(),
                        description: Some("Medication review".to_string()),
                    },
                )
            })
            .expect("deck should be created");

        database
            .with_connection(|connection| {
                crate::cards::create_card_in_connection(
                    connection,
                    &paths,
                    &deck.id,
                    crate::cards::validate_content(
                        paragraph("What is the action of metoprolol?"),
                        paragraph("It selectively blocks beta-1 adrenergic receptors."),
                        "ignored snapshot".to_string(),
                        "ignored snapshot".to_string(),
                    )?,
                )
            })
            .expect("card should be created");

        let destination = root.join("pharmacology.studydeck");
        let exported = database
            .with_connection(|connection| {
                export_deck_from_connection(connection, &paths, &deck.id, &destination)
            })
            .expect("deck should export");
        assert_eq!(exported.card_count, 1);
        assert_eq!(exported.asset_count, 0);
        assert!(destination.is_file());

        let package = read_package(&destination).expect("package should validate");
        let imported = database
            .with_connection(|connection| import_package(connection, &paths, package))
            .expect("package should import");
        assert_ne!(imported.deck.id, deck.id);
        assert_eq!(imported.deck.name, "NCLEX Pharmacology");
        assert_eq!(imported.card_count, 1);

        let imported_cards = database
            .with_connection(|connection| {
                crate::cards::list_cards_from_connection(connection, &paths, &imported.deck.id)
            })
            .expect("imported cards should list");
        assert_eq!(imported_cards.len(), 1);
        assert_eq!(
            imported_cards[0].front_text,
            "What is the action of metoprolol?"
        );
        assert_eq!(
            imported_cards[0].back_text,
            "It selectively blocks beta-1 adrenergic receptors."
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn remaps_image_asset_identifiers() {
        let mut doc = serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "image",
                "attrs": {"assetId": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "src": "old"}
            }]
        });
        let mapping = HashMap::from([(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        )]);
        rewrite_asset_ids(&mut doc, &mapping).expect("image should remap");
        assert_eq!(
            crate::cards::collect_asset_ids(&doc).expect("ids should collect"),
            vec!["bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string()]
        );
    }
}
