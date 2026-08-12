#!/usr/bin/env node

import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (...parts) => readFile(join(root, ...parts), "utf8");
const write = async (path, content) => {
  const target = join(root, path);
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, content, "utf8");
};
const replace = async (path, from, to) => {
  const source = await read(path);
  if (!source.includes(from)) throw new Error(`${path}: source fragment not found`);
  await write(path, source.replace(from, to));
};
const replaceRange = async (path, startMarker, endMarker, replacement) => {
  const source = await read(path);
  const start = source.indexOf(startMarker);
  const end = source.indexOf(endMarker, start);
  if (start < 0 || end < 0) throw new Error(`${path}: replacement range not found`);
  await write(path, source.slice(0, start) + replacement + source.slice(end));
};

await write("src-tauri/src/assets.rs", String.raw`use std::collections::{BTreeSet, HashMap};

use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    asset_lifecycle::{stage_image, ImportImageInput},
    database::Database,
    error::CommandError,
    paths::AppPaths,
    projections::scope::{chunks, push_value},
    util::validate_entity_id,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDto {
    pub id: String,
    pub original_filename: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub created_at: i64,
}

#[tauri::command]
pub fn import_image(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ImportImageInput,
) -> Result<AssetDto, CommandError> {
    stage_image(database.inner(), paths.inner(), input)
}

pub(crate) fn get_assets_for_note(
    connection: &Connection,
    paths: &AppPaths,
    note_id: &str,
) -> Result<Vec<AssetDto>, CommandError> {
    let mut assets = load_assets_for_notes(connection, paths, &[note_id.to_string()])?;
    Ok(assets.remove(note_id).unwrap_or_default())
}

pub(crate) fn load_assets_for_notes(
    connection: &Connection,
    _paths: &AppPaths,
    note_ids: &[String],
) -> Result<HashMap<String, Vec<AssetDto>>, CommandError> {
    let mut result = note_ids
        .iter()
        .cloned()
        .map(|note_id| (note_id, Vec::new()))
        .collect::<HashMap<_, _>>();
    for chunk in chunks(note_ids) {
        let mut values = Vec::<SqlValue>::new();
        let placeholders = chunk
            .iter()
            .map(|note_id| format!("?{}", push_value(&mut values, note_id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT DISTINCT
                 na.note_id, a.id, a.original_filename, a.mime_type, a.byte_size,
                 a.created_at
             FROM note_assets na
             JOIN assets a ON a.id = na.asset_id
             WHERE na.note_id IN ({placeholders})
               AND a.deleted_at IS NULL
               AND a.lifecycle_state = 'ready'
             ORDER BY na.note_id, a.created_at ASC, a.id ASC"
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    AssetDto {
                        id: row.get(1)?,
                        original_filename: row.get(2)?,
                        mime_type: row.get(3)?,
                        byte_size: row.get(4)?,
                        created_at: row.get(5)?,
                    },
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        for (note_id, asset) in rows {
            result.entry(note_id).or_default().push(asset);
        }
    }
    Ok(result)
}

pub(crate) fn ensure_assets_exist(
    connection: &Connection,
    asset_ids: &[String],
) -> Result<(), CommandError> {
    let mut unique = BTreeSet::new();
    for asset_id in asset_ids {
        validate_entity_id(asset_id, "asset")?;
        unique.insert(asset_id.clone());
    }
    let unique = unique.into_iter().collect::<Vec<_>>();
    let mut found = 0_usize;
    for chunk in chunks(&unique) {
        let mut values = Vec::<SqlValue>::new();
        let placeholders = chunk
            .iter()
            .map(|asset_id| format!("?{}", push_value(&mut values, asset_id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT COUNT(*) FROM assets
             WHERE id IN ({placeholders})
               AND deleted_at IS NULL
               AND lifecycle_state IN ('staged', 'ready')"
        );
        let count: i64 = connection
            .query_row(&sql, params_from_iter(values.iter()), |row| row.get(0))
            .map_err(CommandError::database)?;
        found = found.saturating_add(usize::try_from(count).unwrap_or(usize::MAX));
    }
    if found != unique.len() {
        return Err(CommandError::new(
            "asset_not_found",
            "One of the images used by this content is no longer available.",
        ));
    }
    Ok(())
}

pub(crate) fn validate_original_filename(value: &str) -> Result<String, CommandError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > 255
        || trimmed
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\' | '\0'))
    {
        return Err(CommandError::new(
            "invalid_image_name",
            "The selected image has an invalid filename.",
        ));
    }
    Ok(trimmed.to_string())
}

pub(crate) fn detect_image_bytes(
    bytes: &[u8],
) -> Result<(&'static str, &'static str), CommandError> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Ok(("image/png", "png"));
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Ok(("image/jpeg", "jpg"));
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Ok(("image/gif", "gif"));
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Ok(("image/webp", "webp"));
    }
    Err(CommandError::new(
        "unsupported_image",
        "Use a PNG, JPEG, GIF, or WebP image.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_supported_image_signatures() {
        assert_eq!(
            detect_image_bytes(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a])
                .expect("PNG should be recognized"),
            ("image/png", "png")
        );
        assert_eq!(
            detect_image_bytes(&[0xff, 0xd8, 0xff, 0xdb]).expect("JPEG should be recognized"),
            ("image/jpeg", "jpg")
        );
        assert!(detect_image_bytes(b"not an image").is_err());
    }

    #[test]
    fn rejects_unsafe_original_filenames() {
        assert!(validate_original_filename("../image.png").is_err());
        assert!(validate_original_filename("image\n.png").is_err());
        assert_eq!(
            validate_original_filename(" diagram.png ").expect("filename should normalize"),
            "diagram.png"
        );
    }
}
`);

