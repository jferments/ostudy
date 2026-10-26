//! On-device speech recognition through sherpa-onnx, on the platforms whose
//! builds link it. The recognizer is created on first use and kept for later
//! recordings: loading the model takes about a second, recognizing a short
//! question a fraction of that.

use std::path::Path;

/// Whether this build can recognize speech at all.
pub(crate) const SUPPORTED: bool = cfg!(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "android"
));

/// The files of an installed Moonshine model.
pub(crate) struct MoonshineFiles<'a> {
    pub directory: &'a Path,
    pub encoder: &'a str,
    pub decoder: &'a str,
    pub tokens: &'a str,
}

/// The text heard in mono samples in [-1, 1), or why recognition failed.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
pub(crate) fn recognize(
    model: &MoonshineFiles<'_>,
    sample_rate: u32,
    samples: &[f32],
) -> Result<String, String> {
    engine::recognize(model, sample_rate, samples)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "android")))]
pub(crate) fn recognize(
    _model: &MoonshineFiles<'_>,
    _sample_rate: u32,
    _samples: &[f32],
) -> Result<String, String> {
    Err("this build has no speech recognition".into())
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
mod engine {
    use std::{path::PathBuf, sync::Mutex};

    use sherpa_onnx::{
        OfflineModelConfig, OfflineMoonshineModelConfig, OfflineRecognizer, OfflineRecognizerConfig,
    };

    use super::MoonshineFiles;

    /// More threads than this did not make short clips faster.
    const MAX_THREADS: usize = 4;

    struct Loaded {
        directory: PathBuf,
        recognizer: OfflineRecognizer,
    }

    static RECOGNIZER: Mutex<Option<Loaded>> = Mutex::new(None);

    pub(super) fn recognize(
        model: &MoonshineFiles<'_>,
        sample_rate: u32,
        samples: &[f32],
    ) -> Result<String, String> {
        // One recording is recognized at a time; another waits for it.
        let mut loaded = RECOGNIZER
            .lock()
            .map_err(|_| "the recognizer lock was poisoned".to_string())?;
        if loaded.as_ref().map(|current| current.directory.as_path()) != Some(model.directory) {
            *loaded = None;
            let recognizer = create(model)?;
            *loaded = Some(Loaded {
                directory: model.directory.to_path_buf(),
                recognizer,
            });
        }
        let Some(Loaded { recognizer, .. }) = loaded.as_ref() else {
            return Err("no recognizer was loaded".into());
        };
        let stream = recognizer.create_stream();
        stream.accept_waveform(sample_rate as i32, samples);
        recognizer.decode(&stream);
        let result = stream
            .get_result()
            .ok_or_else(|| "the recognizer returned no result".to_string())?;
        Ok(result.text.trim().to_string())
    }

    fn create(model: &MoonshineFiles<'_>) -> Result<OfflineRecognizer, String> {
        let path = |name: &str| Some(model.directory.join(name).to_string_lossy().into_owned());
        let threads = std::thread::available_parallelism()
            .map(|count| count.get().min(MAX_THREADS))
            .unwrap_or(1);
        let config = OfflineRecognizerConfig {
            model_config: OfflineModelConfig {
                moonshine: OfflineMoonshineModelConfig {
                    encoder: path(model.encoder),
                    merged_decoder: path(model.decoder),
                    ..Default::default()
                },
                tokens: path(model.tokens),
                num_threads: threads as i32,
                ..Default::default()
            },
            ..Default::default()
        };
        OfflineRecognizer::create(&config)
            .ok_or_else(|| "the speech model could not be loaded".into())
    }
}
