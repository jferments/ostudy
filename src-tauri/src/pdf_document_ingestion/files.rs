use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
};

use sha2::{Digest, Sha256};

use crate::error::CommandError;

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn write_replacing_file(destination: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let parent = destination.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_document_path",
            "The rendered-source path is invalid.",
        )
    })?;
    fs::create_dir_all(parent).map_err(CommandError::io)?;
    let temporary = destination.with_extension(format!("png.{}.tmp", std::process::id()));
    let _ = fs::remove_file(&temporary);
    write_new_file(&temporary, bytes)?;
    if destination.exists() {
        fs::remove_file(destination).map_err(CommandError::io)?;
    }
    fs::rename(&temporary, destination).map_err(CommandError::io)?;
    sync_file(destination)
}

fn write_new_file(destination: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(CommandError::io)?;
    file.write_all(bytes).map_err(CommandError::io)?;
    file.sync_all().map_err(CommandError::io)
}

pub(crate) fn sync_directory(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(CommandError::io)
}
