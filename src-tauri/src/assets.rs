use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    paths::AppPaths,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;

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

#[tauri::command]
pub fn import_image(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    file_path: String,
) -> Result<AssetDto, CommandError> {
    let source_path = std::fs::canonicalize(&file_path).map_err(CommandError::io)?;
    if !source_path.is_file() {
        return Err(CommandError::new(
            "invalid_image",
            "Select an image file, not a folder.",
        ));
    }

    let metadata = std::fs::metadata(&source_path).map_err(CommandError::io)?;
    if metadata.len() == 0 {
        return Err(CommandError::new(
            "invalid_image",
            "The selected image is empty.",
        ));
    }
    if metadata.len() > MAX_IMAGE_BYTES {
        return Err(CommandError::new(
            "image_too_large",
            "Images cannot exceed 25 MB.",
        ));
    }

    let (mime_type, extension) = detect_image_type(&source_path)?;
    let original_filename = validate_original_filename(
        source_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("image"),
    )?;
    let now = unix_time_millis()?;

    let (asset, final_path) = database.with_connection(|connection| {
        let id = generate_id(connection)?;
        let relative_path = AppPaths::image_relative_path(&id, extension);
        let final_path = paths.resolve_relative(&relative_path);
        let temporary_path = final_path.with_extension(format!("{extension}.tmp"));

        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).map_err(CommandError::io)?;
        }
        fs::copy(&source_path, &temporary_path).map_err(CommandError::io)?;
        File::open(&temporary_path)
            .and_then(|file| file.sync_all())
            .map_err(CommandError::io)?;
        fs::rename(&temporary_path, &final_path).map_err(|error| {
            let _ = fs::remove_file(&temporary_path);
            CommandError::io(error)
        })?;

        let insert_result = connection.execute(
            "INSERT INTO assets (\n\
                 id, original_filename, mime_type, byte_size, relative_path, created_at\n\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                original_filename,
                mime_type,
                i64::try_from(metadata.len()).unwrap_or(i64::MAX),
                relative_path,
                now
            ],
        );

        if let Err(error) = insert_result {
            let _ = fs::remove_file(&final_path);
            return Err(CommandError::database(error));
        }

        let asset = AssetDto {
            id,
            original_filename: original_filename.clone(),
            mime_type: mime_type.to_string(),
            byte_size: i64::try_from(metadata.len()).unwrap_or(i64::MAX),
            file_path: final_path.to_string_lossy().into_owned(),
            created_at: now,
        };

        Ok((asset, final_path))
    })?;

    if !final_path.exists() {
        return Err(CommandError::new(
            "file_error",
            "The image record was saved, but its local file could not be found.",
        ));
    }

    Ok(asset)
}

pub(crate) fn get_assets_for_note(
    connection: &Connection,
    paths: &AppPaths,
    note_id: &str,
) -> Result<Vec<AssetDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT DISTINCT a.id, a.original_filename, a.mime_type, a.byte_size,\n\
                    a.relative_path, a.created_at\n\
             FROM assets a\n\
             JOIN note_assets na ON na.asset_id = a.id\n\
             WHERE na.note_id = ?1 AND a.deleted_at IS NULL\n\
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

pub(crate) fn ensure_assets_exist(
    connection: &Connection,
    asset_ids: &[String],
) -> Result<(), CommandError> {
    for asset_id in asset_ids {
        validate_entity_id(asset_id, "asset")?;
        let exists: i64 = connection
            .query_row(
                "SELECT EXISTS(\n\
                     SELECT 1 FROM assets WHERE id = ?1 AND deleted_at IS NULL\n\
                 )",
                [asset_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if exists == 0 {
            return Err(CommandError::new(
                "asset_not_found",
                "One of the images used by this card is no longer available.",
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

fn detect_image_type(path: &Path) -> Result<(&'static str, &'static str), CommandError> {
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
