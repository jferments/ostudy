fn finalize_assets_with_faults(
    transaction: &Transaction<'_>,
    paths: &AppPaths,
    asset_ids: &[String],
    draft_id: Option<&str>,
    owner_id: Option<&str>,
    faults: PromotionFaults,
) -> Result<DraftFinalization, CommandError> {
    let normalized = normalize_asset_ids(asset_ids.to_vec())?;
    let now = unix_time_millis()?;
    if let Some(draft_id) = draft_id {
        validate_entity_id(draft_id, "asset draft")?;
        ensure_active_draft(transaction, draft_id, now)?;
    }
    ensure_assets_available(transaction, paths, &normalized, draft_id)?;

    let mut promoted = Vec::<(PathBuf, PathBuf)>::new();
    let result = (|| {
        for asset_id in &normalized {
            let asset = load_asset(transaction, asset_id)?;
            if asset.lifecycle_state != "staged" {
                continue;
            }
            let staged_relative = asset.staged_relative_path.as_deref().ok_or_else(|| {
                CommandError::new("asset_stage_missing", "A staged image has no managed path.")
            })?;
            let staged_path = paths.resolve_relative(staged_relative);
            let final_path = paths.resolve_relative(&asset.relative_path);
            create_parent(&final_path)?;
            if final_path.exists() {
                return Err(CommandError::new(
                    "asset_path_conflict",
                    "A managed image path is already occupied.",
                ));
            }
            fs::rename(&staged_path, &final_path).map_err(CommandError::io)?;
            promoted.push((staged_path.clone(), final_path.clone()));
            sync_directory(staged_path.parent())?;
            sync_directory(final_path.parent())?;
            if faults.fail_after_rename {
                return Err(CommandError::new(
                    "asset_test_post_rename_crash",
                    "Injected failure after the managed rename.",
                ));
            }
            transaction
                .execute(
                    "UPDATE assets
                     SET lifecycle_state = 'ready', draft_id = NULL,
                         staged_relative_path = NULL, quarantine_relative_path = NULL,
                         ready_at = COALESCE(ready_at, ?1), verified_at = ?1,
                         updated_at = ?1, lifecycle_reason = '', deleted_at = NULL
                     WHERE id = ?2 AND lifecycle_state = 'staged'",
                    params![now, asset_id],
                )
                .map_err(CommandError::database)?;
            record_event(
                transaction,
                Some(asset_id),
                "promoted",
                Some(staged_relative),
                Some(&asset.relative_path),
                "asset promoted with durable owner",
                now,
            )?;
        }

        let cleanup_paths = if let Some(draft_id) = draft_id {
            let retained = normalized.iter().cloned().collect::<BTreeSet<_>>();
            let cleanup = retire_unused_draft_assets(
                transaction,
                paths,
                draft_id,
                &retained,
                now,
                "unused when owner committed",
            )?;
            transaction
                .execute(
                    "UPDATE asset_drafts
                     SET owner_id = COALESCE(?1, owner_id), updated_at = ?2, committed_at = ?2
                     WHERE id = ?3 AND committed_at IS NULL AND canceled_at IS NULL",
                    params![owner_id, now, draft_id],
                )
                .map_err(CommandError::database)?;
            cleanup
        } else {
            Vec::new()
        };

        Ok(DraftFinalization { cleanup_paths })
    })();

    if let Err(error) = &result {
        if error.code != "asset_test_post_rename_crash" {
            rollback_promoted_files(&promoted);
        }
    }
    result
}

fn rollback_promoted_files(promoted: &[(PathBuf, PathBuf)]) {
    for (staged_path, final_path) in promoted.iter().rev() {
        if !final_path.exists() {
            continue;
        }
        if staged_path.exists() {
            crate::logging::error(
                "asset_promotion_rollback_conflict",
                format!(
                    "staged={} final={}",
                    staged_path.display(),
                    final_path.display()
                ),
            );
            continue;
        }
        if let Err(error) = fs::rename(final_path, staged_path) {
            crate::logging::error(
                "asset_promotion_rollback_failed",
                format!(
                    "staged={} final={} error={error}",
                    staged_path.display(),
                    final_path.display()
                ),
            );
            continue;
        }
        if let Err(error) = sync_directory(final_path.parent()) {
            crate::logging::error("asset_promotion_rollback_sync_failed", error.message);
        }
        if let Err(error) = sync_directory(staged_path.parent()) {
            crate::logging::error("asset_promotion_rollback_sync_failed", error.message);
        }
    }
}

pub(crate) fn retire_unused_editor_assets(
    connection: &Connection,
    paths: &AppPaths,
    draft_id: &str,
    retained: &BTreeSet<String>,
    now: i64,
    reason: &str,
) -> Result<Vec<PathBuf>, CommandError> {
    retire_unused_draft_assets(connection, paths, draft_id, retained, now, reason)
}

pub(crate) fn remove_editor_cleanup_paths(paths: &[PathBuf]) {
    remove_paths_best_effort(paths);
}

fn retire_unused_draft_assets(
    connection: &Connection,
    paths: &AppPaths,
    draft_id: &str,
    retained: &BTreeSet<String>,
    now: i64,
    reason: &str,
) -> Result<Vec<PathBuf>, CommandError> {
    let assets = load_assets_for_draft(connection, draft_id)?;
    let mut cleanup = Vec::new();
    for asset in assets {
        if retained.contains(&asset.id) || reference_count(connection, &asset.id)? > 0 {
            continue;
        }
        if let Some(staged) = asset.staged_relative_path.as_deref() {
            cleanup.push(paths.resolve_relative(staged));
        }
        if paths.resolve_relative(&asset.relative_path).exists() {
            cleanup.push(paths.resolve_relative(&asset.relative_path));
        }
        connection
            .execute(
                "UPDATE assets
                 SET lifecycle_state = 'deleted', draft_id = NULL,
                     deleted_at = COALESCE(deleted_at, ?1), updated_at = ?1,
                     lifecycle_reason = ?2
                 WHERE id = ?3",
                params![now, reason, asset.id],
            )
            .map_err(CommandError::database)?;
        record_event(
            connection,
            Some(&asset.id),
            "deleted",
            asset.staged_relative_path.as_deref(),
            None,
            reason,
            now,
        )?;
    }
    Ok(cleanup)
}

fn abandon_draft_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    draft_id: &str,
    reason: &str,
) -> Result<Vec<PathBuf>, CommandError> {
    let now = unix_time_millis()?;
    let row: Option<(Option<i64>, Option<i64>)> = connection
        .query_row(
            "SELECT committed_at, canceled_at FROM asset_drafts WHERE id = ?1",
            [draft_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((committed_at, canceled_at)) = row else {
        return Ok(Vec::new());
    };
    if committed_at.is_some() || canceled_at.is_some() {
        return Ok(Vec::new());
    }
    let transaction = connection.transaction().map_err(CommandError::database)?;
    let cleanup =
        retire_unused_draft_assets(&transaction, paths, draft_id, &BTreeSet::new(), now, reason)?;
    transaction
        .execute(
            "UPDATE asset_drafts SET updated_at = ?1, canceled_at = ?1 WHERE id = ?2",
            params![now, draft_id],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(cleanup)
}