let assetLifecycle = await read("src-tauri/src/asset_lifecycle.rs");
let assetLifecycleUpdated = assetLifecycle
  .replace(
    "use rusqlite::{params, Connection, OptionalExtension, Transaction};",
    "use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};",
  )
  .replace(
    "    error::CommandError,\n    paths::AppPaths,",
    "    error::CommandError,\n    external_files::{self, ExternalFileKind, ExternalImportPurpose},\n    paths::AppPaths,",
  )
  .replace(
    "pub struct ImportImageInput {\n    pub file_path: String,\n    pub draft_id: String,\n}",
    "pub struct ImportImageInput {\n    pub staged_file_id: String,\n    pub draft_id: String,\n}",
  );
if (assetLifecycleUpdated === assetLifecycle) throw new Error("asset_lifecycle.rs: imports/input not updated");
await write("src-tauri/src/asset_lifecycle.rs", assetLifecycleUpdated);

await replaceRange(
  "src-tauri/src/asset_lifecycle.rs",
  "pub(crate) fn stage_image(\n",
  "pub(crate) fn finalize_assets_for_owner(\n",
  String.raw`pub(crate) fn stage_image(
    database: &Database,
    paths: &AppPaths,
    input: ImportImageInput,
) -> Result<AssetDto, CommandError> {
    validate_entity_id(&input.draft_id, "asset draft")?;
    validate_entity_id(&input.staged_file_id, "staged image")?;
    database.with_connection(|connection| {
        if let Some(asset) = external_files::consumed_import_result::<AssetDto>(
            connection,
            &input.staged_file_id,
        )? {
            return Ok(asset);
        }
        let staged = external_files::resolve_staged_import(
            connection,
            paths,
            &input.staged_file_id,
            ExternalImportPurpose::Image,
            &[
                ExternalFileKind::Png,
                ExternalFileKind::Jpeg,
                ExternalFileKind::Gif,
                ExternalFileKind::Webp,
            ],
        )?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let asset = stage_source_named_in_connection(
            &transaction,
            paths,
            &staged.path,
            &input.draft_id,
            Some(&staged.display_name),
            IngestFaults::default(),
        )?;
        let extension = match asset.mime_type.as_str() {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/gif" => "gif",
            "image/webp" => "webp",
            _ => "bin",
        };
        let staged_asset_path = paths.resolve_relative(AppPaths::staged_asset_relative_path(
            &input.draft_id,
            &asset.id,
            extension,
        ));
        if let Err(error) = external_files::mark_import_consumed(
            &transaction,
            &input.staged_file_id,
            &asset,
            unix_time_millis()?,
        ) {
            drop(transaction);
            let _ = fs::remove_file(staged_asset_path);
            return Err(error);
        }
        if let Err(error) = transaction.commit() {
            let _ = fs::remove_file(staged_asset_path);
            return Err(CommandError::database(error));
        }
        Ok(asset)
    })
}

`,
);

