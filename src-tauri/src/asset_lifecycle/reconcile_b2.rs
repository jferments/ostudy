fn transition_asset(
    connection: &Connection,
    asset: &AssetRecord,
    transition: AssetTransition<'_>,
    now: i64,
) -> Result<(), CommandError> {
    let AssetTransition { state, staged_relative_path, quarantine_relative_path, reason, event } = transition;
    let changed = asset.lifecycle_state != state || asset.staged_relative_path.as_deref() != staged_relative_path || asset.quarantine_relative_path.as_deref() != quarantine_relative_path || (state == "ready" && asset.draft_id.is_some());
    connection.execute("UPDATE assets SET lifecycle_state = ?1, draft_id = CASE WHEN ?1 = 'staged' THEN draft_id ELSE NULL END, staged_relative_path = ?2, quarantine_relative_path = ?3, ready_at = CASE WHEN ?1 = 'ready' THEN COALESCE(ready_at, ?4) ELSE ready_at END, verified_at = CASE WHEN ?1 = 'ready' THEN ?4 ELSE verified_at END, deleted_at = CASE WHEN ?1 = 'deleted' THEN COALESCE(deleted_at, ?4) ELSE NULL END, updated_at = CASE WHEN ?5 THEN ?4 ELSE updated_at END, lifecycle_reason = ?6 WHERE id = ?7", params![state, staged_relative_path, quarantine_relative_path, now, changed, reason, asset.id]).map_err(CommandError::database)?;
    if changed { record_event(connection, Some(&asset.id), event, Some(&asset.relative_path), staged_relative_path.or(quarantine_relative_path).or((state == "ready").then_some(asset.relative_path.as_str())), reason, now)?; }
    Ok(())
}
