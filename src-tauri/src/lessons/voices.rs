//! Saved voices, and the one voice each lesson is spoken in.
//!
//! A model that copies voices (Drama 3) picks a new speaker for every request
//! that carries no recording, so the parts of one lesson sounded like
//! different people. A saved voice is a sample sentence the model spoke once,
//! kept with its words; a request that carries it is spoken in that voice.
//! Trying a voice makes one sample to listen to, and keeping it adds it to the
//! learner's voices and chooses it. A lesson records the voice it is first
//! spoken in and keeps it when the learner later chooses another, so the
//! audio it already has and the audio it gets later sound alike.

use std::fs;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::narration::{
    checked_audio, resolved, speaking_request, speech_settings, write_audio_asset, write_settings,
    Narrating, Resolved, SpeechSettingsDto,
};
use crate::{
    ai::{
        generation,
        profiles::ProfileRequestSnapshot,
        speech::{self, SpeechModel, VoiceReference},
        AiRuntimeState,
    },
    database::Database,
    error::CommandError,
    paths::AppPaths,
    task_cancellation::CancellationSignal,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

/// What every sample says: a teacher's opening, so the voice is heard the way
/// lessons use it.
pub(crate) const SAMPLE: &str = "Hello, everyone, and welcome. Today we are going to take \
this one step at a time, and by the end, I think you will find that it makes good sense.";
const SAVED: &str = "saved:";
const PURPOSE: &str = "lesson_narration";
const MAX_NAME_CHARACTERS: usize = 60;
const MAX_DESCRIPTION_CHARACTERS: usize = 120;
/// A tried voice with no description; keeping it names it "Voice N".
const TRIED_NAME: &str = "New voice";

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpeechVoiceDto {
    pub id: String,
    pub name: String,
    pub model_id: String,
    /// The sample, to listen to.
    pub asset_id: String,
    /// `saved`, or `tried` for the voice being tried.
    pub state: String,
    /// The speech setting's voice that chooses it.
    pub voice: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrySpeechVoiceInput {
    /// What the voice should sound like, in the learner's words, such as "a
    /// calm older woman"; without it the model picks any voice.
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeechVoiceInput {
    pub voice_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenameSpeechVoiceInput {
    pub voice_id: String,
    pub name: String,
}

pub(crate) fn voice_key(id: &str) -> String {
    format!("{SAVED}{id}")
}

fn saved_id(voice: &str) -> Option<&str> {
    voice
        .strip_prefix(SAVED)
        .filter(|id| validate_entity_id(id, "voice").is_ok())
}

/// The learner's saved voices, oldest first, then the voice being tried.
pub(super) fn listed(connection: &Connection) -> Result<Vec<SpeechVoiceDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, model_id, asset_id, state FROM speech_voices
             WHERE state IN ('saved', 'tried')
             ORDER BY state = 'tried', created_at, id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| {
            let id: String = row.get(0)?;
            Ok(SpeechVoiceDto {
                voice: voice_key(&id),
                id,
                name: row.get(1)?,
                model_id: row.get(2)?,
                asset_id: row.get(3)?,
                state: row.get(4)?,
            })
        })
        .map_err(CommandError::database)?;
    rows.collect::<Result<_, _>>()
        .map_err(CommandError::database)
}

/// Whether `voice` chooses one of the learner's saved voices for `model`.
pub(super) fn is_saved_voice(
    connection: &Connection,
    model: &SpeechModel,
    voice: &str,
) -> Result<bool, CommandError> {
    let Some(id) = saved_id(voice) else {
        return Ok(false);
    };
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM speech_voices
                           WHERE id = ?1 AND model_id = ?2 AND state = 'saved')",
            params![id, model.id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

/// A saved voice's sample: its words and where its audio is, while the audio
/// is a ready asset. A removed voice still speaks for the lessons that use it.
fn sample(connection: &Connection, voice: &str) -> Result<Option<(String, String)>, CommandError> {
    let Some(id) = saved_id(voice) else {
        return Ok(None);
    };
    connection
        .query_row(
            "SELECT v.sample_text, a.relative_path FROM speech_voices v
             JOIN assets a ON a.id = v.asset_id
             WHERE v.id = ?1 AND a.lifecycle_state = 'ready' AND a.deleted_at IS NULL",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)
}

/// The recording a request carries to speak in `voice`; none for a preset
/// voice or the model's own choice.
pub(super) fn reference(
    database: &Database,
    paths: &AppPaths,
    voice: &str,
) -> Result<Option<VoiceReference>, CommandError> {
    if saved_id(voice).is_none() {
        return Ok(None);
    }
    let (transcript, relative_path) = database
        .with_connection(|connection| sample(connection, voice))?
        .ok_or_else(missing)?;
    let audio = fs::read(paths.resolve_relative(&relative_path)).map_err(|_| missing())?;
    Ok(Some(VoiceReference { audio, transcript }))
}

fn missing() -> CommandError {
    CommandError::new(
        "speech_voice_missing",
        "The recording of this voice is missing. Choose another voice in Settings.",
    )
}

/// The model and voice a lesson is spoken in. The first time any part of the
/// lesson is spoken, that is the voice chosen now, and it is recorded for the
/// lesson; a lesson whose recorded voice can no longer speak takes the voice
/// chosen now instead. A model that copies voices always speaks a saved voice:
/// when none is chosen yet, one is made and chosen first.
pub(super) async fn lesson_voice(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    request: &ProfileRequestSnapshot,
    settings: &Resolved,
    lesson_id: &str,
) -> Result<(&'static SpeechModel, String), CommandError> {
    let kept = database.with_connection(|connection| {
        let recorded: Option<(String, String)> = connection
            .query_row(
                "SELECT model_id, voice FROM lesson_voices WHERE lesson_id = ?1",
                [lesson_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some((model_id, voice)) = recorded else {
            return Ok(None);
        };
        let Some(model) = speech::speech_model(&model_id) else {
            return Ok(None);
        };
        let speaks = saved_id(&voice).is_none() || sample(connection, &voice)?.is_some();
        Ok(speaks.then_some((model, voice)))
    })?;
    if let Some(kept) = kept {
        return Ok(kept);
    }

    let model = settings.model;
    let chosen = database.with_connection(|connection| {
        Ok(if saved_id(&settings.voice).is_some() {
            sample(connection, &settings.voice)?.map(|_| settings.voice.clone())
        } else {
            Some(settings.voice.clone())
        })
    })?;
    let voice = match chosen {
        Some(voice) if !(model.clones_voices && voice.is_empty()) => voice,
        _ if model.clones_voices => {
            let made = make_sample(database, paths, runtime, request, model, "saved", None).await?;
            database.with_connection(|connection| {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(CommandError::database)?;
                let current = resolved(&transaction)?;
                write_settings(
                    &transaction,
                    current.profile_id.as_deref(),
                    model,
                    &made.voice,
                    unix_time_millis()?,
                )?;
                transaction.commit().map_err(CommandError::database)
            })?;
            made.voice
        }
        _ => return Err(missing()),
    };
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "INSERT INTO lesson_voices(lesson_id, model_id, voice, created_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(lesson_id) DO UPDATE SET
                     model_id = excluded.model_id,
                     voice = excluded.voice,
                     created_at = excluded.created_at",
                params![lesson_id, model.id, voice, now],
            )
            .map_err(CommandError::database)
    })?;
    Ok((model, voice))
}

/// Speaks the sample with no recording, so the model picks a new voice, and
/// keeps it as a voice in `state`. A description goes before the sample as a
/// direction the model follows and does not say; the kept words are the
/// sample's alone. Trying a voice replaces the one tried before; its audio is
/// left unreferenced for asset cleanup.
async fn make_sample(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    request: &ProfileRequestSnapshot,
    model: &'static SpeechModel,
    state: &str,
    description: Option<&str>,
) -> Result<SpeechVoiceDto, CommandError> {
    let spoken = match description {
        Some(description) => format!("[{description}] {SAMPLE}"),
        None => SAMPLE.to_string(),
    };
    let mut accounting = generation::Operation::begin(database, None, None, None, PURPOSE)?;
    let result = speech::speak(
        runtime,
        request,
        model,
        "",
        None,
        &spoken,
        &accounting.context(),
        &CancellationSignal::new(),
    )
    .await;
    accounting.finish_result(&result);
    let audio = result?;
    let inspection = checked_audio(&audio.bytes)?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let id = generate_id(&transaction)?;
        let now = unix_time_millis()?;
        let name = match (state, description) {
            ("tried", Some(description)) => description.chars().take(MAX_NAME_CHARACTERS).collect(),
            ("tried", None) => TRIED_NAME.to_string(),
            _ => next_name(&transaction)?,
        };
        let (asset_id, path) = write_audio_asset(
            &transaction,
            paths,
            &inspection,
            &audio.bytes,
            "voice",
            "voice sample kept",
            now,
        )?;
        let stored = (|| {
            transaction
                .execute("DELETE FROM speech_voices WHERE state = 'tried'", [])
                .map_err(CommandError::database)?;
            transaction
                .execute(
                    "INSERT INTO speech_voices(
                         id, model_id, name, state, sample_text, asset_id, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                    params![id, model.id, name, state, SAMPLE, asset_id, now],
                )
                .map_err(CommandError::database)?;
            transaction.commit().map_err(CommandError::database)
        })();
        if let Err(error) = stored {
            let _ = fs::remove_file(&path);
            return Err(error);
        }
        Ok(SpeechVoiceDto {
            voice: voice_key(&id),
            id,
            name,
            model_id: model.id.to_string(),
            asset_id,
            state: state.to_string(),
        })
    })
}

/// "Voice 1", "Voice 2", and so on, counting every voice ever kept.
fn next_name(connection: &Connection) -> Result<String, CommandError> {
    let kept: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM speech_voices WHERE state IN ('saved', 'removed')",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    Ok(format!("Voice {}", kept + 1))
}

/// Makes a sample in a new voice of the chosen model to listen to.
pub(crate) async fn try_speech_voice(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    input: TrySpeechVoiceInput,
) -> Result<SpeechSettingsDto, CommandError> {
    let description = input
        .description
        .as_deref()
        .map(str::trim)
        .filter(|description| !description.is_empty());
    if description.is_some_and(|description| {
        description.chars().count() > MAX_DESCRIPTION_CHARACTERS || description.contains(['[', ']'])
    }) {
        return Err(CommandError::new(
            "speech_voice_description_invalid",
            "Describe the voice in up to 120 characters, without square brackets.",
        ));
    }
    let settings = database.with_connection(|connection| resolved(connection))?;
    if !settings.model.clones_voices {
        return Err(CommandError::new(
            "speech_voice_presets",
            "This voice model has its own list of voices to choose from.",
        ));
    }
    let request = speaking_request(database, &settings)?;
    let _trying = Narrating::begin("voice sample".to_string())?;
    make_sample(
        database,
        paths,
        runtime,
        &request,
        settings.model,
        "tried",
        description,
    )
    .await?;
    database.with_connection(|connection| speech_settings(connection))
}

#[tauri::command]
pub fn keep_speech_voice(
    database: State<'_, Database>,
    input: SpeechVoiceInput,
) -> Result<SpeechSettingsDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| keep(connection, &input.voice_id, now))
}