let assetStage = await read("src-tauri/src/asset_lifecycle.rs");
let namedStage = assetStage
  .replace(
    "fn stage_source_in_connection(\n    connection: &mut Connection,\n    paths: &AppPaths,\n    source_path: &Path,\n    draft_id: &str,\n    faults: IngestFaults,\n) -> Result<AssetDto, CommandError> {",
    `fn stage_source_in_connection(
    connection: &Connection,
    paths: &AppPaths,
    source_path: &Path,
    draft_id: &str,
    faults: IngestFaults,
) -> Result<AssetDto, CommandError> {
    stage_source_named_in_connection(
        connection,
        paths,
        source_path,
        draft_id,
        None,
        faults,
    )
}

fn stage_source_named_in_connection(
    connection: &Connection,
    paths: &AppPaths,
    source_path: &Path,
    draft_id: &str,
    original_filename_override: Option<&str>,
    faults: IngestFaults,
) -> Result<AssetDto, CommandError> {`,
  )
  .replace(
    `    let original_filename = validate_original_filename(
        source_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("image"),
    )?;`,
    `    let source_filename = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let original_filename =
        validate_original_filename(original_filename_override.unwrap_or(source_filename))?;`,
  )
  .replace(
    `        byte_size: i64::try_from(metadata.len()).unwrap_or(i64::MAX),
        file_path: staged_path.to_string_lossy().into_owned(),
        created_at: now,`,
    `        byte_size: i64::try_from(metadata.len()).unwrap_or(i64::MAX),
        created_at: now,`,
  );
if (namedStage === assetStage) throw new Error("asset_lifecycle.rs: staging function not migrated");
await write("src-tauri/src/asset_lifecycle.rs", namedStage);

const studydeckBlock = String.raw`#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareDeckExportInput {
    pub deck_id: String,
    pub operation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportDeckInput {
    pub operation_id: String,
    pub staged_file_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportResultDto {
    destination_path: String,
    card_count: i64,
    asset_count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResultDto {
    pub deck: DeckSummaryDto,
    pub card_count: i64,
    pub asset_count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeckExportSummary {
    card_count: i64,
    asset_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedDeckExportDto {
    pub prepared_export: crate::external_files::PreparedExportDto,
    pub card_count: i64,
    pub asset_count: i64,
}

#[tauri::command]
pub fn prepare_deck_export(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PrepareDeckExportInput,
) -> Result<PreparedDeckExportDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    validate_entity_id(&input.operation_id, "deck export operation")?;
    if let Some((prepared_export, summary)) = database.with_connection(|connection| {
        crate::external_files::load_prepared_export_summary::<DeckExportSummary>(
            connection,
            paths.inner(),
            &input.operation_id,
            "studydeck",
        )
    })? {
        return Ok(PreparedDeckExportDto {
            prepared_export,
            card_count: summary.card_count,
            asset_count: summary.asset_count,
        });
    }

    let relative = AppPaths::prepared_export_relative_path(&input.operation_id, "studydeck");
    let destination = paths.resolve_relative(&relative);
    let exported = database.with_connection(|connection| {
        export::export_deck_from_connection(connection, &paths, &input.deck_id, &destination)
    })?;
    let deck_name = database.with_connection(|connection| {
        Ok(get_deck_from_connection(connection, &input.deck_id)?.name)
    })?;
    let summary = DeckExportSummary {
        card_count: exported.card_count,
        asset_count: exported.asset_count,
    };
    let suggested_filename = format!("{}.studydeck", portable_filename(&deck_name));
    let prepared_export = database.with_connection(|connection| {
        crate::external_files::register_prepared_export(
            connection,
            paths.inner(),
            crate::external_files::RegisterPreparedExport {
                id: &input.operation_id,
                purpose: "studydeck",
                suggested_filename: &suggested_filename,
                media_type: "application/vnd.ostudy.studydeck",
                relative_path: &relative,
                summary: &summary,
            },
        )
    })?;
    Ok(PreparedDeckExportDto {
        prepared_export,
        card_count: summary.card_count,
        asset_count: summary.asset_count,
    })
}

#[tauri::command]
pub fn import_deck(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ImportDeckInput,
) -> Result<ImportResultDto, CommandError> {
    validate_entity_id(&input.operation_id, "deck import operation")?;
    validate_entity_id(&input.staged_file_id, "staged deck")?;
    if let Some(result) = database.with_connection(|connection| {
        crate::external_files::consumed_import_result::<ImportResultDto>(
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
            &[crate::external_files::ExternalFileKind::Studydeck],
        )
    })?;
    let fingerprint = hash_file(&staged.path)?;
    let package = read::read_package(&staged.path)?;
    let result = database.with_connection(|connection| {
        import::import_package(
            connection,
            &paths,
            &input.operation_id,
            &fingerprint,
            package,
        )
    })?;
    database.with_connection(|connection| {
        crate::external_files::mark_import_consumed(
            connection,
            &input.staged_file_id,
            &result,
            unix_time_millis()?,
        )
    })?;
    Ok(result)
}

fn portable_filename(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, ' ' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = normalized.trim().trim_matches('_');
    if trimmed.is_empty() {
        "ostudy-deck".to_string()
    } else {
        trimmed.chars().take(120).collect()
    }
}

`;
await replaceRange(
  "src-tauri/src/studydeck.rs",
  "#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\", deny_unknown_fields)]\npub struct ExportDeckInput",
  "fn normalize_export_path",
  studydeckBlock,
);

