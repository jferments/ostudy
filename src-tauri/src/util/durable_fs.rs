//! Filesystem mechanics shared by persistence owners, not a new data owner.
//!
//! A successful directory sync means the OS accepted its durability request.
//! It does not flush file contents and is not a power-loss guarantee from a
//! lying device. Callers must sync new file data before publishing a name. Real
//! open/flush failures propagate; no platform has a successful no-op fallback.
//! Domain owners retain journal, transaction, compensation and commit decisions.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Sync an existing directory. Missing paths, ordinary files and symlinks are
/// errors, not evidence that directory metadata was synchronized. On Windows
/// the directory handle needs BACKUP_SEMANTICS and write access for
/// FlushFileBuffers. Filesystems that refuse that operation return their error.
pub(crate) fn sync_directory(path: &Path) -> io::Result<()> {
    if !fs::symlink_metadata(path)?.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            "Directory durability requires an existing non-symlink directory.",
        ));
    }
    sync_directory_platform(path)
}

#[cfg(unix)]
fn sync_directory_platform(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(windows)]
fn sync_directory_platform(path: &Path) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?
        .sync_all()
}

#[cfg(not(any(unix, windows)))]
fn sync_directory_platform(_path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Directory durability is not implemented on this platform.",
    ))
}

/// Preserve callers' explicit distinction between an optional directory and a
/// required parent. Required-parent callers must validate Some before calling.
pub(crate) fn sync_optional_directory(path: Option<&Path>) -> io::Result<()> {
    match path {
        Some(path) => sync_directory(path),
        None => Ok(()),
    }
}

/// Create missing directories and sync each new entry's parent. An existing
/// parent is never removed or relabelled. This is for application-owned paths;
/// it does not claim to sandbox a concurrently hostile filesystem namespace.
pub(crate) fn create_directories(path: &Path) -> io::Result<()> {
    let mut missing = Vec::new();
    let mut cursor = path;
    loop {
        match fs::symlink_metadata(cursor) {
            Ok(metadata) if metadata.is_dir() => break,
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    "The persistence parent is not a directory.",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                missing.push(cursor.to_path_buf());
                cursor = required_parent(cursor)?;
            }
            Err(error) => return Err(error),
        }
    }
    for directory in missing.iter().rev() {
        match fs::create_dir(directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                if !fs::symlink_metadata(directory)?.is_dir() {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
        sync_directory(directory)?;
        sync_directory(required_parent(directory)?)?;
    }
    Ok(())
}

/// Publish a complete private file in the same directory as its destination.
/// Never unlink the old destination before rename. A pre-rename failure leaves
/// it untouched and retains any temporary bytes for diagnosis/recovery. A
/// post-rename sync failure returns an error but leaves the complete new file
/// visible; callers must not infer that the old file is still active or delete
/// the new file as compensation. Domain transactions decide their own recovery.
pub(crate) fn write_replacing_file(destination: &Path, bytes: &[u8]) -> io::Result<()> {
    write_replacing_file_with(destination, bytes, |_| Ok(()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicationPhase {
    BeforeWrite,
    AfterWrite,
    AfterFileSync,
    BeforeRename,
    AfterRename,
    AfterDirectorySync,
}

fn write_replacing_file_with(
    destination: &Path,
    bytes: &[u8],
    mut checkpoint: impl FnMut(PublicationPhase) -> io::Result<()>,
) -> io::Result<()> {
    let parent = required_parent(destination)?;
    create_directories(parent)?;
    match fs::symlink_metadata(destination) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A replacement destination must be a regular file or absent.",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let (temporary, mut file) = new_temporary(parent)?;
    checkpoint(PublicationPhase::BeforeWrite)?;
    file.write_all(bytes)?;
    checkpoint(PublicationPhase::AfterWrite)?;
    file.sync_all()?;
    drop(file);
    checkpoint(PublicationPhase::AfterFileSync)?;
    checkpoint(PublicationPhase::BeforeRename)?;
    fs::rename(&temporary, destination)?;
    checkpoint(PublicationPhase::AfterRename)?;
    sync_directory(parent)?;
    checkpoint(PublicationPhase::AfterDirectorySync)
}

fn required_parent(path: &Path) -> io::Result<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "The persistence path must have an explicit parent directory.",
            )
        })
}

fn new_temporary(parent: &Path) -> io::Result<(PathBuf, File)> {
    // A restarted process can reuse a PID. Only create_new reserves a name;
    // a collision never authorizes unlinking another unfinished write.
    for _ in 0..128 {
        let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".ostudy-write-{}-{sequence}.partial",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "Could not reserve a private publication file without replacing existing evidence.",
    ))
}

#[cfg(test)]
#[path = "durable_fs/publication_tests.rs"]
mod tests;
