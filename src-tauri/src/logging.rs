use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, SyncSender, TrySendError},
        OnceLock, RwLock,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::{json, Map, Value};

const DIAGNOSTIC_DIRECTORY: &str = "diagnostics";
const DIAGNOSTIC_FILE: &str = "ostudy.jsonl";
const PREVIOUS_DIAGNOSTIC_FILE: &str = "ostudy.previous.jsonl";
const MAX_DIAGNOSTIC_BYTES: u64 = 2 * 1024 * 1024;
const MAX_MESSAGE_CHARS: usize = 4_000;
const MAX_FIELD_CHARS: usize = 2_000;
const MAX_FIELD_DEPTH: usize = 8;
const MAX_COLLECTION_ITEMS: usize = 64;
const WRITER_QUEUE_CAPACITY: usize = 2_048;

static DIAGNOSTIC_PATH: OnceLock<PathBuf> = OnceLock::new();
static WRITER: OnceLock<SyncSender<WriterMessage>> = OnceLock::new();
static FILTER: OnceLock<RwLock<FilterConfig>> = OnceLock::new();
static SESSION_ID: OnceLock<String> = OnceLock::new();
static DROPPED_EVENTS: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Self::Off),
            "error" => Some(Self::Error),
            "warn" | "warning" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            "trace" => Some(Self::Trace),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }

    fn allows(self, event_level: Self) -> bool {
        self != Self::Off && event_level <= self
    }
}

#[derive(Clone, Debug)]
struct FilterRule {
    target: String,
    level: LogLevel,
}

#[derive(Clone, Debug)]
struct FilterConfig {
    raw: String,
    default_level: LogLevel,
    rules: Vec<FilterRule>,
}

impl FilterConfig {
    fn default_filter() -> Self {
        Self {
            raw: "info".to_string(),
            default_level: LogLevel::Info,
            rules: Vec::new(),
        }
    }

    fn parse(raw: &str) -> Result<Self, String> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Ok(Self::default_filter());
        }
        let mut default_level = LogLevel::Info;
        let mut default_explicit = false;
        let mut rules = Vec::new();
        for entry in raw.split(',').map(str::trim).filter(|entry| !entry.is_empty()) {
            if let Some((target, level)) = entry.rsplit_once('=') {
                rules.push(FilterRule {
                    target: normalize_target(target)?,
                    level: LogLevel::parse(level).ok_or_else(|| {
                        format!("Unsupported diagnostic level in filter entry: {entry}")
                    })?,
                });
            } else {
                default_level = LogLevel::parse(entry).ok_or_else(|| {
                    format!("Unsupported diagnostic filter entry: {entry}")
                })?;
                default_explicit = true;
            }
        }
        if !default_explicit && !rules.is_empty() {
            default_level = LogLevel::Warn;
        }
        rules.sort_by(|left, right| right.target.len().cmp(&left.target.len()));
        Ok(Self {
            raw: raw.to_string(),
            default_level,
            rules,
        })
    }

    fn enabled(&self, level: LogLevel, target: &str) -> bool {
        self.rules
            .iter()
            .find(|rule| target_matches(&rule.target, target))
            .map(|rule| rule.level)
            .unwrap_or(self.default_level)
            .allows(level)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticConfigSnapshot {
    pub filter: String,
    pub log_path: Option<String>,
    pub dropped_events: u64,
    pub queue_capacity: usize,
    pub maximum_file_bytes: u64,
}

enum WriterMessage {
    Event(String),
    Flush(SyncSender<()>),
    Shutdown,
}

struct WriterState {
    path: PathBuf,
    previous_path: PathBuf,
    current_bytes: u64,
    writer: Option<BufWriter<File>>,
}

impl WriterState {
    fn open(path: PathBuf) -> std::io::Result<Self> {
        let previous_path = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(PREVIOUS_DIAGNOSTIC_FILE);
        let current_bytes = fs::metadata(&path).map(|metadata| metadata.len()).unwrap_or(0);
        Ok(Self {
            writer: Some(open_writer(&path)?),
            path,
            previous_path,
            current_bytes,
        })
    }

    fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        let bytes = u64::try_from(line.len()).unwrap_or(u64::MAX).saturating_add(1);
        if self.current_bytes > 0
            && self.current_bytes.saturating_add(bytes) > MAX_DIAGNOSTIC_BYTES
        {
            self.rotate()?;
        }
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| std::io::Error::other("The diagnostic writer is closed."))?;
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
        self.current_bytes = self.current_bytes.saturating_add(bytes);
        Ok(())
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        if let Some(mut writer) = self.writer.take() {
            writer.flush()?;
        }
        let _ = fs::remove_file(&self.previous_path);
        if self.path.exists() {
            fs::rename(&self.path, &self.previous_path)?;
        }
        self.writer = Some(open_writer(&self.path)?);
        self.current_bytes = 0;
        Ok(())
    }

    fn flush(&mut self) {
        if let Some(writer) = self.writer.as_mut() {
            let _ = writer.flush();
        }
    }
}

