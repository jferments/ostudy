//! Schema-11 collection reader. Queries are fixed application SQL, never donor SQL.

use std::{collections::BTreeMap, path::Path};

use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;

use super::{failure, package::COMPATIBILITY_EXPORT};
use crate::error::CommandError;

const MAX_ROWS: usize = 100_000;
const MAX_TEXT_BYTES: usize = 128 * 1024 * 1024;

#[derive(Deserialize)]
pub(super) struct Field {
    pub name: String,
    pub ord: usize,
}

#[derive(Deserialize)]
pub(super) struct Template {
    pub name: String,
    pub ord: usize,
    pub qfmt: String,
    pub afmt: String,
}

#[derive(Deserialize)]
pub(super) struct Model {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: i64,
    pub flds: Vec<Field>,
    pub tmpls: Vec<Template>,
    #[serde(default)]
    pub css: String,
}

#[derive(Deserialize)]
pub(super) struct Deck {
    pub name: String,
}

pub(super) struct Note {
    pub model_id: i64,
    pub fields: Vec<String>,
    pub tags: Vec<String>,
}

pub(super) struct Card {
    pub id: i64,
    pub note_id: i64,
    pub deck_id: i64,
    pub ordinal: u32,
}

pub(super) struct Collection {
    pub models: BTreeMap<i64, Model>,
    pub decks: BTreeMap<i64, Deck>,
    pub notes: BTreeMap<i64, Note>,
    pub cards: Vec<Card>,
}

pub(super) fn read(path: &Path) -> Result<Collection, CommandError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| failure("Could not read the Anki SQLite collection."))?;
    connection
        .execute_batch("PRAGMA trusted_schema = OFF; PRAGMA query_only = ON;")
        .map_err(|_| failure("Could not secure the Anki SQLite reader."))?;
    for table in ["col", "notes", "cards"] {
        validate_table(&connection, table)?;
    }
    let (version, models, decks): (i64, String, String) = connection
        .query_row("SELECT ver, models, decks FROM col LIMIT 1", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(|_| failure("Missing or unsupported Anki collection metadata."))?;
    if version != 11 {
        return Err(failure(COMPATIBILITY_EXPORT));
    }
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM (SELECT 1 FROM col LIMIT 2)",
            [],
            |row| row.get(0),
        )
        .map_err(|_| failure("Invalid collection metadata."))?;
    if count != 1 || models.len() > 8 * 1024 * 1024 || decks.len() > 8 * 1024 * 1024 {
        return Err(failure("Invalid or excessive Anki collection metadata."));
    }
    let models: BTreeMap<String, Model> = serde_json::from_str(&models)
        .map_err(|_| failure("Invalid Anki note-type definitions."))?;
    let decks: BTreeMap<String, Deck> =
        serde_json::from_str(&decks).map_err(|_| failure("Invalid Anki deck definitions."))?;
    if models.len() > 1000 || decks.len() > 10_000 {
        return Err(failure("Too many Anki note types or decks."));
    }
    let mut parsed_models = BTreeMap::new();
    for (id, mut model) in models {
        model.flds.sort_by_key(|field| field.ord);
        model.tmpls.sort_by_key(|template| template.ord);
        if model.flds.is_empty()
            || model.flds.len() > 128
            || model.tmpls.len() > 1000
            || model.name.len() > 1024
            || model.css.len() > 256 * 1024
            || model.flds.iter().enumerate().any(|(index, field)| {
                field.ord != index || field.name.is_empty() || field.name.len() > 256
            })
            || model.tmpls.iter().enumerate().any(|(index, template)| {
                template.ord != index
                    || template.name.len() > 1024
                    || template.qfmt.len() > 256 * 1024
                    || template.afmt.len() > 256 * 1024
            })
        {
            return Err(failure(
                "Unsupported or excessive Anki note-type structure.",
            ));
        }
        let unique: std::collections::BTreeSet<_> =
            model.flds.iter().map(|field| &field.name).collect();
        if unique.len() != model.flds.len() {
            return Err(failure("Anki note type contains duplicate field names."));
        }
        parsed_models.insert(parse_id(&id)?, model);
    }
    let mut parsed_decks = BTreeMap::new();
    for (id, deck) in decks {
        if deck.name.is_empty() || deck.name.len() > 4096 || deck.name.chars().any(char::is_control)
        {
            return Err(failure("Invalid Anki deck name."));
        }
        parsed_decks.insert(parse_id(&id)?, deck);
    }
    let notes = read_notes(&connection)?;
    let cards = read_cards(&connection)?;
    Ok(Collection {
        models: parsed_models,
        decks: parsed_decks,
        notes,
        cards,
    })
}

