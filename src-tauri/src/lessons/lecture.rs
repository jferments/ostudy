//! A lesson laid out as a lecture: slides with the narration as their notes.
//!
//! Slides are compiled from a saved lesson revision on every read, never
//! stored and never written by a model: a title slide with the objectives, one
//! slide per step with its slide points, and a sources slide naming the
//! documents the lesson was written from. A step's notes are its narration,
//! so playing a slide speaks exactly what the lesson's Listen speaks. The
//! lesson reader's view is the input, so a check's answer never reaches a
//! slide.

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use tauri::State;

use super::{read_lesson, GetLessonInput, LessonDto};
use crate::{database::Database, error::CommandError};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LectureDto {
    pub lesson_id: String,
    /// The revision the slides were compiled from.
    pub revision_number: i64,
    pub title: String,
    pub slides: Vec<LectureSlideDto>,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LectureSlideDto {
    /// Names the slide within its lesson: `title`, `step:<step key>` or
    /// `sources`. A teacher's lecture script is keyed by it.
    pub key: String,
    pub kind: LectureSlideKind,
    /// The step a step slide shows, whose narration can be spoken.
    pub step_key: Option<String>,
    pub title: String,
    pub points: Vec<String>,
    /// The speaker notes: a step's narration, or the lesson's summary.
    pub notes: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LectureSlideKind {
    Title,
    Step,
    Sources,
}

pub(crate) const TITLE_KEY: &str = "title";
pub(crate) const SOURCES_KEY: &str = "sources";

/// A step's slide key. The prefix keeps a step key from naming another slide.
pub(crate) fn step_slide_key(step_key: &str) -> String {
    format!("step:{step_key}")
}

#[tauri::command]
pub fn get_lesson_lecture(
    database: State<'_, Database>,
    input: GetLessonInput,
) -> Result<LectureDto, CommandError> {
    database.with_connection(|connection| read_lecture(connection, input))
}

pub(crate) fn read_lecture(
    connection: &Connection,
    input: GetLessonInput,
) -> Result<LectureDto, CommandError> {
    let lesson = read_lesson(connection, input)?;
    let documents = source_documents(connection, &lesson)?;
    Ok(compile(lesson, documents))
}

fn compile(lesson: LessonDto, documents: Vec<String>) -> LectureDto {
    let content = lesson.content;
    let mut slides = Vec::with_capacity(content.steps.len() + 2);
    slides.push(LectureSlideDto {
        key: TITLE_KEY.to_string(),
        kind: LectureSlideKind::Title,
        step_key: None,
        title: content.title.clone(),
        points: content.objectives,
        notes: content.summary,
    });
    slides.extend(content.steps.into_iter().map(|step| LectureSlideDto {
        key: step_slide_key(&step.key),
        kind: LectureSlideKind::Step,
        step_key: Some(step.key),
        title: step.title,
        points: step.slide_points,
        notes: step.narration,
    }));
    if !documents.is_empty() {
        slides.push(LectureSlideDto {
            key: SOURCES_KEY.to_string(),
            kind: LectureSlideKind::Sources,
            step_key: None,
            title: "Sources".to_string(),
            points: documents,
            notes: String::new(),
        });
    }
    LectureDto {
        lesson_id: lesson.lesson.id,
        revision_number: lesson.revision_number,
        title: content.title,
        slides,
    }
}

/// The titles of the documents the steps' knowledge units come from, in the
/// order the lesson first uses them. Units no longer extracted are skipped.
fn source_documents(
    connection: &Connection,
    lesson: &LessonDto,
) -> Result<Vec<String>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT document.title
             FROM knowledge_units unit
             JOIN knowledge_sources source ON source.id = unit.source_id
             JOIN documents document ON document.id = source.document_id
             WHERE unit.id = ?1",
        )
        .map_err(CommandError::database)?;
    let mut titles: Vec<String> = Vec::new();
    for unit_id in lesson
        .content
        .steps
        .iter()
        .flat_map(|step| &step.knowledge_unit_ids)
    {
        let title: Option<String> = statement
            .query_row([unit_id], |row| row.get(0))
            .optional()
            .map_err(CommandError::database)?;
        if let Some(title) = title {
            if !titles.contains(&title) {
                titles.push(title);
            }
        }
    }
    Ok(titles)
}
