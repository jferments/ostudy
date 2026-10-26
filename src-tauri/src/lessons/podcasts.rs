//! Podcasts: two hosts talking through some of a subject's lessons.
//!
//! A podcast's script is written once by the AI from the saved revisions of
//! the lessons it covers, validated here and kept (release schema 17). Each
//! lesson is a chapter, and each turn is one host speaking. The two hosts
//! keep the voices chosen when the podcast was made, so every turn sounds
//! like the same two people. Like lesson narration, only the kept script is
//! ever spoken, and a turn's audio is kept and reused.

use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::State;

use super::{
    content::LessonContent,
    narration::{resolved, speak_text, LessonStepNarrationDto, Resolved},
    require_subject, revision_content, voices,
};
use crate::{
    ai::{
        document_intelligence::{
            generate_podcast_script, load_verified_document_task_context, PodcastLesson,
            PodcastScriptAuthoring, PODCAST_SCRIPT_PROMPT_VERSION,
        },
        document_provider, generation,
        speech::{self, SpeechModel},
        AiRuntimeState,
    },
    database::Database,
    error::CommandError,
    paths::AppPaths,
    task_cancellation::CancellationSignal,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

pub(crate) const PODCAST_SCRIPT_VERSION: u32 = 1;
/// The hosts' names, in the order of their voices.
pub(crate) const HOSTS: [&str; 2] = ["Ada", "Ben"];
pub(crate) const MIN_MINUTES: u32 = 5;
pub(crate) const MAX_MINUTES: u32 = 30;
pub(crate) const MAX_LESSONS: usize = 12;
pub(crate) const MAX_TITLE_CHARS: usize = 120;
pub(crate) const MAX_TURN_CHARS: usize = 1_000;
pub(crate) const MIN_CHAPTER_TURNS: usize = 2;
pub(crate) const MAX_TURNS: usize = 500;
/// Speaking speed used for a podcast's length.
pub(crate) const WORDS_PER_MINUTE: usize = 150;
/// Matches the table's own check.
const MAX_SCRIPT_BYTES: usize = 524_288;
const PURPOSE: &str = "podcast_authoring";
/// The voices that come with ostudy each host speaks in by default.
const DEFAULT_CLONED_HOSTS: [&str; 2] = ["calm-older-woman", "relaxed-man"];
/// Preset voices each host speaks in by default.
const DEFAULT_PRESET_HOSTS: [&str; 2] = ["af_heart", "am_michael"];

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PodcastScript {
    pub schema_version: u32,
    pub title: String,
    /// One per lesson, in the order the learner picked them.
    pub chapters: Vec<PodcastChapter>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PodcastChapter {
    pub lesson_id: String,
    pub revision_number: i64,
    /// The lesson's title when the podcast was made.
    pub title: String,
    pub turns: Vec<PodcastTurn>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PodcastTurn {
    /// 0 or 1: which of `HOSTS` speaks.
    pub host: usize,
    /// Plain spoken sentences.
    pub text: String,
}

/// The most characters a script of this length may hold: half as long again
/// as asked, at about six characters a word.
pub(crate) fn max_characters(target_minutes: u32) -> usize {
    target_minutes as usize * WORDS_PER_MINUTE * 9
}

/// Checks a script on its own: its chapters, hosts and every turn's text.
pub(crate) fn validate(script: &PodcastScript, target_minutes: u32) -> Result<(), CommandError> {
    if script.schema_version != PODCAST_SCRIPT_VERSION {
        return Err(CommandError::new(
            "podcast_script_version_unsupported",
            "This podcast was written by a newer version of ostudy. Update ostudy to play it.",
        ));
    }
    text("The podcast's title", &script.title, MAX_TITLE_CHARS)?;
    if script.chapters.is_empty() || script.chapters.len() > MAX_LESSONS {
        return Err(invalid(format!(
            "A podcast covers 1 to {MAX_LESSONS} lessons."
        )));
    }
    let mut lessons = BTreeSet::new();
    let mut turns = 0;
    let mut characters = 0;
    for chapter in &script.chapters {
        validate_entity_id(&chapter.lesson_id, "lesson")?;
        if !lessons.insert(chapter.lesson_id.as_str()) {
            return Err(invalid("A podcast covers each lesson once."));
        }
        if chapter.revision_number < 1 {
            return Err(invalid(
                "A chapter names a lesson version that does not exist.",
            ));
        }
        text(
            "A chapter's title",
            &chapter.title,
            super::content::MAX_TITLE_CHARS,
        )?;
        if chapter.turns.len() < MIN_CHAPTER_TURNS {
            return Err(invalid(format!(
                "Each chapter has at least {MIN_CHAPTER_TURNS} turns."
            )));
        }
        for turn in &chapter.turns {
            if turn.host >= HOSTS.len() {
                return Err(invalid("A turn is spoken by someone who is not a host."));
            }
            text("A turn", &turn.text, MAX_TURN_CHARS)?;
            if turn.text.contains(['[', ']']) {
                return Err(invalid("A turn contains square brackets."));
            }
            characters += turn.text.chars().count();
        }
        turns += chapter.turns.len();
    }
    if turns > MAX_TURNS {
        return Err(invalid(format!("A podcast has at most {MAX_TURNS} turns.")));
    }
    if characters > max_characters(target_minutes) {
        return Err(invalid(format!(
            "The podcast is much longer than {target_minutes} minutes."
        )));
    }
    if canonical_json(script)?.len() > MAX_SCRIPT_BYTES {
        return Err(invalid("The podcast script is too long to keep."));
    }
    Ok(())
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
    if value.chars().any(char::is_control) {
        return Err(invalid(format!("{label} contains control characters.")));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::new("podcast_script_invalid", message)
}

fn canonical_json(script: &PodcastScript) -> Result<String, CommandError> {
    serde_json::to_string(script).map_err(|error| {
        CommandError::new(
            "podcast_script_encode_failed",
            format!("The podcast script could not be encoded: {error}"),
        )
    })
}

/// A turn's key: its chapter and its place in the chapter.
fn turn_key(chapter: usize, turn: usize) -> String {
    format!("{chapter}.{turn}")
}

fn spoken_words(script: &PodcastScript) -> usize {
    script
        .chapters
        .iter()
        .flat_map(|chapter| &chapter.turns)
        .map(|turn| turn.text.split_whitespace().count())
        .sum()
}

fn minutes(script: &PodcastScript) -> u32 {
    u32::try_from(spoken_words(script).div_ceil(WORDS_PER_MINUTE).max(1)).unwrap_or(u32::MAX)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListSubjectPodcastsInput {
    pub subject_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PodcastInput {
    pub podcast_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WritePodcastInput {
    pub subject_id: String,
    /// The lessons to talk through, in order; each is read at its newest
    /// saved revision.
    pub lesson_ids: Vec<String>,
    pub target_minutes: u32,
    /// Each host's voice for the speech model in use; the default pair when
    /// left out.
    #[serde(default)]
    pub voices: Option<[String; 2]>,
    pub operation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NarratePodcastTurnInput {
    pub podcast_id: String,
    pub turn_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastSummaryDto {
    pub id: String,
    pub subject_id: String,
    pub title: String,
    pub target_minutes: u32,
    /// How long the script takes to speak.
    pub minutes: u32,
    pub lesson_titles: Vec<String>,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastDto {
    pub id: String,
    pub subject_id: String,
    pub title: String,
    pub target_minutes: u32,
    pub minutes: u32,
    pub hosts: Vec<PodcastHostDto>,
    pub chapters: Vec<PodcastChapterDto>,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastHostDto {
    pub name: String,
    /// The speech setting's voice the host speaks in.
    pub voice: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastChapterDto {
    pub lesson_id: String,
    pub title: String,
    pub turns: Vec<PodcastTurnDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastTurnDto {
    pub key: String,
    pub host: usize,
    pub text: String,
}

/// A kept podcast row.
struct Kept {
    id: String,
    subject_id: String,
    target_minutes: u32,
    model_id: String,
    voices: [String; 2],
    script: PodcastScript,
    created_at: i64,
}

impl From<Kept> for PodcastDto {
    fn from(kept: Kept) -> Self {
        let minutes = minutes(&kept.script);
        let [first, second] = kept.voices;
        Self {
            id: kept.id,
            subject_id: kept.subject_id,
            title: kept.script.title,
            target_minutes: kept.target_minutes,
            minutes,
            hosts: vec![
                PodcastHostDto {
                    name: HOSTS[0].to_string(),
                    voice: first,
                },
                PodcastHostDto {
                    name: HOSTS[1].to_string(),
                    voice: second,
                },
            ],
            chapters: kept
                .script
                .chapters
                .into_iter()
                .enumerate()
                .map(|(chapter_index, chapter)| PodcastChapterDto {
                    lesson_id: chapter.lesson_id,
                    title: chapter.title,
                    turns: chapter
                        .turns
                        .into_iter()
                        .enumerate()
                        .map(|(turn_index, turn)| PodcastTurnDto {
                            key: turn_key(chapter_index, turn_index),
                            host: turn.host,
                            text: turn.text,
                        })
                        .collect(),
                })
                .collect(),
            created_at: kept.created_at,
        }
    }
}

const KEPT_COLUMNS: &str = "id, subject_id, target_minutes, model_id, first_voice, second_voice,
     script_json, created_at";

fn kept_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(Kept, String)> {
    Ok((
        Kept {
            id: row.get(0)?,
            subject_id: row.get(1)?,
            target_minutes: row.get(2)?,
            model_id: row.get(3)?,
            voices: [row.get(4)?, row.get(5)?],
            script: PodcastScript {
                schema_version: 0,
                title: String::new(),
                chapters: Vec::new(),
            },
            created_at: row.get(7)?,
        },
        row.get(6)?,
    ))
}

/// The kept script, checked again, so a damaged row never plays.
fn decoded((mut kept, json): (Kept, String)) -> Result<Kept, CommandError> {
    kept.script = serde_json::from_str(&json)
        .map_err(|_| invalid("The kept podcast is damaged. Make the podcast again."))?;
    validate(&kept.script, kept.target_minutes)?;
    Ok(kept)
}

fn kept(connection: &Connection, podcast_id: &str) -> Result<Kept, CommandError> {
    validate_entity_id(podcast_id, "podcast")?;
    let row = connection
        .query_row(
            &format!("SELECT {KEPT_COLUMNS} FROM podcasts WHERE id = ?1"),
            [podcast_id],
            kept_row,
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "podcast_not_found",
                "That podcast is no longer in the library.",
            )
        })?;
    decoded(row)
}

#[tauri::command]
pub fn list_subject_podcasts(
    database: State<'_, Database>,
    input: ListSubjectPodcastsInput,
) -> Result<Vec<PodcastSummaryDto>, CommandError> {
    database.with_connection(|connection| list(connection, &input.subject_id))
}

/// A subject's podcasts, newest first. A damaged podcast is left out rather
/// than hiding the others.
pub(crate) fn list(
    connection: &Connection,
    subject_id: &str,
) -> Result<Vec<PodcastSummaryDto>, CommandError> {
    validate_entity_id(subject_id, "subject")?;
    require_subject(connection, subject_id)?;
    let mut statement = connection
        .prepare(&format!(
            "SELECT {KEPT_COLUMNS} FROM podcasts WHERE subject_id = ?1
             ORDER BY created_at DESC, id"
        ))
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([subject_id], kept_row)
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows
        .into_iter()
        .filter_map(|row| decoded(row).ok())
        .map(|kept| PodcastSummaryDto {
            minutes: minutes(&kept.script),
            lesson_titles: kept
                .script
                .chapters
                .iter()
                .map(|chapter| chapter.title.clone())
                .collect(),
            title: kept.script.title,
            id: kept.id,
            subject_id: kept.subject_id,
            target_minutes: kept.target_minutes,
            created_at: kept.created_at,
        })
        .collect())
}

#[tauri::command]
pub fn get_podcast(
    database: State<'_, Database>,
    input: PodcastInput,
) -> Result<PodcastDto, CommandError> {
    database.with_connection(|connection| Ok(kept(connection, &input.podcast_id)?.into()))
}

/// Deletes a podcast. Its kept audio is left unreferenced for asset cleanup
/// unless lesson narration shares it.
#[tauri::command]
pub fn delete_podcast(
    database: State<'_, Database>,
    input: PodcastInput,
) -> Result<(), CommandError> {
    database.with_connection(|connection| delete(connection, &input.podcast_id))
}

pub(crate) fn delete(connection: &Connection, podcast_id: &str) -> Result<(), CommandError> {
    validate_entity_id(podcast_id, "podcast")?;
    connection
        .execute("DELETE FROM podcasts WHERE id = ?1", [podcast_id])
        .map_err(CommandError::database)?;
    Ok(())
}

/// The default voices of the two hosts for `model`.
pub(crate) fn default_voices(model: &SpeechModel) -> [String; 2] {
    if model.clones_voices {
        DEFAULT_CLONED_HOSTS.map(voices::built_in_key)
    } else {
        DEFAULT_PRESET_HOSTS.map(str::to_string)
    }
}

/// Whether a host can be given `voice` with `model`: one of the learner's
/// voices for a model that copies voices, else one of the model's presets.
fn host_voice_allowed(
    connection: &Connection,
    model: &SpeechModel,
    voice: &str,
) -> Result<bool, CommandError> {
    if model.clones_voices {
        voices::is_own_voice(connection, model, voice)
    } else {
        Ok(model.voices.contains(&voice))
    }
}

/// The model and voice a host of a kept podcast speaks in. A host whose voice
/// can no longer speak takes their default voice, so the podcast still plays.
fn host_voice(
    database: &Database,
    settings: &Resolved,
    model_id: &str,
    voice: &str,
    host: usize,
) -> Result<(&'static SpeechModel, String), CommandError> {
    let model = speech::speech_model(model_id).unwrap_or(settings.model);
    database.with_connection(|connection| {
        if model.id == model_id && voices::speaks(connection, voice)? {
            return Ok((model, voice.to_string()));
        }
        let fallback = default_voices(model)[host].clone();
        if voices::speaks(connection, &fallback)? {
            Ok((model, fallback))
        } else {
            Err(CommandError::new(
                "speech_voice_missing",
                "The recording of this host's voice is missing.",
            ))
        }
    })
}

/// Speaks one turn of a kept podcast in its host's voice and keeps the audio.
pub(crate) async fn narrate_turn(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    input: NarratePodcastTurnInput,
) -> Result<LessonStepNarrationDto, CommandError> {
    let podcast = database.with_connection(|connection| kept(connection, &input.podcast_id))?;
    let turn = input
        .turn_key
        .split_once('.')
        .and_then(|(chapter, turn)| {
            Some((chapter.parse::<usize>().ok()?, turn.parse::<usize>().ok()?))
        })
        .and_then(|(chapter, turn)| podcast.script.chapters.get(chapter)?.turns.get(turn))
        .ok_or_else(|| {
            CommandError::new("podcast_turn_missing", "That part is not in this podcast.")
        })?;
    let voice = &podcast.voices[turn.host];
    speak_text(
        database,
        paths,
        runtime,
        &turn.text,
        format!("podcast:{}:{}", podcast.id, input.turn_key),
        |settings| host_voice(database, settings, &podcast.model_id, voice, turn.host),
    )
    .await
}

/// What a podcast is written from: each picked lesson's newest saved
/// revision, in the learner's order.
fn picked_lessons(
    connection: &Connection,
    subject_id: &str,
    lesson_ids: &[String],
) -> Result<Vec<PodcastLesson>, CommandError> {
    if lesson_ids.is_empty() || lesson_ids.len() > MAX_LESSONS {
        return Err(CommandError::new(
            "podcast_lessons_invalid",
            format!("Pick 1 to {MAX_LESSONS} lessons."),
        ));
    }
    let mut seen = BTreeSet::new();
    lesson_ids
        .iter()
        .map(|lesson_id| {
            validate_entity_id(lesson_id, "lesson")?;
            if !seen.insert(lesson_id.as_str()) {
                return Err(CommandError::new(
                    "podcast_lessons_invalid",
                    "Pick each lesson once.",
                ));
            }
            let revision_number: Option<i64> = connection
                .query_row(
                    "SELECT MAX(r.revision_number) FROM lessons l
                     JOIN lesson_revisions r ON r.lesson_id = l.id
                     WHERE l.id = ?1 AND l.subject_id = ?2",
                    params![lesson_id, subject_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            let revision_number = revision_number.ok_or_else(|| {
                CommandError::new(
                    "podcast_lessons_invalid",
                    "A picked lesson is not a written lesson of this subject.",
                )
            })?;
            let content: LessonContent =
                revision_content(connection, lesson_id, revision_number)?.content;
            Ok(PodcastLesson {
                lesson_id: lesson_id.clone(),
                revision_number,
                content,
            })
        })
        .collect()
}

/// Writes a podcast from some of a subject's lessons with the AI connection
/// used for cards and guides, and keeps it with its hosts' voices.
pub(crate) async fn write(
    database: &Database,
    runtime: &AiRuntimeState,
    input: WritePodcastInput,
) -> Result<PodcastDto, CommandError> {
    validate_entity_id(&input.subject_id, "subject")?;
    validate_entity_id(&input.operation_id, "podcast writing operation")?;
    if !(MIN_MINUTES..=MAX_MINUTES).contains(&input.target_minutes) {
        return Err(CommandError::new(
            "podcast_length_invalid",
            format!("A podcast is {MIN_MINUTES} to {MAX_MINUTES} minutes long."),
        ));
    }
    let (lessons, model, voices) = database.with_connection(|connection| {
        require_subject(connection, &input.subject_id)?;
        let lessons = picked_lessons(connection, &input.subject_id, &input.lesson_ids)?;
        let model = resolved(connection)?.model;
        let voices = input
            .voices
            .clone()
            .unwrap_or_else(|| default_voices(model));
        for voice in &voices {
            if !host_voice_allowed(connection, model, voice)? {
                return Err(CommandError::new(
                    "podcast_voice_invalid",
                    format!("{voice} is not a voice {} can speak in.", model.label),
                ));
            }
        }
        Ok((lessons, model, voices))
    })?;
    let mut diagnostics =
        generation::Operation::begin(database, Some(&input.operation_id), None, None, PURPOSE)?;
    let result = author(
        database,
        runtime,
        &lessons,
        input.target_minutes,
        diagnostics.context(),
    )
    .await;
    diagnostics.finish_result(&result);
    let (script, provenance) = result?;
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let id = keep(
            connection,
            &input.subject_id,
            input.target_minutes,
            model,
            &voices,
            &script,
            &provenance,
            now,
        )?;
        Ok(kept(connection, &id)?.into())
    })
}

/// Keeps a validated script as a new podcast of the subject.
#[allow(clippy::too_many_arguments)]
pub(crate) fn keep(
    connection: &Connection,
    subject_id: &str,
    target_minutes: u32,
    model: &SpeechModel,
    voices: &[String; 2],
    script: &PodcastScript,
    provenance: &Value,
    now: i64,
) -> Result<String, CommandError> {
    validate(script, target_minutes)?;
    let id = generate_id(connection)?;
    connection
        .execute(
            "INSERT INTO podcasts(
                 id, subject_id, title, target_minutes, model_id, first_voice, second_voice,
                 script_json, provenance_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                id,
                subject_id,
                script.title,
                target_minutes,
                model.id,
                voices[0],
                voices[1],
                canonical_json(script)?,
                provenance.to_string(),
                now
            ],
        )
        .map_err(CommandError::database)?;
    Ok(id)
}

async fn author(
    database: &Database,
    runtime: &AiRuntimeState,
    lessons: &[PodcastLesson],
    target_minutes: u32,
    diagnostics: generation::Context,
) -> Result<(PodcastScript, Value), CommandError> {
    let mut task = load_verified_document_task_context(database, runtime).await?;
    let capabilities = document_provider::load_document_provider_capabilities(
        database,
        &task.profile_id,
        &task.provider_kind,
        &task.model_id,
    )?;
    task.apply_provider_capabilities(&capabilities);
    task.diagnostics = Some(diagnostics);
    let sources = lessons
        .iter()
        .map(|lesson| format!("{}:{}", lesson.lesson_id, lesson.revision_number))
        .collect::<Vec<_>>()
        .join("|");
    task.diagnostic_window_key = Some(format!(
        "{:x}",
        Sha256::digest(format!("podcast|{sources}|{target_minutes}"))
    ));
    let script = generate_podcast_script(
        runtime,
        &task,
        PodcastScriptAuthoring {
            lessons,
            target_minutes,
        },
        &CancellationSignal::new(),
    )
    .await?;
    task.ensure_current(database)?;
    let (total_tokens, usage_complete) = task.usage_summary();
    let provenance = serde_json::json!({
        "provider": task.provider_kind,
        "model": task.model_id,
        "promptVersion": PODCAST_SCRIPT_PROMPT_VERSION,
        "scriptVersion": PODCAST_SCRIPT_VERSION,
        "totalTokens": total_tokens,
        "usageComplete": usage_complete,
    });
    Ok((script, provenance))
}

#[cfg(test)]
#[path = "podcasts_tests.rs"]
mod tests;
