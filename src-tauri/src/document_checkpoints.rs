//! Durable, validated checkpoints for long document AI jobs.
//!
//! Only task outputs that have already passed the provider schema and ostudy's
//! native parser are stored here. Raw provider responses, credentials, prompts,
//! or reasoning content are never persisted. Contract keys bind a checkpoint to
//! the exact provider profile revision, model, extraction backend, scale policy,
//! and task contract so stale results fail closed.

use rusqlite::{params, OptionalExtension};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    ai::document_intelligence::{DocumentOutline, DocumentTaskContext},
    database::Database,
    document_scale::DocumentScalePlan,
    document_semantics::SemanticWindow,
    error::CommandError,
    pdf_backend,
    util::unix_time_millis,
};

const OUTLINE_CHECKPOINT_CONTRACT: &str = "stage47-outline-v1";
const GENERATION_CHECKPOINT_CONTRACT: &str = "stage47-generation-v1";
const MAX_CHECKPOINT_CARDS: usize = 32;

pub(crate) fn load_final_outline(
    database: &Database,
    document_id: &str,
    task: &DocumentTaskContext,
) -> Result<Option<DocumentOutline>, CommandError> {
    let outline_json = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT outline_json
                 FROM document_outlines
                 WHERE document_id = ?1
                   AND profile_id = ?2
                   AND provider_kind = ?3
                   AND model_id = ?4
                   AND profile_revision = ?5
                   AND credential_epoch = ?6
                   AND extraction_backend = ?7",
                params![
                    document_id,
                    task.profile_id,
                    task.provider_kind,
                    task.model_id,
                    task.profile_revision,
                    task.credential_epoch,
                    pdf_backend::active_backend_id(),
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)
    })?;
    let Some(json) = outline_json else {
        return Ok(None);
    };
    match serde_json::from_str::<DocumentOutline>(&json) {
        Ok(outline) => Ok(Some(outline)),
        Err(_) => {
            crate::logging::info(
                "document_final_outline_cache_invalid",
                format!("document={document_id}"),
            );
            Ok(None)
        }
    }
}

pub(crate) fn outline_contract_key(task: &DocumentTaskContext, plan: DocumentScalePlan) -> String {
    hash_text(&format!(
        "{OUTLINE_CHECKPOINT_CONTRACT}|provider={}|profile={}|model={}|revision={}|credential={}|extractor={}|scale={}|outline_tokens={}|outline_pages={}|outline_overlap={}",
        task.provider_kind,
        task.profile_id,
        task.model_id,
        task.profile_revision,
        task.credential_epoch,
        pdf_backend::active_backend_id(),
        plan.label(),
        plan.outline_window_max_tokens,
        plan.outline_window_max_pages,
        plan.outline_window_overlap_tokens,
    ))
}

pub(crate) fn generation_contract_key(
    task: &DocumentTaskContext,
    plan: DocumentScalePlan,
    outline: &DocumentOutline,
    maximum_cards: usize,
) -> Result<String, CommandError> {
    let outline_json = serde_json::to_string(outline).map_err(CommandError::serialization)?;
    Ok(hash_text(&format!(
        "{GENERATION_CHECKPOINT_CONTRACT}|provider={}|profile={}|model={}|revision={}|credential={}|extractor={}|scale={}|generation_tokens={}|generation_pages={}|generation_overlap={}|max_cards={maximum_cards}|outline={}",
        task.provider_kind,
        task.profile_id,
        task.model_id,
        task.profile_revision,
        task.credential_epoch,
        pdf_backend::active_backend_id(),
        plan.label(),
        plan.generation_window_max_tokens,
        plan.generation_window_max_pages,
        plan.generation_window_overlap_tokens,
        hash_text(&outline_json),
    )))
}

pub(crate) fn source_sha256(window: &SemanticWindow) -> String {
    hash_text(&format!(
        "partition={}|title={}|chunks={}-{}|pages={:?}|tokens={}|source={}",
        window.partition_index,
        window.partition_title,
        window.first_chunk_index,
        window.last_chunk_index,
        window.page_numbers,
        window.token_estimate,
        window.source_text,
    ))
}

