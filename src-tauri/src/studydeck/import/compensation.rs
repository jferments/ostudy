//! Compensation for an uncommitted studydeck file promotion. The caller owns
//! the SQL decision and primary error. Failed compensation preserves files and
//! reports bounded recovery facts; it never overwrites another staged file.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::util::durable_fs;

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct CompensationReport {
    pub restored: usize,
    pub failed: usize,
}

pub(super) fn compensate_promotions(
    operation_id: &str,
    promoted: &[(PathBuf, PathBuf)],
) -> CompensationReport {
    let report = compensate_with(promoted, restore_file);
    if report.failed > 0 {
        crate::logging::warn_target(
            "studydeck.import",
            "import_compensation_failed",
            "An uncommitted import could not restore every promoted file. Recovery inputs were retained.",
            serde_json::json!({
                "operationId": operation_id,
                "restoredFiles": report.restored,
                "failedFiles": report.failed,
            }),
        );
    }
    report
}

fn compensate_with(
    promoted: &[(PathBuf, PathBuf)],
    mut restore: impl FnMut(&Path, &Path) -> io::Result<()>,
) -> CompensationReport {
    let mut report = CompensationReport::default();
    for (staged, published) in promoted.iter().rev() {
        match restore(staged, published) {
            Ok(()) => report.restored += 1,
            Err(_) => report.failed += 1,
        }
    }
    report
}

fn restore_file(staged: &Path, published: &Path) -> io::Result<()> {
    match fs::symlink_metadata(staged) {
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Import compensation will not overwrite retained staging evidence.",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    if !fs::symlink_metadata(published)?.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The promoted import file is not an ordinary file.",
        ));
    }
    fs::rename(published, staged)?;
    let staged_parent = staged.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Missing import staging parent.",
        )
    })?;
    let published_parent = published.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Missing import destination parent.",
        )
    })?;
    durable_fs::sync_directory(staged_parent)?;
    durable_fs::sync_directory(published_parent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn failed_compensation_does_not_stop_remaining_recovery_or_hide_its_count() {
        let promoted = [
            (PathBuf::from("first-staged"), PathBuf::from("first-final")),
            (
                PathBuf::from("second-staged"),
                PathBuf::from("second-final"),
            ),
            (PathBuf::from("third-staged"), PathBuf::from("third-final")),
        ];
        let mut visited = Vec::new();
        let report = compensate_with(&promoted, |staged, _| {
            visited.push(staged.to_path_buf());
            if staged == Path::new("second-staged") {
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "injected"))
            } else {
                Ok(())
            }
        });
        assert_eq!(
            report,
            CompensationReport {
                restored: 2,
                failed: 1
            }
        );
        assert_eq!(
            visited,
            ["third-staged", "second-staged", "first-staged"].map(PathBuf::from)
        );
    }

    #[test]
    fn compensation_restores_owned_files_but_preserves_collisions_and_missing_inputs() {
        let root = std::env::temp_dir().join(format!(
            "ostudy-compensation-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("staged")).unwrap();
        fs::create_dir_all(root.join("final")).unwrap();
        let staged = root.join("staged/image.png");
        let published = root.join("final/image.png");
        fs::write(&published, b"retained import bytes").unwrap();
        restore_file(&staged, &published).unwrap();
        assert_eq!(fs::read(&staged).unwrap(), b"retained import bytes");
        assert!(!published.exists());

        fs::write(&published, b"different candidate").unwrap();
        assert_eq!(
            restore_file(&staged, &published).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(&staged).unwrap(), b"retained import bytes");
        assert_eq!(fs::read(&published).unwrap(), b"different candidate");
        let missing = root.join("staged/missing.png");
        let absent = root.join("final/missing.png");
        assert_eq!(
            restore_file(&missing, &absent).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        fs::remove_dir_all(root).unwrap();
    }
}
