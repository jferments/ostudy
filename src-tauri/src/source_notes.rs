//! Source notes: a study set made from a prompt alone gets a document of its
//! own. The AI connection used for cards and guides writes notes on what the
//! learner asked for, native code lays them out as a PDF, and the PDF is
//! staged as an ordinary document import, so reading, cards, the guide and
//! lessons treat it exactly like an upload.

use std::{
    fs::{self, OpenOptions},
    io::Write,
};

use rusqlite::params;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{
    ai::{
        document_intelligence::{
            generate_source_notes, load_verified_document_task_context, SourceNotes,
            MAX_SOURCE_PROMPT_CHARS,
        },
        document_provider, generation, AiRuntimeState,
    },
    database::Database,
    document_canonical::sha256_hex,
    document_study_guide_export::compile_template_pdf,
    error::CommandError,
    external_files::{ExternalFileKind, ExternalImportPurpose, StagedExternalFileDto},
    paths::AppPaths,
    task_cancellation::CancellationSignal,
    util::{unix_time_millis, validate_entity_id},
};

const TEMPLATE: &str = include_str!("source_notes/notes.typ");
const PURPOSE: &str = "source_notes_authoring";
const IMPORT_TTL_MS: i64 = 7 * 24 * 60 * 60 * 1_000;
const MAX_FILE_NAME_CHARS: usize = 96;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteSourceNotesInput {
    pub operation_id: String,
    pub prompt: String,
}

/// Writes notes on what `prompt` asks to learn and stages them as a PDF
/// document, ready for `import_document`.
pub(crate) async fn write(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    input: WriteSourceNotesInput,
) -> Result<StagedExternalFileDto, CommandError> {
    validate_entity_id(&input.operation_id, "source notes operation")?;
    let prompt = input.prompt.trim();
    if prompt.is_empty() || prompt.chars().count() > MAX_SOURCE_PROMPT_CHARS {
        return Err(CommandError::new(
            "source_notes_prompt_invalid",
            format!("Describe what to study in 1 to {MAX_SOURCE_PROMPT_CHARS} characters."),
        ));
    }
    let mut task = load_verified_document_task_context(database, runtime).await?;
    let capabilities = document_provider::load_document_provider_capabilities(
        database,
        &task.profile_id,
        &task.provider_kind,
        &task.model_id,
    )?;
    task.apply_provider_capabilities(&capabilities);
    let mut diagnostics =
        generation::Operation::begin(database, Some(&input.operation_id), None, None, PURPOSE)?;
    task.diagnostics = Some(diagnostics.context());
    task.diagnostic_window_key = Some(format!(
        "{:x}",
        Sha256::digest(format!("source-notes|{prompt}"))
    ));
    let result = generate_source_notes(runtime, &task, prompt, &CancellationSignal::new()).await;
    diagnostics.finish_result(&result);
    let notes = result?;
    let bytes = render(&notes, prompt)?;
    stage(
        database,
        paths,
        &input.operation_id,
        &file_name(&notes.title),
        &bytes,
    )
}

/// Lays the notes out as a PDF.
pub(crate) fn render(notes: &SourceNotes, prompt: &str) -> Result<Vec<u8>, CommandError> {
    let data = json!({
        "title": notes.title,
        "prompt": prompt.split_whitespace().collect::<Vec<_>>().join(" "),
        "sections": notes.sections.iter().map(|section| json!({
            "heading": section.heading,
            "paragraphs": section.paragraphs,
            "keyPoints": section.key_points,
        })).collect::<Vec<_>>(),
    });
    compile_template_pdf(TEMPLATE, &data).map_err(|detail| {
        CommandError::new(
            "source_notes_layout_failed",
            format!("The notes could not be laid out as a document: {detail}"),
        )
    })
}

