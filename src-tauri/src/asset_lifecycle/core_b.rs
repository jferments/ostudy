pub(crate) fn finalize_assets_for_owner(
    transaction: &Transaction<'_>,
    paths: &AppPaths,
    asset_ids: &[String],
    draft_id: Option<&str>,
    owner_id: Option<&str>,
) -> Result<DraftFinalization, CommandError> {
    finalize_assets_with_faults(
        transaction,
        paths,
        asset_ids,
        draft_id,
        owner_id,
        PromotionFaults::default(),
    )
}

pub(crate) fn ensure_assets_available(
    connection: &Connection,
    paths: &AppPaths,
    asset_ids: &[String],
    draft_id: Option<&str>,
) -> Result<(), CommandError> {
    for asset_id in normalize_asset_ids(asset_ids.to_vec())? {
        let asset = load_asset(connection, &asset_id)?;
        match asset.lifecycle_state.as_str() {
            "ready" => {
                let final_path = paths.resolve_relative(&asset.relative_path);
                if !final_path.is_file() {
                    return Err(CommandError::new(
                        "asset_missing",
                        format!(
                            "The image '{}' is missing from the local library.",
                            asset.original_filename
                        ),
                    ));
                }
            }
            "staged" if asset.draft_id.as_deref() == draft_id => {
                let staged = asset.staged_relative_path.as_deref().ok_or_else(|| {
                    CommandError::new("asset_stage_missing", "A staged image has no managed path.")
                })?;
                if !paths.resolve_relative(staged).is_file() {
                    return Err(CommandError::new(
                        "asset_stage_missing",
                        "A staged image is no longer available. Add it again.",
                    ));
                }
            }
            "staged" => {
                return Err(CommandError::new(
                    "asset_draft_mismatch",
                    "A staged image belongs to another editor draft.",
                ));
            }
            "missing" => {
                return Err(CommandError::new(
                    "asset_missing",
                    format!(
                        "The image '{}' is missing from the local library.",
                        asset.original_filename
                    ),
                ));
            }
            "quarantined" => {
                return Err(CommandError::new(
                    "asset_quarantined",
                    "A referenced image was quarantined and must be restored before saving.",
                ));
            }
            _ => {
                return Err(CommandError::new(
                    "asset_not_found",
                    "One of the images used by this content is no longer available.",
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn reconcile_startup(database: &Database, paths: &AppPaths) -> Result<(), CommandError> {
    let cleanup = database.with_connection(|connection| {
        reconcile_in_connection(connection, paths, unix_time_millis()?)
    })?;
    remove_paths_best_effort(&cleanup);
    Ok(())
}

fn validate_draft_purpose(value: &str) -> Result<&str, CommandError> {
    match value {
        "card" | "template" | "import" => Ok(value),
        _ => Err(CommandError::new(
            "invalid_asset_draft_purpose",
            "Choose a supported image draft purpose.",
        )),
    }
}

fn begin_draft_in_connection(
    connection: &mut Connection,
    purpose: &str,
    owner_id: Option<&str>,
) -> Result<AssetDraftDto, CommandError> {
    let purpose = validate_draft_purpose(purpose)?;
    let now = unix_time_millis()?;
    let id = generate_id(connection)?;
    let expires_at = now.saturating_add(DRAFT_TTL_MS);
    connection
        .execute(
            "INSERT INTO asset_drafts (
                 id, purpose, owner_id, created_at, updated_at, expires_at
             ) VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
            params![id, purpose, owner_id, now, expires_at],
        )
        .map_err(CommandError::database)?;
    Ok(AssetDraftDto {
        id,
        purpose: purpose.to_string(),
        expires_at,
    })
}

fn ensure_active_draft(
    connection: &Connection,
    draft_id: &str,
    now: i64,
) -> Result<String, CommandError> {
    type ActiveDraftRow = (String, i64, Option<i64>, Option<i64>, String);
    let row: Option<ActiveDraftRow> = connection
        .query_row(
            "SELECT purpose, expires_at, committed_at, canceled_at, retention_mode
             FROM asset_drafts WHERE id = ?1",
            [draft_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    match row {
        None => Err(CommandError::new(
            "asset_draft_not_found",
            "This image draft no longer exists.",
        )),
        Some((_, _, Some(_), _, _)) => Err(CommandError::new(
            "asset_draft_committed",
            "This image draft has already been saved.",
        )),
        Some((_, _, _, Some(_), _)) => Err(CommandError::new(
            "asset_draft_canceled",
            "This image draft has already been canceled.",
        )),
        Some((_, expires_at, None, None, retention_mode))
            if retention_mode == "ephemeral" && expires_at < now =>
        {
            Err(CommandError::new(
                "asset_draft_expired",
                "This image draft expired. Reopen the editor and add the image again.",
            ))
        }
        Some((purpose, _, None, None, _)) => Ok(purpose),
    }
}

#[cfg(test)]
fn stage_source_in_connection(
    connection: &Connection,
    paths: &AppPaths,
    source_path: &Path,
    draft_id: &str,
    faults: IngestFaults,
) -> Result<AssetDto, CommandError> {
    stage_source_named_in_connection(connection, paths, source_path, draft_id, None, faults)
}

fn stage_source_named_in_connection(
    connection: &Connection,
    paths: &AppPaths,
    source_path: &Path,
    draft_id: &str,
    original_filename_override: Option<&str>,
    faults: IngestFaults,
) -> Result<AssetDto, CommandError> {
    ensure_active_draft(connection, draft_id, unix_time_millis()?)?;
    if !source_path.is_file() {
        return Err(CommandError::new(
            "invalid_image",
            "Select an image file, not a folder.",
        ));
    }
    let metadata = fs::metadata(source_path).map_err(CommandError::io)?;
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
    let mut header_file = File::open(source_path).map_err(CommandError::io)?;
    let mut header = [0_u8; 16];
    let bytes_read = header_file.read(&mut header).map_err(CommandError::io)?;
    let (mime_type, extension) = detect_image_bytes(&header[..bytes_read])?;
    let source_filename = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let original_filename =
        validate_original_filename(original_filename_override.unwrap_or(source_filename))?;
    let now = unix_time_millis()?;
    let id = generate_id(connection)?;
    let relative_path = AppPaths::image_relative_path(&id, extension);
    let staged_relative_path = AppPaths::staged_asset_relative_path(draft_id, &id, extension);
    let staged_path = paths.resolve_relative(&staged_relative_path);
    create_parent(&staged_path)?;

    let copy_result = (|| {
        let mut source = File::open(source_path).map_err(CommandError::io)?;
        let mut destination = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged_path)
            .map_err(CommandError::io)?;
        std::io::copy(&mut source, &mut destination).map_err(CommandError::io)?;
        if faults.fail_after_copy {
            return Err(CommandError::new(
                "asset_test_copy_failure",
                "Injected failure after the staged copy.",
            ));
        }
        destination.sync_all().map_err(CommandError::io)?;
        if faults.fail_after_sync {
            return Err(CommandError::new(
                "asset_test_sync_failure",
                "Injected failure after the staged file flush.",
            ));
        }
        sync_directory(staged_path.parent())?;
        Ok(())
    })();
    if let Err(error) = copy_result {
        let _ = fs::remove_file(&staged_path);
        return Err(error);
    }
    if faults.fail_before_insert {
        let _ = fs::remove_file(&staged_path);
        return Err(CommandError::new(
            "asset_test_database_failure",
            "Injected failure before the asset ledger insert.",
        ));
    }

    let insert = connection.execute(
        "INSERT INTO assets (
             id, original_filename, mime_type, byte_size, relative_path, created_at,
             lifecycle_state, draft_id, staged_relative_path, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'staged', ?7, ?8, ?6)",
        params![
            id,
            original_filename,
            mime_type,
            i64::try_from(metadata.len()).unwrap_or(i64::MAX),
            relative_path,
            now,
            draft_id,
            staged_relative_path,
        ],
    );
    if let Err(error) = insert {
        let _ = fs::remove_file(&staged_path);
        return Err(CommandError::database(error));
    }
    record_event(
        connection,
        Some(&id),
        "staged",
        None,
        Some(&staged_relative_path),
        "editor image staged",
        now,
    )?;
    Ok(AssetDto {
        id,
        original_filename,
        mime_type: mime_type.to_string(),
        byte_size: i64::try_from(metadata.len()).unwrap_or(i64::MAX),
        created_at: now,
    })
}
