//! Speech input: recordings are checked before recognition, and the model is
//! installed only when every downloaded file matches its published hash.

use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

use base64::Engine as _;
use sha2::{Digest, Sha256};

use super::{install, *};

fn pcm(samples: &[i16]) -> String {
    let bytes: Vec<u8> = samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn test_paths(name: &str) -> AppPaths {
    let root =
        std::env::temp_dir().join(format!("ostudy-speech-input-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    AppPaths::new(root).unwrap()
}

#[test]
fn a_recording_is_read_as_samples_between_minus_one_and_one() {
    let input = TranscribeSpeechInput {
        samples_base64: pcm(&[0, 16_384, -32_768, 32_767]),
        sample_rate: 16_000,
    };
    let samples = decode_samples(&input).unwrap();
    assert_eq!(samples.len(), 4);
    assert_eq!(samples[0], 0.0);
    assert_eq!(samples[1], 0.5);
    assert_eq!(samples[2], -1.0);
    assert!(samples[3] < 1.0 && samples[3] > 0.999);
}

#[test]
fn recordings_with_a_bad_rate_or_shape_or_length_are_refused() {
    let refused = |input: TranscribeSpeechInput| decode_samples(&input).unwrap_err().code;
    assert_eq!(
        refused(TranscribeSpeechInput {
            samples_base64: pcm(&[0; 4]),
            sample_rate: 96_000,
        }),
        "invalid_recording"
    );
    assert_eq!(
        refused(TranscribeSpeechInput {
            samples_base64: base64::engine::general_purpose::STANDARD.encode([1_u8, 2, 3]),
            sample_rate: 16_000,
        }),
        "invalid_recording"
    );
    assert_eq!(
        refused(TranscribeSpeechInput {
            samples_base64: "not base64!".into(),
            sample_rate: 16_000,
        }),
        "invalid_recording"
    );
    let too_long = vec![0_i16; (MAX_RECORDING_SECONDS * 8_000 + 1) as usize];
    assert_eq!(
        refused(TranscribeSpeechInput {
            samples_base64: pcm(&too_long),
            sample_rate: 8_000,
        }),
        "invalid_recording"
    );
}

#[test]
fn the_model_is_pinned_to_a_revision_and_hashes() {
    let model = &MOONSHINE_BASE_EN;
    assert_eq!(model.revision.len(), 40);
    for file in model.files() {
        assert_eq!(file.sha256.len(), 64, "{}", file.name);
        assert!(file.sha256.chars().all(|c| c.is_ascii_hexdigit()));
        let url = model.url(file);
        assert!(url.starts_with("https://huggingface.co/"), "{url}");
        assert!(
            url.contains(model.revision) && url.ends_with(file.name),
            "{url}"
        );
    }
    assert_eq!(model.download_bytes(), 141_300_566);
}

#[test]
fn a_library_without_the_model_reports_what_the_first_use_downloads() {
    let paths = test_paths("status");
    let status = status(&paths);
    assert_eq!(status.supported, SUPPORTED);
    assert!(!status.installed);
    assert_eq!(status.download_bytes, MOONSHINE_BASE_EN.download_bytes());
}

/// Serves each request's path from `files` once per connection, answering 404
/// for anything else.
fn serve(files: Vec<(&'static str, Vec<u8>)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut request = [0_u8; 2048];
            let read = stream.read(&mut request).unwrap_or(0);
            let line = String::from_utf8_lossy(&request[..read]).to_string();
            let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
            let body = files
                .iter()
                .find(|(name, _)| path.ends_with(&format!("/{name}")))
                .map(|(_, body)| body.clone());
            let response = match body {
                Some(body) => {
                    let mut response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .into_bytes();
                    response.extend(body);
                    response
                }
                None => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    .to_vec(),
            };
            let _ = stream.write_all(&response);
        }
    });
    format!("http://{address}")
}

fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

fn small_model(id: &'static str, bodies: [&[u8]; 3]) -> SpeechInputModel {
    let file = |name: &'static str, body: &[u8]| ModelFile {
        name,
        sha256: leak(format!("{:x}", Sha256::digest(body))),
        bytes: body.len() as u64,
    };
    SpeechInputModel {
        id,
        label: "Test",
        repository: "example/test",
        revision: "0000000000000000000000000000000000000000",
        encoder: file("encoder.ort", bodies[0]),
        decoder: file("decoder.ort", bodies[1]),
        tokens: file("tokens.txt", bodies[2]),
    }
}

