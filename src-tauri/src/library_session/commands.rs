//! Async IPC leases: admission uses the request's original generation and lasts
//! through the whole future. Domain implementations and wire command names are
//! unchanged; unique Rust function names keep Tauri's macro symbols unambiguous.

use tauri::{AppHandle, Manager};

use crate::error::CommandError;

#[tauri::command(rename = "list_embedding_models")]
pub(crate) async fn leased_list_embedding_models(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::ai::embeddings::list_embedding_models(
        app.state::<crate::database::Database>(),
        app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "test_embedding_profile")]
pub(crate) async fn leased_test_embedding_profile(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::ai::embeddings::test_embedding_profile(
        app.state::<crate::database::Database>(),
        app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "list_ai_provider_models")]
pub(crate) async fn leased_list_ai_provider_models(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::ai::probes::commands::list_ai_provider_models(
        app.state::<crate::database::Database>(),
        app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "test_ai_provider_profile")]
pub(crate) async fn leased_test_ai_provider_profile(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::ai::probes::commands::test_ai_provider_profile(
        app.state::<crate::database::Database>(),
        app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "send_tutor_turn")]
pub(crate) async fn leased_send_tutor_turn(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::tutor::commands::send_tutor_turn(
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "draft_mark_question")]
pub(crate) async fn leased_draft_mark_question(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::annotation_cards::draft_mark_question(
        app.state::<crate::database::Database>().inner(),
        app.state::<crate::ai::AiRuntimeState>().inner(),
        input,
    )
    .await
}

#[tauri::command(rename = "preview_semantic_preparation")]
pub(crate) async fn leased_preview_semantic_preparation(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::knowledge::commands::preview_semantic_preparation(app.clone(), input).await
}

#[tauri::command(rename = "prepare_semantic_material")]
pub(crate) async fn leased_prepare_semantic_material(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::knowledge::commands::prepare_semantic_material(
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "search_semantic_material")]
pub(crate) async fn leased_search_semantic_material(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::knowledge::commands::search_semantic_material(
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "refresh_collection_synthesis")]
pub(crate) async fn leased_refresh_collection_synthesis(
    app: AppHandle,
    collection_id: String,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    crate::knowledge::commands::refresh_collection_synthesis(app.clone(), collection_id).await
}

#[tauri::command(rename = "prepare_collection_synthesis")]
pub(crate) async fn leased_prepare_collection_synthesis(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::knowledge::commands::prepare_collection_synthesis(
        app.state::<crate::database::Database>(),
        app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "inspect_image_set")]
pub(crate) async fn leased_inspect_image_set(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::documents::commands::inspect_image_set(app.clone(), input).await
}

#[tauri::command(rename = "import_image_set")]
pub(crate) async fn leased_import_image_set(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::documents::commands::import_image_set(app.clone(), input).await
}

#[tauri::command(rename = "get_image_set")]
pub(crate) async fn leased_get_image_set(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::documents::commands::get_image_set(app.clone(), input).await
}

#[tauri::command(rename = "get_image_set_preview")]
pub(crate) async fn leased_get_image_set_preview(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::documents::commands::get_image_set_preview(app.clone(), input).await
}

#[tauri::command(rename = "prepare_image_set")]
pub(crate) async fn leased_prepare_image_set(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::documents::commands::prepare_image_set(app.clone(), input).await
}

#[tauri::command(rename = "set_image_set_filters")]
pub(crate) async fn leased_set_image_set_filters(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::documents::commands::set_image_set_filters(app.clone(), input).await
}

#[tauri::command(rename = "amend_image_set")]
pub(crate) async fn leased_amend_image_set(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::documents::commands::amend_image_set(app.clone(), input).await
}

#[tauri::command(rename = "run_document_flashcard_generation")]
pub(crate) async fn leased_run_document_flashcard_generation(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::document_operations::run_document_flashcard_generation(
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        app.state::<crate::ai::AiRuntimeState>(),
        app.state::<crate::pdf_render_backend::PdfRenderRuntime>(),
        input,
    )
    .await
}

#[tauri::command(rename = "generate_more_pdf_flashcards")]
pub(crate) async fn leased_generate_more_pdf_flashcards(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::document_more::generate_more_pdf_flashcards(
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "generate_document_study_guide")]
pub(crate) async fn leased_generate_document_study_guide(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::document_study_guide_commands::generate_document_study_guide(
        app.clone(),
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "read_document_source_asset")]
pub(crate) async fn leased_read_document_source_asset(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::document_study_guide_commands::read_document_source_asset(
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        input,
    )
    .await
}

