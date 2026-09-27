use super::*;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "ostudy-durability-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn path(&self) -> PathBuf {
        self.0.join("published.json")
    }

    fn temporary_files(&self) -> Vec<PathBuf> {
        fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|value| value == "partial"))
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn directory_flush_refuses_missing_and_non_directory_paths() {
    let fixture = Fixture::new();
    assert_eq!(
        sync_directory(&fixture.0.join("missing")).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    fs::write(fixture.path(), b"not a directory").unwrap();
    assert_eq!(
        sync_directory(&fixture.path()).unwrap_err().kind(),
        io::ErrorKind::NotADirectory
    );
    assert!(sync_optional_directory(None).is_ok());
    // This exercises the actual Windows handle/flush adapter on Windows too;
    // another platform's successful execution does not qualify that branch.
    sync_directory(&fixture.0).unwrap();
}

#[cfg(unix)]
#[test]
fn directory_flush_refuses_symlinks_and_temporary_files_are_private() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let fixture = Fixture::new();
    let link = fixture.0.join("directory-link");
    symlink(&fixture.0, &link).unwrap();
    assert_eq!(
        sync_directory(&link).unwrap_err().kind(),
        io::ErrorKind::NotADirectory
    );
    let (path, file) = new_temporary(&fixture.0).unwrap();
    assert_eq!(file.metadata().unwrap().permissions().mode() & 0o777, 0o600);
    drop(file);
    assert!(path.is_file());
}

#[test]
fn replacing_a_file_never_removes_the_old_version_before_publication() {
    for failure in [
        PublicationPhase::BeforeWrite,
        PublicationPhase::AfterWrite,
        PublicationPhase::AfterFileSync,
        PublicationPhase::BeforeRename,
        PublicationPhase::AfterRename,
        PublicationPhase::AfterDirectorySync,
    ] {
        let fixture = Fixture::new();
        let path = fixture.path();
        fs::write(&path, b"old complete document").unwrap();
        let error = write_replacing_file_with(&path, b"new complete document", |phase| {
            let expected: &[u8] = if matches!(
                phase,
                PublicationPhase::AfterRename | PublicationPhase::AfterDirectorySync
            ) {
                b"new complete document"
            } else {
                b"old complete document"
            };
            assert_eq!(fs::read(&path).unwrap(), expected);
            if phase == failure {
                Err(io::Error::other("injected durability failure"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        let published = matches!(
            failure,
            PublicationPhase::AfterRename | PublicationPhase::AfterDirectorySync
        );
        if published {
            assert_eq!(fs::read(&path).unwrap(), b"new complete document");
            assert!(fixture.temporary_files().is_empty());
        } else {
            assert_eq!(fs::read(&path).unwrap(), b"old complete document");
            let retained = fixture.temporary_files();
            assert_eq!(retained.len(), 1);
            if failure != PublicationPhase::BeforeWrite {
                assert_eq!(fs::read(&retained[0]).unwrap(), b"new complete document");
            }
        }
    }
}

#[test]
fn successful_replacement_and_parent_creation_leave_no_partial_file() {
    let fixture = Fixture::new();
    let path = fixture.0.join("new/nested/checkpoint.json");
    write_replacing_file(&path, b"first").unwrap();
    write_replacing_file(&path, b"second").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"second");
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    assert_eq!(
        write_replacing_file(Path::new("no-parent"), b"x")
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
}

#[test]
fn failed_rename_retains_candidate_and_does_not_remove_destination() {
    let fixture = Fixture::new();
    let destination = fixture.path();
    let result = write_replacing_file_with(&destination, b"candidate", |phase| {
        if phase == PublicationPhase::BeforeRename {
            fs::create_dir(&destination)?;
            fs::write(destination.join("sentinel"), b"unrelated content")?;
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(
        fs::read(destination.join("sentinel")).unwrap(),
        b"unrelated content"
    );
    let retained = fixture.temporary_files();
    assert_eq!(retained.len(), 1);
    assert_eq!(fs::read(&retained[0]).unwrap(), b"candidate");
}

#[test]
fn abrupt_exit_during_publication_retains_a_complete_old_or_new_file() {
    const CHILD_ROOT: &str = "OSTUDY_DURABILITY_CHILD_ROOT";
    const CHILD_PHASE: &str = "OSTUDY_DURABILITY_CHILD_PHASE";
    if let (Some(root), Ok(phase)) = (std::env::var_os(CHILD_ROOT), std::env::var(CHILD_PHASE)) {
        let destination = PathBuf::from(root).join("published.json");
        let cutpoint = if phase == "before" {
            PublicationPhase::BeforeRename
        } else {
            PublicationPhase::AfterRename
        };
        write_replacing_file_with(&destination, b"new complete document", |current| {
            if current == cutpoint {
                std::process::exit(77);
            }
            Ok(())
        })
        .unwrap();
        panic!("The requested publication cutpoint was not reached");
    }
    for phase in ["before", "after"] {
        let fixture = Fixture::new();
        fs::write(fixture.path(), b"old complete document").unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "util::durable_fs::tests::abrupt_exit_during_publication_retains_a_complete_old_or_new_file",
                "--nocapture",
            ])
            .env(CHILD_ROOT, &fixture.0)
            .env(CHILD_PHASE, phase)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(77));
        if phase == "before" {
            assert_eq!(fs::read(fixture.path()).unwrap(), b"old complete document");
            let retained = fixture.temporary_files();
            assert_eq!(retained.len(), 1);
            assert_eq!(fs::read(&retained[0]).unwrap(), b"new complete document");
        } else {
            assert_eq!(fs::read(fixture.path()).unwrap(), b"new complete document");
        }
    }
}
