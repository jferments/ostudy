//! Image-set ownership and exact manifest projection. No file or provider IO.

use rusqlite::{params, Connection, OptionalExtension};

use crate::{document_canonical::sha256_hex, error::CommandError};

use super::manifest::{invalid, manifest_path, ImageManifest, ImageMember, MANIFEST_MEDIA_TYPE};

pub(crate) fn load(
    connection: &Connection,
    document_id: &str,
) -> Result<ImageManifest, CommandError> {
    let header: Option<(u32, i64, String)> = connection
        .query_row(
            "SELECT manifest_version, revision, manifest_sha256
             FROM document_image_sets WHERE document_id = ?1",
            [document_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let (version, revision, expected_hash) =
        header.ok_or_else(|| invalid("The selected document is not a retained image set."))?;
    let mut statement = connection
        .prepare(
            "SELECT position, id, original_filename, media_type, relative_path,
                    byte_size, sha256, width, height, pixel_sha256, pixel_contract,
                    quarter_turns, inclusion_json
             FROM document_image_members WHERE document_id = ?1 ORDER BY position",
        )
        .map_err(CommandError::database)?;
    let mut rows = statement
        .query([document_id])
        .map_err(CommandError::database)?;
    let mut members = Vec::new();
    while let Some(row) = rows.next().map_err(CommandError::database)? {
        let position: i64 = row.get(0).map_err(CommandError::database)?;
        if position != members.len() as i64 || members.len() >= 128 {
            return Err(invalid(
                "Image-set positions are missing, repeated or unbounded.",
            ));
        }
        let inclusion: Option<String> = row.get(12).map_err(CommandError::database)?;
        let member = ImageMember {
            id: row.get(1).map_err(CommandError::database)?,
            original_filename: row.get(2).map_err(CommandError::database)?,
            media_type: row.get(3).map_err(CommandError::database)?,
            relative_path: row.get(4).map_err(CommandError::database)?,
            byte_size: unsigned_size(row.get(5).map_err(CommandError::database)?)?,
            sha256: row.get(6).map_err(CommandError::database)?,
            width: row.get(7).map_err(CommandError::database)?,
            height: row.get(8).map_err(CommandError::database)?,
            pixel_sha256: row.get(9).map_err(CommandError::database)?,
            pixel_contract: row.get(10).map_err(CommandError::database)?,
            quarter_turns: row.get(11).map_err(CommandError::database)?,
            inclusion: inclusion
                .map(|value| {
                    serde_json::from_str(&value)
                        .map_err(|_| invalid("Unsupported saved image inclusion crop."))
                })
                .transpose()?,
        };
        members.push(member);
    }
    let manifest = ImageManifest {
        version,
        document_id: document_id.to_string(),
        revision,
        members,
    };
    if manifest.fingerprint()? != expected_hash {
        return Err(invalid(
            "Image-set members do not match the saved manifest fingerprint.",
        ));
    }
    Ok(manifest)
}

pub(crate) fn insert_members(
    connection: &Connection,
    manifest: &ImageManifest,
) -> Result<(), CommandError> {
    manifest.validate()?;
    for (position, member) in manifest.members.iter().enumerate() {
        let inclusion = member
            .inclusion
            .map(|crop| serde_json::to_string(&crop))
            .transpose()
            .map_err(CommandError::serialization)?;
        connection
            .execute(
                "INSERT INTO document_image_members (
                 id, document_id, position, original_filename, media_type, relative_path,
                 byte_size, sha256, width, height, pixel_sha256, pixel_contract,
                 quarter_turns, inclusion_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    member.id,
                    manifest.document_id,
                    position as i64,
                    member.original_filename,
                    member.media_type,
                    member.relative_path,
                    sql_size(member.byte_size)?,
                    member.sha256,
                    member.width,
                    member.height,
                    member.pixel_sha256,
                    member.pixel_contract,
                    member.quarter_turns,
                    inclusion
                ],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

pub(crate) fn validate_manifest_bytes(
    connection: &Connection,
    document_id: &str,
    bytes: &[u8],
) -> Result<(), CommandError> {
    let parsed = ImageManifest::decode(bytes)?;
    let stored = load(connection, document_id)?;
    if parsed != stored {
        return Err(invalid(
            "The retained manifest does not match its document-owned members.",
        ));
    }
    Ok(())
}

/// Read-only startup and complete-restore admission, before conversion writes.
pub(crate) fn validate(connection: &Connection) -> Result<(), CommandError> {
    let invalid_owners: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM document_image_sets AS s
         LEFT JOIN documents AS d ON d.id = s.document_id
         WHERE d.id IS NULL OR d.source_format <> 'image_bundle' OR d.source_unit_kind <> 'image'",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let missing_sets: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM documents AS d
         LEFT JOIN document_image_sets AS s ON s.document_id = d.id
         WHERE d.source_format = 'image_bundle' AND s.document_id IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if invalid_owners != 0 || missing_sets != 0 {
        return Err(invalid("Image-set source ownership is inconsistent."));
    }
    let mut statement = connection
        .prepare(
            "SELECT d.id, d.source_relative_path, d.sha256, d.byte_size, d.source_media_type,
                s.prepared_manifest_sha256
         FROM documents AS d JOIN document_image_sets AS s ON s.document_id = d.id ORDER BY d.id",
        )
        .map_err(CommandError::database)?;
    let mut rows = statement.query([]).map_err(CommandError::database)?;
    while let Some(row) = rows.next().map_err(CommandError::database)? {
        let id: String = row.get(0).map_err(CommandError::database)?;
        let path: String = row.get(1).map_err(CommandError::database)?;
        let hash: String = row.get(2).map_err(CommandError::database)?;
        let size = unsigned_size(row.get(3).map_err(CommandError::database)?)?;
        let media: String = row.get(4).map_err(CommandError::database)?;
        let prepared: String = row.get(5).map_err(CommandError::database)?;
        let manifest = load(connection, &id)?;
        let bytes = manifest.bytes()?;
        if path != manifest_path(&id, &hash)?
            || hash != sha256_hex(&bytes)
            || size != bytes.len() as u64
            || media != MANIFEST_MEDIA_TYPE
        {
            return Err(invalid(
                "The document and image-set manifest ledgers disagree.",
            ));
        }
        if !prepared.is_empty() {
            super::manifest::validate_hash(&prepared)?;
        }
    }
    let invalid_members: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM document_image_members AS m
             LEFT JOIN document_image_sets AS s ON s.document_id = m.document_id
             WHERE s.document_id IS NULL)",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if invalid_members {
        return Err(invalid(
            "An original image has no retained image-set owner.",
        ));
    }
    let mut operations = connection
        .prepare(
            "SELECT i.operation_id, i.request_sha256, i.document_id, s.document_id
             FROM document_image_imports AS i
             LEFT JOIN document_image_sets AS s ON s.document_id = i.document_id",
        )
        .map_err(CommandError::database)?;
    let mut rows = operations.query([]).map_err(CommandError::database)?;
    while let Some(row) = rows.next().map_err(CommandError::database)? {
        let operation: String = row.get(0).map_err(CommandError::database)?;
        let request_hash: String = row.get(1).map_err(CommandError::database)?;
        let document: Option<String> = row.get(2).map_err(CommandError::database)?;
        let owner: Option<String> = row.get(3).map_err(CommandError::database)?;
        crate::util::validate_entity_id(&operation, "image import operation")?;
        super::manifest::validate_hash(&request_hash)?;
        if document != owner {
            return Err(invalid(
                "An image import receipt refers to a different source type.",
            ));
        }
    }
    Ok(())
}

/// SQLite stores signed 64-bit integers; never wrap image sizes at its boundary.
pub(super) fn sql_size(value: u64) -> Result<i64, CommandError> {
    i64::try_from(value).map_err(|_| invalid("Image byte size exceeds SQLite's integer range."))
}

fn unsigned_size(value: i64) -> Result<u64, CommandError> {
    u64::try_from(value).map_err(|_| invalid("A saved image byte size is negative."))
}

#[cfg(test)]
mod integer_tests {
    use super::*;

    #[test]
    fn image_set_sql_sizes_round_trip_without_wrapping_or_truncation() {
        let connection = Connection::open_in_memory().unwrap();
        for size in [0, 1, 25 * 1024 * 1024, 512 * 1024 * 1024, i64::MAX as u64] {
            let saved: i64 = connection
                .query_row("SELECT ?1", [sql_size(size).unwrap()], |row| row.get(0))
                .unwrap();
            assert_eq!(unsigned_size(saved).unwrap(), size);
        }
        assert!(sql_size(i64::MAX as u64 + 1).is_err());
        assert!(sql_size(u64::MAX).is_err());
        assert!(unsigned_size(-1).is_err());
        assert!(unsigned_size(i64::MIN).is_err());
    }
}
