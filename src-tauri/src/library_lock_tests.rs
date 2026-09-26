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
    let root = Path::new(&root);
    acquire_for_process(root).unwrap();
    acquire_for_process(root).expect("same-process recreation must reuse the active library lock");
    assert!(acquire_for_process(&root.join("different-library")).is_err());
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
