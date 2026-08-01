use std::collections::{BTreeSet, HashSet};

use rusqlite::{
    params, params_from_iter,
    types::Value as SqlValue,
    Connection, OptionalExtension, Transaction, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;

use crate::{
    assets::{ensure_assets_exist, AssetDto},
    cards::{get_card_from_connection, validate_content, CardDto, ValidatedContent},
    database::Database,
    error::CommandError,
    paths::AppPaths,
    scheduler::insert_new_schedule,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_TAGS_PER_NOTE: usize = 32;
const MAX_TAG_CHARS: usize = 64;
const MAX_SEARCH_QUERY_CHARS: usize = 500;
const MAX_SEARCH_TAGS: usize = 32;
const MAX_SEARCH_DECKS: usize = 100;
const MAX_SEARCH_LIMIT: i64 = 200;
const MAX_SEARCH_SELECTION: usize = 10_000;
const MAX_BULK_ROWS: usize = 500;
const MAX_BULK_ACTION_NOTES: usize = 10_000;
const MAX_TRANSFER_NOTES: usize = 10_000;
const TAG_SEPARATOR: char = '\u{001f}';

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDto {
    pub id: String,
    pub name: String,
    pub card_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizedCardDto {
    pub id: String,
    pub note_id: String,
    pub deck_id: String,
    pub direction: String,
    pub front_doc: Value,
    pub back_doc: Value,
    pub front_text: String,
    pub back_text: String,
    pub assets: Vec<AssetDto>,
    pub tags: Vec<String>,
    pub due_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardBrowserRowDto {
    pub card_id: String,
    pub note_id: String,
    pub deck_id: String,
    pub deck_name: String,
    pub front_text: String,
    pub back_text: String,
    pub tags: Vec<String>,
    pub due_at: i64,
    pub last_reviewed_at: Option<i64>,
    pub review_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardSearchPageDto {
    pub cards: Vec<CardBrowserRowDto>,
    pub total_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionCardDto {
    pub card_id: String,
    pub note_id: String,
    pub deck_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTagsInput {
    #[serde(default)]
    pub deck_ids: Vec<String>,
    #[serde(default)]
    pub deck_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchCardsInput {
    #[serde(default)]
    pub deck_ids: Vec<String>,
    #[serde(default)]
    pub deck_id: Option<String>,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    #[serde(default = "default_due_filter")]
    pub due_filter: String,
    #[serde(default = "default_reviewed_filter")]
    pub reviewed_filter: String,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrganizedCardInput {
    pub deck_id: String,
    pub front_doc: Value,
    pub back_doc: Value,
    pub front_text: String,
    pub back_text: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub allow_duplicate: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOrganizedCardInput {
    pub note_id: String,
    pub deck_id: String,
    pub revision: i64,
    pub front_doc: Value,
    pub back_doc: Value,
    pub front_text: String,
    pub back_text: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub allow_duplicate: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCheckInput {
    pub deck_id: String,
    pub front_text: String,
    pub back_text: String,
    pub exclude_note_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCardDto {
    pub note_id: String,
    pub deck_id: String,
    pub front_text: String,
    pub back_text: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCardRowInput {
    pub question: String,
    pub answer: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeBulkCardsInput {
    pub deck_id: String,
    pub rows: Vec<BulkCardRowInput>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkAnalyzeResultDto {
    pub unique_count: i64,
    pub duplicate_count: i64,
    pub duplicate_row_indices: Vec<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCreateCardsInput {
    pub deck_id: String,
    pub rows: Vec<BulkCardRowInput>,
    // Accepted only for backward-compatible IPC payloads. Bulk creation always
    // omits exact duplicates after the review step.
    #[serde(default = "default_true")]
    #[allow(dead_code)]
    pub skip_duplicates: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCreateResultDto {
    pub created_count: i64,
    pub skipped_duplicates: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferCardsInput {
    pub note_ids: Vec<String>,
    pub target_deck_id: String,
    pub mode: String,
    #[serde(default = "default_true")]
    pub skip_duplicates: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferCardsResultDto {
    pub affected_count: i64,
    pub skipped_duplicates: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkTagCardsInput {
    pub note_ids: Vec<String>,
    pub tags: Vec<String>,
    pub mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCardsInput {
    pub note_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkActionResultDto {
    pub affected_count: i64,
}

#[derive(Clone)]
struct ValidatedTag {
    name: String,
    normalized: String,
}

struct SourceNote {
    id: String,
    deck_id: String,
    front_doc: String,
    back_doc: String,
    front_text: String,
    back_text: String,
    duplicate_hash: String,
}

type PreparedBulkRow = (
    ValidatedContent,
    Vec<ValidatedTag>,
    (String, String),
    String,
);
