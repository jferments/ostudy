fn verify_asset_file(asset: &AssetRecord, path: &Path) -> Result<(), CommandError> {
    let metadata = fs::metadata(path).map_err(CommandError::io)?;
    if i64::try_from(metadata.len()).unwrap_or(i64::MAX) != asset.byte_size {
        return Err(CommandError::new("asset_size_mismatch", format!("The asset '{}' does not match its database size record.", asset.original_filename)));
    }
    let detected_mime = if asset.mime_type.starts_with("image/") {
        let mut file = File::open(path).map_err(CommandError::io)?;
        let mut header = [0_u8; 16];
        let bytes_read = file.read(&mut header).map_err(CommandError::io)?;
        detect_image_bytes(&header[..bytes_read])?.0
    } else if asset.mime_type.starts_with("audio/") || asset.mime_type.starts_with("video/") {
        if metadata.len() > MAX_IMAGE_BYTES {
            return Err(CommandError::new("asset_too_large", format!("The media asset '{}' exceeds the supported 25 MB size.", asset.original_filename)));
        }
        let bytes = fs::read(path).map_err(CommandError::io)?;
        inspect_media_bytes(&bytes)?.mime_type
    } else {
        return Err(CommandError::new("unsupported_asset_type", format!("The asset '{}' has an unsupported managed file type.", asset.original_filename)));
    };
    if detected_mime != asset.mime_type {
        return Err(CommandError::new("asset_type_mismatch", format!("The asset '{}' does not match its database file type.", asset.original_filename)));
    }
    Ok(())
}
