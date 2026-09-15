fn reconcile_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    now: i64,
) -> Result<Vec<PathBuf>, CommandError> {
    let mut cleanup = Vec::new();
    expire_drafts(connection, paths, now, &mut cleanup)?;
    let asset_ids = {
        let mut statement = connection
            .prepare("SELECT id FROM assets ORDER BY created_at, id")
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };
    for asset_id in asset_ids {
        reconcile_asset(connection, paths, &asset_id, now, &mut cleanup)?;
    }
    quarantine_unknown_files(connection, paths, now)?;
    Ok(cleanup)
}

fn expire_drafts(
    connection: &mut Connection,
    paths: &AppPaths,
    now: i64,
    cleanup: &mut Vec<PathBuf>,
) -> Result<(), CommandError> {
    let draft_ids = {
        let mut statement = connection
            .prepare(
                "SELECT id FROM asset_drafts
                 WHERE committed_at IS NULL AND canceled_at IS NULL
                   AND retention_mode = 'ephemeral' AND expires_at < ?1",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([now], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };
    for draft_id in draft_ids {
        cleanup.extend(abandon_draft_in_connection(
            connection,
            paths,
            &draft_id,
            "draft expired",
        )?);
    }
    Ok(())
}

fn verify_referenced_asset(
    connection: &Connection,
    asset: &AssetRecord,
    path: &Path,
    now: i64,
) -> Result<bool, CommandError> {
    if let Err(error) = verify_asset_file(asset, path) {
        let reason = format!("referenced file verification failed: {}", error.message);
        crate::logging::error(
            "referenced_asset_verification_failed",
            format!(
                "asset_id={} path={} error={}",
                asset.id,
                path.display(),
                error.message
            ),
        );
        transition_asset(
            connection,
            asset,
            AssetTransition {
                state: "missing",
                staged_relative_path: None,
                quarantine_relative_path: None,
                reason: &reason,
                event: "missing",
            },
            now,
        )?;
        return Ok(false);
    }
    Ok(true)
}

