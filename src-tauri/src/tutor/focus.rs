//! Resolve UI identities into saved native material. Practice uses its immutable
//! presentation snapshot, including the actual rotating-cloze question. Reveal
//! state is read from SQLite, never trusted from a webview flag.

use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::{
    database::Database,
    error::CommandError,
    knowledge::{
        adapters::{materialize_source, scope_documents, SourceSeed},
        contracts::{hash, invalid, ScopeKind, SourceKind, SourceRecord, StudyScope},
    },
};

use super::contracts::{TutorFocus, TutorMode};
use crate::practice::question_attempts::tutoring::{self, HintTarget};

#[derive(Default)]
pub(crate) struct FocusedContent {
    pub sources: Vec<SourceRecord>,
    pub question: Option<String>,
    pub private_answer: Option<String>,
    pub hidden: bool,
    pub identity: String,
    pub hint_level: usize,
    pub question_hint: Option<HintTarget>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FocusDescription<'a> {
    pub question: &'a Option<String>,
    pub answer: Option<&'a str>,
    pub answer_hidden: bool,
    pub hint_level: usize,
}

impl FocusedContent {
    pub(crate) fn description(&self) -> FocusDescription<'_> {
        FocusDescription {
            question: &self.question,
            answer: if self.hidden {
                None
            } else {
                self.private_answer.as_deref()
            },
            answer_hidden: self.hidden,
            hint_level: self.hint_level,
        }
    }
}

