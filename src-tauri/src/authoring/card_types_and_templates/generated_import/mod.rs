use super::*;

// Strict, direct import for LLM-generated `.ostudy.json` flashcard files. The
// importer reuses canonical authoring validation and persistence rather than
// maintaining a parallel card-storage path.

const GENERATED_IMPORT_FORMAT: &str = "ostudy-flashcards";
const LEGACY_GENERATED_IMPORT_VERSION: i64 = 1;
const GENERATED_IMPORT_VERSION: i64 = 2;
const SUPPORTED_GENERATED_IMPORT_VERSIONS: [i64; 2] =
    [LEGACY_GENERATED_IMPORT_VERSION, GENERATED_IMPORT_VERSION];

pub(crate) fn is_supported_generated_import_identity(format: &str, version: i64) -> bool {
    format == GENERATED_IMPORT_FORMAT && SUPPORTED_GENERATED_IMPORT_VERSIONS.contains(&version)
}

pub(crate) fn is_generated_import_envelope(value: &serde_json::Value) -> bool {
    value.get("format").and_then(serde_json::Value::as_str) == Some(GENERATED_IMPORT_FORMAT)
}

const MAX_GENERATED_IMPORT_BYTES: usize = 16 * 1024 * 1024;
const MAX_GENERATED_IMPORT_CARDS: usize = 10_000;
const MAX_GENERATED_IMPORT_ERROR_EXAMPLES: usize = 20;

pub(crate) mod question;
mod schema;
use schema::{GeneratedImportCard, GeneratedImportDeckMode, GeneratedImportFile};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardImportResultDto {
    pub deck_id: String,
    pub created_note_count: i64,
    pub created_card_count: i64,
    pub skipped_duplicate_count: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GeneratedImportCreatedNote {
    pub source_index: usize,
    pub note_id: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct GeneratedImportCommitContext {
    pub created_notes: Vec<GeneratedImportCreatedNote>,
}

enum PreparedImportContent {
    Recall(Box<PreparedGeneratedNote>),
    Interactive(Box<crate::cards::questions::ValidatedQuestion>),
}

struct PreparedImportNote {
    source_index: usize,
    prepared: PreparedImportContent,
    tags: Vec<ValidatedTag>,
    normalized: (String, String),
    duplicate_hash: String,
}

struct PreparedImport {
    mode: GeneratedImportDeckMode,
    name: String,
    description: String,
    notes: Vec<PreparedImportNote>,
}

mod apply;
mod prepare;
pub(crate) use prepare::validate_generated_card;

#[cfg(test)]
use apply::import_prepared_flashcards;
use apply::import_prepared_flashcards_with_commit;
#[cfg(test)]
use prepare::prepare_flashcard_import;
use prepare::prepare_flashcard_import_bytes;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportGeneratedFlashcardsInput {
    pub staged_file_id: String,
}

#[tauri::command]
pub fn import_generated_flashcards(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ImportGeneratedFlashcardsInput,
) -> Result<FlashcardImportResultDto, CommandError> {
    validate_entity_id(&input.staged_file_id, "staged generated flashcards")?;
    if let Some(result) = database.with_connection(|connection| {
        crate::external_files::consumed_import_result::<FlashcardImportResultDto>(
            connection,
            &input.staged_file_id,
        )
    })? {
        return Ok(result);
    }
    let staged = database.with_connection(|connection| {
        crate::external_files::resolve_staged_import(
            connection,
            paths.inner(),
            &input.staged_file_id,
            crate::external_files::ExternalImportPurpose::StudyContent,
            &[crate::external_files::ExternalFileKind::GeneratedJson],
        )
    })?;
    let bytes = std::fs::read(&staged.path).map_err(CommandError::io)?;
    let prepared = prepare_flashcard_import_bytes(&bytes)?;
    database.with_connection(|connection| {
        import_prepared_flashcards_with_commit(
            connection,
            prepared,
            |connection, result, _commit_context, now| {
                crate::external_files::mark_import_consumed(
                    connection,
                    &input.staged_file_id,
                    result,
                    now,
                )
            },
        )
    })
}

/// Existing internal authoring hook kept only for the rollback regression.
#[cfg(test)]
pub(crate) fn import_generated_flashcards_bytes_with_commit<F>(
    database: &Database,
    bytes: &[u8],
    before_commit: F,
) -> Result<FlashcardImportResultDto, CommandError>
where
    F: FnOnce(&rusqlite::Connection, &FlashcardImportResultDto, i64) -> Result<(), CommandError>,
{
    import_generated_flashcards_bytes_with_commit_context(
        database,
        bytes,
        |connection, result, _context, now| before_commit(connection, result, now),
    )
}

/// Provenance-aware generated import hook used by document artifact generation.
/// `created_notes` maps retained JSON source indices to the exact note IDs
/// inserted by the authoring transaction after duplicate filtering.
pub(crate) fn import_generated_flashcards_bytes_with_commit_context<F>(
    database: &Database,
    bytes: &[u8],
    before_commit: F,
) -> Result<FlashcardImportResultDto, CommandError>
where
    F: FnOnce(
        &rusqlite::Connection,
        &FlashcardImportResultDto,
        &GeneratedImportCommitContext,
        i64,
    ) -> Result<(), CommandError>,
{
    let prepared = prepare_flashcard_import_bytes(bytes)?;
    database.with_connection(|connection| {
        import_prepared_flashcards_with_commit(connection, prepared, before_commit)
    })
}

mod markdown;
#[cfg(test)]
use markdown::markdown_document;

#[cfg(test)]
mod conformance_tests;
#[cfg(test)]
mod typed_tests;

#[cfg(test)]
mod generated_import_tests;
