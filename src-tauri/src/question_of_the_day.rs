//! Home's question of the day: one flashcard a study day, chosen at random but
//! weighted toward cards the learner has struggled with or not tried yet.
//!
//! The pick is read-only and deterministic. Each studyable recall card draws a
//! key from a hash of the study date and its id, scaled by its weight
//! (weighted sampling without replacement, keeping the largest key), so the
//! question stays the same all day and changes when the study day turns over
//! at the learner's "New day starts at" hour. Showing it is not a review and
//! writes nothing.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    assets::get_assets_for_note,
    authoring::{
        projected_presentation_variant, render_projected_presentation, ProjectedPresentationInput,
    },
    cards::CardPresentationDto,
    database::Database,
    error::CommandError,
    paths::AppPaths,
    study_calendar::StudyTimeContext,
    studyability,
};

/// A new card's weight against an ordinary review's 1.
const NEW_WEIGHT: f64 = 3.0;
/// A struggled card's weight before its lapses add to it.
const STRUGGLED_WEIGHT: f64 = 3.0;
/// The most lapses that add weight, so one leech does not crowd out the rest.
const MAX_LAPSE_WEIGHT: i64 = 3;
/// FSRS difficulty runs from 1 to 10; at this or above a card is hard for the learner.
const HARD_DIFFICULTY: f64 = 7.0;
/// The relearning state: the learner forgot the card at its last review.
const RELEARNING: i64 = 3;

/// Why the card was likely to come up.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionReason {
    /// Never studied.
    New,
    /// Forgotten before, being relearned, or rated hard by the scheduler.
    Struggled,
    Review,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionOfTheDayDto {
    /// The study date the question belongs to, as YYYY-MM-DD.
    pub study_date: String,
    pub card: CardPresentationDto,
    pub deck_name: String,
    /// The card's subject; `None` is No subject.
    pub subject_id: Option<String>,
    pub reason: QuestionReason,
}

#[tauri::command]
pub fn get_question_of_the_day(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
) -> Result<Option<QuestionOfTheDayDto>, CommandError> {
    database.with_connection(|connection| {
        let time = StudyTimeContext::capture(connection)?;
        load_question_of_the_day(connection, paths.inner(), &time)
    })
}

#[derive(Clone, Debug, PartialEq)]
struct Candidate {
    card_id: String,
    state: i64,
    reps: i64,
    lapses: i64,
    difficulty: f64,
}

impl Candidate {
    fn reason(&self) -> QuestionReason {
        if self.state == 0 || self.reps == 0 {
            QuestionReason::New
        } else if self.lapses > 0 || self.state == RELEARNING || self.difficulty >= HARD_DIFFICULTY
        {
            QuestionReason::Struggled
        } else {
            QuestionReason::Review
        }
    }

    fn weight(&self) -> f64 {
        match self.reason() {
            QuestionReason::New => NEW_WEIGHT,
            QuestionReason::Struggled => {
                STRUGGLED_WEIGHT + self.lapses.clamp(0, MAX_LAPSE_WEIGHT) as f64
            }
            QuestionReason::Review => 1.0,
        }
    }
}