let generated = await read("src-tauri/src/authoring/card_types_and_templates/generated_import/mod.rs");
const generatedUpdated = generated
  .replace("#[derive(Clone, Debug, Serialize)]", "#[derive(Clone, Debug, Deserialize, Serialize)]")
  .replace(
    "use prepare::{prepare_flashcard_import, prepare_flashcard_import_bytes};",
    "use prepare::prepare_flashcard_import_bytes;",
  );
await write("src-tauri/src/authoring/card_types_and_templates/generated_import/mod.rs", generatedUpdated);
await replaceRange(
  "src-tauri/src/authoring/card_types_and_templates/generated_import/mod.rs",
  "#[tauri::command]\npub fn import_generated_flashcards(\n",
  "pub(crate) fn import_generated_flashcards_bytes_with_commit",
  String.raw`#[derive(Debug, Deserialize)]
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
        import_prepared_flashcards_with_commit(connection, prepared, |connection, result, now| {
            crate::external_files::mark_import_consumed(
                connection,
                &input.staged_file_id,
                result,
                now,
            )
        })
    })
}

`,
);

let generatedPrepare = await read("src-tauri/src/authoring/card_types_and_templates/generated_import/prepare.rs");
const prepareFunctionStart = generatedPrepare.indexOf("pub(super) fn prepare_flashcard_import(source_path: &str)");
const prepareBytesStart = generatedPrepare.indexOf("pub(super) fn prepare_flashcard_import_bytes", prepareFunctionStart);
if (prepareFunctionStart >= 0 && prepareBytesStart >= 0) {
  await write(
    "src-tauri/src/authoring/card_types_and_templates/generated_import/prepare.rs",
    generatedPrepare.slice(0, prepareFunctionStart) + generatedPrepare.slice(prepareBytesStart),
  );
}

await replace(
  "src-tauri/src/documents.rs",
  "pub struct ImportPdfDocumentInput {\n    pub source_path: String,\n}",
  "pub struct ImportPdfDocumentInput {\n    pub staged_file_id: String,\n}",
);

let documentOperations = await read("src-tauri/src/document_operations.rs");
const documentOperationsUpdated = documentOperations
  .replace(
    "    error::CommandError,\n    paths::AppPaths,",
    "    error::CommandError,\n    external_files::{self, ExternalFileKind, ExternalImportPurpose},\n    paths::AppPaths,",
  )
  .replace(
    "#[derive(Clone, Debug, Serialize)]\n#[serde(rename_all = \"camelCase\")]\npub struct ImportedPdfDocumentDto",
    "#[derive(Clone, Debug, Deserialize, Serialize)]\n#[serde(rename_all = \"camelCase\")]\npub struct ImportedPdfDocumentDto",
  );
await write("src-tauri/src/document_operations.rs", documentOperationsUpdated);
await replaceRange(
  "src-tauri/src/document_operations.rs",
  "#[tauri::command]\npub fn import_pdf_document(\n",
  "/// Run the existing document pipeline",
  String.raw`#[tauri::command]
pub fn import_pdf_document(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ImportPdfDocumentInput,
) -> Result<ImportedPdfDocumentDto, CommandError> {
    validate_entity_id(&input.staged_file_id, "staged PDF")?;
    if let Some(result) = database.with_connection(|connection| {
        external_files::consumed_import_result::<ImportedPdfDocumentDto>(
            connection,
            &input.staged_file_id,
        )
    })? {
        return Ok(result);
    }
    let staged = database.with_connection(|connection| {
        external_files::resolve_staged_import(
            connection,
            paths.inner(),
            &input.staged_file_id,
            ExternalImportPurpose::PdfDocument,
            &[ExternalFileKind::Pdf],
        )
    })?;
    import_managed_pdf(
        database.inner(),
        paths.inner(),
        &staged.path,
        &staged.display_name,
        &input.staged_file_id,
    )
}

`,
);

