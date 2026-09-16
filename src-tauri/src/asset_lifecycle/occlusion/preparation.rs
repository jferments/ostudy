use sha2::{Digest, Sha256};

use super::*;

const IMPORT_VERSION: u32 = 1;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportReceipt {
    occlusion_import_version: u32,
    draft_id: String,
    prepared: PreparedOcclusionImage,
}

pub(super) fn prepare(
    database: &Database,
    paths: &AppPaths,
    input: ImportImageInput,
) -> Result<PreparedOcclusionImage, CommandError> {
    validate_entity_id(&input.draft_id, "asset draft")?;
    validate_entity_id(&input.staged_file_id, "staged image")?;
    let initial = database.with_connection(|connection| {
        if let Some(receipt) = receipt(connection, &input)? {
            return Ok(Ok(receipt));
        }
        ensure_active_draft(connection, &input.draft_id, unix_time_millis()?)?;
        let staged = external_files::resolve_staged_import(
            connection,
            paths,
            &input.staged_file_id,
            ExternalImportPurpose::Image,
            &[
                ExternalFileKind::Png,
                ExternalFileKind::Jpeg,
                ExternalFileKind::Webp,
            ],
        )?;
        let fingerprint: String = connection
            .query_row(
                "SELECT sha256 FROM staged_external_imports WHERE id = ?1",
                [&input.staged_file_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        Ok(Err((
            File::open(staged.path).map_err(CommandError::io)?,
            fingerprint,
        )))
    })?;
    let (file, source_fingerprint) = match initial {
        Ok(receipt) => return Ok(receipt),
        Err(candidate) => candidate,
    };
    let bytes = read_image(file)?;
    if format!("{:x}", Sha256::digest(&bytes)) != source_fingerprint {
        return Err(unavailable());
    }
    let image = CanonicalImage::decode(&bytes)?;
    drop(bytes);
    let (width, height) = image.dimensions();
    let fingerprint = image.fingerprint().to_string();
    let png = image.canonical_png()?;
    // Avoid retaining decoded pixels while copying the canonical staged file.
    drop(image);
    let scratch_id = database.with_connection(|connection| generate_id(connection))?;
    let scratch_path = paths
        .asset_staging_dir()
        .join(format!("{scratch_id}.occlusion-partial"));
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&scratch_path)
        .map_err(CommandError::io)?;
    let scratch = RemoveOnDrop(scratch_path);
    output.write_all(&png).map_err(CommandError::io)?;
    output.sync_all().map_err(CommandError::io)?;
    drop(output);
    drop(png);
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        // Another successful retry or a cancellation may have won during decode.
        if let Some(receipt) = receipt(&transaction, &input)? {
            return Ok(receipt);
        }
        ensure_active_draft(&transaction, &input.draft_id, unix_time_millis()?)?;
        external_files::resolve_staged_import(
            &transaction,
            paths,
            &input.staged_file_id,
            ExternalImportPurpose::Image,
            &[
                ExternalFileKind::Png,
                ExternalFileKind::Jpeg,
                ExternalFileKind::Webp,
            ],
        )?;
        let asset = stage_source_named_in_connection(
            &transaction,
            paths,
            &scratch.0,
            &input.draft_id,
            Some("occlusion-source.png"),
            IngestFaults::default(),
        )?;
        let mut staged = RemoveOnDrop(paths.resolve_relative(
            &AppPaths::staged_asset_relative_path(&input.draft_id, &asset.id, "png"),
        ));
        let prepared = PreparedOcclusionImage {
            asset,
            width,
            height,
            pixel_fingerprint: fingerprint,
        };
        let stored = ImportReceipt {
            occlusion_import_version: IMPORT_VERSION,
            draft_id: input.draft_id.clone(),
            prepared,
        };
        external_files::mark_import_consumed(
            &transaction,
            &input.staged_file_id,
            &stored,
            unix_time_millis()?,
        )?;
        transaction.commit().map_err(CommandError::database)?;
        // No fallible read follows committed success. Replaying a lost response
        // returns the same asset receipt, never another normalized image.
        staged.0.clear();
        Ok(stored.prepared)
    })
}

fn receipt(
    connection: &Connection,
    input: &ImportImageInput,
) -> Result<Option<PreparedOcclusionImage>, CommandError> {
    let Some(value) = external_files::consumed_import_result::<serde_json::Value>(
        connection,
        &input.staged_file_id,
    )?
    else {
        return Ok(None);
    };
    let stored: ImportReceipt = serde_json::from_value(value).map_err(|_| conflict())?;
    if stored.occlusion_import_version != IMPORT_VERSION || stored.draft_id != input.draft_id {
        return Err(conflict());
    }
    Ok(Some(stored.prepared))
}

fn conflict() -> CommandError {
    CommandError::new(
        "occlusion_import_conflict",
        "That file import was already used by another draft or image operation.",
    )
}

struct RemoveOnDrop(PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        if !self.0.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.0);
        }
    }
}
