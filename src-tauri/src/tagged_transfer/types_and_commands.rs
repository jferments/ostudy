use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fs,
    path::Path,
};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use tauri::State;

use crate::{
    authoring::backfill_duplicate_hashes,
    database::Database,
    error::CommandError,
    paths::AppPaths,
    transfer::{self, ExportDeckInput, ExportResultDto, ImportResultDto},
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_PACKAGE_FILE_BYTES: u64 = 640 * 1024 * 1024;
const MAX_PACKAGE_TAGS: usize = 100_000;
const MAX_PACKAGE_TAG_LINKS: i64 = 3_200_000;
const MAX_TAGS_PER_NOTE: usize = 32;
const MAX_TAG_CHARS: usize = 64;

#[derive(Clone)]
struct ExportTag {
    source_id: String,
    name: String,
    normalized_name: String,
}

#[derive(Clone)]
struct SourceNoteSignature {
    source_id: String,
    front_text: String,
    back_text: String,
    created_at: i64,
    updated_at: i64,
    revision: i64,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct NoteSignature {
    front_text: String,
    back_text: String,
    created_at: i64,
    updated_at: i64,
    revision: i64,
}

#[derive(Default)]
struct TagPayload {
    tags: Vec<ExportTag>,
    links: Vec<(String, String)>,
    notes: Vec<SourceNoteSignature>,
}

#[tauri::command(rename = "export_deck")]
pub fn export_deck_with_tags(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ExportDeckInput,
) -> Result<ExportResultDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    let payload =
        database.with_connection(|connection| load_export_payload(connection, &input.deck_id))?;
    let result = transfer::export_deck(database, paths, input)?;

    if let Err(error) = append_payload(Path::new(&result.destination_path), &payload) {
        let _ = fs::remove_file(&result.destination_path);
        return Err(error);
    }
    Ok(result)
}

#[tauri::command(rename = "import_deck")]
pub fn import_deck_with_tags(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    source_path: String,
) -> Result<ImportResultDto, CommandError> {
    let canonical = fs::canonicalize(&source_path).map_err(CommandError::io)?;
    let metadata = fs::metadata(&canonical).map_err(CommandError::io)?;
    if metadata.len() == 0 || metadata.len() > MAX_PACKAGE_FILE_BYTES {
        return Err(CommandError::invalid_package(
            "The selected deck file is empty or exceeds the 640 MB import limit.",
        ));
    }
    let payload = read_payload(&canonical)?;
    let result = transfer::import_deck(database.clone(), paths, source_path)?;

    database.with_connection(|connection| {
        backfill_duplicate_hashes(connection)
            .map_err(|error| CommandError::new("duplicate_backfill_failed", error))?;
        if !payload.tags.is_empty() || !payload.links.is_empty() {
            apply_imported_tags(connection, &result.deck.id, payload)?;
        }
        Ok(())
    })?;
    Ok(result)
}
