//! Retry-safe whole-set source retention. A member failure cannot publish a
//! shortened document; every selected lease is consumed in the same transaction.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::{
    database::Database,
    document_canonical::sha256_hex,
    error::CommandError,
    external_files,
    paths::AppPaths,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

use super::{
    admission,
    files::InitialSource,
    manifest::{self, invalid, ImageManifest, ImageMember, InclusionCrop, MANIFEST_MEDIA_TYPE},
    repository,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageSelectionInput {
    pub staged_file_id: String,
    pub quarter_turns: u8,
    pub inclusion: Option<InclusionCrop>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportImageSetInput {
    pub operation_id: String,
    pub title: String,
    pub members: Vec<ImageSelectionInput>,
}

impl ImportImageSetInput {
    pub(crate) fn validate(&self) -> Result<Vec<String>, CommandError> {
        validate_entity_id(&self.operation_id, "image import operation")?;
        if self.title.trim().is_empty()
            || self.title.chars().count() > 255
            || self.title.chars().any(char::is_control)
        {
            return Err(invalid("Give the image set a title of 1 to 255 characters."));
        }
        let ids = self
            .members
            .iter()
            .map(|member| member.staged_file_id.clone())
            .collect::<Vec<_>>();
        admission::validate_selection(&ids)?;
        for member in &self.members {
            if member.quarter_turns > 3 {
                return Err(invalid(
                    "Image orientation must use zero to three quarter turns.",
                ));
            }
            if let Some(crop) = member.inclusion {
                crop.validate()?;
            }
        }
        Ok(ids)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportedImageSet {
    pub document_id: String,
    pub revision: i64,
}

fn receipt(
    connection: &Connection,
    operation_id: &str,
    request_hash: &str,
) -> Result<Option<ImportedImageSet>, CommandError> {
    let row: Option<(String, Option<String>)> = connection
        .query_row(
            "SELECT request_sha256, document_id FROM document_image_imports WHERE operation_id = ?1",
            [operation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    match row {
        None => Ok(None),
        Some((saved_hash, owner)) => {
            if saved_hash != request_hash {
                return Err(CommandError::new(
                    "image_set_operation_conflict",
                    "This import operation already has different inputs. Reload its result before starting a new import.",
                ));
            }
            let document_id = owner.ok_or_else(|| {
                CommandError::new(
                    "image_set_import_removed",
                    "The source created by this import was deleted. Retrying cannot recreate it.",
                )
            })?;
            Ok(Some(ImportedImageSet {
                document_id,
                revision: 1,
            }))
        }
    }
}

pub(crate) fn import(
    database: &Database,
    paths: &AppPaths,
    input: &ImportImageSetInput,
) -> Result<ImportedImageSet, CommandError> {
    let ids = input.validate()?;
    let request_hash = sha256_hex(&serde_json::to_vec(input).map_err(CommandError::serialization)?);
    if let Some(saved) = database
        .with_connection(|connection| receipt(connection, &input.operation_id, &request_hash))?
    {
        return Ok(saved);
    }
    let inspection = admission::inspect(database, paths, &ids)?;
    if !inspection.can_import {
        return Err(CommandError::new(
            "image_set_selection_failed",
            "One or more selected images failed inspection. Remove or retry those selections; no document was imported.",
        ));
    }
    let (document_id, member_ids) = database.with_connection(|connection| {
        let document_id = generate_id(connection)?;
        let members = ids
            .iter()
            .map(|_| generate_id(connection))
            .collect::<Result<Vec<_>, _>>()?;
        Ok((document_id, members))
    })?;
    let mut members = Vec::with_capacity(ids.len());
    for ((selection, inspected), member_id) in input
        .members
        .iter()
        .zip(&inspection.members)
        .zip(member_ids)
    {
        let image = inspected
            .image
            .as_ref()
            .ok_or_else(|| invalid("Image inspection is incomplete."))?;
        let member = ImageMember {
            relative_path: manifest::original_path(&document_id, &member_id, &image.media_type)?,
            id: member_id,
            original_filename: image.original_filename.clone(),
            media_type: image.media_type.clone(),
            byte_size: image.byte_size,
            sha256: image.sha256.clone(),
            width: image.width,
            height: image.height,
            pixel_sha256: image.pixel_sha256.clone(),
            pixel_contract: image.pixel_contract.to_string(),
            quarter_turns: selection.quarter_turns,
            inclusion: selection.inclusion,
        };
        members.push(member);
    }
    let manifest = ImageManifest {
        version: 1,
        document_id,
        revision: 1,
        members,
    };
    let manifest_bytes = manifest.bytes()?;
    let manifest_hash = sha256_hex(&manifest_bytes);
    let relative_manifest = manifest::manifest_path(&manifest.document_id, &manifest_hash)?;
    let mut files = InitialSource::begin(paths, &manifest, &input.operation_id)?;
    for ((selection, inspected), member) in input
        .members
        .iter()
        .zip(&inspection.members)
        .zip(&manifest.members)
    {
        let image = inspected
            .image
            .as_ref()
            .ok_or_else(|| invalid("Image inspection is incomplete."))?;
        let bytes = admission::read_inspected(database, paths, &selection.staged_file_id, image)?;
        files.write(paths, &member.relative_path, &bytes)?;
    }
    files.write(paths, &relative_manifest, &manifest_bytes)?;
    let now = unix_time_millis()?;
    let result = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        // Another attempt with this operation may have committed while files
        // were copied. Return its receipt and discard only our candidate files.
        if let Some(saved) = receipt(&transaction, &input.operation_id, &request_hash)? {
            return Ok(saved);
        }
        for (selection, member) in input.members.iter().zip(&manifest.members) {
            validate_lease(&transaction, &selection.staged_file_id, member, now)?;
        }
        transaction
            .execute(
                "INSERT INTO documents (
                 id, title, original_filename, mime_type, source_media_type, byte_size,
                 sha256, source_relative_path, staged_relative_path, lifecycle_state,
                 source_format, source_unit_kind, source_unit_count, adapter_backend,
                 extraction_backend, render_backend, outline_state, created_at, updated_at, revision
             ) VALUES (?1, ?2, 'Ordered images', 'application/pdf', ?3, ?4, ?5, ?6, NULL,
                       'processing', 'image_bundle', 'image', ?7, 'pending', 'pending', 'pending',
                       'pending', ?8, ?8, 1)",
                params![
                    manifest.document_id,
                    input.title.trim(),
                    MANIFEST_MEDIA_TYPE,
                    manifest_bytes.len() as i64,
                    manifest_hash,
                    relative_manifest,
                    manifest.members.len() as i64,
                    now
                ],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "INSERT INTO document_image_sets (document_id, manifest_version, revision, manifest_sha256)
             VALUES (?1, 1, 1, ?2)",
                params![manifest.document_id, manifest_hash],
            )
            .map_err(CommandError::database)?;
        repository::insert_members(&transaction, &manifest)?;
        transaction
            .execute(
                "INSERT INTO document_image_imports (operation_id, request_sha256, document_id, created_at)
             VALUES (?1, ?2, ?3, ?4)",
                params![input.operation_id, request_hash, manifest.document_id, now],
            )
            .map_err(CommandError::database)?;
        let result = ImportedImageSet {
            document_id: manifest.document_id.clone(),
            revision: 1,
        };
        for id in &ids {
            external_files::mark_import_consumed(&transaction, id, &result, now)?;
        }
        transaction.commit().map_err(CommandError::database)?;
        Ok(result)
    })?;
    if result.document_id == manifest.document_id {
        files.mark_committed();
    }
    Ok(result)
}

fn validate_lease(
    connection: &Connection,
    id: &str,
    member: &ImageMember,
    now: i64,
) -> Result<(), CommandError> {
    let path = AppPaths::staged_import_relative_path(id, manifest::extension(&member.media_type)?);
    let matched: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM staged_external_imports
         WHERE id = ?1 AND state = 'staged' AND purpose = 'image'
           AND relative_path = ?2 AND byte_size = ?3 AND sha256 = ?4 AND expires_at > ?5)",
            params![id, path, member.byte_size, member.sha256, now],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if !matched {
        return Err(CommandError::new(
            "image_set_selection_changed",
            "An image selection changed or was consumed during import. No partial set was published.",
        ));
    }
    Ok(())
}
