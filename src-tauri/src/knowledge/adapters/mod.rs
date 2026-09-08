//! Bounded material adapters. They resolve authoritative native records and
//! publish semantic projections in short batch transactions. They never contact
//! a provider and never make ordinary card saves depend on AI.

mod cards;
mod documents;
mod guides;
mod scopes;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::{database::Database, error::CommandError, util::unix_time_millis};

use super::{
    contracts::{
        invalid, ContentOrigin, SourceKind, SourceRecord, StudyScope, MAX_PREPARATION_SOURCES,
        MAX_SOURCE_CHARS,
    },
    repository,
};

pub(crate) use scopes::{list_scope_choices, scope_documents};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SourceSeed {
    pub kind: SourceKind,
    pub owner_id: String,
    pub parent_key: String,
}

pub(crate) fn enumerate_sources(
    database: &Database,
    scope: &StudyScope,
    include_conversations: bool,
) -> Result<Vec<SourceSeed>, CommandError> {
    scope.validate()?;
    database.with_connection(|connection| {
        let mut seeds = scopes::enumerate_material(connection, scope)?;
        if include_conversations {
            let scope_json = serde_json::to_string(scope).map_err(CommandError::serialization)?;
            let mut statement = connection
                .prepare(
                    "SELECT t.conversation_id, t.id FROM tutor_turns t
                     JOIN tutor_conversations c ON c.id = t.conversation_id
                     WHERE c.scope_json = ?1 AND t.state = 'complete' AND t.evidence_current = 1
                       AND c.retrieval_enabled = 1 AND t.sequence > c.memory_start_sequence
                     ORDER BY c.id, t.sequence LIMIT 100001",
                )
                .map_err(CommandError::database)?;
            let conversations = statement
                .query_map([scope_json], |row| {
                    Ok(SourceSeed {
                        kind: SourceKind::Conversation,
                        owner_id: row.get(0)?,
                        parent_key: row.get(1)?,
                    })
                })
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?;
            seeds.extend(conversations);
        }
        if seeds.len() > MAX_PREPARATION_SOURCES {
            return Err(invalid(
                "This selection is too large. Prepare a smaller deck or document scope.",
            ));
        }
        Ok(seeds)
    })
}

pub(crate) fn materialize_source(
    database: &Database,
    seed: &SourceSeed,
) -> Result<Vec<SourceRecord>, CommandError> {
    materialize_sources(database, std::slice::from_ref(seed))
}

pub(crate) fn materialize_sources(
    database: &Database,
    seeds: &[SourceSeed],
) -> Result<Vec<SourceRecord>, CommandError> {
    if seeds.len() > 32 {
        return Err(invalid(
            "The semantic source publication batch is too large.",
        ));
    }
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let now = unix_time_millis()?;
        let mut published = Vec::new();
        for seed in seeds {
            let mut sources = read_seed(&transaction, seed)?;
            for source in &mut sources {
                source.refresh_identity()?;
                repository::publish_source(&transaction, source, &seed.parent_key, now)?;
            }
            repository::retire_missing_segments(
                &transaction,
                seed.kind,
                &seed.owner_id,
                &seed.parent_key,
                &sources
                    .iter()
                    .map(|source| source.id.clone())
                    .collect::<Vec<_>>(),
            )?;
            published.extend(sources);
        }
        transaction.commit().map_err(CommandError::database)?;
        Ok(published)
    })
}

pub(crate) fn read_seed(
    connection: &Connection,
    seed: &SourceSeed,
) -> Result<Vec<SourceRecord>, CommandError> {
    match seed.kind {
        SourceKind::Document => documents::read(connection, seed),
        SourceKind::Card => cards::read(connection, seed),
        SourceKind::Guide => guides::read(connection, seed),
        SourceKind::Conversation => read_conversation(connection, seed),
    }
}

