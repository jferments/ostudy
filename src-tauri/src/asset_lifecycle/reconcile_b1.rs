fn quarantine_asset_file(
    connection: &Connection,
    paths: &AppPaths,
    asset: &AssetRecord,
    source: &Path,
    now: i64,
) -> Result<(), CommandError> {
    let suffix = now.to_string();
    let relative = AppPaths::quarantined_asset_relative_path(&asset.id, &suffix);
    let destination = paths.resolve_relative(&relative);
    create_parent(&destination)?;
    fs::rename(source, &destination).map_err(CommandError::io)?;
    sync_directory(source.parent())?;
    sync_directory(destination.parent())?;
    transition_asset(connection, asset, AssetTransition { state: "quarantined", staged_relative_path: None, quarantine_relative_path: Some(&relative), reason: "unreferenced managed file quarantined", event: "quarantined" }, now)
}