#[tauri::command(rename = "render_document_page_tile")]
pub(crate) async fn leased_render_document_page_tile(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input: crate::document_annotations::RenderDocumentPageTileInput =
        serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::document_annotations::render_document_page_tile(
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        app.state::<crate::pdf_render_backend::PdfRenderRuntime>(),
        input,
    )
    .await
}

#[tauri::command(rename = "find_document_figures")]
pub(crate) async fn leased_find_document_figures(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<crate::document_figures::DocumentFiguresDto, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input: crate::document_figures::DocumentFiguresInput =
        serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::document_figures::find_document_figures(
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        app.state::<crate::pdf_render_backend::PdfRenderRuntime>(),
        input,
    )
    .await
}

#[tauri::command(rename = "read_document_figures")]
pub(crate) async fn leased_read_document_figures(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<crate::document_figures::DocumentFiguresDto, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input: crate::document_figures::DocumentFiguresInput =
        serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::document_figures::read_document_figures(
        app.state::<crate::database::Database>().inner(),
        app.state::<crate::paths::AppPaths>().inner(),
        app.state::<crate::pdf_render_backend::PdfRenderRuntime>()
            .inner(),
        app.state::<crate::ai::AiRuntimeState>().inner(),
        input,
    )
    .await
}

#[tauri::command(rename = "prepare_annotated_pdf")]
pub(crate) async fn leased_prepare_annotated_pdf(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<crate::document_annotations::PreparedAnnotatedPdfDto, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input: crate::document_annotations::PrepareAnnotatedPdfInput =
        serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::document_annotations::prepare_annotated_pdf(
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        input,
    )
    .await
}

#[tauri::command(rename = "prepare_notebook_pdf")]
pub(crate) async fn leased_prepare_notebook_pdf(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<crate::notebooks::PreparedNotebookPdfDto, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input: crate::notebooks::PrepareNotebookPdfInput =
        serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::notebooks::prepare_notebook_pdf(
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        input,
    )
    .await
}

#[tauri::command(rename = "get_document_page_text")]
pub(crate) async fn leased_get_document_page_text(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<crate::document_annotations::DocumentPageTextDto, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input: crate::document_annotations::GetDocumentPageTextInput =
        serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::document_annotations::get_document_page_text(
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        app.state::<crate::pdf_render_backend::PdfRenderRuntime>(),
        input,
    )
    .await
}

#[tauri::command(rename = "stream_select_and_stage_external_import")]
pub(crate) async fn leased_stream_select_and_stage_external_import(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::external_file_streaming::stream_select_and_stage_external_import(
        app.clone(),
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        app.state::<crate::platform::PlatformRuntime>(),
        input,
    )
    .await
}

#[tauri::command(rename = "stream_publish_prepared_export")]
pub(crate) async fn leased_stream_publish_prepared_export(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::external_file_streaming::stream_publish_prepared_export(
        app.clone(),
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        app.state::<crate::platform::PlatformRuntime>(),
        input,
    )
    .await
}

#[tauri::command(rename = "share_prepared_export")]
pub(crate) async fn leased_share_prepared_export(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::external_file_commands::share_prepared_export(
        app.clone(),
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        app.state::<crate::platform::PlatformRuntime>(),
        input,
    )
    .await
}

#[tauri::command(rename = "stage_image_collection_file_chunk")]
pub(crate) async fn leased_stage_image_collection_file_chunk(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::external_file_images::stage_image_collection_file_chunk(
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        input,
    )
    .await
}

#[tauri::command(rename = "stage_image_collection_file")]
pub(crate) async fn leased_stage_image_collection_file(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::external_file_images::stage_image_collection_file(
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        input,
    )
    .await
}

#[tauri::command(rename = "import_media")]
pub(crate) async fn leased_import_media(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::media_assets::import_media(
        app.clone(),
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        app.state::<crate::platform::PlatformRuntime>(),
        input,
    )
    .await
}

