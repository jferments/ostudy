//! Spoken narration for lesson steps, made once and kept.
//!
//! A voice only ever reads narration the course writer already finished and
//! the library saved: the text comes from the stored lesson revision, never
//! from the page, so nothing half-written reaches a speech engine. Each step's
//! narration is cut into sentence groups. A group's audio is keyed by its
//! exact text, engine, model, voice and format, so listening again costs
//! nothing, changing the voice makes only what that voice is missing, and a
//! rewritten step re-voices only the sentences that changed.

use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    sync::{LazyLock, Mutex},
};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use super::{require_lesson, revision_content};
use crate::{
    ai::{
        generation,
        profiles::{load_profile_request, ProfileRequestSnapshot},
        speech::{self, SpeechModel, SPEECH_FORMAT, SPEECH_MODELS},
        AiRuntimeState,
    },
    database::Database,
    error::CommandError,
    media::{inspect_media_bytes, MediaKind},
    paths::AppPaths,
    task_cancellation::CancellationSignal,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const PURPOSE: &str = "lesson_narration";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechSettingsDto {
    /// The OpenRouter connection that pays for speech; null when none exists.
    pub profile_id: Option<String>,
    pub model_id: String,
    pub voice: String,
    /// 0 until the setting is first saved.
    pub revision: i64,
    pub models: Vec<SpeechModelDto>,
    pub connections: Vec<SpeechConnectionDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechModelDto {
    pub id: String,
    pub label: String,
    pub voices: Vec<String>,
    pub default_voice: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechConnectionDto {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveSpeechSettingsInput {
    pub profile_id: Option<String>,
    pub model_id: String,
    pub voice: String,
    pub expected_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NarrateLessonStepInput {
    pub lesson_id: String,
    pub revision_number: i64,
    pub step_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonStepNarrationDto {
    /// In speaking order.
    pub segments: Vec<NarrationSegmentDto>,
    /// How many segments this call had to make.
    pub made: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NarrationSegmentDto {
    pub asset_id: String,
    pub characters: i64,
}

#[tauri::command]
pub fn get_speech_settings(
    database: State<'_, Database>,
) -> Result<SpeechSettingsDto, CommandError> {
    database.with_connection(|connection| speech_settings(connection))
}

#[tauri::command]
pub fn save_speech_settings(
    database: State<'_, Database>,
    input: SaveSpeechSettingsInput,
) -> Result<SpeechSettingsDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| save_settings(connection, input, now))
}

pub(crate) fn speech_settings(connection: &Connection) -> Result<SpeechSettingsDto, CommandError> {
    let current = resolved(connection)?;
    Ok(SpeechSettingsDto {
        profile_id: current.profile_id,
        model_id: current.model.id.to_string(),
        voice: current.voice,
        revision: current.revision,
        models: SPEECH_MODELS
            .iter()
            .map(|model| SpeechModelDto {
                id: model.id.to_string(),
                label: model.label.to_string(),
                voices: model.voices.iter().map(|voice| voice.to_string()).collect(),
                default_voice: model.default_voice.to_string(),
            })
            .collect(),
        connections: connections(connection)?,
    })
}

pub(crate) fn save_settings(
    connection: &mut Connection,
    input: SaveSpeechSettingsInput,
    now: i64,
) -> Result<SpeechSettingsDto, CommandError> {
    let model = speech::speech_model(&input.model_id).ok_or_else(|| {
        CommandError::new(
            "speech_model_unknown",
            "That voice model is not one ostudy can use.",
        )
    })?;
    if !(model.voices.is_empty() && input.voice.is_empty()
        || model.voices.contains(&input.voice.as_str()))
    {
        return Err(CommandError::new(
            "speech_voice_unknown",
            "That voice is not offered by this voice model.",
        ));
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    if let Some(profile_id) = &input.profile_id {
        validate_entity_id(profile_id, "AI provider profile")?;
        if !connections(&transaction)?
            .iter()
            .any(|connection| connection.id == *profile_id)
        {
            return Err(CommandError::new(
                "speech_connection_unavailable",
                "Spoken narration needs an enabled OpenRouter connection.",
            ));
        }
    }
    let stored: i64 = transaction
        .query_row(
            "SELECT COALESCE((SELECT revision FROM speech_settings WHERE id = 1), 0)",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if stored != input.expected_revision {
        return Err(CommandError::new(
            "speech_settings_changed",
            "The voice setting changed elsewhere. Review it and save again.",
        ));
    }
    transaction
        .execute(
            "INSERT INTO speech_settings(id, profile_id, model_id, voice, revision, updated_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                 profile_id = excluded.profile_id,
                 model_id = excluded.model_id,
                 voice = excluded.voice,
                 revision = excluded.revision,
                 updated_at = excluded.updated_at",
            params![input.profile_id, model.id, input.voice, stored + 1, now],
        )
        .map_err(CommandError::database)?;
    let saved = speech_settings(&transaction)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(saved)
}

struct Resolved {
    profile_id: Option<String>,
    model: &'static SpeechModel,
    voice: String,
    revision: i64,
}

/// The saved setting, or until one is saved: Kokoro's default voice through
/// the connection that writes documents if it is OpenRouter's, else the most
/// recently changed OpenRouter connection. A saved model this build no longer
/// knows falls back the same way but keeps its revision, so it can be saved over.
fn resolved(connection: &Connection) -> Result<Resolved, CommandError> {
    let saved: Option<(Option<String>, String, String, i64)> = connection
        .query_row(
            "SELECT profile_id, model_id, voice, revision FROM speech_settings WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let usable = connections(connection)?;
    let mut stored_revision = 0;
    if let Some((profile_id, model_id, voice, revision)) = saved {
        stored_revision = revision;
        if let Some(model) = speech::speech_model(&model_id) {
            let profile_id = profile_id.filter(|id| usable.iter().any(|item| item.id == *id));
            return Ok(Resolved {
                profile_id,
                model,
                voice,
                revision,
            });
        }
    }
    let document_default = speech::document_connection(connection)?;
    let profile_id = document_default
        .filter(|id| usable.iter().any(|item| item.id == *id))
        .or_else(|| usable.first().map(|item| item.id.clone()));
    let model = speech::speech_model(speech::KOKORO).expect("Kokoro is in the speech catalog");
    Ok(Resolved {
        profile_id,
        model,
        voice: model.default_voice.to_string(),
        revision: stored_revision,
    })
}

/// Enabled OpenRouter connections, most recently changed first.
fn connections(connection: &Connection) -> Result<Vec<SpeechConnectionDto>, CommandError> {
    Ok(speech::speaking_connections(connection)?
        .into_iter()
        .map(|(id, name)| SpeechConnectionDto { id, name })
        .collect())
}

/// Steps being narrated now, so a second Listen never pays twice.
static NARRATING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

struct Narrating(String);

impl Narrating {
    fn begin(key: String) -> Result<Self, CommandError> {
        let mut active = NARRATING.lock().map_err(|_| busy())?;
        if !active.insert(key.clone()) {
            return Err(busy());
        }
        Ok(Self(key))
    }
}

impl Drop for Narrating {
    fn drop(&mut self) {
        if let Ok(mut active) = NARRATING.lock() {
            active.remove(&self.0);
        }
    }
}

fn busy() -> CommandError {
    CommandError::new(
        "narration_in_progress",
        "This step's narration is already being made.",
    )
}

pub(crate) async fn narrate_lesson_step(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    input: NarrateLessonStepInput,
) -> Result<LessonStepNarrationDto, CommandError> {
    validate_entity_id(&input.lesson_id, "lesson")?;
    let (narration, settings) = database.with_connection(|connection| {
        require_lesson(connection, &input.lesson_id)?;
        let content = revision_content(connection, &input.lesson_id, input.revision_number)?;
        let narration = content
            .content
            .steps
            .iter()
            .find(|step| step.key == input.step_key)
            .map(|step| step.narration.clone())
            .ok_or_else(|| {
                CommandError::new(
                    "lesson_step_missing",
                    "That step is not part of this lesson.",
                )
            })?;
        Ok((narration, resolved(connection)?))
    })?;
    let pieces = segments(&narration, settings.model.max_characters);
    if pieces.is_empty() {
        return Err(CommandError::new(
            "lesson_step_unnarrated",
            "This step has no narration to listen to.",
        ));
    }
    let profile_id = settings.profile_id.clone().ok_or_else(|| {
        CommandError::new(
            "speech_not_configured",
            "Spoken narration needs an OpenRouter connection.",
        )
    })?;
    let request = load_profile_request(database, &profile_id)?;
    let engine = speech::engine(request.profile.provider_kind)?;
    let narrator = Narrator {
        database,
        paths,
        runtime,
        request: &request,
        voice: Voice {
            engine,
            model: settings.model,
            voice: &settings.voice,
        },
    };

    let _narrating = Narrating::begin(format!(
        "{}:{}:{}",
        input.lesson_id, input.revision_number, input.step_key
    ))?;
    let mut found = database.with_connection(|connection| {
        pieces
            .iter()
            .map(|piece| kept_segment(connection, &narrator.voice, piece))
            .collect::<Result<Vec<_>, _>>()
    })?;
    let missing = found.iter().filter(|segment| segment.is_none()).count();
    if missing > 0 {
        let mut accounting = generation::Operation::begin(database, None, None, None, PURPOSE)?;
        let result = narrator
            .make_missing(&pieces, &mut found, accounting.context())
            .await;
        accounting.finish_result(&result);
        result?;
    }
    Ok(LessonStepNarrationDto {
        segments: found.into_iter().flatten().collect(),
        made: missing,
    })
}

struct Voice<'a> {
    engine: &'static str,
    model: &'static SpeechModel,
    voice: &'a str,
}

struct Narrator<'a> {
    database: &'a Database,
    paths: &'a AppPaths,
    runtime: &'a AiRuntimeState,
    request: &'a ProfileRequestSnapshot,
    voice: Voice<'a>,
}

impl Narrator<'_> {
    async fn make_missing(
        &self,
        pieces: &[String],
        found: &mut [Option<NarrationSegmentDto>],
        accounting: generation::Context,
    ) -> Result<(), CommandError> {
        let cancellation = CancellationSignal::new();
        // One piece at a time, in speaking order: the first piece is ready to
        // play soonest, and a failure leaves every piece before it kept.
        for (piece, slot) in pieces.iter().zip(found.iter_mut()) {
            if slot.is_some() {
                continue;
            }
            let audio = speech::speak(
                self.runtime,
                self.request,
                self.voice.model,
                self.voice.voice,
                piece,
                &accounting,
                &cancellation,
            )
            .await?;
            *slot = Some(keep_segment(
                self.database,
                self.paths,
                &self.voice,
                piece,
                &audio.bytes,
            )?);
        }
        Ok(())
    }
}

fn text_key(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// Kept audio for this piece and voice, if its asset is still readable.
fn kept_segment(
    connection: &Connection,
    voice: &Voice<'_>,
    piece: &str,
) -> Result<Option<NarrationSegmentDto>, CommandError> {
    connection
        .query_row(
            "SELECT s.asset_id, s.characters FROM speech_segments s
             JOIN assets a ON a.id = s.asset_id
             WHERE s.text_sha256 = ?1 AND s.engine = ?2 AND s.model_id = ?3
               AND s.voice = ?4 AND s.format = ?5
               AND a.lifecycle_state = 'ready' AND a.deleted_at IS NULL",
            params![
                text_key(piece),
                voice.engine,
                voice.model.id,
                voice.voice,
                SPEECH_FORMAT
            ],
            |row| {
                Ok(NarrationSegmentDto {
                    asset_id: row.get(0)?,
                    characters: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)
}

/// Writes the audio as a ready managed asset and records it as this piece's
/// narration. A segment whose asset went missing is replaced.
fn keep_segment(
    database: &Database,
    paths: &AppPaths,
    voice: &Voice<'_>,
    piece: &str,
    bytes: &[u8],
) -> Result<NarrationSegmentDto, CommandError> {
    let not_audio = || {
        CommandError::new(
            "speech_audio_invalid",
            "The voice returned something other than audio.",
        )
    };
    let inspection = inspect_media_bytes(bytes).map_err(|_| not_audio())?;
    if !matches!(inspection.kind, MediaKind::Audio) {
        return Err(not_audio());
    }
    let characters = i64::try_from(piece.chars().count()).unwrap_or(i64::MAX);
    let byte_size = i64::try_from(bytes.len()).unwrap_or(i64::MAX);
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        if let Some(kept) = kept_segment(&transaction, voice, piece)? {
            return Ok(kept);
        }
        let asset_id = generate_id(&transaction)?;
        let segment_id = generate_id(&transaction)?;
        let relative_path = AppPaths::image_relative_path(&asset_id, inspection.extension);
        let path = paths.resolve_relative(&relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(CommandError::io)?;
        }
        write_new_file(&path, bytes)?;
        let now = unix_time_millis()?;
        let stored = (|| {
            transaction
                .execute(
                    "DELETE FROM speech_segments
                     WHERE text_sha256 = ?1 AND engine = ?2 AND model_id = ?3
                       AND voice = ?4 AND format = ?5",
                    params![
                        text_key(piece),
                        voice.engine,
                        voice.model.id,
                        voice.voice,
                        SPEECH_FORMAT
                    ],
                )
                .map_err(CommandError::database)?;
            transaction
                .execute(
                    "INSERT INTO assets (
                         id, original_filename, mime_type, byte_size, relative_path,
                         created_at, lifecycle_state, ready_at, verified_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'ready', ?6, ?6, ?6)",
                    params![
                        asset_id,
                        format!("narration.{}", inspection.extension),
                        inspection.mime_type,
                        byte_size,
                        relative_path,
                        now
                    ],
                )
                .map_err(CommandError::database)?;
            transaction
                .execute(
                    "INSERT INTO asset_reconciliation_events (
                         id, asset_id, action, from_path, to_path, detail, occurred_at
                     ) VALUES (lower(hex(randomblob(16))), ?1, 'verified', NULL, ?2,
                               'lesson narration kept', ?3)",
                    params![asset_id, relative_path, now],
                )
                .map_err(CommandError::database)?;
            transaction
                .execute(
                    "INSERT INTO speech_segments(
                         id, text_sha256, engine, model_id, voice, format, asset_id,
                         characters, created_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        segment_id,
                        text_key(piece),
                        voice.engine,
                        voice.model.id,
                        voice.voice,
                        SPEECH_FORMAT,
                        asset_id,
                        characters,
                        now
                    ],
                )
                .map_err(CommandError::database)?;
            transaction.commit().map_err(CommandError::database)
        })();
        if let Err(error) = stored {
            let _ = fs::remove_file(&path);
            return Err(error);
        }
        Ok(NarrationSegmentDto {
            asset_id,
            characters,
        })
    })
}

fn write_new_file(path: &std::path::Path, bytes: &[u8]) -> Result<(), CommandError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(CommandError::io)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    if let Err(error) = written {
        let _ = fs::remove_file(path);
        return Err(CommandError::io(error));
    }
    Ok(())
}

/// Cuts narration into groups of whole sentences no longer than `maximum`
/// characters. A sentence longer than that is cut between words.
pub(crate) fn segments(narration: &str, maximum: usize) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let mut characters = narration.trim().chars().peekable();
    while let Some(character) = characters.next() {
        current.push(character);
        let ends = matches!(character, '.' | '!' | '?')
            && characters.peek().is_none_or(|next| next.is_whitespace());
        if ends {
            sentences.push(std::mem::take(&mut current));
        }
    }
    sentences.push(current);

    let mut groups: Vec<String> = Vec::new();
    let mut group = String::new();
    for sentence in sentences
        .iter()
        .flat_map(|sentence| split_long(sentence.trim(), maximum))
        .filter(|sentence| !sentence.is_empty())
    {
        let joined = if group.is_empty() {
            sentence.chars().count()
        } else {
            group.chars().count() + 1 + sentence.chars().count()
        };
        if joined > maximum && !group.is_empty() {
            groups.push(std::mem::take(&mut group));
        }
        if !group.is_empty() {
            group.push(' ');
        }
        group.push_str(&sentence);
    }
    if !group.is_empty() {
        groups.push(group);
    }
    groups
}

fn split_long(sentence: &str, maximum: usize) -> Vec<String> {
    if sentence.chars().count() <= maximum {
        return vec![sentence.to_string()];
    }
    let mut pieces = Vec::new();
    let mut piece = String::new();
    for word in sentence.split_whitespace() {
        let mut word = word.to_string();
        // A single word longer than the limit is cut where it must be.
        while word.chars().count() > maximum {
            if !piece.is_empty() {
                pieces.push(std::mem::take(&mut piece));
            }
            let cut = word
                .char_indices()
                .nth(maximum)
                .map_or(word.len(), |(index, _)| index);
            pieces.push(word[..cut].to_string());
            word = word[cut..].to_string();
        }
        let joined = piece.chars().count() + usize::from(!piece.is_empty()) + word.chars().count();
        if joined > maximum && !piece.is_empty() {
            pieces.push(std::mem::take(&mut piece));
        }
        if !piece.is_empty() {
            piece.push(' ');
        }
        piece.push_str(&word);
    }
    if !piece.is_empty() {
        pieces.push(piece);
    }
    pieces
}

#[cfg(test)]
#[path = "narration_tests.rs"]
mod tests;
