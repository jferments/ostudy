//! Downloading a speech model: the speech input model, or a voice that reads
//! aloud on the device. Each file is fetched from the pinned revision into a
//! staging folder, its size and SHA-256 are checked as it arrives, and the
//! folder is renamed into place only when every file matches.

use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use reqwest::{redirect::Policy, Client};
use sha2::{Digest, Sha256};

use super::{status, ModelFile, SpeechInputStatusDto, MOONSHINE_BASE_EN};
use crate::{error::CommandError, paths::AppPaths};

const MODELS_DIRECTORY: &str = "speech-models";
const STAGING_SUFFIX: &str = ".partial";
const CONNECT_TIMEOUT_SECONDS: u64 = 15;
/// A stalled download fails instead of waiting forever.
const READ_TIMEOUT_SECONDS: u64 = 60;

/// A model as published at a pinned revision of a Hugging Face repository.
pub(crate) struct PinnedModel<'a> {
    /// The folder it is installed in, under `speech-models/`.
    pub id: &'a str,
    pub repository: &'a str,
    /// The commit its files are read from.
    pub revision: &'a str,
    pub files: Vec<&'a ModelFile>,
}

impl PinnedModel<'_> {
    pub(crate) fn download_bytes(&self) -> u64 {
        self.files.iter().map(|file| file.bytes).sum()
    }

    pub(crate) fn url(&self, file: &ModelFile) -> String {
        format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            self.repository, self.revision, file.name
        )
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Progress {
    /// Bytes received so far while the model downloads.
    pub received: Option<u64>,
    /// Why the last download failed, until another one starts.
    pub error: Option<String>,
}

/// Each model's download, by model id.
static PROGRESS: Mutex<BTreeMap<String, Progress>> = Mutex::new(BTreeMap::new());

pub(crate) fn progress(model: &PinnedModel<'_>) -> Progress {
    PROGRESS
        .lock()
        .ok()
        .and_then(|progress| progress.get(model.id).cloned())
        .unwrap_or_default()
}

pub(crate) fn model_directory(paths: &AppPaths, model: &PinnedModel<'_>) -> PathBuf {
    paths.app_data_dir().join(MODELS_DIRECTORY).join(model.id)
}

/// Installed when every file is in place at its published size; the hashes
/// were checked when the files were downloaded.
pub(crate) fn is_installed(paths: &AppPaths, model: &PinnedModel<'_>) -> bool {
    let directory = model_directory(paths, model);
    model.files.iter().all(|file| {
        fs::metadata(directory.join(file.name))
            .map(|metadata| metadata.is_file() && metadata.len() == file.bytes)
            .unwrap_or(false)
    })
}

/// Downloads the speech input model unless it is already installed.
pub(crate) async fn install_speech_input_model(
    paths: &AppPaths,
) -> Result<SpeechInputStatusDto, CommandError> {
    if super::SUPPORTED {
        install(paths, &MOONSHINE_BASE_EN.pinned()).await?;
    }
    Ok(status(paths))
}

/// Downloads `model` unless it is already installed. A second call while its
/// download runs returns at once and leaves that download running.
pub(crate) async fn install(paths: &AppPaths, model: &PinnedModel<'_>) -> Result<(), CommandError> {
    if is_installed(paths, model) {
        return Ok(());
    }
    {
        let mut progress = PROGRESS
            .lock()
            .map_err(|_| install_error("the download lock was poisoned"))?;
        let current = progress.entry(model.id.to_string()).or_default();
        if current.received.is_some() {
            return Ok(());
        }
        *current = Progress {
            received: Some(0),
            error: None,
        };
    }
    let client = client()?;
    let result = download(&client, paths, model, |url| url).await;
    if let Ok(mut progress) = PROGRESS.lock() {
        progress.insert(
            model.id.to_string(),
            Progress {
                received: None,
                error: result.as_ref().err().map(|error| error.message.clone()),
            },
        );
    }
    result
}

pub(super) fn client() -> Result<Client, CommandError> {
    Client::builder()
        // Tests serve the files from a local plain-HTTP server.
        .https_only(!cfg!(test))
        // The pinned file address answers with a redirect to its storage.
        .redirect(Policy::limited(5))
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECONDS))
        .read_timeout(Duration::from_secs(READ_TIMEOUT_SECONDS))
        .user_agent(concat!("ostudy/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| install_error(&error.to_string()))
}

/// Fetches every file into a staging folder and moves it into place.
/// `locate` maps each file's published address to the one fetched, which
/// tests point at a local server.
pub(super) async fn download(
    client: &Client,
    paths: &AppPaths,
    model: &PinnedModel<'_>,
    locate: impl Fn(String) -> String,
) -> Result<(), CommandError> {
    let destination = model_directory(paths, model);
    let staging = destination.with_extension(STAGING_SUFFIX.trim_start_matches('.'));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(CommandError::io)?;
    let mut received = 0;
    for file in &model.files {
        let fetched = fetch(
            client,
            model.id,
            &locate(model.url(file)),
            file,
            &staging,
            received,
        )
        .await;
        if let Err(error) = fetched {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
        received += file.bytes;
    }
    let _ = fs::remove_dir_all(&destination);
    fs::rename(&staging, &destination).map_err(CommandError::io)?;
    Ok(())
}

async fn fetch(
    client: &Client,
    model_id: &str,
    url: &str,
    file: &ModelFile,
    staging: &Path,
    received_before: u64,
) -> Result<(), CommandError> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|error| install_error(&error.to_string()))?;
    if !response.status().is_success() {
        return Err(install_error(&format!(
            "{} answered {}",
            file.name,
            response.status()
        )));
    }
    let mut output = fs::File::create(staging.join(file.name)).map_err(CommandError::io)?;
    let mut hasher = Sha256::new();
    let mut length = 0_u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| install_error(&error.to_string()))?
    {
        length += chunk.len() as u64;
        if length > file.bytes {
            return Err(install_error(&format!(
                "{} is larger than published",
                file.name
            )));
        }
        hasher.update(&chunk);
        output.write_all(&chunk).map_err(CommandError::io)?;
        if let Ok(mut progress) = PROGRESS.lock() {
            if let Some(current) = progress.get_mut(model_id) {
                current.received = Some(received_before + length);
            }
        }
    }
    output.sync_all().map_err(CommandError::io)?;
    let digest = format!("{:x}", hasher.finalize());
    if length != file.bytes || digest != file.sha256 {
        return Err(install_error(&format!(
            "{} does not match its published hash",
            file.name
        )));
    }
    Ok(())
}

fn install_error(detail: &str) -> CommandError {
    crate::logging::warn_target(
        "speech_input",
        "speech_model_download_failed",
        "The speech model download failed.",
        serde_json::json!({ "detail": detail }),
    );
    CommandError::new(
        "speech_model_download_failed",
        "The speech model could not be downloaded. Check the connection and try again.",
    )
}
