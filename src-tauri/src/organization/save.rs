use std::collections::BTreeSet;

use rusqlite::{params, Connection, Transaction, TransactionBehavior};
use serde::Deserialize;
use serde_json::Value;
use tauri::State;

use crate::{
    assets::ensure_assets_exist,
    cards::{get_card_from_connection, validate_content, CardDto, ValidatedContent},
    database::Database,
    error::CommandError,
    paths::AppPaths,
    scheduler::insert_new_schedule,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

use super::{
    ensure_active_deck, ensure_active_note,
    tags::{normalize_tag_names, replace_note_tags, NormalizedTag},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrganizedCardInput {
    pub deck_id: String,
    pub front_doc: Value,
    pub back_doc: Value,
    pub front_text: String,
    pub back_text: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOrganizedCardInput {
    pub note_id: String,
    pub revision: i64,
    pub front_doc: Value,
    pub back_doc: Value,
    pub front_text: String,
    pub back_text: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[tauri::command]
pub fn create_organized_card(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: CreateOrganizedCardInput,
) -> Result<CardDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    let content = validate_content(
        input.front_doc,
        input.back_doc,
        input.front_text,
        input.back_text,
    )?;
    let tags = normalize_tag_names(input.tags)?;
    database.with_connection(|connection| {
        create_organized_card_in_connection(connection, &paths, &input.deck_id, content, tags)
    })
}

#[tauri::command]
pub fn update_organized_card(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: UpdateOrganizedCardInput,
) -> Result<CardDto, CommandError> {
    validate_entity_id(&input.note_id, "note")?;
    if input.revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The card revision is invalid.",
        ));
    }
    let content = validate_content(
        input.front_doc,
        input.back_doc,
        input.front_text,
        input.back_text,
    )?;
    let tags = normalize_tag_names(input.tags)?;
    database.with_connection(|connection| {
        update_organized_card_in_connection(
            connection,
            &paths,
            &input.note_id,
            input.revision,
            content,
            tags,
        )
    })
}

fn create_organized_card_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    deck_id: &str,
    content: ValidatedContent,
    tags: Vec<NormalizedTag>,
) -> Result<CardDto, CommandError> {
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    ensure_active_deck(&transaction, deck_id)?;
    ensure_assets_exist(&transaction, &all_asset_ids(&content))?;
    let note_id = generate_id(&transaction)?;
    let card_id = generate_id(&transaction)?;

    transaction
        .execute(
            "INSERT INTO notes (\n\
                 id, deck_id, note_type, content_format, content_schema_version,\n\
                 front_doc, back_doc, front_text, back_text, created_at, updated_at, revision\n\
             ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, ?6, ?7, ?7, 1)",
            params![
                note_id,
                deck_id,
                content.front_doc_json,
                content.back_doc_json,
                content.front_text,
                content.back_text,
                now,
            ],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO cards (\n\
                 id, note_id, deck_id, direction, template_ordinal, created_at, updated_at\n\
             ) VALUES (?1, ?2, ?3, 'forward', 0, ?4, ?4)",
            params![card_id, note_id, deck_id, now],
        )
        .map_err(CommandError::database)?;
    insert_new_schedule(&transaction, &card_id, now)?;
    replace_note_assets(&transaction, &note_id, &content)?;
    replace_note_tags(&transaction, &note_id, &tags, now)?;
    transaction
        .execute(
            "UPDATE decks SET updated_at = ?1, revision = revision + 1 WHERE id = ?2",
            params![now, deck_id],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;
    get_card_from_connection(connection, paths, &note_id)
}

fn update_organized_card_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    note_id: &str,
    revision: i64,
    content: ValidatedContent,
    tags: Vec<NormalizedTag>,
) -> Result<CardDto, CommandError> {
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    ensure_active_note(&transaction, note_id)?;
    ensure_assets_exist(&transaction, &all_asset_ids(&content))?;
    let deck_id: String = transaction
        .query_row("SELECT deck_id FROM notes WHERE id = ?1", [note_id], |row| row.get(0))
        .map_err(CommandError::database)?;

    let changed = transaction
        .execute(
            "UPDATE notes\n\
             SET front_doc = ?1, back_doc = ?2, front_text = ?3, back_text = ?4,\n\
                 updated_at = ?5, revision = revision + 1\n\
             WHERE id = ?6 AND revision = ?7 AND deleted_at IS NULL",
            params![
                content.front_doc_json,
                content.back_doc_json,
                content.front_text,
                content.back_text,
                now,
                note_id,
                revision,
            ],
        )
        .map_err(CommandError::database)?;
    if changed == 0 {
        return Err(CommandError::new(
            "card_changed",
            "This card changed in another window. Reload it and try again.",
        ));
    }
    transaction
        .execute(
            "UPDATE cards SET updated_at = ?1 WHERE note_id = ?2 AND deleted_at IS NULL",
            params![now, note_id],
        )
        .map_err(CommandError::database)?;
    replace_note_assets(&transaction, note_id, &content)?;
    replace_note_tags(&transaction, note_id, &tags, now)?;
    transaction
        .execute(
            "UPDATE decks SET updated_at = ?1, revision = revision + 1 WHERE id = ?2",
            params![now, deck_id],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;
    get_card_from_connection(connection, paths, note_id)
}

fn all_asset_ids(content: &ValidatedContent) -> Vec<String> {
    content
        .front_asset_ids
        .iter()
        .chain(&content.back_asset_ids)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn replace_note_assets(
    transaction: &Transaction<'_>,
    note_id: &str,
    content: &ValidatedContent,
) -> Result<(), CommandError> {
    transaction
        .execute("DELETE FROM note_assets WHERE note_id = ?1", [note_id])
        .map_err(CommandError::database)?;
    for asset_id in &content.front_asset_ids {
        transaction
            .execute(
                "INSERT INTO note_assets (note_id, asset_id, side) VALUES (?1, ?2, 'front')",
                params![note_id, asset_id],
            )
            .map_err(CommandError::database)?;
    }
    for asset_id in &content.back_asset_ids {
        transaction
            .execute(
                "INSERT INTO note_assets (note_id, asset_id, side) VALUES (?1, ?2, 'back')",
                params![note_id, asset_id],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}
