use tauri::State;

use crate::{database::Database, error::CommandError};

use super::{
    super::{
        catalog::{provider_catalog, ProviderCatalogEntryDto},
        AiRuntimeState,
    },
    contracts::{
        AiProviderProfileDetailDto, AiProviderProfileSummaryDto, AiTaskDefaultsDto,
        ClearAiProviderCredentialInput, CreateAiProviderProfileInput, DeleteAiProviderProfileInput,
        DeleteAiProviderProfileResultDto, SetAiProviderCredentialInput, SetAiTaskDefaultInput,
        UpdateAiProviderProfileInput,
    },
    openrouter_sign_in::{
        self, OpenRouterSignInAttemptInput, OpenRouterSignInStartDto, SaveOpenRouterSignInInput,
    },
    service,
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

/// Starts one-click OpenRouter sign-in; the page it returns is opened in the browser.
#[tauri::command]
pub fn start_openrouter_sign_in(
    runtime: State<'_, AiRuntimeState>,
) -> Result<OpenRouterSignInStartDto, CommandError> {
    openrouter_sign_in::start(runtime.inner())
}

/// Waits for the browser to return and exchanges its code for a key. It
/// touches no library data, so it holds no library lease while the learner
/// is in the browser.
#[tauri::command]
pub async fn wait_for_openrouter_sign_in(
    runtime: State<'_, AiRuntimeState>,
    input: OpenRouterSignInAttemptInput,
) -> Result<(), CommandError> {
    openrouter_sign_in::wait(runtime.inner(), input).await
}

#[tauri::command]
pub fn save_openrouter_sign_in(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: SaveOpenRouterSignInInput,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    openrouter_sign_in::save(database.inner(), runtime.inner(), input)
}

#[tauri::command]
pub fn cancel_openrouter_sign_in(
    runtime: State<'_, AiRuntimeState>,
    input: OpenRouterSignInAttemptInput,
) {
    openrouter_sign_in::cancel(runtime.inner(), input)
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

#[tauri::command]
pub fn get_embedding_options(
    database: State<'_, Database>,
    profile_id: String,
    model_id: String,
) -> Result<super::embedding_options::EmbeddingOptions, CommandError> {
    super::embedding_options::load_embedding_options(database.inner(), &profile_id, &model_id)
}

#[tauri::command]
pub fn save_embedding_options(
    database: State<'_, Database>,
    input: super::embedding_options::SaveEmbeddingOptionsInput,
) -> Result<super::embedding_options::EmbeddingOptions, CommandError> {
    super::embedding_options::save_embedding_options(database.inner(), input)
}
