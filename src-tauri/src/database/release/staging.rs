//! Private disk-backed staging for the one-time legacy release conversion.
//!
//! The candidate is never authoritative and is never renamed over the live
//! database. SQLite backup copies bounded pages into a sibling file (or the OS
//! temp directory for in-memory tests), with a small cache and file-backed temp
//! storage. Dropping the candidate closes SQLite before removing every sidecar.

use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use rusqlite::{backup::Backup, Connection};

const STAGING_PREFIX: &str = ".ostudy-release-conversion-";
const STAGING_CACHE_KIB: i64 = 16 * 1024;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct Candidate {
    path: PathBuf,
    connection: Option<Connection>,
    estimated_bytes: u64,
}

impl Candidate {
    pub(super) fn copy_from(source: &Connection) -> Result<Self, String> {
        Self::copy_from_inner(source, None)
    }

    #[cfg(test)]
    pub(super) fn copy_from_with_page_limit(
        source: &Connection,
        maximum_pages: i64,
    ) -> Result<Self, String> {
        Self::copy_from_inner(source, Some(maximum_pages))
    }

    fn copy_from_inner(source: &Connection, page_limit: Option<i64>) -> Result<Self, String> {
        let estimated_bytes = estimated_copy_bytes(source)?;
        let directory = staging_directory(source)?;
        let path = create_private_candidate_path(&directory)?;
        let mut candidate = Self {
            path,
            connection: None,
            estimated_bytes,
        };
        let connection = Connection::open(&candidate.path).map_err(|error| {
            stage_error(
                "open the disk-backed conversion candidate",
                error,
                estimated_bytes,
            )
        })?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| {
                stage_error("configure the conversion candidate", error, estimated_bytes)
            })?;
        connection
            .execute_batch(&format!(
                "PRAGMA journal_mode = DELETE;
                 PRAGMA synchronous = FULL;
                 PRAGMA temp_store = FILE;
                 PRAGMA mmap_size = 0;
                 PRAGMA cache_size = -{STAGING_CACHE_KIB};"
            ))
            .map_err(|error| {
                stage_error("configure bounded SQLite staging", error, estimated_bytes)
            })?;
        if let Some(maximum_pages) = page_limit {
            connection
                .pragma_update(None, "max_page_count", maximum_pages)
                .map_err(|error| {
                    stage_error("limit the conversion candidate", error, estimated_bytes)
                })?;
        }
        candidate.connection = Some(connection);

        {
            let staged = candidate
                .connection
                .as_mut()
                .expect("candidate connection was installed");
            let backup = Backup::new(source, staged).map_err(|error| {
                stage_error(
                    "begin the legacy-library staging copy",
                    error,
                    estimated_bytes,
                )
            })?;
            backup
                .run_to_completion(128, Duration::from_millis(1), None)
                .map_err(|error| {
                    stage_error(
                        "copy the legacy library into disk-backed staging",
                        error,
                        estimated_bytes,
                    )
                })?;
        }
        candidate
            .connection()
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;")
            .map_err(|error| {
                stage_error(
                    "finalize the disk-backed staging copy",
                    error,
                    estimated_bytes,
                )
            })?;
        Ok(candidate)
    }

    pub(super) fn connection(&self) -> &Connection {
        self.connection
            .as_ref()
            .expect("candidate connection remains open while staged")
    }

    pub(super) fn connection_mut(&mut self) -> &mut Connection {
        self.connection
            .as_mut()
            .expect("candidate connection remains open while staged")
    }

    pub(super) fn estimated_bytes(&self) -> u64 {
        self.estimated_bytes
    }

    #[cfg(test)]
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Candidate {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            drop(connection);
        }
        cleanup_candidate(&self.path);
    }
}

