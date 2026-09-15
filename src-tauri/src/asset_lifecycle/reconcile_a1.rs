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
