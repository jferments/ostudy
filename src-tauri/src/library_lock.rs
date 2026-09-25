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
                return Err("The library ownership lock is not a regular file. It was left unchanged.".into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!("Could not inspect the library ownership lock: {error}"));
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
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Read, Write},
        path::PathBuf,
        process::{Command, Stdio},
        sync::atomic::{AtomicU64, Ordering},
    };

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "ostudy-owner-lock-{}-{}-{}",
            std::process::id(),
            crate::util::unix_time_millis().unwrap(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn one_directory_has_one_owner_and_a_stale_file_is_not_a_stale_lock() {
        let root = root();
        let first = LibraryLock::acquire(&root).unwrap();
        let error = LibraryLock::acquire(&root)
            .err()
            .expect("second owner must fail");
        assert!(error.starts_with(IN_USE));
        drop(first);
        assert!(root.join(LOCK_FILE).is_file());
        drop(LibraryLock::acquire(&root).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn independent_profile_directories_can_run_together() {
        let root = root();
        let production = LibraryLock::acquire(&root.join("production")).unwrap();
        let development = LibraryLock::acquire(&root.join("development")).unwrap();
        drop((production, development));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_non_file_lock_is_preserved_and_refused() {
        let root = root();
        fs::create_dir_all(root.join(LOCK_FILE)).unwrap();
        assert!(LibraryLock::acquire(&root).is_err());
        assert!(root.join(LOCK_FILE).is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_lock_is_preserved_and_refused() {
        let root = root();
        fs::create_dir_all(&root).unwrap();
        let target = root.join("unrelated");
        fs::write(&target, b"preserve").unwrap();
        std::os::unix::fs::symlink(&target, root.join(LOCK_FILE)).unwrap();
        assert!(LibraryLock::acquire(&root).is_err());
        assert_eq!(fs::read(target).unwrap(), b"preserve");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lock_child_process() {
        let Some(root) = std::env::var_os("OSTUDY_OWNER_LOCK_CHILD") else {
            return;
        };
        acquire_for_process(Path::new(&root)).unwrap();
        println!("OWNER_LOCK_READY");
        std::io::stdout().flush().unwrap();
        let mut input = Vec::new();
        std::io::stdin().read_to_end(&mut input).unwrap();
    }

    #[test]
    fn kernel_releases_ownership_after_abrupt_process_exit() {
        let root = root();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "library_lock::tests::lock_child_process",
                "--nocapture",
            ])
            .env("OSTUDY_OWNER_LOCK_CHILD", &root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let output = child.stdout.take().unwrap();
        let ready = BufReader::new(output)
            .lines()
            .map_while(Result::ok)
            .any(|line| line.contains("OWNER_LOCK_READY"));
        if !ready {
            let _ = child.kill();
            let _ = child.wait();
            panic!("lock owner did not report readiness");
        }
        let refused = LibraryLock::acquire(&root).is_err();
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(refused);
        drop(LibraryLock::acquire(&root).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
}
