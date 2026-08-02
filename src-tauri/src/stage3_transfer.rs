//! Optional `.studydeck` metadata for reverse and fill-in-the-blank cards.
//!
//! The core package still contains ordinary notes, cards, tags, and assets. This
//! wrapper appends a small data-only metadata table. During import it validates
//! the complete generated-card graph before normalizing a temporary package for
//! the existing core importer, then restores the validated card identities with
//! fresh local schedules. Reusable authoring templates are intentionally not
//! exported.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{params, Connection, OpenFlags, TransactionBehavior};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    paths::AppPaths,
    scheduler::insert_new_schedule,
    tagged_transfer,
    transfer::{ExportDeckInput, ExportResultDto, ImportResultDto},
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_PACKAGE_FILE_BYTES: u64 = 640 * 1024 * 1024;
const MAX_PACKAGE_NOTES: usize = 100_000;
const MAX_PACKAGE_CARDS: usize = 200_000;
const MAX_CLOZE_SOURCE_CHARS: usize = 200_000;

#[derive(Clone)]
struct Stage3Note {
    source_id: String,
    note_type: String,
    reverse_enabled: bool,
    cloze_source: String,
    cloze_mode: String,
    front_text: String,
    back_text: String,
    created_at: i64,
    updated_at: i64,
    revision: i64,
}

#[derive(Clone)]
struct Stage3Card {
    source_id: String,
    source_note_id: String,
    direction: String,
    template_ordinal: i64,
    created_at: i64,
    updated_at: i64,
}

#[derive(Default)]
struct Stage3Payload {
    notes: Vec<Stage3Note>,
    cards: Vec<Stage3Card>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct NoteSignature {
    front_text: String,
    back_text: String,
    created_at: i64,
    updated_at: i64,
    revision: i64,
}

struct TemporaryPackageGuard {
    path: PathBuf,
}

impl Drop for TemporaryPackageGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[tauri::command(rename = "export_deck")]
pub fn export_deck_with_card_types(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ExportDeckInput,
) -> Result<ExportResultDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    let payload =
        database.with_connection(|connection| load_library_payload(connection, &input.deck_id))?;
    let result = tagged_transfer::export_deck_with_tags(database, paths, input)?;
    if let Err(error) = append_card_type_payload(Path::new(&result.destination_path), &payload) {
        let _ = fs::remove_file(&result.destination_path);
        return Err(error);
    }
    Ok(result)
}

#[tauri::command(rename = "import_deck")]
pub fn import_deck_with_card_types(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    source_path: String,
) -> Result<ImportResultDto, CommandError> {
    let source = fs::canonicalize(&source_path).map_err(CommandError::io)?;
    let metadata = fs::metadata(&source).map_err(CommandError::io)?;
    if metadata.len() == 0 || metadata.len() > MAX_PACKAGE_FILE_BYTES {
        return Err(CommandError::invalid_package(
            "The selected deck file is empty or exceeds the 640 MB import limit.",
        ));
    }
    let payload = read_card_type_payload(&source)?;
    if payload.notes.is_empty() {
        return tagged_transfer::import_deck_with_tags(database, paths, source_path);
    }

    let temporary = temporary_package_path();
    fs::copy(&source, &temporary).map_err(CommandError::io)?;
    let _guard = TemporaryPackageGuard {
        path: temporary.clone(),
    };
    normalize_package_for_core_import(&temporary, &payload)?;
    let mut result = tagged_transfer::import_deck_with_tags(
        database.clone(),
        paths,
        temporary.to_string_lossy().into_owned(),
    )?;
    database.with_connection(|connection| {
        restore_imported_card_types(connection, &result.deck.id, &payload)
    })?;
    result.card_count = i64::try_from(payload.cards.len()).unwrap_or(i64::MAX);
    Ok(result)
}

