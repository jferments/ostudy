//! On-device speech through sherpa-onnx, on the platforms whose builds link
//! it. Text becomes phonemes here, with loqui-g2p (misaki's English rules and
//! lexicons, and a small neural model for words they do not hold), because
//! sherpa-onnx 2.0 no longer carries eSpeak. The voice is loaded on first use
//! and kept for the next piece: loading takes a second or two, and lessons
//! are spoken piece by piece.

use std::path::Path;

/// Whether this build can speak on the device at all.
pub(crate) const SUPPORTED: bool = cfg!(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "android"
));

/// The files of an installed Kokoro model, which reads misaki's phonemes as
/// loqui-g2p writes them.
pub(crate) struct VoiceFiles<'a> {
    pub directory: &'a Path,
    pub model: &'a str,
    pub voices: &'a str,
    pub tokens: &'a str,
}

/// One piece of text, and who says it.
pub(crate) struct Utterance<'a> {
    pub text: &'a str,
    /// The speaker's number in the model's voice table.
    pub speaker: i32,
    /// Whether the speaker reads British English.
    pub british: bool,
}

/// The longest sentence handed to a voice at once, in phoneme characters;
/// Kokoro takes at most 510 tokens in one pass.
const MAX_SENTENCE_PHONEMES: usize = 400;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
/// The utterance spoken as a 16-bit mono WAV file, or why it could not be.
pub(crate) fn speak(voice: &VoiceFiles<'_>, utterance: &Utterance<'_>) -> Result<Vec<u8>, String> {
    engine::speak(voice, utterance)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "android")))]
pub(crate) fn speak(
    _voice: &VoiceFiles<'_>,
    _utterance: &Utterance<'_>,
) -> Result<Vec<u8>, String> {
    Err("this build cannot speak on the device".into())
}

/// Phonemes cut into sentences a voice speaks in one pass: after each full
/// stop, question or exclamation mark, and a sentence longer than
/// `MAX_SENTENCE_PHONEMES` after its last comma or space that fits.
pub(crate) fn sentences(phonemes: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let mut characters = phonemes.chars().peekable();
    while let Some(character) = characters.next() {
        current.push(character);
        let ends = matches!(character, '.' | '!' | '?')
            && characters.peek().is_none_or(|next| next.is_whitespace());
        if ends {
            sentences.push(std::mem::take(&mut current));
        }
    }
    sentences.push(current);
    let mut pieces = Vec::new();
    for sentence in sentences {
        let mut rest = sentence.trim();
        while rest.chars().count() > MAX_SENTENCE_PHONEMES {
            let limit = rest
                .char_indices()
                .nth(MAX_SENTENCE_PHONEMES)
                .map_or(rest.len(), |(index, _)| index);
            let head = &rest[..limit];
            let cut = head
                .rfind(", ")
                .map(|index| index + 1)
                .or_else(|| head.rfind(' '))
                .filter(|&index| index > 0)
                .unwrap_or(limit);
            pieces.push(rest[..cut].trim().to_string());
            rest = rest[cut..].trim_start();
        }
        if !rest.is_empty() {
            pieces.push(rest.to_string());
        }
    }
    pieces
}

/// 16-bit mono WAV bytes of samples in [-1, 1].
pub(crate) fn wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_bytes = u32::try_from(samples.len() * 2).unwrap_or(u32::MAX);
    let mut bytes = Vec::with_capacity(44 + samples.len() * 2);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_bytes.to_le_bytes());
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * 32_767.0).round() as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
mod engine {
    use std::{
        path::PathBuf,
        sync::{Mutex, OnceLock},
    };

    use loqui_g2p::{Dialect, G2p, OovFallback};
    use sherpa_onnx::{
        GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig,
    };

    use super::{sentences, wav, Utterance, VoiceFiles};

    /// More threads than this did not make a piece faster.
    const MAX_THREADS: usize = 4;

    struct Loaded {
        directory: PathBuf,
        tts: OfflineTts,
    }

    static VOICE: Mutex<Option<Loaded>> = Mutex::new(None);
    static AMERICAN: OnceLock<Result<G2p, String>> = OnceLock::new();
    static BRITISH: OnceLock<Result<G2p, String>> = OnceLock::new();

