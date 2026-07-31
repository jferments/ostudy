use std::time::{SystemTime, UNIX_EPOCH};

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
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

pub fn info(event: &str, message: impl AsRef<str>) {
    println!("{}", record("info", event, message.as_ref()));
}

pub fn error(event: &str, message: impl AsRef<str>) {
    eprintln!("{}", record("error", event, message.as_ref()));
}
