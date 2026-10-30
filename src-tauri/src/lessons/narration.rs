//! Spoken narration for lesson steps, made once and kept.
//!
//! A voice only ever reads narration the course writer already finished and
//! the library saved: the text comes from the stored lesson revision, never
//! from the page, so nothing half-written reaches a speech engine. Each step's
//! narration is cut into sentence groups. A group's audio is keyed by its
//! exact text, engine, model, voice and format, so listening again costs
//! nothing and a rewritten step re-voices only the sentences that changed.
//! A lesson keeps the voice it was first spoken in (see `voices`). A voice on
//! the learner's device (`device_voices`) speaks without a connection and is
//! not accounted as an AI request.

use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{LazyLock, Mutex},
};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use super::{
    require_lesson, revision_content,
    voices::{self, SpeechVoiceDto},
};
use crate::{
    ai::{
        generation,
        profiles::{load_profile_request, ProfileRequestSnapshot},
        speech::{self, SpeechModel, VoiceReference, DEVICE_ENGINE, SPEECH_MODELS},
        AiRuntimeState,
    },
    database::Database,
    device_voices,
    error::CommandError,
    media::{inspect_media_bytes, MediaInspection, MediaKind},
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
    /// Voices the learner saved, oldest first, then the voice they are trying,
    /// if any.
    pub voices: Vec<SpeechVoiceDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechModelDto {
    pub id: String,
    pub label: String,
    pub voices: Vec<String>,
    pub default_voice: String,
    /// Whether the learner's own saved voices can speak with this model.
    pub clones_voices: bool,
    /// Whether it speaks on this device, for free and with no connection,
    /// once it is downloaded.
    pub on_device: bool,
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
    let voices = voices::listed(connection)?;
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
                clones_voices: model.clones_voices,
                on_device: model.device.is_some(),
            })
            .collect(),
        connections: connections(connection)?,
        voices,
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
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let offered = if model.voices.is_empty() {
        input.voice.is_empty()
            || model.clones_voices && voices::is_own_voice(&transaction, model, &input.voice)?
    } else {
        model.voices.contains(&input.voice.as_str())
    };
    if !offered {
        return Err(CommandError::new(
            "speech_voice_unknown",
            "That voice is not offered by this voice model.",
        ));
    }
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
    let stored = stored_revision(&transaction)?;
    if stored != input.expected_revision {
        return Err(CommandError::new(
            "speech_settings_changed",
            "The voice setting changed elsewhere. Review it and save again.",
        ));
    }
    write_settings(
        &transaction,
        input.profile_id.as_deref(),
        model,
        &input.voice,
        now,
    )?;
    let saved = speech_settings(&transaction)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(saved)
}

