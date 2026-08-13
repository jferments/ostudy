//! Durable practice-session restoration for mobile process death.
//!
//! The checkpoint stores only a session identity and UI hint. This module
//! re-derives the authoritative position, counts, and current card window from
//! SQLite so a stale or tampered checkpoint cannot skip or duplicate reviews.

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    paths::AppPaths,
    practice::{PracticeCardWindowDto, PracticeSessionDto},
    projections::practice::{load_practice_window, PRACTICE_WINDOW_MAX},
    util::validate_entity_id,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResumePracticeSessionInput {
    pub session_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumedPracticeSessionDto {
    pub session: PracticeSessionDto,
    pub resume_position: i64,
    pub revealed: bool,
    pub rated_count: i64,
    pub correct_count: i64,
    pub completed: bool,
}

#[derive(Debug)]
struct PracticeSessionMetadata {
    card_count: i64,
    shuffle: bool,
    mode: String,
    started_at: i64,
    rated_count: i64,
    correct_count: i64,
    completed_at: Option<i64>,
}

#[derive(Clone, Copy, Debug)]
struct PendingPracticeCard {
    position: i64,
    revealed: bool,
}

#[tauri::command]
pub fn resume_practice_session(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ResumePracticeSessionInput,
) -> Result<ResumedPracticeSessionDto, CommandError> {
    validate_entity_id(&input.session_id, "practice session")?;
    database.with_connection(|connection| {
        let metadata = connection
            .query_row(
                "SELECT card_count, shuffle, mode, started_at,
                        rated_count, correct_count, completed_at
                 FROM practice_sessions WHERE id = ?1",
                [input.session_id.as_str()],
                |row| {
                    Ok(PracticeSessionMetadata {
                        card_count: row.get(0)?,
                        shuffle: row.get::<_, i64>(1)? != 0,
                        mode: row.get(2)?,
                        started_at: row.get(3)?,
                        rated_count: row.get(4)?,
                        correct_count: row.get(5)?,
                        completed_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some(metadata) = metadata else {
            return Err(CommandError::new(
                "practice_session_not_found",
                "That practice session no longer exists.",
            ));
        };

        let pending = connection
            .query_row(
                "SELECT position, CASE WHEN revealed_at IS NULL THEN 0 ELSE 1 END
                 FROM practice_session_cards
                 WHERE session_id = ?1
                   AND rated_at IS NULL
                   AND dismissed_at IS NULL
                 ORDER BY position ASC
                 LIMIT 1",
                [input.session_id.as_str()],
                |row| {
                    Ok(PendingPracticeCard {
                        position: row.get(0)?,
                        revealed: row.get::<_, i64>(1)? != 0,
                    })
                },
            )
            .optional()
            .map_err(CommandError::database)?;

        let completed = metadata.completed_at.is_some() || pending.is_none();
        let (resume_position, revealed) = pending
            .map(|pending| (pending.position, pending.revealed))
            .unwrap_or((metadata.card_count, false));
        let first_window = if completed {
            PracticeCardWindowDto {
                start_position: resume_position,
                cards: Vec::new(),
                has_more: false,
            }
        } else {
            load_practice_window(
                connection,
                paths.inner(),
                &input.session_id,
                resume_position,
                PRACTICE_WINDOW_MAX,
            )?
        };

        Ok(ResumedPracticeSessionDto {
            session: PracticeSessionDto {
                id: input.session_id,
                card_count: metadata.card_count,
                first_window,
                shuffle: metadata.shuffle,
                mode: metadata.mode,
                started_at: metadata.started_at,
            },
            resume_position,
            revealed,
            rated_count: metadata.rated_count,
            correct_count: metadata.correct_count,
            completed,
        })
    })
}
