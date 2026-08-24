use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetryPdfGenerationInput {
    pub document_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetDocumentArchivedInput {
    pub document_id: String,
    pub revision: i64,
    pub archived: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDto {
    pub id: String,
    pub title: String,
    pub original_filename: String,
    pub byte_size: i64,
    pub sha256: String,
    pub lifecycle_state: String,
    pub source_format: String,
    pub source_unit_kind: String,
    pub source_unit_count: i64,
    pub page_count: i64,
    pub extracted_page_count: i64,
    pub rendered_page_count: i64,
    pub extraction_mode: String,
    pub ocr_page_count: i64,
    pub ocr_completed_page_count: i64,
    pub ocr_mean_confidence: Option<f64>,
    pub ocr_engine: String,
    pub scale_mode: String,
    pub estimated_source_tokens: i64,
    pub outline_window_count: i64,
    pub outline_completed_window_count: i64,
    pub generation_window_count: i64,
    pub generation_completed_window_count: i64,
    pub generated_note_count: i64,
    pub generated_card_count: i64,
    pub generated_deck_id: Option<String>,
    pub safe_error_code: String,
    pub safe_error_message: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
    pub archived_at: Option<i64>,
    pub revision: i64,
    pub processing_duration_ms: Option<i64>,
    pub api_token_count: Option<i64>,
    pub api_token_usage_complete: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentGenerationResultDto {
    pub document: DocumentDto,
    pub deck_id: String,
    pub created_note_count: i64,
    pub created_card_count: i64,
    pub skipped_duplicate_count: i64,
}
