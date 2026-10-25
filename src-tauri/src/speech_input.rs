//! Speech input: turns a short recording from the microphone into text on the
//! learner's own device.
//!
//! Recognition runs locally through sherpa-onnx with Moonshine v2, an English
//! model of about 141 MB that is downloaded the first time it is needed, from
//! a pinned revision, and checked file by file against SHA-256 hashes before
//! it is used. Nothing is sent to an AI provider. The webview records and
//! hands over 16-bit PCM; the recording is decoded here and dropped as soon as
//! it has been transcribed.

mod install;
#[cfg(test)]
mod tests;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{error::CommandError, paths::AppPaths, platform::speech_recognition};

pub(crate) use install::install_speech_input_model;

/// One file of a model, as published at the pinned revision.
pub(crate) struct ModelFile {
    pub name: &'static str,
    pub sha256: &'static str,
    pub bytes: u64,
}

pub(crate) struct SpeechInputModel {
    pub id: &'static str,
    pub label: &'static str,
    /// A Hugging Face repository and the commit its files are read from.
    pub repository: &'static str,
    pub revision: &'static str,
    pub encoder: ModelFile,
    pub decoder: ModelFile,
    pub tokens: ModelFile,
}

impl SpeechInputModel {
    pub(crate) fn files(&self) -> [&ModelFile; 3] {
        [&self.encoder, &self.decoder, &self.tokens]
    }

    pub(crate) fn download_bytes(&self) -> u64 {
        self.files().iter().map(|file| file.bytes).sum()
    }

    pub(crate) fn url(&self, file: &ModelFile) -> String {
        format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            self.repository, self.revision, file.name
        )
    }
}

/// Moonshine v2 base, English, 8-bit (MIT). On a 4-core 2.1 GHz CPU it turned
/// a 16.7-second clip into text in about 0.6 s.
pub(crate) const MOONSHINE_BASE_EN: SpeechInputModel = SpeechInputModel {
    id: "moonshine-v2-base-en-2026-02-27",
    label: "Moonshine v2 (English)",
    repository: "csukuangfj2/sherpa-onnx-moonshine-base-en-quantized-2026-02-27",
    revision: "8f4d6c58c03d40bcea40043bb7120a878f2bbef6",
    encoder: ModelFile {
        name: "encoder_model.ort",
        sha256: "7c66495948d0d08ec1af454cd4b5514862ae6511e94712a60e6d83eaec8dc8cf",
        bytes: 31_326_816,
    },
    decoder: ModelFile {
        name: "decoder_model_merged.ort",
        sha256: "d9d7b333af34bc552580576ddcf248a1c6c839e0d3b43b09afb9376ed009899d",
        bytes: 109_424_400,
    },
    tokens: ModelFile {
        name: "tokens.txt",
        sha256: "2870d843e14c1e187bf1913a521562a63b53933814bd7f2145120468f494a049",
        bytes: 549_350,
    },
};

/// The longest recording accepted, which also bounds the request's size.
pub(crate) const MAX_RECORDING_SECONDS: u32 = 120;
const MIN_SAMPLE_RATE: u32 = 8_000;
const MAX_SAMPLE_RATE: u32 = 48_000;