fn load_library_payload(
    connection: &Connection,
    deck_id: &str,
) -> Result<Stage3Payload, CommandError> {
    let notes = {
        let mut statement = connection
            .prepare(
                "SELECT id, note_type, reverse_enabled, cloze_source, cloze_mode,
                        front_text, back_text, created_at, updated_at, revision
                 FROM notes
                 WHERE deck_id = ?1 AND deleted_at IS NULL
                 ORDER BY created_at, id",
            )
            .map_err(CommandError::database)?;
        let notes = statement
            .query_map([deck_id], |row| {
                Ok(Stage3Note {
                    source_id: row.get(0)?,
                    note_type: row.get(1)?,
                    reverse_enabled: row.get::<_, i64>(2)? == 1,
                    cloze_source: row.get(3)?,
                    cloze_mode: row.get(4)?,
                    front_text: row.get(5)?,
                    back_text: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    revision: row.get(9)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        notes
    };
    let cards = {
        let mut statement = connection
            .prepare(
                "SELECT c.id, c.note_id, c.direction, c.template_ordinal,
                        c.created_at, c.updated_at
                 FROM cards c
                 JOIN notes n ON n.id = c.note_id
                 WHERE c.deck_id = ?1
                   AND c.deleted_at IS NULL
                   AND n.deleted_at IS NULL
                 ORDER BY c.created_at, c.id",
            )
            .map_err(CommandError::database)?;
        let cards = statement
            .query_map([deck_id], |row| {
                Ok(Stage3Card {
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
        cards
    };
    let payload = Stage3Payload { notes, cards };
    validate_payload(&payload)?;
    Ok(payload)
}

fn append_card_type_payload(path: &Path, payload: &Stage3Payload) -> Result<(), CommandError> {
    let mut package = Connection::open(path).map_err(CommandError::database)?;
    package
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = DELETE;
             PRAGMA synchronous = FULL;
             CREATE TABLE IF NOT EXISTS card_type_metadata (
                 source_note_id TEXT PRIMARY KEY,
                 reverse_enabled INTEGER NOT NULL CHECK(reverse_enabled IN (0, 1)),
                 cloze_source TEXT NOT NULL,
                 cloze_mode TEXT NOT NULL
                     CHECK(cloze_mode IN ('one_at_a_time', 'all_together'))
             ) STRICT;",
        )
        .map_err(CommandError::database)?;
    let transaction = package
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    for note in &payload.notes {
        transaction
            .execute(
                "INSERT INTO card_type_metadata (
                     source_note_id, reverse_enabled, cloze_source, cloze_mode
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    note.source_id,
                    i64::from(note.reverse_enabled),
                    note.cloze_source,
                    note.cloze_mode,
                ],
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
            "The exported deck did not pass its final card-type integrity check.",
        ));
    }
    Ok(())
}

fn read_card_type_payload(path: &Path) -> Result<Stage3Payload, CommandError> {
    let package =
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|error| {
            CommandError::invalid_package(format!("Could not open the deck: {error}"))
        })?;
    package
        .execute_batch("PRAGMA trusted_schema = OFF; PRAGMA query_only = ON;")
        .map_err(|error| {
            CommandError::invalid_package(format!("Could not secure the deck reader: {error}"))
        })?;
    if !table_exists(&package, "card_type_metadata")? {
        return Ok(Stage3Payload::default());
    }

    let notes = {
        let mut statement = package
            .prepare(
                "SELECT n.source_id, n.note_type,
                        m.reverse_enabled, m.cloze_source, m.cloze_mode,
                        n.front_text, n.back_text, n.created_at, n.updated_at, n.revision
                 FROM notes n
                 JOIN card_type_metadata m ON m.source_note_id = n.source_id
                 ORDER BY n.created_at, n.source_id",
            )
            .map_err(CommandError::database)?;
        let notes = statement
            .query_map([], |row| {
                Ok(Stage3Note {
                    source_id: row.get(0)?,
                    note_type: row.get(1)?,
                    reverse_enabled: row.get::<_, i64>(2)? == 1,
                    cloze_source: row.get(3)?,
                    cloze_mode: row.get(4)?,
                    front_text: row.get(5)?,
                    back_text: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    revision: row.get(9)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        notes
    };
    let cards = {
        let mut statement = package
            .prepare(
                "SELECT source_id, source_note_id, direction, template_ordinal,
                        created_at, updated_at
                 FROM cards ORDER BY created_at, source_id",
            )
            .map_err(CommandError::database)?;
        let cards = statement
            .query_map([], |row| {
                Ok(Stage3Card {
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
        cards
    };
    let payload = Stage3Payload { notes, cards };
    validate_payload(&payload)?;
    Ok(payload)
}

fn validate_payload(payload: &Stage3Payload) -> Result<(), CommandError> {
    if payload.notes.is_empty() || payload.notes.len() > MAX_PACKAGE_NOTES {
        return Err(CommandError::invalid_package(
            "The deck contains an invalid number of source notes.",
        ));
    }
    if payload.cards.is_empty() || payload.cards.len() > MAX_PACKAGE_CARDS {
        return Err(CommandError::invalid_package(
            "The deck contains an invalid number of generated cards.",
        ));
    }
    let note_ids = payload
        .notes
        .iter()
        .map(|note| note.source_id.as_str())
        .collect::<BTreeSet<_>>();
    if note_ids.len() != payload.notes.len() {
        return Err(CommandError::invalid_package(
            "The deck contains duplicate source notes.",
        ));
    }
    let mut cards_by_note = BTreeMap::<&str, Vec<&Stage3Card>>::new();
    let mut card_ids = BTreeSet::new();
    for card in &payload.cards {
        validate_entity_id(&card.source_id, "exported card")?;
        validate_entity_id(&card.source_note_id, "exported note")?;
        if !card_ids.insert(card.source_id.as_str())
            || !note_ids.contains(card.source_note_id.as_str())
            || card.template_ordinal < 0
            || card.created_at < 0
            || card.updated_at < card.created_at
        {
            return Err(CommandError::invalid_package(
                "The deck contains an invalid generated-card record.",
            ));
        }
        cards_by_note
            .entry(card.source_note_id.as_str())
            .or_default()
            .push(card);
    }

    for note in &payload.notes {
        validate_entity_id(&note.source_id, "exported note")?;
        if note.created_at < 0
            || note.updated_at < note.created_at
            || note.revision < 1
            || note.cloze_source.chars().count() > MAX_CLOZE_SOURCE_CHARS
            || !matches!(note.cloze_mode.as_str(), "one_at_a_time" | "all_together")
        {
            return Err(CommandError::invalid_package(
                "The deck contains invalid card-type metadata.",
            ));
        }
        let cards = cards_by_note.get(note.source_id.as_str()).ok_or_else(|| {
            CommandError::invalid_package("A source note has no generated cards.")
        })?;
        let forward = cards
            .iter()
            .filter(|card| card.direction == "forward")
            .collect::<Vec<_>>();
        if forward.len() != 1 {
            return Err(CommandError::invalid_package(
                "Every source note must contain exactly one primary card.",
            ));
        }
        let keys = cards
            .iter()
            .map(|card| (card.direction.as_str(), card.template_ordinal))
            .collect::<BTreeSet<_>>();
        if keys.len() != cards.len() {
            return Err(CommandError::invalid_package(
                "A source note contains duplicate generated-card identities.",
            ));
        }
        match note.note_type.as_str() {
            "basic" => {
                if !note.cloze_source.is_empty()
                    || cards.iter().any(|card| {
                        !matches!(card.direction.as_str(), "forward" | "reverse")
                            || card.template_ordinal != 0
                    })
                    || cards
                        .iter()
                        .filter(|card| card.direction == "reverse")
                        .count()
                        > 1
                    || note.reverse_enabled != cards.iter().any(|card| card.direction == "reverse")
                {
                    return Err(CommandError::invalid_package(
                        "A Basic note contains inconsistent reverse-card metadata.",
                    ));
                }
            }
            "cloze" => {
                if note.reverse_enabled || note.cloze_source.trim().is_empty() {
                    return Err(CommandError::invalid_package(
                        "A fill-in-the-blank note contains invalid metadata.",
                    ));
                }
                if note.cloze_mode == "all_together" {
                    if cards.len() != 1
                        || forward[0].template_ordinal != 0
                        || cards[0].direction != "forward"
                    {
                        return Err(CommandError::invalid_package(
                            "An all-blanks card contains inconsistent generated cards.",
                        ));
                    }
                } else if cards.iter().any(|card| {
                    !matches!(card.direction.as_str(), "forward" | "cloze")
                        || card.template_ordinal <= 0
                }) {
                    return Err(CommandError::invalid_package(
                        "A one-at-a-time blank card contains an invalid blank identity.",
                    ));
                }
            }
            _ => {
                return Err(CommandError::invalid_package(
                    "The deck contains an unsupported source-note type.",
                ));
            }
        }
    }
    Ok(())
}

fn normalize_package_for_core_import(
    path: &Path,
    payload: &Stage3Payload,
) -> Result<(), CommandError> {
    let mut package = Connection::open(path).map_err(CommandError::database)?;
    let transaction = package
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    for note in &payload.notes {
        let primary = payload
            .cards
            .iter()
            .find(|card| card.source_note_id == note.source_id && card.direction == "forward")
            .ok_or_else(|| {
                CommandError::invalid_package("A source note is missing its primary card.")
            })?;
        transaction
            .execute(
                "DELETE FROM cards
                 WHERE source_note_id = ?1 AND source_id != ?2",
                params![note.source_id, primary.source_id],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE cards
                 SET direction = 'forward', template_ordinal = 0
                 WHERE source_id = ?1",
                [primary.source_id.as_str()],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE notes SET note_type = 'basic' WHERE source_id = ?1",
                [note.source_id.as_str()],
            )
            .map_err(CommandError::database)?;
    }
    transaction.commit().map_err(CommandError::database)
}

fn restore_imported_card_types(
    connection: &mut Connection,
    deck_id: &str,
    payload: &Stage3Payload,
) -> Result<(), CommandError> {
    let source_to_local = match_imported_notes(connection, deck_id, &payload.notes)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let imported_at = unix_time_millis()?;

    for note in &payload.notes {
        let local_note_id = source_to_local.get(&note.source_id).ok_or_else(|| {
            CommandError::invalid_package("A card type could not be matched to its imported note.")
        })?;
        transaction
            .execute(
                "UPDATE notes
                 SET note_type = ?1, reverse_enabled = ?2,
                     cloze_source = ?3, cloze_mode = ?4
                 WHERE id = ?5 AND deck_id = ?6 AND deleted_at IS NULL",
                params![
                    note.note_type,
                    i64::from(note.reverse_enabled),
                    note.cloze_source,
                    note.cloze_mode,
                    local_note_id,
                    deck_id,
                ],
            )
            .map_err(CommandError::database)?;
        let local_primary: String = transaction
            .query_row(
                "SELECT id FROM cards
                 WHERE note_id = ?1 AND deleted_at IS NULL",
                [local_note_id.as_str()],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        let primary = payload
            .cards
            .iter()
            .find(|card| card.source_note_id == note.source_id && card.direction == "forward")
            .ok_or_else(|| {
                CommandError::invalid_package("A source note is missing its primary card.")
            })?;
        transaction
            .execute(
                "UPDATE cards
                 SET direction = 'forward', template_ordinal = ?1,
                     created_at = ?2, updated_at = ?3
                 WHERE id = ?4",
                params![
                    primary.template_ordinal,
                    primary.created_at,
                    primary.updated_at,
                    local_primary,
                ],
            )
            .map_err(CommandError::database)?;

        for card in payload.cards.iter().filter(|card| {
            card.source_note_id == note.source_id && card.source_id != primary.source_id
        }) {
            let card_id = choose_card_id(&transaction, &card.source_id)?;
            transaction
                .execute(
                    "INSERT INTO cards (
                         id, note_id, deck_id, direction, template_ordinal,
                         created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        card_id,
                        local_note_id,
                        deck_id,
                        card.direction,
                        card.template_ordinal,
                        card.created_at,
                        card.updated_at,
                    ],
                )
                .map_err(CommandError::database)?;
            insert_new_schedule(&transaction, &card_id, imported_at)?;
        }
    }
    transaction.commit().map_err(CommandError::database)
}

fn match_imported_notes(
    connection: &Connection,
    deck_id: &str,
    source_notes: &[Stage3Note],
) -> Result<HashMap<String, String>, CommandError> {
    let local_notes = {
        let mut statement = connection
            .prepare(
                "SELECT id, front_text, back_text, created_at, updated_at, revision
                 FROM notes
                 WHERE deck_id = ?1 AND deleted_at IS NULL
                 ORDER BY rowid",
            )
            .map_err(CommandError::database)?;
        let local_notes = statement
            .query_map([deck_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    NoteSignature {
                        front_text: row.get(1)?,
                        back_text: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                        revision: row.get(5)?,
                    },
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        local_notes
    };
    let local_ids = local_notes
        .iter()
        .map(|(id, _)| id.as_str())
        .collect::<BTreeSet<_>>();
    let mut result = HashMap::new();
    let mut used_local = BTreeSet::new();
    for source in source_notes {
        if local_ids.contains(source.source_id.as_str()) {
            result.insert(source.source_id.clone(), source.source_id.clone());
            used_local.insert(source.source_id.clone());
        }
    }

    let mut source_groups = BTreeMap::<NoteSignature, VecDeque<String>>::new();
    for source in source_notes {
        if result.contains_key(&source.source_id) {
            continue;
        }
        source_groups
            .entry(note_signature(source))
            .or_default()
            .push_back(source.source_id.clone());
    }
    let mut local_groups = BTreeMap::<NoteSignature, VecDeque<String>>::new();
    for (local_id, signature) in local_notes {
        if used_local.contains(&local_id) {
            continue;
        }
        local_groups
            .entry(signature)
            .or_default()
            .push_back(local_id);
    }
    for (signature, mut source_ids) in source_groups {
        let mut local_ids = local_groups.remove(&signature).ok_or_else(|| {
            CommandError::invalid_package(
                "Imported card types could not be matched to the imported notes.",
            )
        })?;
        if source_ids.len() != local_ids.len() {
            return Err(CommandError::invalid_package(
                "Imported card types could not be matched to duplicate notes.",
            ));
        }
        while let (Some(source_id), Some(local_id)) =
            (source_ids.pop_front(), local_ids.pop_front())
        {
            result.insert(source_id, local_id);
        }
    }
    if result.len() != source_notes.len() {
        return Err(CommandError::invalid_package(
            "Imported card types could not be matched to every note.",
        ));
    }
    Ok(result)
}

fn note_signature(note: &Stage3Note) -> NoteSignature {
    NoteSignature {
        front_text: note.front_text.clone(),
        back_text: note.back_text.clone(),
        created_at: note.created_at,
        updated_at: note.updated_at,
        revision: note.revision,
    }
}

fn choose_card_id(connection: &Connection, preferred: &str) -> Result<String, CommandError> {
    validate_entity_id(preferred, "exported card")?;
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM cards WHERE id = ?1)",
            [preferred],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if exists == 0 {
        Ok(preferred.to_string())
    } else {
        generate_id(connection)
    }
}

fn table_exists(connection: &Connection, name: &str) -> Result<bool, CommandError> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1
             )",
            [name],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    Ok(exists == 1)
}

fn temporary_package_path() -> PathBuf {
    let suffix = unix_time_millis().unwrap_or_default();
    std::env::temp_dir().join(format!(
        "ostudy-stage3-import-{}-{suffix}.studydeck",
        std::process::id()
    ))
}
