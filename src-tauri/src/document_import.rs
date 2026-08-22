//! Format-neutral managed document acquisition.
//!
//! External-file staging owns platform selection and content detection. This
//! module consumes only a validated app-private staged file, copies it into the
//! immutable document source bundle, and records the app-owned document format
//! before any parser or provider work begins.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    database::Database,
    document_domain::DocumentFormat,
    error::CommandError,
    external_files::{self, ExternalFileKind, ExternalImportPurpose},
    paths::AppPaths,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_DOCUMENT_BYTES: u64 = 512 * 1024 * 1024;
const COPY_BUFFER_BYTES: usize = 1024 * 1024;
const DOCUMENT_KINDS: &[ExternalFileKind] = &[
    ExternalFileKind::Pdf,
    ExternalFileKind::Doc,
    ExternalFileKind::Docx,
    ExternalFileKind::Ppt,
    ExternalFileKind::Pptx,
    ExternalFileKind::Odt,
    ExternalFileKind::Odp,
    ExternalFileKind::Rtf,
    ExternalFileKind::Epub,
    ExternalFileKind::Excel,
    ExternalFileKind::Ods,
    ExternalFileKind::Csv,
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportDocumentInput {
    pub staged_file_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedDocumentDto {
    pub document_id: String,
}

/// Commit a validated staged document into immutable managed storage.
///
/// The staging layer has already content-detected the source and encoded that
/// result into the managed transfer extension. `resolve_staged_import` verifies
/// the ledger-to-path identity and content hash before this function trusts that
/// app-owned extension as the format handoff.
#[tauri::command]
pub fn import_document(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ImportDocumentInput,
) -> Result<ImportedDocumentDto, CommandError> {
    validate_entity_id(&input.staged_file_id, "staged document")?;
    if let Some(result) = database.with_connection(|connection| {
        external_files::consumed_import_result::<ImportedDocumentDto>(
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
            ExternalImportPurpose::Document,
            DOCUMENT_KINDS,
        )
    })?;
    let format = staged_document_format(&staged.path)?;
    import_managed_document(
        database.inner(),
        paths.inner(),
        &staged.path,
        &staged.display_name,
        &input.staged_file_id,
        format,
    )
}

fn import_managed_document(
    database: &Database,
    paths: &AppPaths,
    source_path: &Path,
    source_display_name: &str,
    staged_file_id: &str,
    format: DocumentFormat,
) -> Result<ImportedDocumentDto, CommandError> {
    let source = fs::canonicalize(source_path).map_err(CommandError::io)?;
    if !source.is_file() {
        return Err(CommandError::new(
            "invalid_document_source",
            "Select a supported document file.",
        ));
    }
    let metadata = fs::metadata(&source).map_err(CommandError::io)?;
    if metadata.len() == 0 || metadata.len() > MAX_DOCUMENT_BYTES {
        return Err(CommandError::new(
            "invalid_document_size",
            "The document is empty or exceeds the 512 MB document limit.",
        ));
    }
    let original_filename = validate_document_filename(source_display_name)?;
    let title = validate_document_title(
        Path::new(&original_filename)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Imported document"),
    )?;
    let document_id = database.with_connection(|connection| generate_id(connection))?;
    let extension = format.default_extension();
    let staged_relative =
        AppPaths::staged_document_relative_path_with_extension(&document_id, extension);
    let staged_path = paths.resolve_relative(&staged_relative);
    let (byte_size, sha256) = copy_document_with_hash(&source, &staged_path)?;

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
            "This exact document is already in the document library.",
        ));
    }

    let source_relative =
        AppPaths::document_source_relative_path_with_extension(&document_id, extension);
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
    let result = ImportedDocumentDto {
        document_id: document_id.clone(),
    };
    let insert_result = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "INSERT INTO documents (
                     id, title, original_filename, mime_type, source_media_type,
                     byte_size, sha256, source_relative_path, staged_relative_path,
                     lifecycle_state, source_format, source_unit_kind, adapter_backend,
                     extraction_backend, render_backend, outline_state,
                     created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, ?3, 'application/pdf', ?4,
                     ?5, ?6, ?7, NULL,
                     'processing', ?8, ?9, 'pending',
                     'pending', 'pending', 'pending',
                     ?10, ?10, 1
                 )",
                params![
                    document_id,
                    title,
                    original_filename,
                    source_media_type(format),
                    byte_size,
                    sha256,
                    source_relative,
                    format.as_str(),
                    format.source_unit_kind().as_str(),
                    now,
                ],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_import_insert_failed",
                    format!("document={document_id} format={}", format.as_str()),
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