    fn phonemizer(british: bool) -> Result<&'static G2p, String> {
        let (cell, dialect) = if british {
            (&BRITISH, Dialect::British)
        } else {
            (&AMERICAN, Dialect::American)
        };
        cell.get_or_init(|| {
            G2p::new(dialect, OovFallback::Neural).map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
    }

    pub(super) fn speak(
        voice: &VoiceFiles<'_>,
        utterance: &Utterance<'_>,
    ) -> Result<Vec<u8>, String> {
        let phonemes = phonemizer(utterance.british)?.phonemize(utterance.text);
        // A word nothing could phonemize is left out rather than read as a mark.
        let phonemes = phonemes.replace(loqui_g2p::UNKNOWN, "");
        let sentences: Vec<Vec<i32>> = sentences(&phonemes)
            .iter()
            .map(|sentence| sentence.chars().map(|character| character as i32).collect())
            .collect();
        if sentences.is_empty() {
            return Err("there was nothing to say".into());
        }
        // One piece is spoken at a time; another waits for it.
        let mut loaded = VOICE
            .lock()
            .map_err(|_| "the voice lock was poisoned".to_string())?;
        if loaded.as_ref().map(|current| current.directory.as_path()) != Some(voice.directory) {
            *loaded = None;
            let tts = create(voice)?;
            *loaded = Some(Loaded {
                directory: voice.directory.to_path_buf(),
                tts,
            });
        }
        let Some(Loaded { tts, .. }) = loaded.as_ref() else {
            return Err("no voice was loaded".into());
        };
        let config = GenerationConfig {
            sid: utterance.speaker,
            phoneme_codepoints: Some(sentences),
            ..Default::default()
        };
        let audio = tts
            .generate_with_config("", &config, None::<fn(&[f32], f32) -> bool>)
            .ok_or_else(|| "the voice made no audio".to_string())?;
        let samples = audio.samples();
        let rate = u32::try_from(audio.sample_rate()).map_err(|_| "a bad sample rate")?;
        if samples.is_empty() || rate == 0 {
            return Err("the voice made no audio".into());
        }
        Ok(wav(samples, rate))
    }

    fn create(voice: &VoiceFiles<'_>) -> Result<OfflineTts, String> {
        let path = |name: &str| Some(voice.directory.join(name).to_string_lossy().into_owned());
        let threads = std::thread::available_parallelism()
            .map(|count| count.get().min(MAX_THREADS))
            .unwrap_or(1);
        let mut config = OfflineTtsConfig::default();
        config.model.num_threads = threads as i32;
        config.model.kokoro = OfflineTtsKokoroModelConfig {
            model: path(voice.model),
            voices: path(voice.voices),
            tokens: path(voice.tokens),
            ..Default::default()
        };
        OfflineTts::create(&config).ok_or_else(|| "the voice could not be loaded".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phonemes_are_cut_after_sentences_and_long_ones_after_a_comma() {
        assert_eq!(
            sentences("ə bˈIk. ðə kˈɑɹ? jˈɛs"),
            ["ə bˈIk.", "ðə kˈɑɹ?", "jˈɛs"]
        );
        assert!(sentences("  ").is_empty());
        let long = format!("{}, {}", "ə ".repeat(150).trim(), "b ".repeat(150).trim());
        let cut = sentences(&long);
        assert_eq!(cut.len(), 2, "{cut:?}");
        assert!(cut[0].ends_with(','));
        assert!(cut
            .iter()
            .all(|piece| piece.chars().count() <= MAX_SENTENCE_PHONEMES));
    }

    #[test]
    fn audio_is_written_as_a_16_bit_mono_wav_file() {
        let bytes = wav(&[0.0, 1.0, -1.0], 24_000);
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(&bytes[8..16], b"WAVEfmt ");
        assert_eq!(
            u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
            24_000
        );
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(bytes.len(), 44 + 6);
        assert_eq!(i16::from_le_bytes([bytes[46], bytes[47]]), 32_767);
        assert!(crate::media::inspect_media_bytes(&bytes).is_ok());
    }
}
