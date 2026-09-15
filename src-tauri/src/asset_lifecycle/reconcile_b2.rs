fn transition_asset(
    connection: &Connection,
    asset: &AssetRecord,
    transition: AssetTransition<'_>,
    now: i64,
) -> Result<(), CommandError> {
    let AssetTransition {
        state,
        staged_relative_path,
        quarantine_relative_path,
        reason,
        event,
    } = transition;
    let changed = asset.lifecycle_state != state
        || asset.staged_relative_path.as_deref() != staged_relative_path
        || asset.quarantine_relative_path.as_deref() != quarantine_relative_path
        || (state == "ready" && asset.draft_id.is_some());
    connection
        .execute(
            "UPDATE assets
             SET lifecycle_state = ?1,
                 draft_id = CASE WHEN ?1 = 'staged' THEN draft_id ELSE NULL END,
                 staged_relative_path = ?2,
                 quarantine_relative_path = ?3,
                 ready_at = CASE WHEN ?1 = 'ready' THEN COALESCE(ready_at, ?4) ELSE ready_at END,
                 verified_at = CASE WHEN ?1 = 'ready' THEN ?4 ELSE verified_at END,
                 deleted_at = CASE WHEN ?1 = 'deleted' THEN COALESCE(deleted_at, ?4) ELSE NULL END,
                 updated_at = CASE WHEN ?5 THEN ?4 ELSE updated_at END,
                 lifecycle_reason = ?6
             WHERE id = ?7",
            params![
                state,
                staged_relative_path,
                quarantine_relative_path,
                now,
                changed,
                reason,
                asset.id,
            ],
        )
        .map_err(CommandError::database)?;
    if changed {
        record_event(
            connection,
            Some(&asset.id),
            event,
            asset
                .staged_relative_path
                .as_deref()
                .or(asset.quarantine_relative_path.as_deref())
                .or(Some(&asset.relative_path)),
            staged_relative_path
                .or(quarantine_relative_path)
                .or((state == "ready").then_some(asset.relative_path.as_str())),
            reason,
            now,
        )?;
    }
    Ok(())
}

fn load_asset(connection: &Connection, asset_id: &str) -> Result<AssetRecord, CommandError> {
    connection
        .query_row(
            "SELECT id, original_filename, mime_type, byte_size, relative_path,
                    lifecycle_state, draft_id, staged_relative_path,
                    quarantine_relative_path, created_at, updated_at
             FROM assets WHERE id = ?1",
            [asset_id],
            |row| {
                Ok(AssetRecord {
                    id: row.get(0)?,
                    original_filename: row.get(1)?,
                    mime_type: row.get(2)?,
                    byte_size: row.get(3)?,
                    relative_path: row.get(4)?,
                    lifecycle_state: row.get(5)?,
                    draft_id: row.get(6)?,
                    staged_relative_path: row.get(7)?,
                    quarantine_relative_path: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "asset_not_found",
                "One of the images used by this content is no longer available.",
            )
        })
}

fn load_assets_for_draft(
    connection: &Connection,
    draft_id: &str,
) -> Result<Vec<AssetRecord>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, original_filename, mime_type, byte_size, relative_path,
                    lifecycle_state, draft_id, staged_relative_path,
                    quarantine_relative_path, created_at, updated_at
             FROM assets WHERE draft_id = ?1 ORDER BY created_at, id",
        )
        .map_err(CommandError::database)?;
    let assets = statement
        .query_map([draft_id], |row| {
            Ok(AssetRecord {
                id: row.get(0)?,
                original_filename: row.get(1)?,
                mime_type: row.get(2)?,
                byte_size: row.get(3)?,
                relative_path: row.get(4)?,
                lifecycle_state: row.get(5)?,
                draft_id: row.get(6)?,
                staged_relative_path: row.get(7)?,
                quarantine_relative_path: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(assets)
}

fn reference_count(connection: &Connection, asset_id: &str) -> Result<i64, CommandError> {
    connection
        .query_row(
            "SELECT
                 (SELECT COUNT(*) FROM note_assets WHERE asset_id = ?1) +
                 (SELECT COUNT(*) FROM card_template_assets WHERE asset_id = ?1)",
            [asset_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

fn normalize_asset_ids(values: Vec<String>) -> Result<Vec<String>, CommandError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_entity_id(&value, "asset")?;
        unique.insert(value);
    }
    Ok(unique.into_iter().collect())
}

fn record_event(
    connection: &Connection,
    asset_id: Option<&str>,
    action: &str,
    from_path: Option<&str>,
    to_path: Option<&str>,
    detail: &str,
    occurred_at: i64,
) -> Result<(), CommandError> {
    let id = generate_id(connection)?;
    connection
        .execute(
            "INSERT INTO asset_reconciliation_events (
                 id, asset_id, action, from_path, to_path, detail, occurred_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                asset_id,
                action,
                from_path,
                to_path,
                detail,
                occurred_at
            ],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