fn estimated_copy_bytes(source: &Connection) -> Result<u64, String> {
    let pages: i64 = source
        .query_row("PRAGMA page_count", [], |row| row.get(0))
        .map_err(|error| {
            format!("Could not estimate legacy-library pages before staging: {error}")
        })?;
    let page_size: i64 = source
        .query_row("PRAGMA page_size", [], |row| row.get(0))
        .map_err(|error| {
            format!("Could not estimate legacy-library page size before staging: {error}")
        })?;
    let pages = u64::try_from(pages.max(0)).unwrap_or(u64::MAX);
    let page_size = u64::try_from(page_size.max(0)).unwrap_or(u64::MAX);
    Ok(pages.saturating_mul(page_size))
}

fn staging_directory(source: &Connection) -> Result<PathBuf, String> {
    let file: String = source
        .query_row("PRAGMA database_list", [], |row| row.get(2))
        .map_err(|error| {
            format!("Could not locate the live database for private staging: {error}")
        })?;
    if !file.is_empty() && file != ":memory:" {
        let path = PathBuf::from(file);
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            return Ok(parent.to_path_buf());
        }
        return std::env::current_dir().map_err(|error| {
            format!("Could not resolve the conversion staging directory: {error}")
        });
    }
    Ok(std::env::temp_dir())
}

fn create_private_candidate_path(directory: &Path) -> Result<PathBuf, String> {
    let timestamp = crate::util::unix_time_millis().map_err(|error| error.message)?;
    for _ in 0..32 {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!(
            "{STAGING_PREFIX}{}-{timestamp}-{sequence}.sqlite3",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => {
                drop(file);
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "Could not create the private disk-backed conversion candidate: {error}"
                ))
            }
        }
    }
    Err("Could not allocate a unique private conversion candidate.".to_string())
}

fn stage_error(operation: &str, error: rusqlite::Error, estimated_bytes: u64) -> String {
    let full = matches!(
        &error,
        rusqlite::Error::SqliteFailure(code, _)
            if code.extended_code == rusqlite::ffi::SQLITE_FULL
    );
    if full {
        return format!(
            "Not enough free disk space to stage the legacy library safely. Approximately {estimated_bytes} bytes of database content must be copied before conversion; the live database was not modified."
        );
    }
    format!("Could not {operation}: {error}")
}

fn cleanup_candidate(path: &Path) {
    for candidate in [
        path.to_path_buf(),
        sidecar(path, "-wal"),
        sidecar(path, "-shm"),
        sidecar(path, "-journal"),
    ] {
        let _ = fs::remove_file(candidate);
    }
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "ostudy-release-stage-{label}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        root
    }

    #[test]
    fn candidate_is_disk_backed_private_and_removed_after_drop() {
        let root = temporary_root("drop");
        let source_path = root.join("library.sqlite3");
        let source = Connection::open(&source_path).unwrap();
        source
            .execute_batch(
                "CREATE TABLE payload(value BLOB NOT NULL);
                 INSERT INTO payload VALUES (zeroblob(262144));",
            )
            .unwrap();

        let candidate = Candidate::copy_from(&source).unwrap();
        let path = candidate.path().to_path_buf();
        assert!(path.is_file());
        assert_ne!(path, source_path);
        assert!(candidate.estimated_bytes() > 0);
        assert_eq!(
            candidate
                .connection()
                .query_row("SELECT length(value) FROM payload", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            262_144
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        drop(candidate);
        assert!(!path.exists());
        drop(source);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sqlite_full_is_actionable_and_leaves_no_candidate_files() {
        let root = temporary_root("full");
        let source_path = root.join("library.sqlite3");
        let source = Connection::open(&source_path).unwrap();
        source
            .execute_batch(
                "CREATE TABLE payload(value BLOB NOT NULL);
                 INSERT INTO payload VALUES (zeroblob(524288));",
            )
            .unwrap();

        let error = match Candidate::copy_from_with_page_limit(&source, 1) {
            Ok(_) => panic!("bounded staging should fail when SQLite cannot grow"),
            Err(error) => error,
        };
        assert!(error.contains("free disk space"));
        assert!(error.contains("live database was not modified"));
        let leaked = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(STAGING_PREFIX)
            });
        assert!(!leaked);
        drop(source);
        fs::remove_dir_all(root).unwrap();
    }
}
