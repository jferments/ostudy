//! Exclusive file-lock adapter used before any library initialization.
//!
//! Rust 1.97.1's std::fs::File::try_lock always returns Unsupported on Android
//! (rust-lang/rust#148325; fixed upstream by #157038 for Rust 1.98). Bionic
//! already exposes flock, so use it without weakening process exclusion or
//! changing the repository's pinned compiler. Other platforms retain std locks.

use std::fs::{File, TryLockError};

pub(super) fn try_lock_exclusive(file: &File) -> Result<(), TryLockError> {
    #[cfg(target_os = "android")]
    {
        native_flock::try_lock_exclusive(file)
    }
    #[cfg(not(target_os = "android"))]
    {
        file.try_lock()
    }
}

// Compile the identical Android adapter on Linux in tests so the native syscall
// contract and interoperability are exercised by the ordinary Rust suite.
#[cfg(any(target_os = "android", all(test, target_os = "linux")))]
mod native_flock {
    use std::{
        fs::{File, TryLockError},
        io,
        os::{fd::AsRawFd, raw::c_int},
    };

    const LOCK_EX: c_int = 2;
    const LOCK_NB: c_int = 4;

    unsafe extern "C" {
        fn flock(fd: c_int, operation: c_int) -> c_int;
    }

    pub(super) fn try_lock_exclusive(file: &File) -> Result<(), TryLockError> {
        loop {
            // SAFETY: File owns a live descriptor for the duration of this call.
            // flock takes no pointers and does not close or take ownership of it.
            // LOCK_NB prevents waiting on another process. The retained File
            // releases the lock when closed, including on process termination.
            if unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } == 0 {
                return Ok(());
            }
            let error = io::Error::last_os_error();
            match error.kind() {
                io::ErrorKind::Interrupted => continue,
                io::ErrorKind::WouldBlock => return Err(TryLockError::WouldBlock),
                _ => return Err(TryLockError::Error(error)),
            }
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "file_lock_tests.rs"]
mod tests;
