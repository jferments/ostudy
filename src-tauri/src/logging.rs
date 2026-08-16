use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex, OnceLock},
};

const DIAGNOSTIC_DIRECTORY: &str = "diagnostics";
const DIAGNOSTIC_FILE: &str = "ostudy.jsonl";
const PREVIOUS_DIAGNOSTIC_FILE: &str = "ostudy.previous.jsonl";
const MAX_DIAGNOSTIC_BYTES: u64 = 2 * 1024 * 1024;

static DIAGNOSTIC_PATH: OnceLock<PathBuf> = OnceLock::new();
static FILE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn timestamp_millis() -> i64 {
    crate::platform::now_utc_millis().unwrap_or_default()
}

fn record(level: &str, event: &str, message: &str) -> serde_json::Value {
    serde_json::json!({
        "timestampMs": timestamp_millis(),
        "level": level,
        "event": event,
        "message": message,
        "application": "ostudy",
        "version": env!("CARGO_PKG_VERSION"),
    })
}

pub fn initialize(app_data_dir: &Path) {
    let directory = app_data_dir.join(DIAGNOSTIC_DIRECTORY);
    if let Err(error) = fs::create_dir_all(&directory) {
        eprintln!("ostudy diagnostic directory could not be created: {error}");
        return;
    }
    let _ = DIAGNOSTIC_PATH.set(directory.join(DIAGNOSTIC_FILE));
}

pub fn diagnostic_log_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(DIAGNOSTIC_DIRECTORY).join(DIAGNOSTIC_FILE)
}

pub fn info(event: &str, message: impl AsRef<str>) {
    emit("info", event, message.as_ref(), false);
}

pub fn error(event: &str, message: impl AsRef<str>) {
    emit("error", event, message.as_ref(), true);
}

fn emit(level: &str, event: &str, message: &str, is_error: bool) {
    let line = record(level, event, message).to_string();
    if is_error {
        eprintln!("{line}");
    } else {
        println!("{line}");
    }
    #[cfg(target_os = "android")]
    android_log(if is_error { 6 } else { 4 }, &line);
    append_persistent(&line);
}

fn append_persistent(line: &str) {
    let Some(path) = DIAGNOSTIC_PATH.get() else {
        return;
    };
    let Ok(_guard) = FILE_LOCK.lock() else {
        return;
    };
    if fs::metadata(path)
        .map(|metadata| metadata.len() >= MAX_DIAGNOSTIC_BYTES)
        .unwrap_or(false)
    {
        if let Some(parent) = path.parent() {
            let previous = parent.join(PREVIOUS_DIAGNOSTIC_FILE);
            let _ = fs::remove_file(&previous);
            let _ = fs::rename(path, previous);
        }
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

#[cfg(target_os = "android")]
fn android_log(priority: i32, line: &str) {
    use std::ffi::{c_char, CString};

    #[link(name = "log")]
    extern "C" {
        fn __android_log_write(priority: i32, tag: *const c_char, text: *const c_char) -> i32;
    }

    let Ok(tag) = CString::new("ostudy") else {
        return;
    };
    let sanitized = line.replace('\0', "�");
    let Ok(text) = CString::new(sanitized) else {
        return;
    };
    // SAFETY: Android's liblog copies both NUL-terminated strings during this
    // call. `tag` and `text` remain alive until the call returns, and priority
    // is one of Android's documented log priorities.
    unsafe {
        __android_log_write(priority, tag.as_ptr(), text.as_ptr());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_path_is_app_private_and_stable() {
        let root = Path::new("/tmp/ostudy-test-data");
        assert_eq!(
            diagnostic_log_path(root),
            root.join("diagnostics").join("ostudy.jsonl")
        );
    }
}
