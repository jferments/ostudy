use tauri::State;

use crate::{database::Database, error::CommandError};

use super::{
    contracts::{
        AiProviderProfileDetailDto, AiProviderProfileSummaryDto, AiTaskDefaultsDto,
        ClearAiProviderCredentialInput, CreateAiProviderProfileInput,
        DeleteAiProviderProfileInput, DeleteAiProviderProfileResultDto,
        SetAiProviderCredentialInput, SetAiTaskDefaultInput, UpdateAiProviderProfileInput,
    },
    service,
    super::{catalog::{provider_catalog, ProviderCatalogEntryDto}, AiRuntimeState},
};

#[tauri::command]
pub fn get_ai_provider_catalog() -> Vec<ProviderCatalogEntryDto> {
    provider_catalog()
}

#[tauri::command]
pub fn list_ai_provider_profiles(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
) -> Result<Vec<AiProviderProfileSummaryDto>, CommandError> {
    service::list_profiles(database.inner(), runtime.inner())
}

#[tauri::command]
pub fn get_ai_provider_profile(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    profile_id: String,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    service::get_profile(database.inner(), runtime.inner(), &profile_id)
}

#[tauri::command]
pub fn create_ai_provider_profile(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: CreateAiProviderProfileInput,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    service::create_profile(database.inner(), runtime.inner(), input)
}

#[tauri::command]
pub fn update_ai_provider_profile(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: UpdateAiProviderProfileInput,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    service::update_profile(database.inner(), runtime.inner(), input)
}

#[tauri::command]
pub fn delete_ai_provider_profile(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: DeleteAiProviderProfileInput,
) -> Result<DeleteAiProviderProfileResultDto, CommandError> {
    service::delete_profile(database.inner(), runtime.inner(), input)
}

#[tauri::command]
pub fn set_ai_provider_credential(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: SetAiProviderCredentialInput,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    service::set_credential(database.inner(), runtime.inner(), input)
}

#[tauri::command]
pub fn clear_ai_provider_credential(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: ClearAiProviderCredentialInput,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    service::clear_credential(database.inner(), runtime.inner(), input)
}

#[tauri::command]
pub fn get_ai_task_defaults(
    database: State<'_, Database>,
) -> Result<AiTaskDefaultsDto, CommandError> {
    service::get_task_defaults(database.inner())
}

#[tauri::command]
pub fn set_ai_task_default(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: SetAiTaskDefaultInput,
) -> Result<AiTaskDefaultsDto, CommandError> {
    service::set_task_default(database.inner(), runtime.inner(), input)
}