fn validate_table(connection: &Connection, table: &str) -> Result<(), CommandError> {
    let sql: String = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name = ?1 AND type = 'table'",
            [table],
            |row| row.get(0),
        )
        .map_err(|_| failure("Anki collection lacks a required ordinary table."))?;
    if sql.to_ascii_uppercase().contains("VIRTUAL") {
        return Err(failure(
            "Virtual tables are not accepted in Anki collections.",
        ));
    }
    let hidden: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_xinfo(?1) WHERE hidden <> 0",
            [table],
            |row| row.get(0),
        )
        .map_err(|_| failure("Unable to validate Anki table columns."))?;
    if hidden != 0 {
        return Err(failure(
            "Generated or hidden Anki database columns are unsupported.",
        ));
    }
    Ok(())
}

fn read_notes(connection: &Connection) -> Result<BTreeMap<i64, Note>, CommandError> {
    let mut statement = connection
        .prepare("SELECT id, mid, flds, tags FROM notes LIMIT 100001")
        .map_err(|_| failure("Unsupported Anki notes schema."))?;
    let mut rows = statement
        .query([])
        .map_err(|_| failure("Unable to read Anki notes."))?;
    let mut notes = BTreeMap::new();
    let mut bytes = 0_usize;
    while let Some(row) = rows.next().map_err(|_| failure("Corrupt Anki note row."))? {
        let id: i64 = row
            .get(0)
            .map_err(|_| failure("Invalid Anki note identifier."))?;
        let model_id: i64 = row
            .get(1)
            .map_err(|_| failure("Invalid Anki note type identifier."))?;
        let fields: String = row
            .get(2)
            .map_err(|_| failure("Invalid Anki note fields."))?;
        let tags: String = row.get(3).map_err(|_| failure("Invalid Anki note tags."))?;
        bytes = bytes
            .saturating_add(fields.len())
            .saturating_add(tags.len());
        if id <= 0
            || model_id <= 0
            || fields.len() > 512 * 1024
            || tags.len() > 16 * 1024
            || notes.len() >= MAX_ROWS
            || bytes > MAX_TEXT_BYTES
        {
            return Err(failure(
                "Anki notes exceed the supported identifier, count, or text limits.",
            ));
        }
        let note = Note {
            model_id,
            fields: fields.split('\u{1f}').map(str::to_string).collect(),
            tags: tags.split_whitespace().map(str::to_string).collect(),
        };
        if notes.insert(id, note).is_some() {
            return Err(failure("Duplicate Anki note identifiers."));
        }
    }
    Ok(notes)
}

fn read_cards(connection: &Connection) -> Result<Vec<Card>, CommandError> {
    let mut statement = connection
        .prepare("SELECT id, nid, did, ord, odid FROM cards LIMIT 100001")
        .map_err(|_| failure("Unsupported Anki cards schema."))?;
    let mut rows = statement
        .query([])
        .map_err(|_| failure("Unable to read Anki cards."))?;
    let mut cards = Vec::new();
    let mut ids = std::collections::BTreeSet::new();
    while let Some(row) = rows.next().map_err(|_| failure("Corrupt Anki card row."))? {
        let id: i64 = row
            .get(0)
            .map_err(|_| failure("Invalid Anki card identifier."))?;
        let note_id: i64 = row
            .get(1)
            .map_err(|_| failure("Invalid Anki card note identifier."))?;
        let deck_id: i64 = row
            .get(2)
            .map_err(|_| failure("Invalid Anki deck identifier."))?;
        let ordinal: i64 = row
            .get(3)
            .map_err(|_| failure("Invalid Anki template ordinal."))?;
        let original_deck: i64 = row
            .get(4)
            .map_err(|_| failure("Invalid Anki original deck."))?;
        if id <= 0
            || note_id <= 0
            || deck_id <= 0
            || original_deck < 0
            || !(0..999).contains(&ordinal)
            || cards.len() >= MAX_ROWS
            || !ids.insert(id)
        {
            return Err(failure(
                "Anki cards contain invalid, duplicate, or excessive records.",
            ));
        }
        cards.push(Card {
            id,
            note_id,
            deck_id: if original_deck > 0 {
                original_deck
            } else {
                deck_id
            },
            ordinal: ordinal as u32,
        });
    }
    Ok(cards)
}

fn parse_id(value: &str) -> Result<i64, CommandError> {
    value
        .parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| failure("Invalid Anki metadata identifier."))
}