#[test]
fn a_model_whose_files_match_their_hashes_is_installed() {
    let paths = test_paths("install");
    let model = small_model("good", [b"encoder", b"decoder", b"tokens"]);
    let base = serve(vec![
        ("encoder.ort", b"encoder".to_vec()),
        ("decoder.ort", b"decoder".to_vec()),
        ("tokens.txt", b"tokens".to_vec()),
    ]);
    let locate = |url: String| format!("{base}/{}", url.rsplit('/').next().unwrap());
    tauri::async_runtime::block_on(install::download(
        &install::client().unwrap(),
        &paths,
        &model,
        locate,
    ))
    .unwrap();
    assert!(install::is_installed(&paths, &model));
    let directory = install::model_directory(&paths, &model);
    assert_eq!(
        std::fs::read(directory.join("tokens.txt")).unwrap(),
        b"tokens"
    );
}

#[test]
fn a_file_that_does_not_match_its_hash_installs_nothing() {
    let paths = test_paths("tampered");
    let model = small_model("tampered", [b"encoder", b"decoder", b"tokens"]);
    let base = serve(vec![
        ("encoder.ort", b"encoder".to_vec()),
        ("decoder.ort", b"DECODER".to_vec()),
        ("tokens.txt", b"tokens".to_vec()),
    ]);
    let locate = |url: String| format!("{base}/{}", url.rsplit('/').next().unwrap());
    let error = tauri::async_runtime::block_on(install::download(
        &install::client().unwrap(),
        &paths,
        &model,
        locate,
    ))
    .unwrap_err();
    assert_eq!(error.code, "speech_model_download_failed");
    assert!(!install::is_installed(&paths, &model));
    let directory = install::model_directory(&paths, &model);
    assert!(!directory.exists());
    assert!(!directory.with_extension("partial").exists());
}

#[test]
fn a_missing_file_installs_nothing() {
    let paths = test_paths("missing");
    let model = small_model("missing", [b"encoder", b"decoder", b"tokens"]);
    let base = serve(vec![("encoder.ort", b"encoder".to_vec())]);
    let locate = |url: String| format!("{base}/{}", url.rsplit('/').next().unwrap());
    assert!(tauri::async_runtime::block_on(install::download(
        &install::client().unwrap(),
        &paths,
        &model,
        locate
    ))
    .is_err());
    assert!(!install::is_installed(&paths, &model));
}

/// The samples of a 16-bit mono PCM WAV file.
fn read_wav(bytes: &[u8]) -> (u32, Vec<f32>) {
    let sample_rate = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
    let mut offset = 12;
    while &bytes[offset..offset + 4] != b"data" {
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
        offset += 8 + size as usize;
    }
    let samples = bytes[offset + 8..]
        .chunks_exact(2)
        .map(|pair| f32::from(i16::from_le_bytes([pair[0], pair[1]])) / 32_768.0)
        .collect();
    (sample_rate, samples)
}

/// Recognizes a spoken fixture with the real model and checks the wait after
/// the recording ends. The model is not part of the repository, so this runs
/// only when `OSTUDY_SPEECH_MODEL_DIR` names a folder holding its files.
#[test]
fn a_spoken_question_is_recognized_quickly_when_the_model_is_present() {
    let Ok(directory) = std::env::var("OSTUDY_SPEECH_MODEL_DIR") else {
        return;
    };
    if !SUPPORTED {
        return;
    }
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/speech/question.wav");
    let (sample_rate, samples) = read_wav(&std::fs::read(fixture).unwrap());
    assert_eq!(sample_rate, 16_000);
    let directory = std::path::PathBuf::from(directory);
    // The first call loads the model; the second is what a learner waits for.
    recognize(&directory, sample_rate, &samples).unwrap();
    let started = std::time::Instant::now();
    let text = recognize(&directory, sample_rate, &samples).unwrap();
    let elapsed = started.elapsed();
    let lower = text.to_lowercase();
    assert!(lower.contains("ask not what your country"), "{text}");
    assert!(elapsed.as_millis() < 1_000, "{elapsed:?} for: {text}");
}
