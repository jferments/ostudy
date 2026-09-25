//! One cooperating application process owns a learner-library directory.
//!
//! SQLite locks protect transactions, not a database/assets directory rename.
//! Acquire this lock before startup recovery and retain it until process exit.
//! The persistent lock file is never deleted: unlinking it would permit two
//! processes to lock different inodes under the same pathname.

use std::{
    fs::{self, File, OpenOptions, TryLockError},
    path::Path,
    sync::OnceLock,
};

pub(crate) const IN_USE: &str = "OSTUDY_LIBRARY_IN_USE";
const LOCK_FILE: &str = "library-owner.lock";
static PROCESS_LOCK: OnceLock<LibraryLock> = OnceLock::new();

struct LibraryLock {
    _file: File,
}

impl LibraryLock {
    fn acquire(root: &Path) -> Result<Self, String> {
        fs::create_dir_all(root)
            .map_err(|error| format!("Could not open library storage: {error}"))?;
        let path = root.join(LOCK_FILE);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if !metadata.file_type().is_file() => {
                return Err(
                    "The library ownership lock is not a regular file. It was left unchanged."
                        .into(),
                );
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Could not inspect the library ownership lock: {error}"
                ));
            }
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|error| format!("Could not open the library ownership lock: {error}"))?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(format!(
                "{IN_USE}: Another oStudy process is using this library. Close that instance before opening the same library. No restore was attempted."
            )),
            Err(TryLockError::Error(error)) => Err(format!(
                "Could not acquire the library ownership lock: {error}. Startup was refused before opening or restoring learner data."
            )),
        }
    }
}

pub(crate) fn acquire_for_process(root: &Path) -> Result<(), String> {
    let lock = LibraryLock::acquire(root)?;
    PROCESS_LOCK
        .set(lock)
        .map_err(|_| "This process already owns an oStudy library.".to_string())
}

#[cfg(test)]
#[path = "library_lock_tests.rs"]
mod tests;