pub(crate) fn load_outline_fragments(
    database: &Database,
    document_id: &str,
    contract_key: &str,
    windows: &[SemanticWindow],
) -> Result<Vec<Option<DocumentOutline>>, CommandError> {
    database.with_connection(|connection| {
        let mut cached = vec![None; windows.len()];
        let mut statement = connection
            .prepare(
                "SELECT window_index, source_sha256, fragment_json
                 FROM document_outline_checkpoints
                 WHERE document_id = ?1 AND contract_key = ?2
                 ORDER BY window_index",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map(params![document_id, contract_key], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(CommandError::database)?;
        for row in rows {
            let (index, stored_sha, json) = row.map_err(CommandError::database)?;
            let Ok(index) = usize::try_from(index) else {
                continue;
            };
            let Some(window) = windows.get(index) else {
                continue;
            };
            if stored_sha != source_sha256(window) {
                continue;
            }
            if let Ok(fragment) = serde_json::from_str::<DocumentOutline>(&json) {
                cached[index] = Some(fragment);
            }
        }
        Ok(cached)
    })
}

pub(crate) fn persist_outline_fragment(
    database: &Database,
    document_id: &str,
    contract_key: &str,
    window: &SemanticWindow,
    fragment: &DocumentOutline,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let json = serde_json::to_string(fragment).map_err(CommandError::serialization)?;
    database.with_connection(|connection| {
        connection
            .execute(
                "INSERT INTO document_outline_checkpoints (
                     document_id, contract_key, source_sha256, window_index,
                     partition_index, partition_title, page_start, page_end,
                     token_estimate, fragment_json, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
                 ON CONFLICT(document_id, contract_key, window_index) DO UPDATE SET
                     source_sha256 = excluded.source_sha256,
                     partition_index = excluded.partition_index,
                     partition_title = excluded.partition_title,
                     page_start = excluded.page_start,
                     page_end = excluded.page_end,
                     token_estimate = excluded.token_estimate,
                     fragment_json = excluded.fragment_json,
                     updated_at = excluded.updated_at",
                params![
                    document_id,
                    contract_key,
                    source_sha256(window),
                    i64::try_from(window.window_index).unwrap_or(i64::MAX),
                    i64::try_from(window.partition_index).unwrap_or(i64::MAX),
                    window.partition_title,
                    window.page_start,
                    window.page_end,
                    i64::try_from(window.token_estimate).unwrap_or(i64::MAX),
                    json,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

pub(crate) fn load_generation_cards(
    database: &Database,
    document_id: &str,
    contract_key: &str,
    window: &SemanticWindow,
) -> Result<Option<Vec<Value>>, CommandError> {
    let row = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT source_sha256, cards_json, card_count
                 FROM document_generation_checkpoints
                 WHERE document_id = ?1 AND contract_key = ?2 AND window_index = ?3",
                params![
                    document_id,
                    contract_key,
                    i64::try_from(window.window_index).unwrap_or(i64::MAX),
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::database)
    })?;
    let Some((stored_sha, json, count)) = row else {
        return Ok(None);
    };
    if stored_sha != source_sha256(window) {
        return Ok(None);
    }
    let Ok(cards) = serde_json::from_str::<Vec<Value>>(&json) else {
        return Ok(None);
    };
    if cards.len() > MAX_CHECKPOINT_CARDS || i64::try_from(cards.len()).unwrap_or(i64::MAX) != count
    {
        return Ok(None);
    }
    Ok(Some(cards))
}

pub(crate) fn persist_generation_cards(
    database: &Database,
    document_id: &str,
    contract_key: &str,
    window: &SemanticWindow,
    cards: &[Value],
) -> Result<(), CommandError> {
    if cards.len() > MAX_CHECKPOINT_CARDS {
        return Err(CommandError::new(
            "document_checkpoint_invalid",
            "A generation window produced too many validated cards to checkpoint safely.",
        ));
    }
    let now = unix_time_millis()?;
    let json = serde_json::to_string(cards).map_err(CommandError::serialization)?;
    database.with_connection(|connection| {
        connection
            .execute(
                "INSERT INTO document_generation_checkpoints (
                     document_id, contract_key, source_sha256, window_index,
                     partition_index, partition_title, page_start, page_end,
                     token_estimate, cards_json, card_count, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
                 ON CONFLICT(document_id, contract_key, window_index) DO UPDATE SET
                     source_sha256 = excluded.source_sha256,
                     partition_index = excluded.partition_index,
                     partition_title = excluded.partition_title,
                     page_start = excluded.page_start,
                     page_end = excluded.page_end,
                     token_estimate = excluded.token_estimate,
                     cards_json = excluded.cards_json,
                     card_count = excluded.card_count,
                     updated_at = excluded.updated_at",
                params![
                    document_id,
                    contract_key,
                    source_sha256(window),
                    i64::try_from(window.window_index).unwrap_or(i64::MAX),
                    i64::try_from(window.partition_index).unwrap_or(i64::MAX),
                    window.partition_title,
                    window.page_start,
                    window.page_end,
                    i64::try_from(window.token_estimate).unwrap_or(i64::MAX),
                    json,
                    i64::try_from(cards.len()).unwrap_or(i64::MAX),
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

pub(crate) fn clear_generation_checkpoints(
    database: &Database,
    document_id: &str,
) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        connection
            .execute(
                "DELETE FROM document_generation_checkpoints WHERE document_id = ?1",
                [document_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

pub(crate) fn count_outline_checkpoints(
    database: &Database,
    document_id: &str,
    contract_key: &str,
) -> Result<i64, CommandError> {
    count_checkpoints(
        database,
        "document_outline_checkpoints",
        document_id,
        contract_key,
    )
}

pub(crate) fn count_generation_checkpoints(
    database: &Database,
    document_id: &str,
    contract_key: &str,
) -> Result<i64, CommandError> {
    count_checkpoints(
        database,
        "document_generation_checkpoints",
        document_id,
        contract_key,
    )
}

fn count_checkpoints(
    database: &Database,
    table: &str,
    document_id: &str,
    contract_key: &str,
) -> Result<i64, CommandError> {
    debug_assert!(matches!(
        table,
        "document_outline_checkpoints" | "document_generation_checkpoints"
    ));
    database.with_connection(|connection| {
        connection
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM {table} WHERE document_id = ?1 AND contract_key = ?2"
                ),
                params![document_id, contract_key],
                |row| row.get(0),
            )
            .map_err(CommandError::database)
    })
}

fn hash_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_semantics::SemanticWindow;

    fn window() -> SemanticWindow {
        SemanticWindow {
            window_index: 0,
            partition_index: 2,
            partition_title: "Chapter 3".to_string(),
            first_chunk_index: 10,
            last_chunk_index: 12,
            page_numbers: vec![21, 22],
            visual_page_numbers: vec![22],
            page_start: 21,
            page_end: 22,
            token_estimate: 800,
            source_text: "bounded source".to_string(),
        }
    }

    #[test]
    fn window_hash_changes_when_partition_identity_changes() {
        let first = window();
        let mut second = first.clone();
        second.partition_title = "Chapter 4".to_string();
        assert_ne!(source_sha256(&first), source_sha256(&second));
    }
}
