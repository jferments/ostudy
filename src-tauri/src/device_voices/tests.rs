//! Voices on the device: each model is pinned to a revision and hashes, a
//! model that is not downloaded refuses to speak, and an installed one reads
//! study text intelligibly.

use super::*;

fn test_paths(name: &str) -> AppPaths {
    let root = std::env::temp_dir().join(format!(
        "ostudy-device-voices-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    AppPaths::new(root).unwrap()
}

#[test]
fn every_voice_model_is_pinned_to_a_revision_and_hashes() {
    for model in DEVICE_VOICE_MODELS {
        let pinned = model.pinned();
        assert_eq!(model.revision.len(), 40, "{}", model.id);
        assert!(model.id.starts_with("device/"), "{}", model.id);
        for file in &pinned.files {
            assert_eq!(file.sha256.len(), 64, "{}", file.name);
            assert!(file.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            let url = pinned.url(file);
            assert!(url.starts_with("https://huggingface.co/"), "{url}");
            assert!(
                url.contains(model.revision) && url.ends_with(file.name),
                "{url}"
            );
        }
        assert!(model.sample.starts_with("/voices/device/"));
        let sample = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../public")
            .join(model.sample.trim_start_matches('/'));
        assert!(sample.is_file(), "{}", sample.display());
        let mut numbers: Vec<_> = model
            .speakers
            .iter()
            .map(|speaker| speaker.number)
            .collect();
        numbers.sort_unstable();
        numbers.dedup();
        assert_eq!(numbers.len(), model.speakers.len(), "{}", model.id);
    }
    assert_eq!(KOKORO.pinned().download_bytes(), 142_404_640);
}

#[test]
fn a_library_without_a_voice_reports_what_each_download_takes() {
    let paths = test_paths("status");
    let status = status(&paths);
    assert_eq!(status.supported, SUPPORTED);
    assert_eq!(status.models.len(), 1);
    assert!(status.models.iter().all(|model| !model.installed));
    assert_eq!(status.models[0].model_id, KOKORO.id);
    assert_eq!(
        status.models[0].download_bytes,
        KOKORO.pinned().download_bytes()
    );
}

#[test]
fn a_voice_that_is_not_downloaded_refuses_to_speak() {
    let paths = test_paths("missing");
    let error =
        tauri::async_runtime::block_on(speak(&paths, &KOKORO, "af_heart", "Hello.")).unwrap_err();
    let expected = if SUPPORTED {
        "device_voice_missing"
    } else {
        "device_voice_unsupported"
    };
    assert_eq!(error.code, expected);
    let unknown = tauri::async_runtime::block_on(install_device_voice(
        &paths,
        InstallDeviceVoiceInput {
            model_id: "device/none".into(),
        },
    ))
    .unwrap_err();
    assert_eq!(unknown.code, "device_voice_unknown");
}

/// Speaks study text with a real model and hears it back with the speech
/// input model. The models are not part of the repository, so this runs only
/// when `OSTUDY_DEVICE_VOICE_DIR` names a folder holding each voice model's
/// files in a folder named like its id (`kokoro-v1.0-int8`) and
/// `OSTUDY_SPEECH_MODEL_DIR` names the Moonshine files.
#[test]
fn study_text_is_spoken_intelligibly_when_the_models_are_present() {
    let (Ok(voices), Ok(moonshine)) = (
        std::env::var("OSTUDY_DEVICE_VOICE_DIR"),
        std::env::var("OSTUDY_SPEECH_MODEL_DIR"),
    ) else {
        return;
    };
    if !SUPPORTED {
        return;
    }
    let text = "Parietal cells secrete hydrochloric acid into the stomach, while bicarbonate \
                leaves across the basolateral membrane.";
    for (model, voice) in [(&KOKORO, "af_heart"), (&KOKORO, "bm_george")] {
        let directory = std::path::Path::new(&voices).join(model.id.trim_start_matches("device/"));
        let files = VoiceFiles {
            directory: &directory,
            model: model.model.name,
            voices: model.voices.name,
            tokens: model.tokens.name,
        };
        let audio = speech_synthesis::speak(
            &files,
            &Utterance {
                text,
                speaker: model.speaker(voice).unwrap().number,
                british: voice.starts_with('b'),
            },
        )
        .unwrap();
        let seconds = (audio.len() - 44) as f32 / 2.0 / 24_000.0;
        assert!(seconds > 4.0 && seconds < 15.0, "{seconds}");
        let samples: Vec<f32> = audio[44..]
            .chunks_exact(2)
            .map(|pair| f32::from(i16::from_le_bytes([pair[0], pair[1]])) / 32_768.0)
            .collect();
        let heard =
            crate::speech_input::recognize(std::path::Path::new(&moonshine), 24_000, &samples)
                .unwrap()
                .to_lowercase();
        for word in [
            "parietal",
            "hydrochloric",
            "stomach",
            "bicarbonate",
            "membrane",
        ] {
            assert!(heard.contains(word), "{}: {heard}", model.id);
        }
    }
}
