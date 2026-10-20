//! Spoken audio from an OpenAI-style `/audio/speech` endpoint.
//!
//! Speech is a separate request from text generation: the caller hands over
//! finished text and gets audio bytes back. OpenRouter is the only route
//! implemented; a local Kokoro server speaks the same request shape and plugs
//! in here later. The catalog below names the models ostudy has tried, with
//! the voices and request size each accepts.

use serde_json::{json, Value};

use rusqlite::Connection;

use super::{
    catalog::{AiTaskKind, ProviderKind},
    endpoint_policy::join_endpoint,
    gateway::AudioResponse,
    generation,
    profiles::{enabled_profiles_of_kind, task_profile_id, ProfileRequestSnapshot},
    AiRuntimeState,
};
use crate::{error::CommandError, task_cancellation::CancellationSignal};

/// The only audio format every catalog model returns.
pub(crate) const SPEECH_FORMAT: &str = "mp3";
/// A spoken segment's audio is far below this; it bounds a broken response.
const MAX_SPEECH_RESPONSE_BYTES: usize = 24 * 1024 * 1024;
/// Speech takes longer than its text's length suggests; a segment of a
/// thousand characters took 12 s on the slower catalog model.
const MIN_SPEECH_TIMEOUT_MS: i64 = 120_000;

#[derive(Debug)]
pub(crate) struct SpeechModel {
    pub id: &'static str,
    pub label: &'static str,
    /// Empty when the model chooses its own voice and takes no `voice` field.
    pub voices: &'static [&'static str],
    pub default_voice: &'static str,
    /// The most characters sent in one request.
    pub max_characters: usize,
}

pub(crate) const KOKORO: &str = "hexgrad/kokoro-82m";
pub(crate) const DRAMA: &str = "fish-audio/drama-3";

pub(crate) const SPEECH_MODELS: &[SpeechModel] = &[
    SpeechModel {
        id: KOKORO,
        label: "Kokoro",
        voices: &[
            "af_heart",
            "af_bella",
            "af_nicole",
            "af_sky",
            "am_michael",
            "am_adam",
            "am_fenrir",
            "bf_emma",
            "bf_isabella",
            "bm_george",
            "bm_lewis",
        ],
        default_voice: "af_heart",
        // OpenRouter lists a 4,096-character context.
        max_characters: 1_500,
    },
    SpeechModel {
        id: DRAMA,
        label: "Drama 3",
        voices: &[],
        default_voice: "",
        max_characters: 1_500,
    },
];

pub(crate) fn speech_model(id: &str) -> Option<&'static SpeechModel> {
    SPEECH_MODELS.iter().find(|model| model.id == id)
}

/// The engine name stored with kept audio, so audio from one route is never
/// mistaken for another's.
pub(crate) fn engine(provider: ProviderKind) -> Result<&'static str, CommandError> {
    match provider {
        ProviderKind::Openrouter => Ok("openrouter"),
        _ => Err(CommandError::new(
            "speech_provider_unsupported",
            "Spoken narration needs an OpenRouter connection.",
        )),
    }
}

/// Connections that can speak, most recently changed first, as
/// `(id, display name)`.
pub(crate) fn speaking_connections(
    connection: &Connection,
) -> Result<Vec<(String, String)>, CommandError> {
    enabled_profiles_of_kind(connection, ProviderKind::Openrouter)
}

/// The connection that writes documents, when it can speak.
pub(crate) fn document_connection(connection: &Connection) -> Result<Option<String>, CommandError> {
    task_profile_id(connection, AiTaskKind::FlashcardGeneration)
}

pub(crate) fn request_body(model: &SpeechModel, voice: &str, text: &str) -> Value {
    let mut body = json!({
        "model": model.id,
        "input": text,
        "response_format": SPEECH_FORMAT,
    });
    if !voice.is_empty() {
        body["voice"] = Value::String(voice.to_string());
    }
    body
}

/// Speaks one finished piece of text. The request is accounted under the
/// caller's operation; the provider reports no token usage for speech.
pub(crate) async fn speak(
    runtime: &AiRuntimeState,
    request: &ProfileRequestSnapshot,
    model: &SpeechModel,
    voice: &str,
    text: &str,
    accounting: &generation::Context,
    cancellation: &CancellationSignal,
) -> Result<AudioResponse, CommandError> {
    let profile = &request.profile;
    profile.validate_for_provider_request()?;
    engine(profile.provider_kind)?;
    if text.trim().is_empty() || text.chars().count() > model.max_characters {
        return Err(CommandError::new(
            "speech_text_invalid",
            "This narration piece is empty or too long to speak in one request.",
        ));
    }
    let url = join_endpoint(&profile.base_url, "audio/speech")?;
    let body = request_body(model, voice, text);
    let credential =
        runtime
            .secrets
            .resolve(&profile.id, profile.auth_mode, request.credential_storage)?;
    let bearer = credential
        .as_ref()
        .map(|secret| secret.as_utf8())
        .transpose()?;
    let mut attempt = accounting.start_attempt(
        "lesson_narration",
        &body,
        None,
        profile.provider_kind.as_str(),
    )?;
    let response = tokio::select! {
        _ = cancellation.canceled() => Err(CommandError::new(
            "speech_canceled",
            "Narration was stopped.",
        )),
        response = runtime.gateway.post_json_for_audio(
            url,
            bearer,
            &body,
            profile.request_timeout_ms.max(MIN_SPEECH_TIMEOUT_MS),
            MAX_SPEECH_RESPONSE_BYTES,
        ) => response.map_err(|failure| CommandError::new(failure.safe.code, failure.safe.message)),
    };
    match &response {
        Ok(audio) => attempt.non_generative_response(&json!({
            "id": audio.generation_id,
            "audioBytes": audio.bytes.len(),
        })),
        Err(error) => attempt.failed(&error.code),
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalog_default_voice_is_one_of_its_voices() {
        for model in SPEECH_MODELS {
            assert!(
                model.voices.is_empty() && model.default_voice.is_empty()
                    || model.voices.contains(&model.default_voice),
                "{}",
                model.id
            );
            assert!(model.max_characters >= 200);
        }
    }

    #[test]
    fn a_model_that_picks_its_own_voice_gets_no_voice_field() {
        let drama = speech_model(DRAMA).unwrap();
        let body = request_body(drama, "", "Hello.");
        assert!(body.get("voice").is_none());
        assert_eq!(body["response_format"], "mp3");
        let kokoro = speech_model(KOKORO).unwrap();
        assert_eq!(
            request_body(kokoro, "bm_george", "Hello.")["voice"],
            "bm_george"
        );
    }
}
