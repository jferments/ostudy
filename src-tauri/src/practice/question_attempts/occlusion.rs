//! Session-authorized image pixels and self-assessed reveal. An authoring preview
//! can accept a draft definition; this route cannot. It selects image, target,
//! geometry and phase exclusively from the captured native attempt.

use tauri::{AppHandle, Manager};

use super::*;
use crate::{
    asset_lifecycle::occlusion::{open_managed_image, read_image, run_image_worker},
    cards::questions::{
        occlusion::{CanonicalImage, RenderPhase, MAX_RENDER_EDGE},
        QuestionInstance, RevealAnswerRequest,
    },
    paths::AppPaths,
};

#[path = "occlusion_cache.rs"]
mod cache;
#[cfg(test)]
#[path = "occlusion_tests.rs"]
mod tests;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevealPracticeQuestionInput {
    pub target: AttemptIdentityInput,
    pub request: RevealAnswerRequest,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OcclusionImageInput {
    pub target: AttemptIdentityInput,
    pub expected_revision: i64,
    pub maximum_edge: u32,
}

#[tauri::command]
pub fn reveal_practice_question_answer(
    database: State<'_, Database>,
    input: RevealPracticeQuestionInput,
) -> Result<AttemptView, CommandError> {
    database.with_connection(|connection| reveal(connection, input))
}

#[tauri::command]
pub async fn read_practice_occlusion_image(
    app: AppHandle,
    input: OcclusionImageInput,
) -> Result<tauri::ipc::Response, CommandError> {
    validate_image_input(&input)?;
    run_image_worker(app, move |app| {
        let bytes = render(&app.state::<Database>(), &app.state::<AppPaths>(), &input)?;
        Ok(tauri::ipc::Response::new(bytes))
    })
    .await
}

pub(super) fn reveal(
    connection: &mut Connection,
    input: RevealPracticeQuestionInput,
) -> Result<AttemptView, CommandError> {
    validate_target(&input.target)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let stored = storage::load(&transaction, &input.target)?;
    let mut attempt = stored.attempt.clone();
    let now = unix_time_millis()?;
    let view = attempt.reveal_answer(input.request, now)?;
    if attempt.revision() != stored.attempt.revision() {
        stored.require_mutable()?;
        storage::persist(&transaction, &stored, &attempt, now)?;
        mark_question_exposed(&transaction, &input.target, now)?;
    }
    // Build the response before durable success. Feedback assets are reconciled
    // by a separate read; an image or media error cannot undo a committed reveal.
    transaction.commit().map_err(CommandError::database)?;
    Ok(view)
}

fn validate_image_input(input: &OcclusionImageInput) -> Result<(), CommandError> {
    validate_target(&input.target)?;
    if input.expected_revision < 1 || !(1..=MAX_RENDER_EDGE).contains(&input.maximum_edge) {
        return Err(CommandError::new(
            "invalid_occlusion_request",
            "Use the captured question revision and a supported image size.",
        ));
    }
    Ok(())
}

fn authorized_source(
    connection: &Connection,
    paths: &AppPaths,
    input: &OcclusionImageInput,
) -> Result<(QuestionInstance, RenderPhase, std::fs::File), CommandError> {
    let stored = storage::load(connection, &input.target)?;
    stored.require_available()?;
    if stored.attempt.revision() != input.expected_revision {
        return Err(conflict());
    }
    let instance = stored.attempt.instance();
    let plan = instance.occlusion_mask_plan()?;
    let retained: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM question_attempt_assets
             WHERE instance_id = ?1 AND asset_id = ?2)",
            params![input.target.instance_id, plan.image_asset_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if !retained {
        return Err(invalid_storage());
    }
    let phase = match stored.attempt.view()?.progress {
        AttemptProgress::Answering { .. } => RenderPhase::Hidden,
        AttemptProgress::Revealed { .. } => RenderPhase::Revealed,
        AttemptProgress::Checked { .. } => return Err(invalid_storage()),
    };
    let file = open_managed_image(connection, paths, &plan.image_asset_id, None)?;
    Ok((instance.clone(), phase, file))
}

pub(super) fn render(
    database: &Database,
    paths: &AppPaths,
    input: &OcclusionImageInput,
) -> Result<Vec<u8>, CommandError> {
    validate_image_input(input)?;
    let (instance, phase, file) = database
        .with_connection(|connection| authorized_source(connection, paths, input))?;
    // Do not hold SQLite's mutex while reading/decoding/encoding large images.
    let source = read_image(file)?;
    let key = cache::key(&instance, &source, phase, input.maximum_edge);
    let bytes = if let Some(bytes) = cache::get(&key) {
        bytes
    } else {
        let image = CanonicalImage::decode(&source)?;
        std::sync::Arc::new(instance.render_occlusion(&image, phase, input.maximum_edge)?.png)
    };
    // A reveal, deletion or library change while rendering must not acknowledge
    // pixels under an obsolete revision. Cache hits need the same authorization.
    database.with_connection(|connection| {
        let (current, current_phase, _file) = authorized_source(connection, paths, input)?;
        if current.fingerprint() != instance.fingerprint() || current_phase != phase {
            return Err(conflict());
        }
        Ok(())
    })?;
    cache::insert(key, bytes.clone());
    Ok(bytes.as_ref().clone())
}
