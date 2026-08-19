//! Rebuildable, validated checkpoints for long document-AI jobs.
//!
//! Checkpoints are acceleration data, not canonical learner data. Provider
//! results enter this store only after native validation. A cache read/write
//! failure is logged and degrades to recomputation; it must not discard an
//! otherwise valid model result. Contract identity includes only inputs that can
//! change semantics, not credential rotation or unrelated profile revisions.

use rusqlite::{params, OptionalExtension};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    ai::document_intelligence::{
        DocumentOutline, DocumentTaskContext, OUTLINE_PROMPT_VERSION, OUTLINE_SCHEMA_VERSION,
    },
    database::Database,
    document_scale::DocumentScalePlan,
    document_semantics::SemanticWindow,
    util::unix_time_millis,
};

const OUTLINE_CHECKPOINT_CONTRACT: &str = "document-outline-v3";
const GENERATION_CHECKPOINT_CONTRACT: &str = "document-generation-v2";
const MAX_CHECKPOINT_CARDS: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckpointIdentity {
    pub provider_kind: String,
    pub profile_id: String,
    pub model_id: String,
    pub extraction_backend: String,
    pub provider_capability_fingerprint: String,
    pub reasoning_effort: String,
}

impl CheckpointIdentity {
    pub(crate) fn from_task(
        task: &DocumentTaskContext,
        extraction_backend: &str,
        provider_capability_fingerprint: &str,
    ) -> Self {
        Self {
            provider_kind: task.provider_kind.clone(),
            profile_id: task.profile_id.clone(),
            model_id: task.model_id.clone(),
            extraction_backend: extraction_backend.to_string(),
            provider_capability_fingerprint: provider_capability_fingerprint.to_string(),
            reasoning_effort: task.reasoning_effort.clone(),
        }
    }

    fn material(&self) -> String {
        format!(
            "provider={}|profile={}|model={}|extractor={}|capabilities={}|reasoning={}",
            self.provider_kind,
            self.profile_id,
            self.model_id,
            self.extraction_backend,
            self.provider_capability_fingerprint,
            self.reasoning_effort,
        )
    }
}

pub(crate) fn load_final_outline_best_effort(
    database: &Database,
    document_id: &str,
    identity: &CheckpointIdentity,
) -> Option<DocumentOutline> {
    let result = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT outline_json
                 FROM document_outlines
                 WHERE document_id = ?1
                   AND profile_id = ?2
                   AND provider_kind = ?3
                   AND model_id = ?4
                   AND extraction_backend = ?5
                   AND schema_version = ?6
                   AND prompt_version = ?7",
                params![
                    document_id,
                    identity.profile_id,
                    identity.provider_kind,
                    identity.model_id,
                    identity.extraction_backend,
                    OUTLINE_SCHEMA_VERSION,
                    OUTLINE_PROMPT_VERSION,
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(crate::error::CommandError::database)
    });
    match result {
        Ok(Some(json)) => match serde_json::from_str::<DocumentOutline>(&json) {
            Ok(outline) => Some(outline),
            Err(error) => {
                cache_log(
                    "document_final_outline_cache_invalid",
                    document_id,
                    format!("decode={error}"),
                );
                None
            }
        },
        Ok(None) => None,
        Err(error) => {
            cache_log(
                "document_checkpoint_read_failed",
                document_id,
                format!(
                    "stage=final_outline code={} detail={}",
                    error.code, error.message
                ),
            );
            None
        }
    }
}

pub(crate) fn outline_contract_key(
    identity: &CheckpointIdentity,
    plan: DocumentScalePlan,
) -> String {
    hash_text(&format!(
        "{OUTLINE_CHECKPOINT_CONTRACT}|{}|{}|outline_schema={OUTLINE_SCHEMA_VERSION}|outline_prompt={OUTLINE_PROMPT_VERSION}",
        identity.material(),
        plan.planning_fingerprint(),
    ))
}

