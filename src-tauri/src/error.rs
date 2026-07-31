use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn database(error: impl std::fmt::Display) -> Self {
        eprintln!("Database error: {error}");
        Self::new(
            "database_error",
            "The local database operation failed. See the terminal for details.",
        )
    }

    pub fn io(error: impl std::fmt::Display) -> Self {
        eprintln!("File operation error: {error}");
        Self::new(
            "file_error",
            "The file operation failed. See the terminal for details.",
        )
    }

    pub fn serialization(error: impl std::fmt::Display) -> Self {
        eprintln!("Serialization error: {error}");
        Self::new(
            "serialization_error",
            "The card content could not be read or saved.",
        )
    }

    pub fn invalid_package(message: impl Into<String>) -> Self {
        Self::new("invalid_deck_package", message)
    }
}
