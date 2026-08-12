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

pub fn info(event: &str, message: impl AsRef<str>) {
    println!("{}", record("info", event, message.as_ref()));
}

pub fn error(event: &str, message: impl AsRef<str>) {
    eprintln!("{}", record("error", event, message.as_ref()));
}