fn open_writer(path: &Path) -> std::io::Result<BufWriter<File>> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map(BufWriter::new)
}

pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let location = panic_info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()))
            .unwrap_or_else(|| "unknown".to_string());
        error_target(
            "app.panic",
            "panic_captured",
            "A Rust panic reached the application panic hook.",
            json!({ "location": location }),
        );
        let _ = flush(Duration::from_millis(250));
        previous(panic_info);
    }));
}

pub fn initialize(app_data_dir: &Path) {
    let configured = std::env::var("OSTUDY_LOG")
        .ok()
        .and_then(|value| FilterConfig::parse(&value).ok())
        .unwrap_or_else(FilterConfig::default_filter);
    let _ = FILTER.set(RwLock::new(configured));
    let _ = SESSION_ID.set(format!(
        "{:x}-{:x}",
        timestamp_millis().max(0),
        std::process::id()
    ));

    let directory = app_data_dir.join(DIAGNOSTIC_DIRECTORY);
    if let Err(error) = fs::create_dir_all(&directory) {
        eprintln!("ostudy diagnostic directory could not be created: {error}");
        return;
    }
    let path = directory.join(DIAGNOSTIC_FILE);
    let _ = DIAGNOSTIC_PATH.set(path.clone());
    if WRITER.get().is_some() {
        return;
    }
    let Ok(mut state) = WriterState::open(path) else {
        eprintln!("ostudy diagnostic log could not be opened");
        return;
    };
    let (sender, receiver) = mpsc::sync_channel(WRITER_QUEUE_CAPACITY);
    if thread::Builder::new()
        .name("ostudy-diagnostics".to_string())
        .spawn(move || {
            while let Ok(message) = receiver.recv() {
                match message {
                    WriterMessage::Event(line) => {
                        if let Err(error) = state.write_line(&line) {
                            eprintln!("ostudy diagnostic event could not be written: {error}");
                        }
                    }
                    WriterMessage::Flush(response) => {
                        state.flush();
                        let _ = response.send(());
                    }
                    WriterMessage::Shutdown => {
                        state.flush();
                        break;
                    }
                }
            }
        })
        .is_ok()
    {
        let _ = WRITER.set(sender);
    }
}

pub fn diagnostic_log_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir
        .join(DIAGNOSTIC_DIRECTORY)
        .join(DIAGNOSTIC_FILE)
}

pub fn snapshot() -> DiagnosticConfigSnapshot {
    DiagnosticConfigSnapshot {
        filter: current_filter(),
        log_path: DIAGNOSTIC_PATH
            .get()
            .map(|path| path.to_string_lossy().into_owned()),
        dropped_events: DROPPED_EVENTS.load(Ordering::Relaxed),
        queue_capacity: WRITER_QUEUE_CAPACITY,
        maximum_file_bytes: MAX_DIAGNOSTIC_BYTES,
    }
}

pub fn current_filter() -> String {
    FILTER
        .get()
        .and_then(|filter| filter.read().ok().map(|filter| filter.raw.clone()))
        .unwrap_or_else(|| "info".to_string())
}

pub fn set_filter(raw: &str) -> Result<DiagnosticConfigSnapshot, String> {
    let parsed = FilterConfig::parse(raw)?;
    let filter = FILTER.get_or_init(|| RwLock::new(FilterConfig::default_filter()));
    *filter
        .write()
        .map_err(|_| "The diagnostic filter lock is unavailable.".to_string())? = parsed;
    Ok(snapshot())
}

pub fn enabled(level: LogLevel, target: &str) -> bool {
    FILTER
        .get()
        .and_then(|filter| filter.read().ok().map(|filter| filter.enabled(level, target)))
        .unwrap_or_else(|| FilterConfig::default_filter().enabled(level, target))
}

pub fn info(event: &str, message: impl AsRef<str>) {
    info_target("app", event, message, Value::Null);
}

pub fn warn(event: &str, message: impl AsRef<str>) {
    warn_target("app", event, message, Value::Null);
}

pub fn error(event: &str, message: impl AsRef<str>) {
    error_target("app", event, message, Value::Null);
}

pub fn debug(event: &str, message: impl AsRef<str>) {
    debug_target("app", event, message, Value::Null);
}

pub fn trace(event: &str, message: impl AsRef<str>) {
    trace_target("app", event, message, Value::Null);
}