#[tauri::command]
pub fn rename_speech_voice(
    database: State<'_, Database>,
    input: RenameSpeechVoiceInput,
) -> Result<SpeechSettingsDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| rename(connection, input, now))
}

#[tauri::command]
pub fn remove_speech_voice(
    database: State<'_, Database>,
    input: SpeechVoiceInput,
) -> Result<SpeechSettingsDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| remove(connection, &input.voice_id, now))
}

fn voice_state(
    connection: &Connection,
    id: &str,
) -> Result<(&'static SpeechModel, String), CommandError> {
    validate_entity_id(id, "voice")?;
    let found: Option<(String, String)> = connection
        .query_row(
            "SELECT model_id, state FROM speech_voices WHERE id = ?1 AND state != 'removed'",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let unknown = || CommandError::new("speech_voice_unknown", "That voice is not one of yours.");
    let (model_id, state) = found.ok_or_else(unknown)?;
    let model = speech::speech_model(&model_id).ok_or_else(unknown)?;
    Ok((model, state))
}

/// Keeps the voice being tried as a saved voice and chooses it.
pub(crate) fn keep(
    connection: &mut Connection,
    id: &str,
    now: i64,
) -> Result<SpeechSettingsDto, CommandError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let (model, state) = voice_state(&transaction, id)?;
    if state == "tried" {
        // A described voice keeps its description as its name.
        let numbered = next_name(&transaction)?;
        transaction
            .execute(
                "UPDATE speech_voices
                 SET state = 'saved', updated_at = ?4,
                     name = CASE WHEN name = ?2 THEN ?3 ELSE name END
                 WHERE id = ?1",
                params![id, TRIED_NAME, numbered, now],
            )
            .map_err(CommandError::database)?;
    }
    let current = resolved(&transaction)?;
    write_settings(
        &transaction,
        current.profile_id.as_deref(),
        model,
        &voice_key(id),
        now,
    )?;
    let saved = speech_settings(&transaction)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(saved)
}

