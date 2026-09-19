//! Retained image-set bytes, identities and edit intent join the complete learner
//! preservation fixture. This source is deliberately not yet OCR-prepared.

use super::*;
use crate::{
    document_canonical::sha256_hex,
    documents::image_sets::{
        import::{self, ImageSelectionInput, ImportImageSetInput},
        manifest::{self, InclusionCrop},
        repository,
    },
};

pub(super) fn seed(database: &Database, paths: &AppPaths, image: &[u8]) -> Vec<String> {
    let mut selections = Vec::new();
    for number in [950, 951] {
        let selection = id(number);
        let path = AppPaths::staged_import_relative_path(&selection, "png");
        fs::write(paths.resolve_relative(&path), image).unwrap();
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO staged_external_imports (
                             id, operation_id, purpose, detected_kind, display_name, media_type,
                             byte_size, sha256, relative_path, state, result_json,
                             created_at, updated_at, expires_at
                         ) VALUES (?1, ?1, 'image', 'png', 'overlap.png', 'image/png',
                                   ?2, ?3, ?4, 'staged', '', 1, 1, 9223372036854775807)",
                        params![selection, image.len() as i64, sha256_hex(image), path],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();
        selections.push(ImageSelectionInput {
            staged_file_id: selection,
            quarter_turns: if number == 950 { 1 } else { 0 },
            inclusion: Some(InclusionCrop {
                left: 0.0,
                top: 0.25,
                right: 0.75,
                bottom: 1.0,
            }),
        });
    }
    let result = import::import(
        database,
        paths,
        &ImportImageSetInput {
            operation_id: id(952),
            title: "Retained ordered screenshots".to_string(),
            members: selections,
        },
    )
    .unwrap();
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE documents SET lifecycle_state = 'failed',
                         safe_error_code = 'document_generation_interrupted',
                         safe_error_message = 'Synthetic retained source awaiting explicit preparation.'
                     WHERE id = ?1",
                    [&result.document_id],
                )
                .map_err(CommandError::database)?;
            let saved = repository::load(connection, &result.document_id)?;
            let mut files = saved
                .members
                .iter()
                .map(|member| member.relative_path.clone())
                .collect::<Vec<_>>();
            files.push(manifest::manifest_path(
                &saved.document_id,
                &saved.fingerprint()?,
            )?);
            Ok(files)
        })
        .unwrap()
}

pub(super) fn verify(connection: &Connection) -> Result<(), CommandError> {
    repository::validate(connection)?;
    let mut statement = connection
        .prepare("SELECT document_id FROM document_image_sets ORDER BY document_id")
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(CommandError::database)?;
    for row in rows {
        let saved = repository::load(connection, &row.map_err(CommandError::database)?)?;
        assert_eq!(saved.members.len(), 2);
        assert_ne!(saved.members[0].id, saved.members[1].id);
        assert_eq!(saved.members[0].sha256, saved.members[1].sha256);
        assert_eq!(saved.members[0].quarter_turns, 1);
        assert_eq!(saved.members[1].quarter_turns, 0);
        assert_eq!(saved.members[0].inclusion.unwrap().top, 0.25);
        assert_eq!(saved.members[1].inclusion.unwrap().right, 0.75);
    }
    Ok(())
}
