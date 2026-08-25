use tauri::State;

use crate::{database::Database, error::CommandError, util::validate_entity_id};

use super::{
    super::AiRuntimeState,
    contracts::{AiProfileReferenceInput, AiProviderModelsDto, AiProviderProbeResultDto},
    repository, runner,
};

#[tauri::command]
pub async fn list_ai_provider_models(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: AiProfileReferenceInput,
) -> Result<AiProviderModelsDto, CommandError> {
    runner::list_models(database.inner(), runtime.inner(), input).await
}

#[tauri::command]
pub async fn test_ai_provider_profile(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: AiProfileReferenceInput,
) -> Result<AiProviderProbeResultDto, CommandError> {
    runner::test_profile(database.inner(), runtime.inner(), input).await
}

#[tauri::command]
pub fn get_ai_provider_probe_result(
    database: State<'_, Database>,
    profile_id: String,
) -> Result<Option<AiProviderProbeResultDto>, CommandError> {
    validate_entity_id(&profile_id, "AI provider profile")?;
    repository::load_probe_result(database.inner(), &profile_id)
}