fn read_conversation(
    connection: &Connection,
    seed: &SourceSeed,
) -> Result<Vec<SourceRecord>, CommandError> {
    let row = connection
        .query_row(
            "SELECT c.title, t.user_text, t.assistant_text, t.updated_at
             FROM tutor_turns t JOIN tutor_conversations c ON c.id = t.conversation_id
             WHERE t.id = ?1 AND c.id = ?2 AND t.state = 'complete' AND t.evidence_current = 1
               AND c.retrieval_enabled = 1 AND t.sequence > c.memory_start_sequence",
            rusqlite::params![seed.parent_key, seed.owner_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((title, user, assistant, revision)) = row else {
        return Ok(Vec::new());
    };
    let mut result = Vec::new();
    for (role, text) in [("User message", user), ("Tutor message", assistant)] {
        for (index, part) in split_text(&text, MAX_SOURCE_CHARS - 100)
            .into_iter()
            .enumerate()
        {
            let mut source = blank_record(seed, &title, &revision.to_string());
            source.segment_key = format!("{}:{role}:{index}", seed.parent_key);
            source.text =
                format!("{role} (historical conversation, not a verified source):\n{part}");
            source.origin = ContentOrigin::Conversation;
            source.conversation_id = Some(seed.owner_id.clone());
            source.turn_id = Some(seed.parent_key.clone());
            result.push(source);
        }
    }
    Ok(result)
}

pub(crate) fn blank_record(seed: &SourceSeed, title: &str, revision: &str) -> SourceRecord {
    SourceRecord {
        id: String::new(),
        kind: seed.kind,
        owner_id: seed.owner_id.clone(),
        segment_key: seed.parent_key.clone(),
        owner_revision: revision.to_string(),
        content_hash: String::new(),
        title: title.chars().take(500).collect(),
        text: String::new(),
        origin: ContentOrigin::UserAuthored,
        document_id: None,
        note_id: None,
        card_id: None,
        guide_id: None,
        conversation_id: None,
        turn_id: None,
        locator: None,
        images: Vec::new(),
    }
}

/// Preserve complete UTF-8 text, preferring a paragraph or sentence boundary.
/// Related context is expanded at retrieval time instead of being copied into
/// overlapping authoritative source records.
pub(crate) fn split_text(text: &str, maximum_chars: usize) -> Vec<String> {
    if maximum_chars == 0 || text.trim().is_empty() {
        return Vec::new();
    }
    let mut parts = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        let hard_end = remaining
            .char_indices()
            .nth(maximum_chars)
            .map_or(remaining.len(), |(offset, _)| offset);
        let prefix = &remaining[..hard_end];
        let end = if hard_end == remaining.len() {
            hard_end
        } else {
            prefix
                .rfind("\n\n")
                .map(|offset| offset + 2)
                .or_else(|| prefix.rfind(". ").map(|offset| offset + 2))
                .filter(|offset| *offset >= hard_end / 2)
                .unwrap_or(hard_end)
        };
        parts.push(remaining[..end].to_string());
        remaining = &remaining[end..];
    }
    parts
}

pub(crate) fn segment_record(mut source: SourceRecord) -> Vec<SourceRecord> {
    let text_parts = split_text(&source.text, MAX_SOURCE_CHARS);
    let text_parts = if text_parts.is_empty() && !source.images.is_empty() {
        vec![String::new()]
    } else {
        text_parts
    };
    let images = std::mem::take(&mut source.images);
    let image_groups = if images.is_empty() {
        vec![Vec::new()]
    } else {
        images.chunks(6).map(<[_]>::to_vec).collect()
    };
    let mut result = Vec::new();
    for (text_index, text) in text_parts.into_iter().enumerate() {
        for (image_index, image_group) in image_groups.iter().enumerate() {
            let mut part = source.clone();
            part.segment_key = format!("{}:{text_index}:{image_index}", source.segment_key);
            part.text = text.clone();
            part.images = image_group.clone();
            result.push(part);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitting_preserves_unicode_and_complete_text() {
        let text = "αβγδεζηθ. Paragraph two.\n\nAnother paragraph with 日本語.";
        let parts = split_text(text, 12);
        assert_eq!(parts.concat(), text);
        assert!(parts.iter().all(|part| part.chars().count() <= 12));
        assert!(split_text("   ", 10).is_empty());
    }

    #[test]
    fn splitting_never_discards_a_long_unbroken_token() {
        let text = "A".repeat(25_000);
        let parts = split_text(&text, MAX_SOURCE_CHARS);
        assert_eq!(parts.len(), 4);
        assert_eq!(parts.concat(), text);
    }
}
