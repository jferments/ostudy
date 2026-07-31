use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::error::CommandError;

pub fn unix_time_millis() -> Result<i64, CommandError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            CommandError::new(
                "invalid_system_clock",
                format!("The system clock is before the Unix epoch: {error}"),
            )
        })?;

    i64::try_from(duration.as_millis()).map_err(|_| {
        CommandError::new(
            "invalid_system_clock",
            "The current timestamp is outside SQLite's supported integer range.",
        )
    })
}

pub fn generate_id(connection: &Connection) -> Result<String, CommandError> {
    connection
        .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
        .map_err(CommandError::database)
}

pub fn validate_entity_id(value: &str, label: &str) -> Result<(), CommandError> {
    let valid = value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(CommandError::new(
            "invalid_identifier",
            format!("The {label} identifier is invalid."),
        ))
    }
}

pub fn bool_to_integer(value: bool) -> i64 {
    i64::from(value)
}