await replaceRange(
  "src-tauri/src/document_operations.rs",
  "fn import_managed_pdf(\n",
  "fn deleted_pdf_citation",
  String.raw`fn import_managed_pdf(
    database: &Database,
    paths: &AppPaths,
    source_path: &Path,
    source_display_name: &str,
    staged_file_id: &str,
) -> Result<ImportedPdfDocumentDto, CommandError> {
    let source = fs::canonicalize(source_path).map_err(CommandError::io)?;
    if !source.is_file() {
        return Err(CommandError::new("invalid_pdf", "Select a PDF file."));
    }
    let metadata = fs::metadata(&source).map_err(CommandError::io)?;
    if metadata.len() == 0 || metadata.len() > MAX_PDF_BYTES {
        return Err(CommandError::new(
            "invalid_pdf_size",
            "The PDF is empty or exceeds the 512 MB document limit.",
        ));
    }
    let original_filename = validate_pdf_filename(source_display_name)?;
    let title = validate_document_title(
        Path::new(&original_filename)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Imported PDF"),
    )?;
    let document_id = database.with_connection(|connection| generate_id(connection))?;
    let staged_relative = AppPaths::staged_document_relative_path(&document_id);
    let staged_path = paths.resolve_relative(&staged_relative);
    let (byte_size, sha256) = copy_pdf_with_hash(&source, &staged_path)?;

    let duplicate = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT id FROM documents WHERE sha256 = ?1 LIMIT 1",
                [sha256.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)
    })?;
    if duplicate.is_some() {
        let _ = fs::remove_file(&staged_path);
        return Err(CommandError::new(
            "document_already_imported",
            "This exact PDF is already in the document library.",
        ));
    }

    let source_relative = AppPaths::document_source_relative_path(&document_id);
    let destination = paths.resolve_relative(&source_relative);
    let directory = destination.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_document_path",
            "The managed document path is invalid.",
        )
    })?;
    fs::create_dir_all(directory).map_err(CommandError::io)?;
    fs::rename(&staged_path, &destination).map_err(CommandError::io)?;
    sync_file(&destination)?;
    sync_directory(directory)?;

    let now = unix_time_millis()?;
    let result = ImportedPdfDocumentDto {
        document_id: document_id.clone(),
    };
    let insert_result = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "INSERT INTO documents (
                     id, title, original_filename, mime_type, byte_size, sha256,
                     source_relative_path, staged_relative_path, lifecycle_state,
                     source_format, source_unit_kind,
                     extraction_backend, render_backend, outline_state,
                     created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, ?3, 'application/pdf', ?4, ?5,
                     ?6, NULL, 'processing', ?7, ?8,
                     'pending', 'pending', 'pending', ?9, ?9, 1
                 )",
                params![
                    document_id,
                    title,
                    original_filename,
                    byte_size,
                    sha256,
                    source_relative,
                    DocumentFormat::Pdf.as_str(),
                    SourceUnitKind::Page.as_str(),
                    now,
                ],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_import_insert_failed",
                    format!("document={document_id} format=pdf"),
                    error,
                )
            })?;
        external_files::mark_import_consumed(&transaction, staged_file_id, &result, now)?;
        transaction.commit().map_err(CommandError::database)
    });
    if let Err(error) = insert_result {
        let _ = fs::remove_dir_all(directory);
        return Err(error);
    }
    Ok(result)
}

`,
);

const backupBlock = String.raw`#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareLibraryBackupInput {
    pub operation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareLibraryRestoreInput {
    pub staged_file_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryBackupResultDto {
    destination_path: String,
    deck_count: i64,
    template_count: i64,
    review_count: i64,
    asset_count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRestoreResultDto {
    pub deck_count: i64,
    pub template_count: i64,
    pub review_count: i64,
    pub asset_count: i64,
    pub restart_required: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryBackupSummaryDto {
    deck_count: i64,
    template_count: i64,
    review_count: i64,
    asset_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedLibraryBackupDto {
    pub prepared_export: crate::external_files::PreparedExportDto,
    pub deck_count: i64,
    pub template_count: i64,
    pub review_count: i64,
    pub asset_count: i64,
}

#[derive(Clone, Debug)]
struct BackupSummary {
    deck_count: i64,
    template_count: i64,
    review_count: i64,
    asset_count: i64,
}

`;
await replaceRange(
  "src-tauri/src/library_backup.rs",
  "#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\", deny_unknown_fields)]\npub struct ExportLibraryBackupInput",
  "pub(crate) struct PendingRestoreGuard",
  backupBlock,
);