pub(crate) fn resolve(
    database: &Database,
    scope: &StudyScope,
    focus: Option<&TutorFocus>,
    mode: TutorMode,
    conversation_id: &str,
) -> Result<FocusedContent, CommandError> {
    let Some(focus) = focus else {
        return Ok(FocusedContent::default());
    };
    focus.validate()?;
    match focus {
        TutorFocus::Card {
            session_id,
            card_id,
            note_revision,
        } => {
            let captured = database.with_connection(|connection| {
                let deck_id: String = connection
                    .query_row(
                        "SELECT n.deck_id FROM cards c JOIN notes n ON n.id = c.note_id
                         JOIN decks d ON d.id = n.deck_id
                         WHERE c.id = ?1 AND n.revision = ?2 AND c.deleted_at IS NULL
                           AND n.deleted_at IS NULL AND d.deleted_at IS NULL",
                        params![card_id, note_revision],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(CommandError::database)?
                    .ok_or_else(changed)?;
                let mut allowed = false;
                for member in scope.material_scopes() {
                    allowed |= match member.kind {
                        ScopeKind::General | ScopeKind::Selection => false,
                        ScopeKind::Library => true,
                        ScopeKind::Deck => member.id.as_deref() == Some(deck_id.as_str()),
                        ScopeKind::Document | ScopeKind::Guide => connection.query_row(
                            "SELECT EXISTS(SELECT 1 FROM document_decks WHERE deck_id=?1 AND document_id=?2)
                             OR EXISTS(SELECT 1 FROM document_note_topics nt JOIN cards c ON c.note_id=nt.note_id
                                WHERE c.id=?3 AND nt.document_id=?2)",
                            params![deck_id,member.id,card_id], |row|row.get::<_,bool>(0),
                        ).map_err(CommandError::database)?,
                    };
                }
                if !allowed {
                    return Err(invalid("This card does not belong to the conversation's material selection."));
                }
                let Some(session_id) = session_id else {
                    if mode == TutorMode::Hint {
                        return Err(invalid("Open the current Practice question before asking for a hint."));
                    }
                    return Ok((None, None, false, 0, None));
                };
                let snapshot = connection
                    .query_row(
                        "SELECT snap.front_text, snap.back_text, p.revealed_at IS NULL
                         FROM practice_session_card_snapshots snap
                         JOIN practice_session_cards p ON p.session_id = snap.session_id AND p.card_id = snap.card_id
                         WHERE snap.session_id = ?1 AND snap.card_id = ?2 AND snap.note_revision = ?3
                           AND p.dismissed_at IS NULL",
                        params![session_id, card_id, note_revision],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, bool>(2)?)),
                    )
                    .optional()
                    .map_err(CommandError::database)?
                    .ok_or_else(changed)?;
                let typed = tutoring::focus(connection, session_id, card_id, *note_revision)?;
                let (question, answer, hidden, question_hint) = match typed {
                    Some(projection) => {
                        if projection.hidden != snapshot.2 {
                            return Err(changed());
                        }
                        let target = projection.hidden.then_some(projection.target);
                        (
                            projection.prompt,
                            projection.answer,
                            projection.hidden,
                            target,
                        )
                    }
                    None => (snapshot.0, Some(snapshot.1), snapshot.2, None),
                };
                if hidden && mode != TutorMode::Hint {
                    return Err(CommandError::new(
                        "tutor_answer_not_revealed",
                        "Use a hint while the answer is hidden. Reveal the card in Practice before asking for an explanation or a quiz.",
                    ));
                }
                if mode == TutorMode::Hint && !hidden {
                    return Err(invalid("This card's answer is already revealed. Ask for an explanation instead."));
                }
                let prior: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM tutor_turns WHERE conversation_id = ?1 AND mode = 'hint'
                         AND state = 'complete' AND json_extract(request_json, '$.focus.cardId') = ?2
                         AND json_extract(request_json, '$.focus.sessionId') = ?3",
                        params![conversation_id, card_id, session_id],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                Ok((
                    Some(question),
                    answer,
                    hidden,
                    (prior as usize + 1).min(3),
                    question_hint,
                ))
            })?;
            let (question, answer, hidden, hint_level, question_hint) = captured;
            let sources = if question_hint.is_some() {
                Vec::new()
            } else {
                materialize_source(
                    database,
                    &SourceSeed {
                        kind: SourceKind::Card,
                        owner_id: card_id.clone(),
                        parent_key: "presentation".to_string(),
                    },
                )?
            };
            let identity = hash(
                &serde_json::to_string(&(
                    focus,
                    &question,
                    &answer,
                    hidden,
                    question_hint.as_ref().map(|hint| &hint.target.instance_id),
                ))
                .map_err(CommandError::serialization)?,
            );
            Ok(FocusedContent {
                sources,
                question,
                private_answer: answer,
                hidden,
                identity,
                hint_level,
                question_hint,
            })
        }
        TutorFocus::Guide {
            document_id,
            guide_id,
            topic_id,
            content_hash,
        } => {
            database.with_connection(|connection| {
                if !scope_documents(connection, scope)?.contains(document_id) {
                    return Err(invalid(
                        "This guide does not belong to the conversation's material selection.",
                    ));
                }
                let current: bool = connection
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM document_study_guides g
                     WHERE g.id = ?1 AND g.document_id = ?2 AND g.state = 'active'
                       AND (?3 IS NULL OR EXISTS(SELECT 1 FROM document_study_guide_sections s
                           WHERE s.guide_id = g.id AND s.topic_id = ?3 AND s.content_hash = ?4)))",
                        params![guide_id, document_id, topic_id, content_hash],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                if !current {
                    return Err(changed());
                }
                Ok(())
            })?;
            let sources = materialize_source(
                database,
                &SourceSeed {
                    kind: SourceKind::Guide,
                    owner_id: guide_id.clone(),
                    parent_key: topic_id.clone().unwrap_or_else(|| "overview".to_string()),
                },
            )?;
            let identity = hash(
                &serde_json::to_string(
                    &sources
                        .iter()
                        .map(|source| (&source.id, &source.content_hash))
                        .collect::<Vec<_>>(),
                )
                .map_err(CommandError::serialization)?,
            );
            Ok(FocusedContent {
                sources,
                identity,
                ..FocusedContent::default()
            })
        }
    }
}

pub(crate) fn changed() -> CommandError {
    CommandError::new(
        "tutor_focus_changed",
        "The focused study material changed or is no longer available. Reopen the current card or guide section before retrying.",
    )
}
