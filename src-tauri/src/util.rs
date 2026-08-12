use rusqlite::Connection;

use crate::error::CommandError;

pub fn unix_time_millis() -> Result<i64, CommandError> {
    crate::platform::now_utc_millis()
        .map_err(|message| CommandError::new("invalid_system_clock", message))
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
