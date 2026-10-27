//! Voices that read aloud on the learner's own device, for free.
//!
//! Two voice models are offered: Kokoro, the more natural one, and KittenTTS,
//! a small, quick one. Neither is bundled. The learner hears a short sample of
//! each in Settings (`public/voices/device/`), chooses one, and it is
//! downloaded from a pinned revision and checked file by file against SHA-256
//! hashes, like the speech input model. Speaking runs through sherpa-onnx, so
//! no text goes to an AI provider and nothing is paid for.

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    error::CommandError,
    paths::AppPaths,
    platform::speech_synthesis::{self, Utterance, VoiceFamily, VoiceFiles},
    speech_input::{install, ModelFile, PinnedModel},
};

/// Whether this build can speak on the device at all.
pub(crate) const SUPPORTED: bool = speech_synthesis::SUPPORTED;

/// One speaker in a voice model's voice table.
#[derive(Debug)]
pub(crate) struct Speaker {
    pub voice: &'static str,
    pub number: i32,
}

#[derive(Debug)]
pub(crate) struct DeviceVoiceModel {
    /// Also the speech model id that chooses it.
    pub id: &'static str,
    pub label: &'static str,
    /// One line on what it is like, for choosing between them.
    pub summary: &'static str,
    pub family: VoiceFamily,
    /// A Hugging Face repository and the commit its files are read from.
    pub repository: &'static str,
    pub revision: &'static str,
    pub model: ModelFile,
    pub voices: ModelFile,
    pub tokens: ModelFile,
    /// The voices offered, in the order Settings lists them.
    pub speakers: &'static [Speaker],
    /// Where the frontend plays its sample of two voices.
    pub sample: &'static str,
}

impl DeviceVoiceModel {
    pub(crate) fn pinned(&self) -> PinnedModel<'_> {
        PinnedModel {
            id: self.id,
            repository: self.repository,
            revision: self.revision,
            files: vec![&self.model, &self.voices, &self.tokens],
        }
    }

    pub(crate) fn speaker(&self, voice: &str) -> Option<&Speaker> {
        self.speakers.iter().find(|speaker| speaker.voice == voice)
    }
}

const fn speaker(voice: &'static str, number: i32) -> Speaker {
    Speaker { voice, number }
}

/// Kokoro v1.0 (Apache-2.0), 8-bit, with its American and British English
/// voices. On a 4-core cloud CPU it spoke a 9-second sentence in about 11 s.
pub(crate) const KOKORO: DeviceVoiceModel = DeviceVoiceModel {
    id: "device/kokoro-v1.0-int8",
    label: "Kokoro",
    summary: "The most natural voices. Speaks a little slower than real time on an older computer.",
    family: VoiceFamily::Kokoro,
    repository: "csukuangfj/kokoro-int8-multi-lang-v1_0",
    revision: "57adb37fc0e8d4bd430cfc1b8f8c2294945b9af2",
    model: ModelFile {
        name: "model.int8.onnx",
        sha256: "75635ea3b85a32d00094dcdd61a258aba87c160f7c264c304a7b3ae9d734195e",
        bytes: 114_202_993,
    },
    voices: ModelFile {
        name: "voices.bin",
        sha256: "1c5a5b983d3d50d8586d437a51f3faa2da7919ce76a013c081e65671a3447c29",
        bytes: 28_200_960,
    },
    tokens: ModelFile {
        name: "tokens.txt",
        sha256: "6ebb6bb288f20f3ae8d004d3c2ca27697da27c037d75e81a60e2a6a663f95425",
        bytes: 687,
    },
    // Numbers from the model's `speaker_names`.
    speakers: &[
        speaker("af_heart", 3),
        speaker("af_bella", 2),
        speaker("af_nicole", 6),
        speaker("af_sarah", 9),
        speaker("af_sky", 10),
        speaker("am_michael", 16),
        speaker("am_adam", 11),
        speaker("am_fenrir", 14),
        speaker("am_puck", 18),
        speaker("bf_emma", 21),
        speaker("bf_isabella", 22),
        speaker("bm_george", 26),
        speaker("bm_lewis", 27),
        speaker("bm_fable", 25),
    ],
    sample: "/voices/device/kokoro.mp3",
};

/// KittenTTS nano v0.2 (Apache-2.0), 16-bit. It spoke the same sentence in
/// about 3 s.
pub(crate) const KITTEN: DeviceVoiceModel = DeviceVoiceModel {
    id: "device/kitten-nano-v0.2",
    label: "KittenTTS",
    summary: "A small, quick download that speaks faster than real time, even on a phone.",
    family: VoiceFamily::Kitten,
    repository: "csukuangfj/kitten-nano-en-v0_2-fp16",
    revision: "7d14a97d38072576ddca7a673ab5bb49c43bb169",
    model: ModelFile {
        name: "model.fp16.onnx",
        sha256: "f24264d818087e643828da7a992e892e703c412bf147e152b825da567031d5ce",
        bytes: 23_804_897,
    },
    voices: ModelFile {
        name: "voices.bin",
        sha256: "42a40a24a352a38657d6cb86ceee51bbc2b7780b29e04fb60bcdf959adccea01",
        bytes: 8_192,
    },
    tokens: ModelFile {
        name: "tokens.txt",
        sha256: "934a4188addc7665dd3410256bb622169242357fbb99d840d9351209b486dabb",
        bytes: 1_064,
    },
    speakers: &[
        speaker("expr-voice-3-f", 3),
        speaker("expr-voice-4-m", 4),
        speaker("expr-voice-2-f", 1),
        speaker("expr-voice-2-m", 0),
        speaker("expr-voice-3-m", 2),
        speaker("expr-voice-4-f", 5),
        speaker("expr-voice-5-f", 7),
        speaker("expr-voice-5-m", 6),
    ],
    sample: "/voices/device/kitten.mp3",
};

