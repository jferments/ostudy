fn verify_asset_file(asset: &AssetRecord, path: &Path) -> Result<(), CommandError> {
    let metadata = fs::metadata(path).map_err(CommandError::io)?;
    if i64::try_from(metadata.len()).unwrap_or(i64::MAX) != asset.byte_size {
        return Err(CommandError::new(
            "asset_size_mismatch",
            format!(
                "The asset '{}' does not match its database size record.",
                asset.original_filename
            ),
        ));
    }

    let detected_mime = if asset.mime_type.starts_with("image/") {
        let mut file = File::open(path).map_err(CommandError::io)?;
        let mut header = [0_u8; 16];
        let bytes_read = file.read(&mut header).map_err(CommandError::io)?;
        detect_image_bytes(&header[..bytes_read])?.0
    } else if asset.mime_type.starts_with("audio/") || asset.mime_type.starts_with("video/") {
        // Media inspection needs bounded container/track data rather than only a
        // signature header. Asset admission caps media at 25 MiB, and startup
        // verifies files sequentially, so this keeps peak verification memory
        // bounded without weakening the image-only authoring boundary.
        if metadata.len() > MAX_IMAGE_BYTES {
            return Err(CommandError::new(
                "asset_too_large",
                format!(
                    "The media asset '{}' exceeds the supported 25 MB size.",
                    asset.original_filename
                ),
            ));
        }
        let bytes = fs::read(path).map_err(CommandError::io)?;
        inspect_media_bytes(&bytes)?.mime_type
    } else {
        return Err(CommandError::new(
            "unsupported_asset_type",
            format!(
                "The asset '{}' has an unsupported managed file type.",
                asset.original_filename
            ),
        ));
    };

    if detected_mime != asset.mime_type {
        return Err(CommandError::new(
            "asset_type_mismatch",
            format!(
                "The asset '{}' does not match its database file type.",
                asset.original_filename
            ),
        ));
    }
    Ok(())
}

fn create_parent(path: &Path) -> Result<(), CommandError> {
    let parent = path.parent().ok_or_else(|| {
        CommandError::new("invalid_asset_path", "The managed image path is invalid.")
    })?;
    fs::create_dir_all(parent).map_err(CommandError::io)
}

#[cfg(unix)]
fn sync_directory(path: Option<&Path>) -> Result<(), CommandError> {
    let path = path.ok_or_else(|| {
        CommandError::new(
            "invalid_asset_path",
            "The managed image directory is invalid.",
        )
    })?;
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(CommandError::io)
}

#[cfg(not(unix))]
fn sync_directory(_path: Option<&Path>) -> Result<(), CommandError> {
    Ok(())
}

fn relative_to_app_data(paths: &AppPaths, path: &Path) -> Result<String, CommandError> {
    path.strip_prefix(paths.app_data_dir())
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| {
            CommandError::new(
                "invalid_asset_path",
                "A managed image escaped the application data directory.",
            )
        })
}

fn remove_paths_best_effort(paths: &[PathBuf]) {
    for path in paths {
        if let Err(error) = fs::remove_file(path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                crate::logging::error(
                    "asset_cleanup_failed",
                    format!("path={} error={error}", path.display()),
                );
            }
        }
    }
}

