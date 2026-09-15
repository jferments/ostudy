use crate::{error::CommandError, media::inspect_managed_asset_bytes};

pub(super) fn validate_managed_asset_bytes(
    data: &[u8],
    recorded_mime_type: &str,
) -> Result<(), CommandError> {
    let (detected_mime_type, _) = inspect_managed_asset_bytes(data)?;
    if detected_mime_type != recorded_mime_type {
        return Err(CommandError::new(
            "device_sync_asset_invalid",
            "A managed asset does not match its database file type.",
        ));
    }
    Ok(())
}
