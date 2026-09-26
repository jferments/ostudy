use super::{native_flock, try_lock_exclusive};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ostudy-native-lock-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        Self(path)
    }

    fn open(&self) -> File {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.0)
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn android_library_lock_excludes_competitors_and_releases_on_close() {
    let fixture = Fixture::new();
    fs::write(&fixture.0, b"preserved lock-file contents").unwrap();
    let owner = fixture.open();
    let competitor = fixture.open();
    native_flock::try_lock_exclusive(&owner).unwrap();
    assert!(matches!(
        native_flock::try_lock_exclusive(&competitor),
        Err(TryLockError::WouldBlock)
    ));
    assert!(matches!(
        try_lock_exclusive(&competitor),
        Err(TryLockError::WouldBlock)
    ));
    assert_eq!(fs::read(&fixture.0).unwrap(), b"preserved lock-file contents");
    drop(owner);
    native_flock::try_lock_exclusive(&competitor).unwrap();
}

#[test]
fn android_library_lock_respects_an_existing_standard_library_owner() {
    let fixture = Fixture::new();
    let owner = fixture.open();
    let competitor = fixture.open();
    try_lock_exclusive(&owner).unwrap();
    assert!(matches!(
        native_flock::try_lock_exclusive(&competitor),
        Err(TryLockError::WouldBlock)
    ));
    drop(owner);
    native_flock::try_lock_exclusive(&competitor).unwrap();
}

#[test]
fn android_library_lock_is_retained_until_the_last_duplicate_handle_closes() {
    let fixture = Fixture::new();
    let owner = fixture.open();
    native_flock::try_lock_exclusive(&owner).unwrap();
    let duplicate = owner.try_clone().unwrap();
    let competitor = fixture.open();
    drop(owner);
    assert!(matches!(
        native_flock::try_lock_exclusive(&competitor),
        Err(TryLockError::WouldBlock)
    ));
    drop(duplicate);
    try_lock_exclusive(&competitor).unwrap();
}