await replaceRange(
  "src-tauri/src/library_backup.rs",
  "#[tauri::command]\npub fn export_library_backup(\n",
  "pub(crate) fn apply_pending_restore",
  String.raw`#[tauri::command]
pub fn prepare_library_backup(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PrepareLibraryBackupInput,
) -> Result<PreparedLibraryBackupDto, CommandError> {
    crate::util::validate_entity_id(&input.operation_id, "library backup operation")?;
    if let Some((prepared_export, summary)) = database.with_connection(|connection| {
        crate::external_files::load_prepared_export_summary::<LibraryBackupSummaryDto>(
            connection,
            paths.inner(),
            &input.operation_id,
            "library_backup",
        )
    })? {
        return Ok(PreparedLibraryBackupDto {
            prepared_export,
            deck_count: summary.deck_count,
            template_count: summary.template_count,
            review_count: summary.review_count,
            asset_count: summary.asset_count,
        });
    }
    let relative = AppPaths::prepared_export_relative_path(&input.operation_id, "ostudybackup");
    let destination = paths.resolve_relative(&relative);
    let exported = export_backup(database.inner(), paths.inner(), &destination)?;
    let summary = LibraryBackupSummaryDto {
        deck_count: exported.deck_count,
        template_count: exported.template_count,
        review_count: exported.review_count,
        asset_count: exported.asset_count,
    };
    let suggested_filename = format!(
        "ostudy-library-{}.ostudybackup",
        chrono::Utc::now().format("%Y-%m-%d")
    );
    let prepared_export = database.with_connection(|connection| {
        crate::external_files::register_prepared_export(
            connection,
            paths.inner(),
            crate::external_files::RegisterPreparedExport {
                id: &input.operation_id,
                purpose: "library_backup",
                suggested_filename: &suggested_filename,
                media_type: "application/vnd.ostudy.backup",
                relative_path: &relative,
                summary: &summary,
            },
        )
    })?;
    Ok(PreparedLibraryBackupDto {
        prepared_export,
        deck_count: summary.deck_count,
        template_count: summary.template_count,
        review_count: summary.review_count,
        asset_count: summary.asset_count,
    })
}

#[tauri::command]
pub fn prepare_library_restore(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PrepareLibraryRestoreInput,
) -> Result<LibraryRestoreResultDto, CommandError> {
    crate::util::validate_entity_id(&input.staged_file_id, "staged library backup")?;
    if let Some(result) = database.with_connection(|connection| {
        crate::external_files::consumed_import_result::<LibraryRestoreResultDto>(
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
            crate::external_files::ExternalImportPurpose::LibraryBackup,
            &[crate::external_files::ExternalFileKind::Ostudybackup],
        )
    })?;
    let summary = validate_backup_package(&staged.path, None)?;
    let pending = pending_restore_path(&paths);
    let temporary = pending.with_extension("ostudybackup.tmp");
    remove_file_if_exists(&temporary).map_err(CommandError::io)?;
    copy_and_sync(&staged.path, &temporary)?;
    replace_file(&temporary, &pending)?;
    let result = LibraryRestoreResultDto {
        deck_count: summary.deck_count,
        template_count: summary.template_count,
        review_count: summary.review_count,
        asset_count: summary.asset_count,
        restart_required: true,
    };
    database.with_connection(|connection| {
        crate::external_files::mark_import_consumed(
            connection,
            &input.staged_file_id,
            &result,
            unix_time_millis()?,
        )
    })?;
    Ok(result)
}

`,
);

let lib = await read("src-tauri/src/lib.rs");
const libUpdated = lib
  .replace("            library_backup::export_library_backup,", "            library_backup::prepare_library_backup,")
  .replace("            studydeck::export_deck,", "            studydeck::prepare_deck_export,");
if (libUpdated === lib) throw new Error("lib.rs: path-free command registry not updated");
await write("src-tauri/src/lib.rs", libUpdated);

await rm(join(root, "scripts", "apply-platform-p2-pathfree-native.mjs"), { force: true });
await rm(join(root, ".github", "workflows", "apply-platform-p2-pathfree-native.yml"), { force: true });
console.log("Applied path-free native import/export migration.");
