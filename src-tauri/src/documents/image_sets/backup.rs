//! Whole-library transfer integration for independently retained original images.
//! Originals use the existing source_asset envelope, but never become model-visible
//! canonical assets merely because they are included in a backup.

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    cards::questions::occlusion::CanonicalImage,
    document_backup::table_exists,
    error::CommandError,
};

use super::{manifest, repository};

type SourceAsset = (String, String, String, String, i64, String);
type StoredFile = (String, String, i64, String);

pub(crate) fn originals(connection: &Connection) -> Result<Vec<SourceAsset>, CommandError> {
    if !table_exists(connection, "document_image_members")? {
        return Ok(Vec::new());
    }
    repository::validate(connection)?;
    let mut statement = connection.prepare(
        "SELECT document_id, id, media_type, relative_path, byte_size, sha256
         FROM document_image_members ORDER BY document_id, position",
    ).map_err(CommandError::database)?;
    let rows = statement.query_map([], |row| {
        let id: String = row.get(1)?;
        Ok((row.get(0)?, format!("original-member-{id}"), row.get(2)?,
            row.get(3)?, row.get(4)?, row.get(5)?))
    }).map_err(CommandError::database)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(CommandError::database)
}

pub(crate) fn original_count(connection: &Connection) -> Result<i64, CommandError> {
    if !table_exists(connection, "document_image_members")? {
        return Ok(0);
    }
    connection.query_row("SELECT COUNT(*) FROM document_image_members", [], |row| row.get(0))
        .map_err(CommandError::database)
}

pub(crate) fn original_record(
    connection: &Connection,
    document_id: &str,
    relative_path: &str,
) -> Result<Option<StoredFile>, CommandError> {
    if !table_exists(connection, "document_image_members")? {
        return Ok(None);
    }
    connection.query_row(
        "SELECT relative_path, media_type, byte_size, sha256 FROM document_image_members
         WHERE document_id = ?1 AND relative_path = ?2",
        params![document_id, relative_path],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).optional().map_err(CommandError::database)
}

pub(crate) fn valid_original_path(document_id: &str, path: &str) -> bool {
    let prefix = format!("documents/{document_id}/originals/");
    let Some(file) = path.strip_prefix(&prefix) else { return false };
    let Some((id, extension)) = file.split_once('.') else { return false };
    let media = match extension {
        "png" => "image/png",
        "jpg" => "image/jpeg",
        "webp" => "image/webp",
        _ => return false,
    };
    manifest::original_path(document_id, id, media).is_ok_and(|expected| expected == path)
}

pub(crate) fn valid_manifest_path(document_id: &str, path: &str) -> bool {
    let prefix = format!("documents/{document_id}/source-");
    let Some(hash) = path.strip_prefix(&prefix).and_then(|suffix| suffix.strip_suffix(".imageset")) else {
        return false;
    };
    manifest::manifest_path(document_id, hash).is_ok_and(|expected| expected == path)
}

pub(crate) fn validate_original_bytes(
    connection: &Connection,
    document_id: &str,
    relative_path: &str,
    bytes: &[u8],
) -> Result<(), CommandError> {
    if !valid_original_path(document_id, relative_path) {
        return Ok(());
    }
    let saved = repository::load(connection, document_id)?;
    let member = saved.members.iter().find(|member| member.relative_path == relative_path)
        .ok_or_else(|| manifest::invalid("The original image has no retained owner."))?;
    let image = CanonicalImage::decode(bytes)?;
    if image.dimensions() != (member.width, member.height)
        || image.fingerprint() != member.pixel_sha256
        || crate::document_canonical::sha256_hex(bytes) != member.sha256
    {
        return Err(manifest::invalid("The original image does not match its admitted pixels."));
    }
    Ok(())
}