pub(crate) fn generation_contract_key(
    identity: &CheckpointIdentity,
    plan: DocumentScalePlan,
    outline: &DocumentOutline,
    maximum_cards: usize,
) -> Option<String> {
    let outline_json = match serde_json::to_string(outline) {
        Ok(value) => value,
        Err(error) => {
            crate::logging::error(
                "document_checkpoint_contract_failed",
                format!("stage=generation_outline detail={error}"),
            );
            return None;
        }
    };
    Some(hash_text(&format!(
        "{GENERATION_CHECKPOINT_CONTRACT}|{}|{}|max_cards={maximum_cards}|outline={}",
        identity.material(),
        plan.planning_fingerprint(),
        hash_text(&outline_json),
    )))
}

pub(crate) fn source_sha256(window: &SemanticWindow) -> String {
    let locator = window.source_locator();
    hash_text(&format!(
        "partition={}|title={}|chunks={}-{}|locator={}|tokens={}|source={}",
        window.partition_index,
        window.partition_title,
        window.first_chunk_index,
        window.last_chunk_index,
        locator.stable_identity(),
        window.token_estimate,
        window.source_text,
    ))
}

pub(crate) fn load_outline_fragments_best_effort(
    database: &Database,
    document_id: &str,
    contract_key: &str,
    windows: &[SemanticWindow],
) -> Vec<Option<DocumentOutline>> {
    let result = database.with_connection(|connection| {
        let mut cached = vec![None; windows.len()];
        let mut statement = connection
            .prepare(
                "SELECT window_index, source_sha256, fragment_json
                 FROM document_outline_checkpoints
                 WHERE document_id = ?1 AND contract_key = ?2
                 ORDER BY window_index",
            )
            .map_err(crate::error::CommandError::database)?;
        let rows = statement
            .query_map(params![document_id, contract_key], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(crate::error::CommandError::database)?;
        for row in rows {
            let (index, stored_sha, json) = row.map_err(crate::error::CommandError::database)?;
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
    });
    match result {
        Ok(cached) => cached,
        Err(error) => {
            cache_log(
                "document_checkpoint_read_failed",
                document_id,
                format!("stage=outline code={} detail={}", error.code, error.message),
            );
            vec![None; windows.len()]
        }
    }
}

pub(crate) fn persist_outline_fragment_best_effort(
    database: &Database,
    document_id: &str,
    contract_key: &str,
    window: &SemanticWindow,
    fragment: &DocumentOutline,
) -> bool {
    let json = match serde_json::to_string(fragment) {
        Ok(value) => value,
        Err(error) => {
            cache_log(
                "document_checkpoint_write_failed",
                document_id,
                format!(
                    "stage=outline window={} serialize={error}",
                    window.window_index
                ),
            );
            return false;
        }
    };
    let now = match unix_time_millis() {
        Ok(value) => value,
        Err(error) => {
            cache_log(
                "document_checkpoint_write_failed",
                document_id,
                format!(
                    "stage=outline window={} clock={}",
                    window.window_index, error.message
                ),
            );
            return false;
        }
    };
    let locator = window.source_locator();
    let result = database.with_connection(|connection| {
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
                    to_i64(window.window_index),
                    to_i64(window.partition_index),
                    window.partition_title,
                    locator.first(),
                    locator.last(),
                    to_i64(window.token_estimate),
                    json,
                    now,
                ],
            )
            .map_err(crate::error::CommandError::database)?;
        Ok(())
    });
    if let Err(error) = result {
        cache_log(
            "document_checkpoint_write_failed",
            document_id,
            format!(
                "stage=outline window={} code={} detail={}",
                window.window_index, error.code, error.message
            ),
        );
        return false;
    }
    true
}