pub(crate) fn load_question_of_the_day(
    connection: &Connection,
    paths: &AppPaths,
    time: &StudyTimeContext,
) -> Result<Option<QuestionOfTheDayDto>, CommandError> {
    let study_date = time.local_date.to_string();
    let candidates = studyable_recall_cards(connection, time)?;
    let Some(chosen) = choose(&study_date, &candidates) else {
        return Ok(None);
    };
    let reason = chosen.reason();
    let row = connection
        .query_row(
            "SELECT c.note_id, c.deck_id, c.direction, c.template_ordinal, n.note_type,
                    n.front_doc, n.back_doc, n.front_text, n.back_text, n.cloze_source,
                    n.cloze_mode, n.revision, n.created_at, n.updated_at, s.reps, d.name,
                    d.subject_id
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             WHERE c.id = ?1",
            params![chosen.card_id],
            |row| {
                Ok(ChosenRow {
                    note_id: row.get(0)?,
                    deck_id: row.get(1)?,
                    direction: row.get(2)?,
                    template_ordinal: row.get(3)?,
                    note_type: row.get(4)?,
                    front_doc: row.get(5)?,
                    back_doc: row.get(6)?,
                    front_text: row.get(7)?,
                    back_text: row.get(8)?,
                    cloze_source: row.get(9)?,
                    cloze_mode: row.get(10)?,
                    revision: row.get(11)?,
                    created_at: row.get(12)?,
                    updated_at: row.get(13)?,
                    reps: row.get(14)?,
                    deck_name: row.get(15)?,
                    subject_id: row.get(16)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let invalid = || {
        CommandError::new(
            "invalid_generated_card",
            "The question of the day has no question or answer to show.",
        )
    };
    let variant = projected_presentation_variant(
        &row.note_type,
        &row.cloze_source,
        &row.cloze_mode,
        row.reps,
    )?;
    let rendered = render_projected_presentation(ProjectedPresentationInput {
        direction: &row.direction,
        template_ordinal: row.template_ordinal,
        note_type: &row.note_type,
        front_doc_json: row.front_doc.as_deref().ok_or_else(invalid)?,
        back_doc_json: row.back_doc.as_deref().ok_or_else(invalid)?,
        front_text: &row.front_text,
        back_text: &row.back_text,
        cloze_source: &row.cloze_source,
        cloze_mode: &row.cloze_mode,
        reps: row.reps,
        presentation_variant: (!variant.is_empty()).then_some(variant.as_str()),
    })?;
    let assets = get_assets_for_note(connection, paths, &row.note_id)?;
    Ok(Some(QuestionOfTheDayDto {
        study_date,
        card: CardPresentationDto {
            id: chosen.card_id.clone(),
            note_id: row.note_id,
            deck_id: row.deck_id,
            direction: row.direction,
            front_doc: rendered.front_doc,
            back_doc: rendered.back_doc,
            front_text: rendered.front_text,
            back_text: rendered.back_text,
            assets,
            created_at: row.created_at,
            updated_at: row.updated_at,
            revision: row.revision,
        },
        deck_name: row.deck_name,
        subject_id: row.subject_id,
        reason,
    }))
}

struct ChosenRow {
    note_id: String,
    deck_id: String,
    direction: String,
    template_ordinal: i64,
    note_type: String,
    front_doc: Option<String>,
    back_doc: Option<String>,
    front_text: String,
    back_text: String,
    cloze_source: String,
    cloze_mode: String,
    revision: i64,
    created_at: i64,
    updated_at: i64,
    reps: i64,
    deck_name: String,
    subject_id: Option<String>,
}

/// Every card a session could study now, due or not, except interactive
/// questions, whose answers are checked in a session and never shown here.
fn studyable_recall_cards(
    connection: &Connection,
    time: &StudyTimeContext,
) -> Result<Vec<Candidate>, CommandError> {
    let sql = format!(
        "SELECT c.id, s.state, s.reps, s.lapses, s.difficulty
         FROM cards c
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         WHERE {studyable} AND n.note_type <> 'interactive'",
        studyable = studyability::studyable_card_sql("?1"),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params![time.now_utc_millis], |row| {
            Ok(Candidate {
                card_id: row.get(0)?,
                state: row.get(1)?,
                reps: row.get(2)?,
                lapses: row.get(3)?,
                difficulty: row.get(4)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>();
    rows.map_err(CommandError::database)
}

/// The day's card: the largest `u^(1/weight)`, with `u` in (0, 1] drawn from
/// the study date and the card's id. Ties fall to the smaller id.
fn choose<'a>(study_date: &str, candidates: &'a [Candidate]) -> Option<&'a Candidate> {
    candidates
        .iter()
        .map(|candidate| {
            (
                draw(study_date, &candidate.card_id).ln() / candidate.weight(),
                candidate,
            )
        })
        .max_by(|(left, left_card), (right, right_card)| {
            left.total_cmp(right)
                .then_with(|| right_card.card_id.cmp(&left_card.card_id))
        })
        .map(|(_, candidate)| candidate)
}

fn draw(study_date: &str, card_id: &str) -> f64 {
    let digest = Sha256::new()
        .chain_update(study_date.as_bytes())
        .chain_update([0])
        .chain_update(card_id.as_bytes())
        .finalize();
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    // The top 53 bits as a float in (0, 1].
    ((u64::from_be_bytes(bytes) >> 11) as f64 + 1.0) / (1_u64 << 53) as f64
}

#[cfg(test)]
#[path = "question_of_the_day_tests.rs"]
mod tests;