fn stored_revision(connection: &Connection) -> Result<i64, CommandError> {
    connection
        .query_row(
            "SELECT COALESCE((SELECT revision FROM speech_settings WHERE id = 1), 0)",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

/// Writes the setting as the next revision. The caller validated every value.
pub(super) fn write_settings(
    connection: &Connection,
    profile_id: Option<&str>,
    model: &SpeechModel,
    voice: &str,
    now: i64,
) -> Result<(), CommandError> {
    let next = stored_revision(connection)? + 1;
    connection
        .execute(
            "INSERT INTO speech_settings(id, profile_id, model_id, voice, revision, updated_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                 profile_id = excluded.profile_id,
                 model_id = excluded.model_id,
                 voice = excluded.voice,
                 revision = excluded.revision,
                 updated_at = excluded.updated_at",
            params![profile_id, model.id, voice, next, now],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

pub(super) struct Resolved {
    pub profile_id: Option<String>,
    pub model: &'static SpeechModel,
    pub voice: String,
    pub revision: i64,
}

/// The saved setting, or until one is saved: Drama 3 (Joshi's pick) through
/// the connection that writes documents if it is OpenRouter's, else the most
/// recently changed OpenRouter connection. A saved model this build no longer
/// knows falls back the same way but keeps its revision, so it can be saved over.
pub(super) fn resolved(connection: &Connection) -> Result<Resolved, CommandError> {
    let saved: Option<(Option<String>, String, String, i64)> = connection
        .query_row(
            "SELECT profile_id, model_id, voice, revision FROM speech_settings WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let usable = connections(connection)?;
    let default_profile = || -> Result<Option<String>, CommandError> {
        Ok(speech::document_connection(connection)?
            .filter(|id| usable.iter().any(|item| item.id == *id))
            .or_else(|| usable.first().map(|item| item.id.clone())))
    };
    let mut stored_revision = 0;
    if let Some((profile_id, model_id, voice, revision)) = saved {
        stored_revision = revision;
        if let Some(model) = speech::speech_model(&model_id) {
            // Removing a service clears the saved voice's connection (ON
            // DELETE SET NULL), and Settings has no way to choose none, so a
            // voice without a usable connection speaks through the default
            // one rather than silently turning lesson audio off.
            let profile_id = match profile_id {
                Some(id) if usable.iter().any(|item| item.id == id) => Some(id),
                _ => default_profile()?,
            };
            return Ok(Resolved {
                profile_id,
                model,
                voice: chosen_voice(model, voice),
                revision,
            });
        }
    }
    let profile_id = default_profile()?;
    let model = speech::speech_model(speech::DRAMA).expect("Drama 3 is in the speech catalog");
    Ok(Resolved {
        profile_id,
        model,
        voice: chosen_voice(model, model.default_voice.to_string()),
        revision: stored_revision,
    })
}

/// A model that copies voices with none chosen speaks the default voice.
fn chosen_voice(model: &SpeechModel, voice: String) -> String {
    if model.clones_voices && voice.is_empty() {
        voices::default_voice()
    } else {
        voice
    }
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

pub(super) struct Narrating(String);

impl Narrating {
    pub(super) fn begin(key: String) -> Result<Self, CommandError> {
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
        "This narration is already being made.",
    )
}

pub(crate) async fn narrate_lesson_step(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    input: NarrateLessonStepInput,
) -> Result<LessonStepNarrationDto, CommandError> {
    validate_entity_id(&input.lesson_id, "lesson")?;
    let narration = database.with_connection(|connection| {
        require_lesson(connection, &input.lesson_id)?;
        let content = revision_content(connection, &input.lesson_id, input.revision_number)?;
        content
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
            })
    })?;
    speak_saved(
        database,
        paths,
        runtime,
        &input.lesson_id,
        &narration,
        format!(
            "{}:{}:{}",
            input.lesson_id, input.revision_number, input.step_key
        ),
    )
    .await
}

/// The connection that pays for speech, as the setting names it.
pub(super) fn speaking_request(
    database: &Database,
    settings: &Resolved,
) -> Result<ProfileRequestSnapshot, CommandError> {
    let profile_id = settings.profile_id.as_deref().ok_or_else(|| {
        CommandError::new(
            "speech_not_configured",
            "Spoken narration needs an OpenRouter connection.",
        )
    })?;
    load_profile_request(database, profile_id)
}

/// Speaks text the library already saved for a lesson in the lesson's voice,
/// making only the pieces that are not kept yet. `key` names what is being
/// spoken, so the same text is never made twice at once.
pub(crate) async fn speak_saved(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    lesson_id: &str,
    text: &str,
    key: String,
) -> Result<LessonStepNarrationDto, CommandError> {
    speak_text(database, paths, runtime, text, key, |settings| {
        voices::lesson_voice(database, settings, lesson_id)
    })
    .await
}

/// Speaks saved text in the model and voice `choose` picks, given the speech
/// setting, through the setting's connection. Kept pieces are reused.
pub(super) async fn speak_text(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    text: &str,
    key: String,
    choose: impl FnOnce(&Resolved) -> Result<(&'static SpeechModel, String), CommandError>,
) -> Result<LessonStepNarrationDto, CommandError> {
    let (pieces, narrator, _narrating) = prepare(
        database,
        paths,
        runtime,
        text,
        key,
        PURPOSE,
        usize::MAX,
        choose,
    )?;
    let mut found = database.with_connection(|connection| {
        pieces
            .iter()
            .map(|piece| kept_segment(connection, &narrator.voice, piece))
            .collect::<Result<Vec<_>, _>>()
    })?;
    let missing = found.iter().filter(|segment| segment.is_none()).count();
    if missing > 0 {
        narrator.make_missing(&pieces, &mut found, PURPOSE).await?;
    }
    Ok(LessonStepNarrationDto {
        segments: found.into_iter().flatten().collect(),
        made: missing,
    })
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpokenPieceDto {
    pub segment: NarrationSegmentDto,
    /// How many pieces the whole text is spoken in.
    pub pieces: usize,
    /// Whether this call had to make the piece.
    pub made: bool,
}

/// Speaks one piece of saved text, counted from 0, in the speech setting's
/// model and voice. Long text is listened to piece by piece, so the first
/// piece plays while the next is made; no piece is longer than `longest`
/// characters or than the model takes. `purpose` names the operation the
/// request is accounted under.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn speak_piece(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    text: &str,
    index: usize,
    longest: usize,
    key: String,
    purpose: &'static str,
) -> Result<SpokenPieceDto, CommandError> {
    let (pieces, narrator, _narrating) = prepare(
        database,
        paths,
        runtime,
        text,
        format!("{key}:{index}"),
        purpose,
        longest,
        |settings| Ok((settings.model, settings.voice.clone())),
    )?;
    let piece = pieces.get(index).ok_or_else(|| {
        CommandError::new("speech_piece_missing", "There is no more to listen to.")
    })?;
    let kept =
        database.with_connection(|connection| kept_segment(connection, &narrator.voice, piece))?;
    if let Some(segment) = kept {
        return Ok(SpokenPieceDto {
            segment,
            pieces: pieces.len(),
            made: false,
        });
    }
    let mut found = [None];
    narrator
        .make_missing(std::slice::from_ref(piece), &mut found, purpose)
        .await?;
    let [Some(segment)] = found else {
        return Err(CommandError::new(
            "speech_audio_invalid",
            "The voice returned something other than audio.",
        ));
    };
    Ok(SpokenPieceDto {
        segment,
        pieces: pieces.len(),
        made: true,
    })
}

/// The pieces `text` is spoken in, none longer than `longest` characters, and
/// who speaks them, once the setting has a connection and nothing else is
/// speaking `key`.
#[allow(clippy::too_many_arguments)]
fn prepare<'a>(
    database: &'a Database,
    paths: &'a AppPaths,
    runtime: &'a AiRuntimeState,
    text: &str,
    key: String,
    stage: &'static str,
    longest: usize,
    choose: impl FnOnce(&Resolved) -> Result<(&'static SpeechModel, String), CommandError>,
) -> Result<(Vec<String>, Narrator<'a>, Narrating), CommandError> {
    // A voice reads HCO₃⁻ as letters and marks; it hears "bicarbonate".
    let text = &super::spoken::spoken(text);
    let settings = database.with_connection(|connection| resolved(connection))?;
    if segments(text, settings.model.max_characters).is_empty() {
        return Err(CommandError::new(
            "lesson_step_unnarrated",
            "There is nothing here to listen to.",
        ));
    }
    let narrating = Narrating::begin(key)?;
    let (model, voice) = choose(&settings)?;
    let (request, engine) = match model.device {
        Some(device) => {
            if !device_voices::SUPPORTED {
                return Err(device_voices::unsupported());
            }
            if !device_voices::is_installed(paths, device) {
                return Err(device_voices::not_installed(device));
            }
            (None, DEVICE_ENGINE)
        }
        None => {
            let request = speaking_request(database, &settings)?;
            let engine = speech::engine(request.profile.provider_kind)?;
            (Some(request), engine)
        }
    };
    let pieces = segments(text, longest.min(model.max_characters));
    let narrator = Narrator {
        database,
        paths,
        runtime,
        request,
        stage,
        voice: Voice {
            engine,
            model,
            reference: voices::reference(database, paths, &voice)?,
            voice,
        },
    };
    Ok((pieces, narrator, narrating))
}

/// What kept audio is keyed by, and what a request needs to speak it.
struct Voice {
    engine: &'static str,
    model: &'static SpeechModel,
    /// A preset voice, a saved voice's key, or empty for the model's choice.
    voice: String,
    reference: Option<VoiceReference>,
}

struct Narrator<'a> {
    database: &'a Database,
    paths: &'a AppPaths,
    runtime: &'a AiRuntimeState,
    /// The connection that pays; none for a voice on the device.
    request: Option<ProfileRequestSnapshot>,
    stage: &'static str,
    voice: Voice,
}

impl Narrator<'_> {
    /// Makes the pieces not kept yet. A provider's requests are accounted
    /// under one operation of `purpose`; a voice on the device costs nothing
    /// and is not.
    async fn make_missing(
        &self,
        pieces: &[String],
        found: &mut [Option<NarrationSegmentDto>],
        purpose: &'static str,
    ) -> Result<(), CommandError> {
        let Some(request) = &self.request else {
            return self.make_missing_with(pieces, found, None).await;
        };
        let mut accounting =
            generation::Operation::begin(self.database, None, None, None, purpose)?;
        let result = self
            .make_missing_with(pieces, found, Some((request, accounting.context())))
            .await;
        accounting.finish_result(&result);
        result
    }

    async fn make_missing_with(
        &self,
        pieces: &[String],
        found: &mut [Option<NarrationSegmentDto>],
        provider: Option<(&ProfileRequestSnapshot, generation::Context)>,
    ) -> Result<(), CommandError> {
        let cancellation = CancellationSignal::new();
        // One piece at a time, in speaking order: the first piece is ready to
        // play soonest, and a failure leaves every piece before it kept.
        for (piece, slot) in pieces.iter().zip(found.iter_mut()) {
            if slot.is_some() {
                continue;
            }
            let bytes = match (&provider, self.voice.model.device) {
                (_, Some(device)) => {
                    device_voices::speak(self.paths, device, &self.voice.voice, piece).await?
                }
                (Some((request, accounting)), None) => {
                    speech::speak(
                        self.runtime,
                        request,
                        &speech::Speaker {
                            model: self.voice.model,
                            voice: &self.voice.voice,
                            reference: self.voice.reference.as_ref(),
                        },
                        self.stage,
                        piece,
                        accounting,
                        &cancellation,
                    )
                    .await?
                    .bytes
                }
                (None, None) => {
                    return Err(CommandError::new(
                        "speech_not_configured",
                        "Spoken narration needs an OpenRouter connection.",
                    ))
                }
            };
            *slot = Some(keep_segment(
                self.database,
                self.paths,
                &self.voice,
                piece,
                &bytes,
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
    voice: &Voice,
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
                voice.model.format()
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
    voice: &Voice,
    piece: &str,
    bytes: &[u8],
) -> Result<NarrationSegmentDto, CommandError> {
    let inspection = checked_audio(bytes)?;
    let characters = i64::try_from(piece.chars().count()).unwrap_or(i64::MAX);
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        if let Some(kept) = kept_segment(&transaction, voice, piece)? {
            return Ok(kept);
        }
        let segment_id = generate_id(&transaction)?;
        let now = unix_time_millis()?;
        let (asset_id, path) = write_audio_asset(
            &transaction,
            paths,
            &inspection,
            bytes,
            "narration",
            "lesson narration kept",
            now,
        )?;
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
                        voice.model.format()
                    ],
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
                        voice.model.format(),
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

/// Audio a voice returned, refused when it is anything else.
pub(super) fn checked_audio(bytes: &[u8]) -> Result<MediaInspection, CommandError> {
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
    Ok(inspection)
}

/// Writes checked audio to a new managed file and records it as a ready asset
/// inside `transaction`. The caller records what keeps the asset referenced
/// and commits; if that fails, it removes the returned file.
pub(super) fn write_audio_asset(
    transaction: &Connection,
    paths: &AppPaths,
    inspection: &MediaInspection,
    bytes: &[u8],
    name: &str,
    detail: &str,
    now: i64,
) -> Result<(String, PathBuf), CommandError> {
    let asset_id = generate_id(transaction)?;
    let relative_path = AppPaths::image_relative_path(&asset_id, inspection.extension);
    let path = paths.resolve_relative(&relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    write_new_file(&path, bytes)?;
    let byte_size = i64::try_from(bytes.len()).unwrap_or(i64::MAX);
    let recorded = (|| {
        transaction.execute(
            "INSERT INTO assets (
                 id, original_filename, mime_type, byte_size, relative_path,
                 created_at, lifecycle_state, ready_at, verified_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'ready', ?6, ?6, ?6)",
            params![
                asset_id,
                format!("{name}.{}", inspection.extension),
                inspection.mime_type,
                byte_size,
                relative_path,
                now
            ],
        )?;
        transaction.execute(
            "INSERT INTO asset_reconciliation_events (
                 id, asset_id, action, from_path, to_path, detail, occurred_at
             ) VALUES (lower(hex(randomblob(16))), ?1, 'verified', NULL, ?2, ?3, ?4)",
            params![asset_id, relative_path, detail, now],
        )
    })();
    if let Err(error) = recorded {
        let _ = fs::remove_file(&path);
        return Err(CommandError::database(error));
    }
    Ok((asset_id, path))
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