pub(crate) fn rename(
    connection: &mut Connection,
    input: RenameSpeechVoiceInput,
    now: i64,
) -> Result<SpeechSettingsDto, CommandError> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARACTERS {
        return Err(CommandError::new(
            "speech_voice_name_invalid",
            "Give the voice a name of up to 60 characters.",
        ));
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    voice_state(&transaction, &input.voice_id)?;
    transaction
        .execute(
            "UPDATE speech_voices SET name = ?2, updated_at = ?3 WHERE id = ?1",
            params![input.voice_id, name, now],
        )
        .map_err(CommandError::database)?;
    let saved = speech_settings(&transaction)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(saved)
}

/// Removes a voice from the learner's voices. Lessons spoken in it keep it.
pub(crate) fn remove(
    connection: &mut Connection,
    id: &str,
    now: i64,
) -> Result<SpeechSettingsDto, CommandError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    voice_state(&transaction, id)?;
    let key = voice_key(id);
    if resolved(&transaction)?.voice == key {
        return Err(CommandError::new(
            "speech_voice_in_use",
            "Choose another voice before removing this one.",
        ));
    }
    let spoken: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM lesson_voices WHERE voice = ?1)",
            [&key],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if spoken {
        transaction
            .execute(
                "UPDATE speech_voices SET state = 'removed', updated_at = ?2 WHERE id = ?1",
                params![id, now],
            )
            .map_err(CommandError::database)?;
    } else {
        transaction
            .execute("DELETE FROM speech_voices WHERE id = ?1", [id])
            .map_err(CommandError::database)?;
    }
    let saved = speech_settings(&transaction)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(saved)
}

#[cfg(test)]
#[path = "voices_tests.rs"]
mod tests;