pub fn info_target(target: &str, event: &str, message: impl AsRef<str>, fields: Value) {
    emit(LogLevel::Info, target, event, message.as_ref(), fields);
}

pub fn warn_target(target: &str, event: &str, message: impl AsRef<str>, fields: Value) {
    emit(LogLevel::Warn, target, event, message.as_ref(), fields);
}

pub fn error_target(target: &str, event: &str, message: impl AsRef<str>, fields: Value) {
    emit(LogLevel::Error, target, event, message.as_ref(), fields);
}

pub fn debug_target(target: &str, event: &str, message: impl AsRef<str>, fields: Value) {
    emit(LogLevel::Debug, target, event, message.as_ref(), fields);
}

pub fn trace_target(target: &str, event: &str, message: impl AsRef<str>, fields: Value) {
    emit(LogLevel::Trace, target, event, message.as_ref(), fields);
}

pub fn debug_lazy<F>(target: &str, event: &str, build: F)
where
    F: FnOnce() -> (String, Value),
{
    let target = normalize_target(target).unwrap_or_else(|_| "ostudy.app".to_string());
    if enabled(LogLevel::Debug, &target) {
        let (message, fields) = build();
        emit(LogLevel::Debug, &target, event, &message, fields);
    }
}

fn emit(level: LogLevel, target: &str, event: &str, message: &str, fields: Value) {
    let target = normalize_target(target).unwrap_or_else(|_| "ostudy.app".to_string());
    if !enabled(level, &target) {
        return;
    }
    let thread_name = thread::current().name().unwrap_or("unnamed").to_string();
    let line = json!({
        "schemaVersion": 1,
        "timestampMs": timestamp_millis(),
        "level": level.as_str(),
        "target": target,
        "event": normalize_event(event),
        "message": redact_text(&bounded_text(message, MAX_MESSAGE_CHARS)),
        "fields": sanitize_value(fields, 0),
        "application": "ostudy",
        "version": env!("CARGO_PKG_VERSION"),
        "platform": platform_name(),
        "sessionId": SESSION_ID.get().map(String::as_str).unwrap_or("uninitialized"),
        "thread": thread_name,
    })
    .to_string();

    if level <= LogLevel::Warn || cfg!(debug_assertions) {
        if level <= LogLevel::Warn {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    }
    let Some(sender) = WRITER.get() else {
        return;
    };
    match sender.try_send(WriterMessage::Event(line)) {
        Ok(()) => emit_drop_summary(sender),
        Err(TrySendError::Full(_)) => {
            DROPPED_EVENTS.fetch_add(1, Ordering::Relaxed);
        }
        Err(TrySendError::Disconnected(_)) => {
            if level == LogLevel::Error {
                eprintln!("ostudy diagnostic writer is unavailable");
            }
        }
    }
}

fn emit_drop_summary(sender: &SyncSender<WriterMessage>) {
    let dropped = DROPPED_EVENTS.swap(0, Ordering::AcqRel);
    if dropped == 0 {
        return;
    }
    let line = json!({
        "schemaVersion": 1,
        "timestampMs": timestamp_millis(),
        "level": "warn",
        "target": "ostudy.diagnostics.writer",
        "event": "diagnostic_events_dropped",
        "message": "Diagnostic events were dropped because the bounded writer queue was full.",
        "fields": { "droppedEvents": dropped },
        "application": "ostudy",
        "version": env!("CARGO_PKG_VERSION"),
        "platform": platform_name(),
    })
    .to_string();
    if sender.try_send(WriterMessage::Event(line)).is_err() {
        DROPPED_EVENTS.fetch_add(dropped, Ordering::Relaxed);
    }
}

pub fn flush(timeout: Duration) -> bool {
    let Some(sender) = WRITER.get() else {
        return true;
    };
    let (response_sender, response_receiver) = mpsc::sync_channel(0);
    sender.send(WriterMessage::Flush(response_sender)).is_ok()
        && response_receiver.recv_timeout(timeout).is_ok()
}

pub fn shutdown() {
    let _ = flush(Duration::from_secs(1));
    if let Some(sender) = WRITER.get() {
        let _ = sender.send(WriterMessage::Shutdown);
    }
}

fn normalize_target(value: &str) -> Result<String, String> {
    let mut target = value.trim().to_ascii_lowercase().replace("::", ".");
    if target.is_empty() {
        return Err("Invalid diagnostic target.".to_string());
    }
    if !target.starts_with("ostudy.") && target != "ostudy" {
        target = format!("ostudy.{target}");
    }
    if target.len() > 128
        || !target.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '_' | '-')
        })
    {
        return Err("Invalid diagnostic target.".to_string());
    }
    Ok(target)
}

fn target_matches(configured: &str, actual: &str) -> bool {
    actual == configured
        || actual
            .strip_prefix(configured)
            .is_some_and(|remainder| remainder.starts_with('.'))
}

