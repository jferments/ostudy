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
    transition_asset(
        connection,
        asset,
        AssetTransition {
            state: "quarantined",
            staged_relative_path: None,
            quarantine_relative_path: Some(&relative),
            reason: "unreferenced managed file quarantined",
            event: "quarantined",
        },
        now,
    )
}

fn quarantine_unknown_files(
    connection: &Connection,
    paths: &AppPaths,
    now: i64,
) -> Result<(), CommandError> {
    // Recreate the conservative holding directory if an external cleanup tool
    // removed it between launches.
    fs::create_dir_all(paths.asset_quarantine_dir()).map_err(CommandError::io)?;
    let mut known = HashSet::new();
    let mut statement = connection
        .prepare("SELECT relative_path, staged_relative_path, quarantine_relative_path FROM assets")
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(CommandError::database)?;
    for row in rows {
        let (ready, staged, quarantined) = row.map_err(CommandError::database)?;
        known.insert(ready);
        if let Some(value) = staged {
            known.insert(value);
        }
        if let Some(value) = quarantined {
            known.insert(value);
        }
    }

    for directory in [paths.images_dir(), paths.asset_staging_dir()] {
        for entry in fs::read_dir(directory).map_err(CommandError::io)? {
            let entry = entry.map_err(CommandError::io)?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let relative = relative_to_app_data(paths, &path)?;
            if known.contains(&relative) {
                continue;
            }
            let orphan_id = generate_id(connection)?;
            let destination_relative =
                AppPaths::quarantined_asset_relative_path(&orphan_id, &now.to_string());
            let destination = paths.resolve_relative(&destination_relative);
            fs::rename(&path, &destination).map_err(CommandError::io)?;
            sync_directory(path.parent())?;
            sync_directory(destination.parent())?;
            record_event(
                connection,
                None,
                "orphan_quarantined",
                Some(&relative),
                Some(&destination_relative),
                "unknown managed file moved out of active namespace",
                now,
            )?;
        }
    }
    Ok(())
}

struct AssetTransition<'a> {
    state: &'a str,
    staged_relative_path: Option<&'a str>,
    quarantine_relative_path: Option<&'a str>,
    reason: &'a str,
    event: &'a str,
}