#[tauri::command(rename = "read_media_asset")]
pub(crate) async fn leased_read_media_asset(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::media_assets::read_media_asset(
        app.state::<crate::database::Database>(),
        app.state::<crate::paths::AppPaths>(),
        input,
    )
    .await
}

#[tauri::command(rename = "prepare_occlusion_image")]
pub(crate) async fn leased_prepare_occlusion_image(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::asset_lifecycle::occlusion::prepare_occlusion_image(app.clone(), input).await
}

#[tauri::command(rename = "preview_occlusion_image")]
pub(crate) async fn leased_preview_occlusion_image(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::asset_lifecycle::occlusion::preview_occlusion_image(app.clone(), input).await
}

#[tauri::command(rename = "read_practice_occlusion_image")]
pub(crate) async fn leased_read_practice_occlusion_image(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::practice::question_attempts::occlusion::read_practice_occlusion_image(app.clone(), input)
        .await
}

#[tauri::command(rename = "import_deck")]
pub(crate) async fn leased_import_deck(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::studydeck::import_deck(app.clone(), input).await
}

#[tauri::command(rename = "join_device_pairing")]
pub(crate) async fn leased_join_device_pairing(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::device_sync::join_device_pairing(
        app.clone(),
        app.state::<crate::paths::AppPaths>(),
        app.state::<crate::platform::PlatformRuntime>(),
        input,
    )
    .await
}

#[tauri::command(rename = "reject_usb_device_pairing")]
pub(crate) async fn leased_reject_usb_device_pairing(
    app: AppHandle,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    crate::device_sync::reject_usb_device_pairing(app.clone()).await
}

#[tauri::command(rename = "scan_device_pairing_qr_async")]
pub(crate) async fn leased_scan_device_pairing_qr_async(
    app: AppHandle,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    crate::device_sync_qr_commands::scan_device_pairing_qr_async(app.clone()).await
}

#[tauri::command(rename = "compose_study_target_bundle")]
pub(crate) async fn leased_compose_study_target_bundle(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::study_materials::composition::compose_study_target_bundle(
        app.state::<crate::database::Database>(),
        app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "write_subject_course")]
pub(crate) async fn leased_write_subject_course(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::lessons::writing::write_subject_course(
        app.state::<crate::database::Database>(),
        app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "update_lesson_from_sources")]
pub(crate) async fn leased_update_lesson_from_sources(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::lessons::writing::update_lesson_from_sources(
        app.state::<crate::database::Database>(),
        app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "narrate_lesson_step")]
pub(crate) async fn leased_narrate_lesson_step(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::lessons::narration::narrate_lesson_step(
        &app.state::<crate::database::Database>(),
        &app.state::<crate::paths::AppPaths>(),
        &app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "try_speech_voice")]
pub(crate) async fn leased_try_speech_voice(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::lessons::voices::try_speech_voice(
        &app.state::<crate::database::Database>(),
        &app.state::<crate::paths::AppPaths>(),
        &app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "write_lesson_lecture_script")]
pub(crate) async fn leased_write_lesson_lecture_script(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::lessons::teaching::write_lesson_lecture_script(
        app.state::<crate::database::Database>(),
        app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "narrate_lecture_part")]
pub(crate) async fn leased_narrate_lecture_part(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::lessons::teaching::narrate_lecture_part(
        &app.state::<crate::database::Database>(),
        &app.state::<crate::paths::AppPaths>(),
        &app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "write_podcast")]
pub(crate) async fn leased_write_podcast(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::lessons::podcasts::write(
        &app.state::<crate::database::Database>(),
        &app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "narrate_podcast_turn")]
pub(crate) async fn leased_narrate_podcast_turn(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::lessons::podcasts::narrate_turn(
        &app.state::<crate::database::Database>(),
        &app.state::<crate::paths::AppPaths>(),
        &app.state::<crate::ai::AiRuntimeState>(),
        input,
    )
    .await
}

#[tauri::command(rename = "prepare_library_backup_async")]
pub(crate) async fn leased_prepare_library_backup_async(
    app: AppHandle,
    input: serde_json::Value,
    library_generation: Option<u64>,
) -> Result<impl tauri::ipc::IpcResponse, CommandError> {
    let _lease = super::enter(library_generation)?;
    let input = serde_json::from_value(input).map_err(CommandError::serialization)?;
    crate::library_backup_commands::prepare_library_backup_async(app.clone(), input).await
}
