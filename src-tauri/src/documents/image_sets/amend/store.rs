//! SQL portion of a source amendment. The caller stages files first.

use rusqlite::{params, Connection, TransactionBehavior};

use crate::{
    database::Database, document_canonical::sha256_hex, error::CommandError, external_files,
    paths::AppPaths, util::unix_time_millis,
};

use super::super::{
    manifest,
    operations::{self, EditResult},
    repository, storage,
};
use super::{AmendImageSetInput, ImageManifest};

pub(super) fn require_revision(
    manifest: &ImageManifest,
    filter_revision: i64,
    input: &AmendImageSetInput,
) -> Result<(), CommandError> {
    if manifest.revision != input.expected_revision
        || filter_revision != input.expected_filter_revision
    {
        return Err(CommandError::new(
            "image_set_changed",
            "The image source or filters changed. Reload before applying these edits.",
        ));
    }
    Ok(())
}

pub(super) fn publish(
    database: &Database,
    input: &AmendImageSetInput,
    hash: &str,
    expected: &ImageManifest,
    next: &ImageManifest,
) -> Result<EditResult, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        if let Some(saved) = operations::load(&transaction, &input.operation_id, &input.document_id, hash)? {
            return Ok(saved);
        }
        let current = repository::load(&transaction, &input.document_id)?;
        let settings = storage::settings(&transaction, &input.document_id)?;
        require_revision(&current, settings.revision, input)?;
        if current != *expected {
            return Err(manifest::invalid("The retained image source changed while files were staged."));
        }
        let result = EditResult {
            document_id: input.document_id.clone(),
            revision: next.revision,
            filter_revision: settings.revision,
        };
        if current != *next {
            freeze_projection(&transaction, &current)?;
            transaction.execute(
                "UPDATE document_image_members SET position = position + 128 WHERE document_id = ?1",
                [&current.document_id],
            ).map_err(CommandError::database)?;
            for (position, member) in next.members[..current.members.len()].iter().enumerate() {
                let crop = member.inclusion.map(|crop| serde_json::to_string(&crop))
                    .transpose().map_err(CommandError::serialization)?;
                let changed = transaction.execute(
                    "UPDATE document_image_members SET position = ?1, quarter_turns = ?2, inclusion_json = ?3
                     WHERE document_id = ?4 AND id = ?5",
                    params![position as i64, member.quarter_turns, crop, current.document_id, member.id],
                ).map_err(CommandError::database)?;
                if changed != 1 {
                    return Err(manifest::invalid("An edited image no longer has its source owner."));
                }
            }
            for (index, (selection, member)) in input.append.iter()
                .zip(&next.members[current.members.len()..]).enumerate()
            {
                require_lease(&transaction, &selection.staged_file_id, member, now)?;
                let crop = member.inclusion.map(|crop| serde_json::to_string(&crop))
                    .transpose().map_err(CommandError::serialization)?;
                transaction.execute(
                    "INSERT INTO document_image_members (id, document_id, position, original_filename,
                     media_type, relative_path, byte_size, sha256, width, height, pixel_sha256,
                     pixel_contract, quarter_turns, inclusion_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                    params![member.id, next.document_id, (current.members.len() + index) as i64,
                        member.original_filename, member.media_type, member.relative_path,
                        repository::sql_size(member.byte_size)?, member.sha256, member.width, member.height,
                        member.pixel_sha256, member.pixel_contract, member.quarter_turns, crop],
                ).map_err(CommandError::database)?;
                external_files::mark_import_consumed(&transaction, &selection.staged_file_id, &result, now)?;
            }
            let bytes = next.bytes()?;
            let fingerprint = sha256_hex(&bytes);
            transaction.execute(
                "UPDATE document_image_sets SET revision = ?1, manifest_sha256 = ?2 WHERE document_id = ?3",
                params![next.revision, fingerprint, next.document_id],
            ).map_err(CommandError::database)?;
            transaction.execute(
                "UPDATE documents SET source_relative_path = ?1, sha256 = ?2, byte_size = ?3,
                 source_unit_count = ?4 WHERE id = ?5",
                params![manifest::manifest_path(&next.document_id, &fingerprint)?, fingerprint,
                    bytes.len() as i64, next.members.len() as i64, next.document_id],
            ).map_err(CommandError::database)?;
            storage::invalidate(&transaction, &next.document_id, now)?;
        }
        operations::save(&transaction, &input.operation_id, hash, &result, now)?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(result)
    })
}

fn freeze_projection(connection: &Connection, current: &ImageManifest) -> Result<(), CommandError> {
    let Some(mut receipt) = storage::load(connection, &current.document_id)? else {
        return Ok(());
    };
    if receipt.source_manifest.is_some() {
        return Ok(());
    }
    receipt.source_manifest = Some(current.clone());
    let json = serde_json::to_string(&receipt).map_err(CommandError::serialization)?;
    if json.len() > super::super::receipt::MAX_RECEIPT_BYTES {
        return Err(manifest::invalid(
            "The retained source snapshot exceeds its receipt budget.",
        ));
    }
    connection.execute(
        "UPDATE document_image_receipts SET payload_json = ?1, payload_sha256 = ?2 WHERE document_id = ?3",
        params![json, sha256_hex(json.as_bytes()), current.document_id],
    ).map_err(CommandError::database)?;
    Ok(())
}

fn require_lease(
    connection: &Connection,
    id: &str,
    member: &super::ImageMember,
    now: i64,
) -> Result<(), CommandError> {
    let path = AppPaths::staged_import_relative_path(id, manifest::extension(&member.media_type)?);
    let matched: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM staged_external_imports WHERE id = ?1 AND state = 'staged'
         AND purpose = 'image' AND relative_path = ?2 AND byte_size = ?3 AND sha256 = ?4 AND expires_at > ?5)",
        params![id, path, repository::sql_size(member.byte_size)?, member.sha256, now],
        |row| row.get(0),
    ).map_err(CommandError::database)?;
    if !matched {
        return Err(manifest::invalid(
            "An appended selection changed or expired. No partial edit was saved.",
        ));
    }
    Ok(())
}
