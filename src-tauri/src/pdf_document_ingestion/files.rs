use std::path::Path;

use sha2::{Digest, Sha256};

use crate::{error::CommandError, util::durable_fs};

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn write_replacing_file(destination: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    if destination.parent().is_none() {
        return Err(CommandError::new(
            "invalid_document_path",
            "The rendered-source path is invalid.",
        ));
    }
    // The old derivative stays complete until the synced replacement is renamed.
    // A failed rename never removes either the previous file or retained candidate.
    durable_fs::write_replacing_file(destination, bytes).map_err(CommandError::io)
}

pub(crate) fn sync_directory(path: &Path) -> Result<(), CommandError> {
    durable_fs::sync_directory(path).map_err(CommandError::io)
}
