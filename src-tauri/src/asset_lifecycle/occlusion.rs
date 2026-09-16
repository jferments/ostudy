//! Native image-occlusion authoring adapters. Decode/encode work runs outside
//! both the WebView thread and the database lock. The existing staging ledger
//! owns normalized PNG files and retry receipts; previews never persist images.

use std::{io::Write, path::Component};

use tauri::{AppHandle, Manager};
use tokio::sync::Semaphore;

use super::*;
use crate::cards::questions::{
    occlusion::{CanonicalImage, RenderPhase},
    ValidatedQuestion,
};

mod preparation;
#[cfg(test)]
mod tests;

static IMAGE_WORKER: Semaphore = Semaphore::const_new(1);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparedOcclusionImage {
    pub asset: AssetDto,
    pub width: u32,
    pub height: u32,
    pub pixel_fingerprint: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewOcclusionInput {
    pub definition_json: String,
    pub presentation_key: String,
    pub asset_draft_id: Option<String>,
    pub revealed: bool,
    pub maximum_edge: u32,
}

#[tauri::command]
pub async fn prepare_occlusion_image(
    app: AppHandle,
    input: ImportImageInput,
) -> Result<PreparedOcclusionImage, CommandError> {
    run_image_worker(app, move |app| {
        preparation::prepare(&app.state::<Database>(), &app.state::<AppPaths>(), input)
    })
    .await
}

/// Authoring preview accepts a complete editable definition. Practice never
/// calls this route: its image route resolves the frozen instance and the
/// permitted reveal phase from SQLite rather than trusting a client Boolean.
#[tauri::command]
pub async fn preview_occlusion_image(
    app: AppHandle,
    input: PreviewOcclusionInput,
) -> Result<tauri::ipc::Response, CommandError> {
    run_image_worker(app, move |app| {
        let bytes = preview(&app.state::<Database>(), &app.state::<AppPaths>(), input)?;
        Ok(tauri::ipc::Response::new(bytes))
    })
    .await
}

pub(crate) async fn run_image_worker<T, F>(app: AppHandle, work: F) -> Result<T, CommandError>
where
    T: Send + 'static,
    F: FnOnce(AppHandle) -> Result<T, CommandError> + Send + 'static,
{
    // Admit before spawning or allocating pixel buffers. No unbounded queue of
    // blocking decoders, and no simultaneous full-resolution mobile decodes.
    let permit = IMAGE_WORKER.try_acquire().map_err(|_| {
        CommandError::new(
            "occlusion_worker_busy",
            "Another image is being prepared. Retry when it finishes.",
        )
    })?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        work(app)
    })
    .await
    .map_err(|_| {
        CommandError::new(
            "occlusion_worker_interrupted",
            "Image preparation stopped before it could return a result. Retry the same operation.",
        )
    })?
}

fn preview(
    database: &Database,
    paths: &AppPaths,
    input: PreviewOcclusionInput,
) -> Result<Vec<u8>, CommandError> {
    if let Some(draft_id) = &input.asset_draft_id {
        validate_entity_id(draft_id, "asset draft")?;
    }
    let question = ValidatedQuestion::parse(&input.definition_json)?;
    let plan = question.occlusion_mask_plan(&input.presentation_key)?;
    let file = database.with_connection(|connection| {
        open_managed_image(
            connection,
            paths,
            &plan.image_asset_id,
            input.asset_draft_id.as_deref(),
        )
    })?;
    let image = CanonicalImage::decode(&read_image(file)?)?;
    let phase = if input.revealed {
        RenderPhase::Revealed
    } else {
        RenderPhase::Hidden
    };
    Ok(image
        .render(
            &question,
            &input.presentation_key,
            phase,
            input.maximum_edge,
        )?
        .png)
}

/// Resolve and open under the ledger lock; pixel bytes are read after releasing
/// it. Only a ready asset or a staged asset belonging to the admitted draft can
/// be opened. No client-authored filesystem path enters this boundary.
pub(crate) fn open_managed_image(
    connection: &Connection,
    paths: &AppPaths,
    asset_id: &str,
    draft_id: Option<&str>,
) -> Result<File, CommandError> {
    validate_entity_id(asset_id, "asset")?;
    crate::assets::ensure_assets_exist(connection, &[asset_id.to_string()])?;
    let asset = load_asset(connection, asset_id)?;
    let relative = match asset.lifecycle_state.as_str() {
        "ready" => asset.relative_path,
        "staged" if asset.draft_id.as_deref() == draft_id && draft_id.is_some() => {
            ensure_active_draft(
                connection,
                draft_id.unwrap_or_default(),
                unix_time_millis()?,
            )?;
            asset.staged_relative_path.ok_or_else(unavailable)?
        }
        _ => return Err(unavailable()),
    };
    if !relative.starts_with("assets/")
        || !Path::new(&relative)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(unavailable());
    }
    let file = File::open(paths.resolve_relative(&relative)).map_err(CommandError::io)?;
    let metadata = file.metadata().map_err(CommandError::io)?;
    if !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_IMAGE_BYTES
        || i64::try_from(metadata.len()).ok() != Some(asset.byte_size)
    {
        return Err(unavailable());
    }
    Ok(file)
}

pub(crate) fn read_image(file: File) -> Result<Vec<u8>, CommandError> {
    let expected = file.metadata().map_err(CommandError::io)?.len();
    if expected == 0 || expected > MAX_IMAGE_BYTES {
        return Err(unavailable());
    }
    let mut bytes = Vec::new();
    file.take(MAX_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(CommandError::io)?;
    if u64::try_from(bytes.len()).ok() != Some(expected) {
        return Err(unavailable());
    }
    Ok(bytes)
}

fn unavailable() -> CommandError {
    CommandError::new(
        "occlusion_image_unavailable",
        "The image is missing, changed, too large, or belongs to an unavailable draft.",
    )
}
