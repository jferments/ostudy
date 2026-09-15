fn reconcile_asset(
    connection: &mut Connection,
    paths: &AppPaths,
    asset_id: &str,
    now: i64,
    cleanup: &mut Vec<PathBuf>,
) -> Result<(), CommandError> {
    let asset = load_asset(connection, asset_id)?;
    let references = reference_count(connection, asset_id)?;
    let final_path = paths.resolve_relative(&asset.relative_path);
    let staged_path = asset
        .staged_relative_path
        .as_deref()
        .map(|value| paths.resolve_relative(value));
    let quarantine_path = asset
        .quarantine_relative_path
        .as_deref()
        .map(|value| paths.resolve_relative(value));
    let final_exists = final_path.is_file();
    let staged_exists = staged_path.as_ref().is_some_and(|path| path.is_file());
    let quarantine_exists = quarantine_path.as_ref().is_some_and(|path| path.is_file());

    if references > 0 {
        if final_exists {
            if !verify_referenced_asset(connection, &asset, &final_path, now)? {
                return Ok(());
            }
            transition_asset(
                connection,
                &asset,
                AssetTransition {
                    state: "ready",
                    staged_relative_path: None,
                    quarantine_relative_path: None,
                    reason: "",
                    event: "restored",
                },
                now,
            )?;
            return Ok(());
        }
        if staged_exists {
            let staged = staged_path.as_ref().expect("checked staged path");
            create_parent(&final_path)?;
            fs::rename(staged, &final_path).map_err(CommandError::io)?;
            sync_directory(staged.parent())?;
            sync_directory(final_path.parent())?;
            if !verify_referenced_asset(connection, &asset, &final_path, now)? {
                return Ok(());
            }
            transition_asset(
                connection,
                &asset,
                AssetTransition {
                    state: "ready",
                    staged_relative_path: None,
                    quarantine_relative_path: None,
                    reason: "recovered referenced staged file",
                    event: "restored",
                },
                now,
            )?;
            return Ok(());
        }
        if quarantine_exists {
            let quarantined = quarantine_path.as_ref().expect("checked quarantine path");
            create_parent(&final_path)?;
            fs::rename(quarantined, &final_path).map_err(CommandError::io)?;
            sync_directory(quarantined.parent())?;
            sync_directory(final_path.parent())?;
            if !verify_referenced_asset(connection, &asset, &final_path, now)? {
                return Ok(());
            }
            transition_asset(
                connection,
                &asset,
                AssetTransition {
                    state: "ready",
                    staged_relative_path: None,
                    quarantine_relative_path: None,
                    reason: "restored referenced quarantine file",
                    event: "restored",
                },
                now,
            )?;
            return Ok(());
        }
        transition_asset(
            connection,
            &asset,
            AssetTransition {
                state: "missing",
                staged_relative_path: asset.staged_relative_path.as_deref(),
                quarantine_relative_path: asset.quarantine_relative_path.as_deref(),
                reason: "referenced file absent",
                event: "missing",
            },
            now,
        )?;
        return Ok(());
    }

    match asset.lifecycle_state.as_str() {
        "staged" => {
            let draft_active = asset
                .draft_id
                .as_deref()
                .is_some_and(|draft_id| ensure_active_draft(connection, draft_id, now).is_ok());
            if draft_active {
                if staged_exists {
                    return Ok(());
                }
                if final_exists {
                    let staged = staged_path.as_ref().ok_or_else(|| {
                        CommandError::new(
                            "asset_stage_missing",
                            "A staged asset has no managed staging path.",
                        )
                    })?;
                    create_parent(staged)?;
                    fs::rename(&final_path, staged).map_err(CommandError::io)?;
                    sync_directory(final_path.parent())?;
                    sync_directory(staged.parent())?;
                    record_event(
                        connection,
                        Some(&asset.id),
                        "restored",
                        Some(&asset.relative_path),
                        asset.staged_relative_path.as_deref(),
                        "returned interrupted import rename to active staging",
                        now,
                    )?;
                    return Ok(());
                }
            }
            if staged_exists {
                cleanup.push(staged_path.expect("checked staged path"));
            }
            if final_exists {
                quarantine_asset_file(connection, paths, &asset, &final_path, now)?;
            } else {
                transition_asset(
                    connection,
                    &asset,
                    AssetTransition {
                        state: "deleted",
                        staged_relative_path: asset.staged_relative_path.as_deref(),
                        quarantine_relative_path: None,
                        reason: "unreferenced staged asset expired",
                        event: "deleted",
                    },
                    now,
                )?;
            }
        }
        "ready" => {
            if !final_exists {
                transition_asset(
                    connection,
                    &asset,
                    AssetTransition {
                        state: "deleted",
                        staged_relative_path: None,
                        quarantine_relative_path: None,
                        reason: "unreferenced ready file absent",
                        event: "deleted",
                    },
                    now,
                )?;
            } else if now.saturating_sub(asset.updated_at.max(asset.created_at))
                >= RECOVERY_GRACE_MS
            {
                quarantine_asset_file(connection, paths, &asset, &final_path, now)?;
            }
        }
        "missing" => {
            if final_exists {
                verify_asset_file(&asset, &final_path)?;
                transition_asset(
                    connection,
                    &asset,
                    AssetTransition {
                        state: "ready",
                        staged_relative_path: None,
                        quarantine_relative_path: None,
                        reason: "unreferenced file returned",
                        event: "restored",
                    },
                    now,
                )?;
            } else {
                transition_asset(
                    connection,
                    &asset,
                    AssetTransition {
                        state: "deleted",
                        staged_relative_path: None,
                        quarantine_relative_path: None,
                        reason: "unreferenced missing asset retired",
                        event: "deleted",
                    },
                    now,
                )?;
            }
        }
        "quarantined" => {
            if quarantine_exists
                && now.saturating_sub(asset.updated_at.max(asset.created_at)) >= RECOVERY_GRACE_MS
            {
                cleanup.push(quarantine_path.expect("checked quarantine path"));
                transition_asset(
                    connection,
                    &asset,
                    AssetTransition {
                        state: "deleted",
                        staged_relative_path: None,
                        quarantine_relative_path: asset.quarantine_relative_path.as_deref(),
                        reason: "quarantine grace elapsed",
                        event: "deleted",
                    },
                    now,
                )?;
            }
        }
        "deleted" => {
            if let Some(path) = staged_path.filter(|path| path.exists()) {
                cleanup.push(path);
            }
            if final_exists {
                cleanup.push(final_path);
            }
            if let Some(path) = quarantine_path.filter(|path| path.exists()) {
                cleanup.push(path);
            }
        }
        _ => {}
    }
    Ok(())
}