fn normalize_event(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    if normalized.is_empty()
        || normalized.len() > 96
        || !normalized.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        "diagnostic_event".to_string()
    } else {
        normalized
    }
}

fn sanitize_value(value: Value, depth: usize) -> Value {
    if depth >= MAX_FIELD_DEPTH {
        return Value::String("[maximum diagnostic depth reached]".to_string());
    }
    match value {
        Value::String(value) => Value::String(redact_text(&bounded_text(&value, MAX_FIELD_CHARS))),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .take(MAX_COLLECTION_ITEMS)
                .map(|value| sanitize_value(value, depth + 1))
                .collect(),
        ),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .take(MAX_COLLECTION_ITEMS)
                .map(|(key, value)| {
                    let value = if sensitive_key(&key) {
                        Value::String("[redacted]".to_string())
                    } else {
                        sanitize_value(value, depth + 1)
                    };
                    (bounded_text(&key, 96), value)
                })
                .collect::<Map<_, _>>(),
        ),
        other => other,
    }
}

fn sensitive_key(key: &str) -> bool {
    let key = key
        .to_ascii_lowercase()
        .replace('-', "")
        .replace('_', "");
    [
        "authorization",
        "apikey",
        "credential",
        "password",
        "prompt",
        "completion",
        "requestbody",
        "responsebody",
        "secret",
        "token",
        "qrcontents",
        "documenttext",
        "cardcontent",
    ]
    .iter()
    .any(|candidate| key.contains(candidate))
}

fn redact_text(value: &str) -> String {
    value
        .split_whitespace()
        .map(|token| {
            let trimmed = token.trim_matches(|character: char| {
                matches!(character, '"' | '\'' | ',' | ';' | '(' | ')' | '[' | ']')
            });
            if secret_like_token(trimmed) {
                token.replace(trimmed, "[redacted]")
            } else if let Some((key, _)) = trimmed.split_once('=') {
                if sensitive_key(key) {
                    token.replace(trimmed, &format!("{key}=[redacted]"))
                } else {
                    token.to_string()
                }
            } else {
                token.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn secret_like_token(value: &str) -> bool {
    value.starts_with("sk-")
        || value.starts_with("sk-or-")
        || value.starts_with("AIza")
        || looks_like_jwt(value)
}

fn looks_like_jwt(value: &str) -> bool {
    let segments = value.split('.').collect::<Vec<_>>();
    segments.len() == 3
        && segments.iter().all(|segment| {
            segment.len() >= 8
                && segment.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
                })
        })
}

fn bounded_text(value: &str, maximum: usize) -> String {
    let length = value.chars().count();
    if length <= maximum {
        return value.to_string();
    }
    format!(
        "{}… [truncated {} characters]",
        value.chars().take(maximum).collect::<String>(),
        length.saturating_sub(maximum)
    )
}

fn timestamp_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or_default()
}

fn platform_name() -> &'static str {
    if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_choose_the_longest_matching_target() {
        let filter = FilterConfig::parse("warn,database=debug,database.migration=trace")
            .expect("filter should parse");
        assert!(filter.enabled(LogLevel::Trace, "ostudy.database.migration"));
        assert!(filter.enabled(LogLevel::Debug, "ostudy.database.connection"));
        assert!(!filter.enabled(LogLevel::Info, "ostudy.ai.gateway"));
    }

    #[test]
    fn diagnostic_fields_redact_secrets_and_user_content() {
        let value = sanitize_value(
            json!({
                "apiKey": "sk-or-secret-value",
                "prompt": "private flashcard content",
                "safeCount": 4
            }),
            0,
        );
        assert_eq!(value["apiKey"], "[redacted]");
        assert_eq!(value["prompt"], "[redacted]");
        assert_eq!(value["safeCount"], 4);
    }

    #[test]
    fn writer_keeps_only_current_and_previous_files() {
        let root = std::env::temp_dir().join(format!(
            "ostudy-logging-test-{}-{}",
            std::process::id(),
            timestamp_millis()
        ));
        fs::create_dir_all(&root).expect("temporary directory should exist");
        let path = root.join(DIAGNOSTIC_FILE);
        let mut writer = WriterState::open(path.clone()).expect("writer should open");
        writer.current_bytes = MAX_DIAGNOSTIC_BYTES;
        writer.write_line("{\"event\":\"rotation\"}").expect("write should rotate");
        writer.flush();
        assert!(path.exists());
        assert!(root.join(PREVIOUS_DIAGNOSTIC_FILE).exists());
        assert_eq!(fs::read_dir(&root).expect("directory should read").count(), 2);
        let _ = fs::remove_dir_all(root);
    }
}
