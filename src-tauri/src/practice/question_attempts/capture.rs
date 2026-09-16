//! Session-owned capture. Shuffling is native and happens once per instance;
//! restoration reads the frozen checkpoint, never the latest editable note.

use sha2::{Digest, Sha256};

use super::*;
use crate::cards::questions::{
    DisplayOrder, InstanceIdentity, QuestionBody, QuestionInstance, QuestionKind, ValidatedQuestion,
};

#[derive(Clone)]
pub(crate) struct PreparedQuestion {
    pub attempt: QuestionAttempt,
    pub kind: QuestionKind,
    pub stem: String,
}

pub(crate) fn prepare(
    identity: InstanceIdentity,
    definition_json: &str,
    presentation_key: &str,
    shuffle_seed: &str,
) -> Result<PreparedQuestion, CommandError> {
    let question = ValidatedQuestion::parse(definition_json)?;
    let kind = question.kind();
    let stem = question.prompt_text(presentation_key)?;
    let mut order = DisplayOrder::default();
    match &question.definition().question {
        QuestionBody::SingleChoice { options, .. } | QuestionBody::SelectAll { options, .. } => {
            order.item_ids = options.iter().map(|item| item.id.clone()).collect();
        }
        QuestionBody::Matching { pairs, .. } => {
            order.item_ids = pairs.iter().map(|pair| pair.left.id.clone()).collect();
            order.target_ids = pairs.iter().map(|pair| pair.right.id.clone()).collect();
        }
        QuestionBody::Grouping {
            categories, items, ..
        } => {
            order.item_ids = items.iter().map(|item| item.item.id.clone()).collect();
            order.target_ids = categories.iter().map(|item| item.id.clone()).collect();
        }
        QuestionBody::Ordering { items, .. } => {
            order.item_ids = items.iter().map(|item| item.id.clone()).collect();
        }
        QuestionBody::TrueFalse { .. } => {}
        QuestionBody::ImageOcclusion { .. } => {
            return Err(CommandError::new(
                "question_image_not_prepared",
                "Prepare a managed masked image before studying this question.",
            ));
        }
    }
    shuffle(&mut order.item_ids, shuffle_seed, b"items");
    shuffle(&mut order.target_ids, shuffle_seed, b"targets");
    match &question.definition().question {
        QuestionBody::Ordering { items, .. }
            if order.item_ids.iter().eq(items.iter().map(|item| &item.id)) =>
        {
            order.item_ids.rotate_left(1);
        }
        QuestionBody::Matching { pairs, .. }
            if order
                .item_ids
                .iter()
                .zip(&order.target_ids)
                .all(|(left, right)| {
                    pairs
                        .iter()
                        .any(|pair| &pair.left.id == left && &pair.right.id == right)
                }) =>
        {
            order.target_ids.rotate_left(1);
        }
        _ => {}
    }
    let instance =
        QuestionInstance::capture(identity, question, presentation_key.to_string(), order)?;
    Ok(PreparedQuestion {
        attempt: QuestionAttempt::start(instance)?,
        kind,
        stem,
    })
}

fn shuffle(ids: &mut [String], seed: &str, lane: &[u8]) {
    ids.sort_by_cached_key(|id| {
        let mut hash = Sha256::new();
        hash.update(seed.as_bytes());
        hash.update([0]);
        hash.update(lane);
        hash.update([0]);
        hash.update(id.as_bytes());
        (format!("{:x}", hash.finalize()), id.clone())
    });
}

pub(crate) fn insert(
    connection: &Connection,
    session_id: &str,
    captured: &PreparedQuestion,
    now: i64,
) -> Result<(), CommandError> {
    if connection.is_autocommit() {
        return Err(CommandError::new(
            "question_transaction_required",
            "Question capture belongs to the session transaction.",
        ));
    }
    let instance = captured.attempt.instance();
    let identity = instance.identity();
    crate::assets::ensure_assets_exist(connection, instance.asset_ids())?;
    let current: bool = connection
        .query_row(
            "SELECT EXISTS(
             SELECT 1 FROM practice_session_cards p
             JOIN cards c ON c.id = p.card_id
             JOIN notes n ON n.id = c.note_id
             WHERE p.session_id = ?1 AND p.card_id = ?2 AND n.id = ?3
               AND n.revision = ?4 AND n.note_type = 'interactive'
         )",
            params![
                session_id,
                identity.card_id,
                identity.note_id,
                identity.note_revision
            ],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if !current {
        return Err(invalid_storage());
    }
    connection
        .execute(
            "INSERT INTO question_attempts (
             instance_id, session_id, card_id, note_id, question_kind, instance_hash,
             revision, phase, assisted, checkpoint_json, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 'answering', 0, ?7, ?8, ?8)",
            params![
                identity.instance_id,
                session_id,
                identity.card_id,
                identity.note_id,
                captured.kind.as_str(),
                instance.fingerprint(),
                captured.attempt.to_json()?,
                now
            ],
        )
        .map_err(CommandError::database)?;
    for asset_id in instance.asset_ids() {
        connection
            .execute(
                "INSERT INTO question_attempt_assets (instance_id, asset_id) VALUES (?1, ?2)",
                params![identity.instance_id, asset_id],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}