fn staged_document_format(path: &Path) -> Result<DocumentFormat, CommandError> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("pdf") => Ok(DocumentFormat::Pdf),
        Some("doc") => Ok(DocumentFormat::Doc),
        Some("docx") => Ok(DocumentFormat::Docx),
        Some("ppt") => Ok(DocumentFormat::Ppt),
        Some("pptx") => Ok(DocumentFormat::Pptx),
        Some("odt") => Ok(DocumentFormat::Odt),
        Some("odp") => Ok(DocumentFormat::Odp),
        Some("rtf") => Ok(DocumentFormat::Rtf),
        Some("epub") => Ok(DocumentFormat::Epub),
        Some("xlsx") => Ok(DocumentFormat::Excel),
        Some("ods") => Ok(DocumentFormat::Ods),
        Some("csv") => Ok(DocumentFormat::Csv),
        _ => Err(CommandError::new(
            "unsupported_document_format",
            "The staged document format is not supported by this build.",
        )),
    }
}

fn source_media_type(format: DocumentFormat) -> &'static str {
    match format {
        DocumentFormat::Pdf => "application/pdf",
        DocumentFormat::Doc => "application/msword",
        DocumentFormat::Docx => {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        }
        DocumentFormat::Ppt => "application/vnd.ms-powerpoint",
        DocumentFormat::Pptx => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        }
        DocumentFormat::Odt => "application/vnd.oasis.opendocument.text",
        DocumentFormat::Odp => "application/vnd.oasis.opendocument.presentation",
        DocumentFormat::Rtf => "application/rtf",
        DocumentFormat::Epub => "application/epub+zip",
        DocumentFormat::Excel => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        }
        DocumentFormat::Ods => "application/vnd.oasis.opendocument.spreadsheet",
        DocumentFormat::Csv => "text/csv",
        _ => "application/octet-stream",
    }
}

fn copy_document_with_hash(
    source: &Path,
    destination: &Path,
) -> Result<(i64, String), CommandError> {
    let mut input = File::open(source).map_err(CommandError::io)?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(CommandError::io)?;
    let result = (|| {
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
        let mut total = 0_u64;
        loop {
            let read = input.read(&mut buffer).map_err(CommandError::io)?;
            if read == 0 {
                break;
            }
            total = total.saturating_add(read as u64);
            if total > MAX_DOCUMENT_BYTES {
                return Err(CommandError::new(
                    "invalid_document_size",
                    "The document exceeds the 512 MB document limit.",
                ));
            }
            hasher.update(&buffer[..read]);
            output
                .write_all(&buffer[..read])
                .map_err(CommandError::io)?;
        }
        output.sync_all().map_err(CommandError::io)?;
        Ok((
            i64::try_from(total).unwrap_or(i64::MAX),
            format!("{:x}", hasher.finalize()),
        ))
    })();
    if result.is_err() {
        drop(output);
        let _ = fs::remove_file(destination);
    }
    result
}

fn validate_document_filename(value: &str) -> Result<String, CommandError> {
    let trimmed = value.trim();
    let valid = !trimmed.is_empty()
        && trimmed.chars().count() <= 255
        && !trimmed
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\' | '\0'));
    if !valid {
        return Err(CommandError::new(
            "invalid_document_name",
            "Select a document with a valid filename.",
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_document_title(value: &str) -> Result<String, CommandError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 200 {
        return Err(CommandError::new(
            "invalid_document_title",
            "The document title must contain 1 to 200 characters.",
        ));
    }
    Ok(trimmed.to_string())
}

fn sync_file(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(CommandError::io)
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(CommandError::io)
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), CommandError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_staging_extensions_map_to_document_formats() {
        assert_eq!(
            staged_document_format(Path::new("/tmp/import.docx")).unwrap(),
            DocumentFormat::Docx
        );
        assert_eq!(
            staged_document_format(Path::new("/tmp/import.xlsx")).unwrap(),
            DocumentFormat::Excel
        );
        assert_eq!(
            staged_document_format(Path::new("/tmp/import.pdf")).unwrap(),
            DocumentFormat::Pdf
        );
    }

    #[test]
    fn document_filename_validation_rejects_path_components() {
        assert_eq!(
            validate_document_filename("../lecture.docx")
                .unwrap_err()
                .code,
            "invalid_document_name"
        );
        assert_eq!(
            validate_document_filename("lecture.docx").unwrap(),
            "lecture.docx"
        );
    }
}