/// Whether this build can recognize speech at all.
pub(crate) const SUPPORTED: bool = speech_recognition::SUPPORTED;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeechInputStatusDto {
    /// False where this build has no speech recognition.
    pub supported: bool,
    pub model_label: &'static str,
    /// What the first use downloads.
    pub download_bytes: u64,
    pub installed: bool,
    /// Bytes received so far while the model downloads.
    pub installing_bytes: Option<u64>,
    /// Why the last download failed, until another one starts.
    pub install_error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TranscribeSpeechInput {
    /// Mono 16-bit little-endian PCM.
    pub samples_base64: String,
    pub sample_rate: u32,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeechTranscriptDto {
    /// Empty when nothing was heard.
    pub text: String,
    pub duration_ms: u64,
    /// How long recognition took after the recording ended.
    pub elapsed_ms: u64,
}

#[tauri::command]
pub fn get_speech_input_status(
    paths: State<'_, AppPaths>,
) -> Result<SpeechInputStatusDto, CommandError> {
    Ok(status(paths.inner()))
}

pub(crate) fn status(paths: &AppPaths) -> SpeechInputStatusDto {
    let model = &MOONSHINE_BASE_EN;
    let progress = install::progress();
    SpeechInputStatusDto {
        supported: SUPPORTED,
        model_label: model.label,
        download_bytes: model.download_bytes(),
        installed: SUPPORTED && install::is_installed(paths, model),
        installing_bytes: progress.received,
        install_error: progress.error,
    }
}

pub(crate) async fn transcribe_speech(
    paths: &AppPaths,
    input: TranscribeSpeechInput,
) -> Result<SpeechTranscriptDto, CommandError> {
    let samples = decode_samples(&input)?;
    let duration_ms = samples.len() as u64 * 1000 / u64::from(input.sample_rate);
    if !SUPPORTED {
        return Err(unsupported());
    }
    let model = &MOONSHINE_BASE_EN;
    if !install::is_installed(paths, model) {
        return Err(CommandError::new(
            "speech_model_missing",
            "The speech model has not been downloaded yet.",
        ));
    }
    let directory = install::model_directory(paths, model);
    let started = std::time::Instant::now();
    let text = tauri::async_runtime::spawn_blocking(move || {
        recognize(&directory, input.sample_rate, &samples)
    })
    .await
    .map_err(|error| recognition_error(error.to_string()))??;
    Ok(SpeechTranscriptDto {
        text,
        duration_ms,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

pub(crate) fn recognize(
    directory: &std::path::Path,
    sample_rate: u32,
    samples: &[f32],
) -> Result<String, CommandError> {
    let model = &MOONSHINE_BASE_EN;
    let files = speech_recognition::MoonshineFiles {
        directory,
        encoder: model.encoder.name,
        decoder: model.decoder.name,
        tokens: model.tokens.name,
    };
    speech_recognition::recognize(&files, sample_rate, samples).map_err(recognition_error)
}

/// The recording as samples in [-1, 1), after checking its rate and length.
pub(crate) fn decode_samples(input: &TranscribeSpeechInput) -> Result<Vec<f32>, CommandError> {
    let invalid = |message: &str| CommandError::new("invalid_recording", message.to_string());
    if !(MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&input.sample_rate) {
        return Err(invalid("The recording's sample rate is not supported."));
    }
    let max_bytes = (MAX_RECORDING_SECONDS * MAX_SAMPLE_RATE * 2) as usize;
    // Base64 holds 3 bytes in every 4 characters.
    if input.samples_base64.len() > max_bytes / 3 * 4 + 4 {
        return Err(invalid("The recording is too long."));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&input.samples_base64)
        .map_err(|_| invalid("The recording could not be read."))?;
    if bytes.len() % 2 != 0 {
        return Err(invalid("The recording could not be read."));
    }
    let count = bytes.len() / 2;
    if count as u64 > u64::from(MAX_RECORDING_SECONDS) * u64::from(input.sample_rate) {
        return Err(invalid("The recording is too long."));
    }
    Ok(bytes
        .chunks_exact(2)
        .map(|pair| f32::from(i16::from_le_bytes([pair[0], pair[1]])) / 32_768.0)
        .collect())
}

fn unsupported() -> CommandError {
    CommandError::new(
        "speech_input_unsupported",
        "Speech input is not available on this device yet.",
    )
}

pub(crate) fn recognition_error(detail: String) -> CommandError {
    crate::logging::warn_target(
        "speech_input",
        "speech_recognition_failed",
        "Speech recognition failed.",
        serde_json::json!({ "detail": detail }),
    );
    CommandError::new(
        "speech_recognition_failed",
        "The recording could not be turned into text.",
    )
}
