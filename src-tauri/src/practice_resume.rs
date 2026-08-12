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

#[tauri::command]
pub fn resume_practice_session(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ResumePracticeSessionInput,
) -> Result<ResumedPracticeSessionDto, CommandError> {
    validate_entity_id(&input.session_id, "practice session")?;
    database.with_connection(|connection| {
        let metadata: Option<(i64, i64, String, i64, i64, i64, Option<i64>)> = connection
            .query_row(
                "SELECT card_count, shuffle, mode, started_at,
                        rated_count, correct_count, completed_at
                 FROM practice_sessions WHERE id = ?1",
                [input.session_id.as_str()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some((card_count, shuffle, mode, started_at, rated_count, correct_count, completed_at)) =
            metadata
        else {
            return Err(CommandError::new(
                "practice_session_not_found",
                "That practice session no longer exists.",
            ));
        };

        let pending: Option<(i64, i64)> = connection
            .query_row(
                "SELECT position, CASE WHEN revealed_at IS NULL THEN 0 ELSE 1 END
                 FROM practice_session_cards
                 WHERE session_id = ?1
                   AND rated_at IS NULL
                   AND dismissed_at IS NULL
                 ORDER BY position ASC
                 LIMIT 1",
                [input.session_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(CommandError::database)?;

        let completed = completed_at.is_some() || pending.is_none();
        let (resume_position, revealed) = pending
            .map(|(position, revealed)| (position, revealed != 0))
            .unwrap_or((card_count, false));
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
                card_count,
                first_window,
                shuffle: shuffle != 0,
                mode,
                started_at,
            },
            resume_position,
            revealed,
            rated_count,
            correct_count,
            completed,
        })
    })
}
