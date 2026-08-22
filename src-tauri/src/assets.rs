use std::collections::{BTreeSet, HashMap};

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
    if bytes.starts_with(b"BM") {
        return Ok(("image/bmp", "bmp"));
    }
    if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        return Ok(("image/tiff", "tif"));
    }
    if bytes.len() >= 44
        && bytes.starts_with(&[1, 0, 0, 0])
        && &bytes[40..44] == b" EMF"
    {
        return Ok(("image/emf", "emf"));
    }
    if bytes.starts_with(&[0xd7, 0xcd, 0xc6, 0x9a])
        || (bytes.len() >= 6
            && matches!(&bytes[..2], [1, 0] | [2, 0])
            && &bytes[2..4] == [9, 0]
            && &bytes[4..6] == [0, 3])
    {
        return Ok(("image/wmf", "wmf"));
    }
    if looks_like_svg(bytes) {
        return Ok(("image/svg+xml", "svg"));
    }
    Err(CommandError::new(
        "unsupported_image",
        "Use a supported image file.",
    ))
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let prefix = &bytes[..bytes.len().min(4096)];
    let Ok(text) = std::str::from_utf8(prefix.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(prefix))
    else {
        return false;
    };
    let trimmed = text.trim_start();
    trimmed.starts_with("<svg")
        || (trimmed.starts_with("<?xml") && trimmed.find("<svg").is_some_and(|index| index < 2048))
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
        assert_eq!(
            detect_image_bytes(b"BMfixture").expect("BMP should be recognized"),
            ("image/bmp", "bmp")
        );
        assert_eq!(
            detect_image_bytes(b"II*\0fixture").expect("TIFF should be recognized"),
            ("image/tiff", "tif")
        );
        assert_eq!(
            detect_image_bytes(b"<?xml version=\"1.0\"?><svg></svg>")
                .expect("SVG should be recognized"),
            ("image/svg+xml", "svg")
        );
        let mut emf = vec![0_u8; 44];
        emf[..4].copy_from_slice(&[1, 0, 0, 0]);
        emf[40..44].copy_from_slice(b" EMF");
        assert_eq!(
            detect_image_bytes(&emf).expect("EMF should be recognized"),
            ("image/emf", "emf")
        );
        assert_eq!(
            detect_image_bytes(&[0xd7, 0xcd, 0xc6, 0x9a]).expect("WMF should be recognized"),
            ("image/wmf", "wmf")
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
