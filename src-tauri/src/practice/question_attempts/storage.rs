use super::*;

pub(super) struct StoredAttempt {
    pub attempt: QuestionAttempt,
    session_id: String,
    card_id: String,
    instance_id: String,
    instance_hash: String,
    rated_at: Option<i64>,
    dismissed_at: Option<i64>,
    completed_at: Option<i64>,
    material_unavailable: bool,
}

impl StoredAttempt {
    pub fn require_available(&self) -> Result<(), CommandError> {
        if self.material_unavailable {
            return Err(CommandError::new(
                "practice_question_unavailable",
                "This question is no longer available for practice.",
            ));
        }
        Ok(())
    }

    pub fn require_mutable(&self) -> Result<(), CommandError> {
        if self.rated_at.is_some()
            || self.dismissed_at.is_some()
            || self.completed_at.is_some()
            || self.material_unavailable
        {
            return Err(CommandError::new(
                "practice_question_unavailable",
                "This question is no longer an active, unrated part of the session.",
            ));
        }
        Ok(())
    }
}

struct Row {
    checkpoint: String,
    fingerprint: String,
    revision: i64,
    assisted: bool,
    phase: String,
    first_checked_at: Option<i64>,
    verdict: Option<String>,
    note_id: String,
    note_revision: i64,
    rated_at: Option<i64>,
    dismissed_at: Option<i64>,
    completed_at: Option<i64>,
    material_unavailable: bool,
    question_kind: String,
}