pub(crate) fn load_generation_cards_best_effort(
    database: &Database,
    document_id: &str,
    contract_key: &str,
    window: &SemanticWindow,
) -> Option<Vec<Value>> {
    let result = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT source_sha256, cards_json, card_count
                 FROM document_generation_checkpoints
                 WHERE document_id = ?1 AND contract_key = ?2 AND window_index = ?3",
                params![document_id, contract_key, to_i64(window.window_index)],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(crate::error::CommandError::database)
    });
    let row = match result {
        Ok(value) => value,
        Err(error) => {
            cache_log(
                "document_checkpoint_read_failed",
                document_id,
                format!(
                    "stage=generation window={} code={} detail={}",
                    window.window_index, error.code, error.message
                ),
            );
            return None;
        }
    };
    let (stored_sha, json, count) = row?;
    if stored_sha != source_sha256(window) {
        return None;
    }
    let Ok(cards) = serde_json::from_str::<Vec<Value>>(&json) else {
        return None;
    };
    if cards.len() > MAX_CHECKPOINT_CARDS || to_i64(cards.len()) != count {
        return None;
    }
    Some(cards)
}

pub(crate) fn persist_generation_cards_best_effort(
    database: &Database,
    document_id: &str,
    contract_key: &str,
    window: &SemanticWindow,
    cards: &[Value],
) -> bool {
    if cards.len() > MAX_CHECKPOINT_CARDS {
        cache_log(
            "document_checkpoint_write_failed",
            document_id,
            format!(
                "stage=generation window={} too_many_cards={}",
                window.window_index,
                cards.len()
            ),
        );
        return false;
    }
    let json = match serde_json::to_string(cards) {
        Ok(value) => value,
        Err(error) => {
            cache_log(
                "document_checkpoint_write_failed",
                document_id,
                format!(
                    "stage=generation window={} serialize={error}",
                    window.window_index
                ),
            );
            return false;
        }
    };
    let now = match unix_time_millis() {
        Ok(value) => value,
        Err(error) => {
            cache_log(
                "document_checkpoint_write_failed",
                document_id,
                format!(
                    "stage=generation window={} clock={}",
                    window.window_index, error.message
                ),
            );
            return false;
        }
    };
    let locator = window.source_locator();
    let result = database.with_connection(|connection| {
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
                    to_i64(window.window_index),
                    to_i64(window.partition_index),
                    window.partition_title,
                    locator.first(),
                    locator.last(),
                    to_i64(window.token_estimate),
                    json,
                    to_i64(cards.len()),
                    now,
                ],
            )
            .map_err(crate::error::CommandError::database)?;
        Ok(())
    });
    if let Err(error) = result {
        cache_log(
            "document_checkpoint_write_failed",
            document_id,
            format!(
                "stage=generation window={} code={} detail={}",
                window.window_index, error.code, error.message
            ),
        );
        return false;
    }
    true
}

pub(crate) fn clear_generation_checkpoints_best_effort(database: &Database, document_id: &str) {
    let result = database.with_connection(|connection| {
        connection
            .execute(
                "DELETE FROM document_generation_checkpoints WHERE document_id = ?1",
                [document_id],
            )
            .map_err(crate::error::CommandError::database)?;
        Ok(())
    });
    if let Err(error) = result {
        cache_log(
            "document_checkpoint_cleanup_failed",
            document_id,
            format!("code={} detail={}", error.code, error.message),
        );
    }
}

fn cache_log(event: &str, document_id: &str, detail: String) {
    crate::logging::error(event, format!("document={document_id} {detail}"));
}

fn hash_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn window_hash_changes_when_semantic_locator_changes() {
        let first = window();
        let mut second = first.clone();
        second.page_numbers = vec![22, 23];
        second.page_start = 22;
        second.page_end = 23;
        assert_ne!(source_sha256(&first), source_sha256(&second));
    }

    #[test]
    fn checkpoint_identity_excludes_credential_epoch_and_profile_revision_by_design() {
        let identity = CheckpointIdentity {
            provider_kind: "llama_cpp".to_string(),
            profile_id: "profile".to_string(),
            model_id: "model".to_string(),
            extraction_backend: "extractor-v1".to_string(),
            provider_capability_fingerprint: "caps".to_string(),
            reasoning_effort: "none".to_string(),
        };
        let material = identity.material();
        assert!(!material.contains("credential"));
        assert!(!material.contains("revision"));
        assert!(material.contains("model=model"));
    }
}