pub(crate) const DEVICE_VOICE_MODELS: [&DeviceVoiceModel; 2] = [&KOKORO, &KITTEN];

pub(crate) fn device_voice_model(id: &str) -> Option<&'static DeviceVoiceModel> {
    DEVICE_VOICE_MODELS.into_iter().find(|model| model.id == id)
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceVoiceStatusDto {
    pub model_id: &'static str,
    pub label: &'static str,
    pub summary: &'static str,
    /// What installing it downloads.
    pub download_bytes: u64,
    /// The sample of two of its voices.
    pub sample: &'static str,
    pub installed: bool,
    /// Bytes received so far while it downloads.
    pub installing_bytes: Option<u64>,
    /// Why the last download failed, until another one starts.
    pub install_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceVoicesDto {
    /// False where this build cannot speak on the device.
    pub supported: bool,
    pub models: Vec<DeviceVoiceStatusDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallDeviceVoiceInput {
    pub model_id: String,
}

#[tauri::command]
pub fn get_device_voices(paths: State<'_, AppPaths>) -> Result<DeviceVoicesDto, CommandError> {
    Ok(status(paths.inner()))
}

pub(crate) fn status(paths: &AppPaths) -> DeviceVoicesDto {
    DeviceVoicesDto {
        supported: SUPPORTED,
        models: DEVICE_VOICE_MODELS
            .iter()
            .map(|model| {
                let pinned = model.pinned();
                let progress = install::progress(&pinned);
                DeviceVoiceStatusDto {
                    model_id: model.id,
                    label: model.label,
                    summary: model.summary,
                    download_bytes: pinned.download_bytes(),
                    sample: model.sample,
                    installed: is_installed(paths, model),
                    installing_bytes: progress.received,
                    install_error: progress.error,
                }
            })
            .collect(),
    }
}

pub(crate) fn is_installed(paths: &AppPaths, model: &DeviceVoiceModel) -> bool {
    SUPPORTED && install::is_installed(paths, &model.pinned())
}

/// Downloads a voice model unless it is already installed. A second call
/// while it downloads returns at once with its progress.
pub(crate) async fn install_device_voice(
    paths: &AppPaths,
    input: InstallDeviceVoiceInput,
) -> Result<DeviceVoicesDto, CommandError> {
    let model = device_voice_model(&input.model_id).ok_or_else(unknown)?;
    if !SUPPORTED {
        return Err(unsupported());
    }
    install::install(paths, &model.pinned()).await?;
    Ok(status(paths))
}

/// Speaks one piece of text in `voice` and returns it as a WAV file.
pub(crate) async fn speak(
    paths: &AppPaths,
    model: &'static DeviceVoiceModel,
    voice: &str,
    text: &str,
) -> Result<Vec<u8>, CommandError> {
    if !SUPPORTED {
        return Err(unsupported());
    }
    if !is_installed(paths, model) {
        return Err(not_installed(model));
    }
    let speaker = model.speaker(voice).ok_or_else(|| {
        CommandError::new(
            "speech_voice_unknown",
            "That voice is not offered by this voice model.",
        )
    })?;
    let directory = install::model_directory(paths, &model.pinned());
    let number = speaker.number;
    let british = voice.starts_with('b');
    let text = text.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let files = VoiceFiles {
            directory: &directory,
            family: model.family,
            model: model.model.name,
            voices: model.voices.name,
            tokens: model.tokens.name,
        };
        speech_synthesis::speak(
            &files,
            &Utterance {
                text: &text,
                speaker: number,
                british,
            },
        )
    })
    .await
    .map_err(|error| speech_error(error.to_string()))?
    .map_err(speech_error)
}

pub(crate) fn not_installed(model: &DeviceVoiceModel) -> CommandError {
    CommandError::new(
        "device_voice_missing",
        format!(
            "{} has not been downloaded yet. Download it in Settings → AI, under Voice.",
            model.label
        ),
    )
}

fn unknown() -> CommandError {
    CommandError::new(
        "device_voice_unknown",
        "That voice is not one ostudy offers.",
    )
}

fn unsupported() -> CommandError {
    CommandError::new(
        "device_voice_unsupported",
        "Voices on this device are not available in this build yet.",
    )
}

fn speech_error(detail: String) -> CommandError {
    crate::logging::warn_target(
        "device_voices",
        "device_speech_failed",
        "Speaking on the device failed.",
        serde_json::json!({ "detail": detail }),
    );
    CommandError::new(
        "device_speech_failed",
        "This could not be read aloud on this device.",
    )
}