pub(super) fn load(
    connection: &Connection,
    target: &AttemptIdentityInput,
) -> Result<StoredAttempt, CommandError> {
    let row = connection
        .query_row(
            "SELECT qa.checkpoint_json, qa.instance_hash, qa.revision,
                    qa.assisted, qa.phase, qa.first_checked_at, qa.verdict,
                    qa.note_id, snap.note_revision, psc.rated_at,
                    psc.dismissed_at, ps.completed_at,
                    (c.deleted_at IS NOT NULL OR n.deleted_at IS NOT NULL
                     OR d.deleted_at IS NOT NULL OR d.archived_at IS NOT NULL
                     OR c.suspended_at IS NOT NULL OR c.archive_suspended_at IS NOT NULL
                     OR COALESCE(c.buried_until, 0) > ?4),
                    qa.question_kind
             FROM question_attempts qa
             JOIN practice_session_cards psc
               ON psc.session_id = qa.session_id AND psc.card_id = qa.card_id
             JOIN practice_sessions ps ON ps.id = qa.session_id
             JOIN practice_session_card_snapshots snap
               ON snap.question_instance_id = qa.instance_id
              AND snap.session_id = qa.session_id AND snap.card_id = qa.card_id
              AND snap.note_id = qa.note_id AND snap.card_type = qa.question_kind
             JOIN cards c ON c.id = qa.card_id AND c.note_id = qa.note_id
             JOIN notes n ON n.id = qa.note_id AND n.note_type = 'interactive'
             JOIN decks d ON d.id = c.deck_id
             WHERE qa.session_id = ?1 AND qa.card_id = ?2 AND qa.instance_id = ?3",
            params![
                target.session_id,
                target.card_id,
                target.instance_id,
                unix_time_millis()?
            ],
            |row| {
                Ok(Row {
                    checkpoint: row.get(0)?,
                    fingerprint: row.get(1)?,
                    revision: row.get(2)?,
                    assisted: row.get::<_, i64>(3)? != 0,
                    phase: row.get(4)?,
                    first_checked_at: row.get(5)?,
                    verdict: row.get(6)?,
                    note_id: row.get(7)?,
                    note_revision: row.get(8)?,
                    rated_at: row.get(9)?,
                    dismissed_at: row.get(10)?,
                    completed_at: row.get(11)?,
                    material_unavailable: row.get::<_, i64>(12)? != 0,
                    question_kind: row.get(13)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "practice_question_not_found",
                "That captured question is not part of this practice session.",
            )
        })?;
    let attempt = QuestionAttempt::restore(&row.checkpoint)?;
    let identity = attempt.instance().identity();
    let view = attempt.view()?;
    let summary = summary(&view);
    let kind = attempt.instance().kind().as_str();
    if identity.instance_id != target.instance_id
        || identity.card_id != target.card_id
        || identity.note_id != row.note_id
        || identity.note_revision != row.note_revision
        || attempt.instance().fingerprint() != row.fingerprint
        || attempt.revision() != row.revision
        || view.assisted != row.assisted
        || summary.phase != row.phase
        || summary.checked_at != row.first_checked_at
        || summary.verdict != row.verdict.as_deref()
        || kind != row.question_kind
    {
        return Err(invalid_storage());
    }
    Ok(StoredAttempt {
        attempt,
        session_id: target.session_id.clone(),
        card_id: target.card_id.clone(),
        instance_id: target.instance_id.clone(),
        instance_hash: row.fingerprint,
        rated_at: row.rated_at,
        dismissed_at: row.dismissed_at,
        completed_at: row.completed_at,
        material_unavailable: row.material_unavailable,
    })
}

struct Summary {
    phase: &'static str,
    checked_at: Option<i64>,
    verdict: Option<&'static str>,
}

fn summary(view: &AttemptView) -> Summary {
    match &view.progress {
        AttemptProgress::Answering { .. } => Summary {
            phase: "answering",
            checked_at: None,
            verdict: None,
        },
        AttemptProgress::Revealed { revealed_at_ms, .. } => Summary {
            phase: "revealed",
            checked_at: Some(*revealed_at_ms),
            verdict: None,
        },
        AttemptProgress::Checked {
            checked_at_ms,
            evaluation,
            ..
        } => Summary {
            phase: "checked",
            checked_at: Some(*checked_at_ms),
            verdict: Some(match evaluation.verdict {
                Verdict::Correct => "correct",
                Verdict::Incorrect => "incorrect",
                Verdict::Incomplete => "incomplete",
            }),
        },
    }
}

pub(super) fn persist(
    connection: &Connection,
    prior: &StoredAttempt,
    attempt: &QuestionAttempt,
    now: i64,
) -> Result<(), CommandError> {
    if connection.is_autocommit() {
        return Err(CommandError::new(
            "question_transaction_required",
            "A question change requires its owning transaction.",
        ));
    }
    if attempt.instance().fingerprint() != prior.instance_hash {
        return Err(invalid_storage());
    }
    let checkpoint = attempt.to_json()?;
    let view = attempt.view()?;
    let summary = summary(&view);
    let changed = connection
        .execute(
            "UPDATE question_attempts SET revision = ?1, phase = ?2, assisted = ?3,
                 checkpoint_json = ?4, first_checked_at = ?5, verdict = ?6,
                 updated_at = MAX(updated_at, ?7)
             WHERE instance_id = ?8 AND revision = ?9 AND instance_hash = ?10
               AND session_id = ?11 AND card_id = ?12",
            params![
                attempt.revision(),
                summary.phase,
                i64::from(view.assisted),
                checkpoint,
                summary.checked_at,
                summary.verdict,
                now,
                prior.instance_id,
                prior.attempt.revision(),
                prior.instance_hash,
                prior.session_id,
                prior.card_id
            ],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(conflict());
    }
    Ok(())
}

pub(super) fn visible_assets(
    connection: &Connection,
    instance_id: &str,
    view: &AttemptView,
) -> Result<Vec<AssetDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT a.id, a.original_filename, a.mime_type, a.byte_size, a.created_at
             FROM assets a JOIN question_attempt_assets qa ON qa.asset_id = a.id
             WHERE qa.instance_id = ?1 AND a.id = ?2
               AND a.deleted_at IS NULL AND a.lifecycle_state = 'ready'",
        )
        .map_err(CommandError::database)?;
    let mut assets = Vec::new();
    for id in view.asset_ids()? {
        let asset = statement
            .query_row(params![instance_id, id], |row| {
                Ok(AssetDto {
                    id: row.get(0)?,
                    original_filename: row.get(1)?,
                    mime_type: row.get(2)?,
                    byte_size: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "question_asset_unavailable",
                    "Media retained by this question is unavailable. The saved answer is unchanged.",
                )
            })?;
        assets.push(asset);
    }
    Ok(assets)
}
