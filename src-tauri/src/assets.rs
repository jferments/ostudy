use std::{fs::File, io::Read, path::Path};

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::{
    asset_lifecycle::{stage_image, ImportImageInput},
    database::Database,
    error::CommandError,
    paths::AppPaths,
    util::validate_entity_id,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDto {
    pub id: String,
    pub original_filename: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub file_path: String,
    pub created_at: i64,
}

/// Stages an image inside an explicit editor/import draft.
///
/// The stable Tauri command name remains `import_image`; only its payload gains
/// the draft identity needed to make cancel, save, and crash behavior explicit.
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
    let mut statement = connection
        .prepare(
            "SELECT DISTINCT a.id, a.original_filename, a.mime_type, a.byte_size,
                    a.relative_path, a.created_at
             FROM assets a
             JOIN note_assets na ON na.asset_id = a.id
             WHERE na.note_id = ?1
               AND a.deleted_at IS NULL
               AND a.lifecycle_state = 'ready'
             ORDER BY a.created_at ASC, a.id ASC",
        )
        .map_err(CommandError::database)?;

    let rows = statement
        .query_map([note_id], |row| {
            let relative_path: String = row.get(4)?;
            Ok(AssetDto {
                id: row.get(0)?,
                original_filename: row.get(1)?,
                mime_type: row.get(2)?,
                byte_size: row.get(3)?,
                file_path: paths
                    .resolve_relative(&relative_path)
                    .to_string_lossy()
                    .into_owned(),
                created_at: row.get(5)?,
            })
        })
        .map_err(CommandError::database)?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

/// Compatibility validation used by paths that are converted to explicit
/// finalization in the same Stage 3.3 change set. A staged row is not sufficient
/// for a durable save by itself; the caller must also finalize it with the
/// matching draft before committing.
pub(crate) fn ensure_assets_exist(
    connection: &Connection,
    asset_ids: &[String],
) -> Result<(), CommandError> {
    for asset_id in asset_ids {
        validate_entity_id(asset_id, "asset")?;
        let exists: i64 = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM assets
                     WHERE id = ?1
                       AND deleted_at IS NULL
                       AND lifecycle_state IN ('staged', 'ready')
                 )",
                [asset_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if exists == 0 {
            return Err(CommandError::new(
                "asset_not_found",
                "One of the images used by this content is no longer available.",
            ));
        }
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

pub(crate) fn detect_image_type(
    path: &Path,
) -> Result<(&'static str, &'static str), CommandError> {
    let mut file = File::open(path).map_err(CommandError::io)?;
    let mut header = [0_u8; 16];
    let bytes_read = file.read(&mut header).map_err(CommandError::io)?;
    detect_image_bytes(&header[..bytes_read])
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
