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
    let staged_path = asset.staged_relative_path.as_deref().map(|value| paths.resolve_relative(value));
    let quarantine_path = asset.quarantine_relative_path.as_deref().map(|value| paths.resolve_relative(value));
    let final_exists = final_path.is_file();
    let staged_exists = staged_path.as_ref().is_some_and(|path| path.is_file());
    let quarantine_exists = quarantine_path.as_ref().is_some_and(|path| path.is_file());
    if references > 0 && final_exists {
        if !verify_referenced_asset(connection, &asset, &final_path, now)? { return Ok(()); }
        transition_asset(connection, &asset, AssetTransition { state: "ready", staged_relative_path: None, quarantine_relative_path: None, reason: "", event: "restored" }, now)?;
        return Ok(());
    }
    Ok(())
}
