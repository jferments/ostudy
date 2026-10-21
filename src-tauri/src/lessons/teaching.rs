//! A teacher's lecture for a lesson: what the teacher says over each slide,
//! and the questions they stop to ask the class.
//!
//! The lecture's slides come from the saved lesson revision (`lecture.rs`).
//! A script is written once by the AI from that revision, validated here
//! against the same slides and kept beside it. The teacher's reply to every
//! option of a question is written with the script, so answering is graded
//! here, needs no provider and costs nothing. The reader never receives a
//! question's correct option or its replies until it is answered.

use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::State;

use super::{
    lecture::{read_lecture, LectureDto, LectureSlideKind},
    narration::{speak_saved, LessonStepNarrationDto},
    require_lesson, revision_content, GetLessonInput,
};
use crate::{
    ai::{
        document_intelligence::{
            generate_lecture_script, load_document_task_context, LectureScriptAuthoring,
            LECTURE_SCRIPT_PROMPT_VERSION,
        },
        document_provider, generation, AiRuntimeState,
    },
    database::Database,
    error::CommandError,
    paths::AppPaths,
    task_cancellation::CancellationSignal,
    util::{unix_time_millis, validate_entity_id},
};

pub(crate) const LECTURE_SCRIPT_VERSION: u32 = 1;
pub(crate) const MAX_TALK_CHARS: usize = 3_000;
pub(crate) const MAX_QUESTION_CHARS: usize = 500;
pub(crate) const MAX_OPTION_CHARS: usize = 300;
pub(crate) const MAX_REPLY_CHARS: usize = 1_200;
pub(crate) const MIN_OPTIONS: usize = 2;
pub(crate) const MAX_OPTIONS: usize = 4;
pub(crate) const MAX_CLASS_QUESTIONS: usize = 4;
/// Matches the table's own check.
const MAX_SCRIPT_BYTES: usize = 262_144;
const PURPOSE: &str = "lecture_authoring";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LectureScript {
    pub schema_version: u32,
    /// One per lecture slide, in the lecture's order.
    pub slides: Vec<ScriptSlide>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ScriptSlide {
    pub slide_key: String,
    /// What the teacher says over the slide, in plain spoken sentences.
    pub talk: String,
    /// Asked after the talk, only on a step slide.
    pub question: Option<ClassQuestion>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ClassQuestion {
    pub prompt: String,
    pub options: Vec<ClassOption>,
    pub correct_option_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ClassOption {
    pub id: String,
    pub text: String,
    /// What the teacher says when a learner picks this option.
    pub reply: String,
}

/// Checks a script against the slides of the lecture it was written for.
pub(crate) fn validate(script: &LectureScript, lecture: &LectureDto) -> Result<(), CommandError> {
    if script.schema_version != LECTURE_SCRIPT_VERSION {
        return Err(CommandError::new(
            "lecture_script_version_unsupported",
            "This lecture was written by a newer version of ostudy. Update ostudy to play it.",
        ));
    }
    if script.slides.len() != lecture.slides.len() {
        return Err(invalid(format!(
            "The lecture has {} slides but the script talks over {}.",
            lecture.slides.len(),
            script.slides.len()
        )));
    }
    let mut questions = 0;
    for (slide, shown) in script.slides.iter().zip(&lecture.slides) {
        if slide.slide_key != shown.key {
            return Err(invalid(format!(
                "The script's slide {} should be {}.",
                slide.slide_key, shown.key
            )));
        }
        text("A slide's talk", &slide.talk, MAX_TALK_CHARS)?;
        let Some(question) = &slide.question else {
            continue;
        };
        if shown.kind != LectureSlideKind::Step {
            return Err(invalid("Only a step slide can ask the class a question."));
        }
        questions += 1;
        text("A question", &question.prompt, MAX_QUESTION_CHARS)?;
        if !(MIN_OPTIONS..=MAX_OPTIONS).contains(&question.options.len()) {
            return Err(invalid(format!(
                "A question has {MIN_OPTIONS} to {MAX_OPTIONS} options."
            )));
        }
        let mut ids = BTreeSet::new();
        for option in &question.options {
            validate_entity_id(&option.id, "lecture question option")?;
            if !ids.insert(option.id.as_str()) {
                return Err(invalid("Two options of a question have the same identity."));
            }
            text("An option", &option.text, MAX_OPTION_CHARS)?;
            text("A teacher's reply", &option.reply, MAX_REPLY_CHARS)?;
        }
        if !ids.contains(question.correct_option_id.as_str()) {
            return Err(invalid(
                "A question's correct option is not one of its options.",
            ));
        }
    }
    if questions > MAX_CLASS_QUESTIONS {
        return Err(invalid(format!(
            "A lecture asks the class at most {MAX_CLASS_QUESTIONS} questions."
        )));
    }
    if canonical_json(script)?.len() > MAX_SCRIPT_BYTES {
        return Err(invalid("The lecture script is too long to keep."));
    }
    Ok(())
}

/// Option identities depend only on the slide and position, so the same
/// answer always decodes to the same script.
pub(crate) fn option_id(slide_key: &str, index: usize) -> String {
    let digest = Sha256::digest(format!("lecture-question|{slide_key}|{index}").as_bytes());
    format!("{digest:x}")[..32].to_string()
}

fn canonical_json(script: &LectureScript) -> Result<String, CommandError> {
    serde_json::to_string(script).map_err(|error| {
        CommandError::new(
            "lecture_script_encode_failed",
            format!("The lecture script could not be encoded: {error}"),
        )
    })
}

fn text(label: &str, value: &str, maximum: usize) -> Result<(), CommandError> {
    if value.trim().is_empty() {
        return Err(invalid(format!("{label} is empty.")));
    }
    if value.chars().count() > maximum {
        return Err(invalid(format!(
            "{label} is longer than {maximum} characters."
        )));
    }
    if value.chars().any(|c| c.is_control() && c != '\n') {
        return Err(invalid(format!("{label} contains control characters.")));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::new("lecture_script_invalid", message)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LectureScriptInput {
    pub lesson_id: String,
    pub revision_number: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteLectureScriptInput {
    pub lesson_id: String,
    pub revision_number: i64,
    pub operation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnswerLectureQuestionInput {
    pub lesson_id: String,
    pub revision_number: i64,
    pub slide_key: String,
    pub option_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NarrateLecturePartInput {
    pub lesson_id: String,
    pub revision_number: i64,
    pub slide_key: String,
    /// Speak the teacher's reply to this option instead of the slide's talk.
    pub reply_to: Option<String>,
}

/// The script as a learner sees it: no correct option and no replies.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LectureScriptDto {
    pub revision_number: i64,
    pub slides: Vec<ScriptSlideDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptSlideDto {
    pub slide_key: String,
    pub talk: String,
    pub question: Option<ClassQuestionDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassQuestionDto {
    pub prompt: String,
    pub options: Vec<ClassOptionDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassOptionDto {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LectureAnswerDto {
    pub correct: bool,
    pub correct_option_id: String,
    /// What the teacher says back.
    pub reply: String,
}

impl From<(i64, LectureScript)> for LectureScriptDto {
    fn from((revision_number, script): (i64, LectureScript)) -> Self {
        Self {
            revision_number,
            slides: script
                .slides
                .into_iter()
                .map(|slide| ScriptSlideDto {
                    slide_key: slide.slide_key,
                    talk: slide.talk,
                    question: slide.question.map(|question| ClassQuestionDto {
                        prompt: question.prompt,
                        options: question
                            .options
                            .into_iter()
                            .map(|option| ClassOptionDto {
                                id: option.id,
                                text: option.text,
                            })
                            .collect(),
                    }),
                })
                .collect(),
        }
    }
}

/// The kept script for one revision, if it has one.
#[tauri::command]
pub fn get_lesson_lecture_script(
    database: State<'_, Database>,
    input: LectureScriptInput,
) -> Result<Option<LectureScriptDto>, CommandError> {
    validate_entity_id(&input.lesson_id, "lesson")?;
    database.with_connection(|connection| {
        Ok(
            kept_script(connection, &input.lesson_id, input.revision_number)?
                .map(|script| LectureScriptDto::from((input.revision_number, script))),
        )
    })
}

/// Grades one answer to a question the teacher asks and returns the
/// teacher's reply. Nothing is saved: a lecture question is practice, never a
/// review.
#[tauri::command]
pub fn answer_lecture_question(
    database: State<'_, Database>,
    input: AnswerLectureQuestionInput,
) -> Result<LectureAnswerDto, CommandError> {
    database.with_connection(|connection| answer(connection, &input))
}

pub(crate) fn answer(
    connection: &Connection,
    input: &AnswerLectureQuestionInput,
) -> Result<LectureAnswerDto, CommandError> {
    validate_entity_id(&input.lesson_id, "lesson")?;
    let question = kept_script(connection, &input.lesson_id, input.revision_number)?
        .ok_or_else(script_missing)?
        .slides
        .into_iter()
        .find(|slide| slide.slide_key == input.slide_key)
        .and_then(|slide| slide.question)
        .ok_or_else(|| {
            CommandError::new(
                "lecture_question_missing",
                "That slide does not ask a question.",
            )
        })?;
    let option = question
        .options
        .iter()
        .find(|option| option.id == input.option_id)
        .ok_or_else(|| {
            CommandError::new(
                "lecture_option_missing",
                "That answer is not one of the question's options.",
            )
        })?;
    Ok(LectureAnswerDto {
        correct: option.id == question.correct_option_id,
        reply: option.reply.clone(),
        correct_option_id: question.correct_option_id,
    })
}

/// The text the teacher speaks for one part of the lecture: a slide's talk,
/// or the reply to one option of its question.
pub(crate) fn spoken_text(
    connection: &Connection,
    lesson_id: &str,
    revision_number: i64,
    slide_key: &str,
    reply_to: Option<&str>,
) -> Result<String, CommandError> {
    let slide = kept_script(connection, lesson_id, revision_number)?
        .ok_or_else(script_missing)?
        .slides
        .into_iter()
        .find(|slide| slide.slide_key == slide_key)
        .ok_or_else(|| {
            CommandError::new(
                "lecture_slide_missing",
                "That slide is not part of this lecture.",
            )
        })?;
    match reply_to {
        None => Ok(slide.talk),
        Some(option_id) => slide
            .question
            .and_then(|question| {
                question
                    .options
                    .into_iter()
                    .find(|option| option.id == option_id)
            })
            .map(|option| option.reply)
            .ok_or_else(|| {
                CommandError::new(
                    "lecture_option_missing",
                    "That answer is not one of the question's options.",
                )
            }),
    }
}

/// Speaks one part of the kept lecture with the lesson voice: a slide's talk,
/// or the teacher's reply to one answer. Like a step's narration, only the
/// saved script is ever spoken, and kept audio is reused.
pub(crate) async fn narrate_lecture_part(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    input: NarrateLecturePartInput,
) -> Result<LessonStepNarrationDto, CommandError> {
    validate_entity_id(&input.lesson_id, "lesson")?;
    let text = database.with_connection(|connection| {
        require_lesson(connection, &input.lesson_id)?;
        spoken_text(
            connection,
            &input.lesson_id,
            input.revision_number,
            &input.slide_key,
            input.reply_to.as_deref(),
        )
    })?;
    let key = format!(
        "lecture:{}:{}:{}:{}",
        input.lesson_id,
        input.revision_number,
        input.slide_key,
        input.reply_to.as_deref().unwrap_or("talk")
    );
    speak_saved(database, paths, runtime, &text, key).await
}

fn script_missing() -> CommandError {
    CommandError::new(
        "lecture_script_missing",
        "This lesson has no teacher's lecture yet.",
    )
}

/// The kept script, checked again against the revision's slides, so a
/// damaged or out-of-date row never plays.
pub(crate) fn kept_script(
    connection: &Connection,
    lesson_id: &str,
    revision_number: i64,
) -> Result<Option<LectureScript>, CommandError> {
    let row: Option<String> = connection
        .query_row(
            "SELECT script_json FROM lecture_scripts
             WHERE lesson_id = ?1 AND revision_number = ?2",
            params![lesson_id, revision_number],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some(json) = row else {
        return Ok(None);
    };
    let script: LectureScript = serde_json::from_str(&json)
        .map_err(|_| invalid("The kept lecture script is damaged. Write the lecture again."))?;
    let lecture = read_lecture(
        connection,
        GetLessonInput {
            lesson_id: lesson_id.to_string(),
            revision_number: Some(revision_number),
        },
    )?;
    validate(&script, &lecture)?;
    Ok(Some(script))
}

/// Keeps a validated script for its revision, replacing an earlier one.
pub(crate) fn keep_script(
    connection: &Connection,
    lesson_id: &str,
    revision_number: i64,
    script: &LectureScript,
    provenance: &Value,
    now: i64,
) -> Result<(), CommandError> {
    let lecture = read_lecture(
        connection,
        GetLessonInput {
            lesson_id: lesson_id.to_string(),
            revision_number: Some(revision_number),
        },
    )?;
    validate(script, &lecture)?;
    connection
        .execute(
            "INSERT INTO lecture_scripts(
                 lesson_id, revision_number, script_json, provenance_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(lesson_id, revision_number) DO UPDATE SET
                 script_json = excluded.script_json,
                 provenance_json = excluded.provenance_json,
                 created_at = excluded.created_at",
            params![
                lesson_id,
                revision_number,
                canonical_json(script)?,
                provenance.to_string(),
                now
            ],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

/// Writes the teacher's lecture for one revision with the AI connection used
/// for cards and guides, and keeps it. Writing again replaces it.
pub(crate) async fn write_lesson_lecture_script(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: WriteLectureScriptInput,
) -> Result<LectureScriptDto, CommandError> {
    write(database.inner(), runtime.inner(), input).await
}

pub(crate) async fn write(
    database: &Database,
    runtime: &AiRuntimeState,
    input: WriteLectureScriptInput,
) -> Result<LectureScriptDto, CommandError> {
    validate_entity_id(&input.lesson_id, "lesson")?;
    validate_entity_id(&input.operation_id, "lecture writing operation")?;
    let (lesson, lecture) = database.with_connection(|connection| {
        require_lesson(connection, &input.lesson_id)?;
        let lesson = revision_content(connection, &input.lesson_id, input.revision_number)?;
        let lecture = read_lecture(
            connection,
            GetLessonInput {
                lesson_id: input.lesson_id.clone(),
                revision_number: Some(input.revision_number),
            },
        )?;
        Ok((lesson.content, lecture))
    })?;
    let mut diagnostics =
        generation::Operation::begin(database, Some(&input.operation_id), None, None, PURPOSE)?;
    let result = author(database, runtime, &lesson, &lecture, diagnostics.context()).await;
    diagnostics.finish_result(&result);
    let (script, provenance) = result?;
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        keep_script(
            connection,
            &input.lesson_id,
            input.revision_number,
            &script,
            &provenance,
            now,
        )
    })?;
    Ok(LectureScriptDto::from((input.revision_number, script)))
}

async fn author(
    database: &Database,
    runtime: &AiRuntimeState,
    lesson: &super::content::LessonContent,
    lecture: &LectureDto,
    diagnostics: generation::Context,
) -> Result<(LectureScript, Value), CommandError> {
    let mut task = load_document_task_context(database)?;
    let capabilities = document_provider::load_document_provider_capabilities(
        database,
        &task.profile_id,
        &task.provider_kind,
        &task.model_id,
    )?;
    task.apply_provider_capabilities(&capabilities);
    task.diagnostics = Some(diagnostics);
    task.diagnostic_window_key = Some(format!(
        "{:x}",
        Sha256::digest(format!(
            "lecture|{}|{}",
            lecture.lesson_id, lecture.revision_number
        ))
    ));
    let script = generate_lecture_script(
        runtime,
        &task,
        LectureScriptAuthoring { lesson, lecture },
        &CancellationSignal::new(),
    )
    .await?;
    task.ensure_current(database)?;
    let (total_tokens, usage_complete) = task.usage_summary();
    let provenance = serde_json::json!({
        "provider": task.provider_kind,
        "model": task.model_id,
        "promptVersion": LECTURE_SCRIPT_PROMPT_VERSION,
        "scriptVersion": LECTURE_SCRIPT_VERSION,
        "totalTokens": total_tokens,
        "usageComplete": usage_complete,
    });
    Ok((script, provenance))
}