/// A file name from the notes' title, without characters a file name cannot hold.
fn file_name(title: &str) -> String {
    let name = title
        .chars()
        .filter(|character| !character.is_control() && !r#"/\:*?"<>|"#.contains(*character))
        .take(MAX_FILE_NAME_CHARS)
        .collect::<String>();
    let name = name.trim().trim_matches('.').trim();
    format!("{}.pdf", if name.is_empty() { "Study notes" } else { name })
}

/// Stages `bytes` as a document selection under the operation's identity.
fn stage(
    database: &Database,
    paths: &AppPaths,
    operation_id: &str,
    display_name: &str,
    bytes: &[u8],
) -> Result<StagedExternalFileDto, CommandError> {
    let kind = ExternalFileKind::Pdf;
    let relative = AppPaths::staged_import_relative_path(operation_id, kind.extension());
    let path = paths.resolve_relative(&relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(CommandError::io)?;
    file.write_all(bytes).map_err(CommandError::io)?;
    file.sync_all().map_err(CommandError::io)?;
    let now = unix_time_millis()?;
    let staged = StagedExternalFileDto {
        id: operation_id.to_string(),
        purpose: ExternalImportPurpose::Document,
        detected_kind: kind,
        display_name: display_name.to_string(),
        media_type: kind.media_type().to_string(),
        byte_size: bytes.len() as i64,
        sha256: sha256_hex(bytes),
        expires_at: now.saturating_add(IMPORT_TTL_MS),
    };
    let inserted = database.with_connection(|connection| {
        connection
            .execute(
                "INSERT INTO staged_external_imports (
                     id, operation_id, purpose, detected_kind, display_name, media_type,
                     byte_size, sha256, relative_path, state, created_at, updated_at, expires_at
                 ) VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'staged', ?9, ?9, ?10)",
                params![
                    staged.id,
                    staged.purpose.as_str(),
                    kind.as_str(),
                    staged.display_name,
                    staged.media_type,
                    staged.byte_size,
                    staged.sha256,
                    relative,
                    now,
                    staged.expires_at
                ],
            )
            .map_err(CommandError::database)
    });
    if let Err(error) = inserted {
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    Ok(staged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::document_intelligence::SourceNotesSection;

    fn notes() -> SourceNotes {
        SourceNotes {
            title: "Cell membranes".to_string(),
            sections: vec![
                SourceNotesSection {
                    heading: "The lipid bilayer".to_string(),
                    paragraphs: vec!["Phospholipids form two layers.".to_string()],
                    key_points: vec!["Tails face inward.".to_string()],
                },
                SourceNotesSection {
                    heading: "Transport #[strong] $x$".to_string(),
                    paragraphs: vec![
                        "Some molecules cross freely; others need channels.".to_string()
                    ],
                    key_points: vec![],
                },
            ],
        }
    }

    #[test]
    fn renders_notes_as_a_pdf_with_markup_kept_as_text() {
        let bytes = render(&notes(), "Membranes for my biology exam").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn stages_the_pdf_as_a_document_selection() {
        let root = std::env::temp_dir().join(format!(
            "ostudy-source-notes-{}-{}",
            std::process::id(),
            unix_time_millis().unwrap()
        ));
        let paths = AppPaths::new(root.clone()).unwrap();
        let database = Database::open_in_memory().unwrap();
        let bytes = render(&notes(), "").unwrap();
        let id = "a".repeat(32);
        let staged = stage(&database, &paths, &id, &file_name("Cells: a/b?"), &bytes).unwrap();
        assert_eq!(staged.display_name, "Cells ab.pdf");
        let resolved = database
            .with_connection(|connection| {
                crate::external_files::resolve_staged_import(
                    connection,
                    &paths,
                    &id,
                    ExternalImportPurpose::Document,
                    &[ExternalFileKind::Pdf],
                )
            })
            .unwrap();
        assert_eq!(resolved.display_name, "Cells ab.pdf");
        assert!(stage(&database, &paths, &id, "Again.pdf", &bytes).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_names_fall_back_when_the_title_has_nothing_usable() {
        assert_eq!(file_name("..//"), "Study notes.pdf");
    }
}
