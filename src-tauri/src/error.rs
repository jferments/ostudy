use serde::Serialize;

use crate::logging;

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

    #[track_caller]
    pub fn database(error: impl std::fmt::Display) -> Self {
        let caller = std::panic::Location::caller();
        Self::database_at(
            "database_error",
            format!("caller={}:{}", caller.file(), caller.line()),
            error,
        )
    }

    pub fn database_at(
        event: &str,
        context: impl AsRef<str>,
        error: impl std::fmt::Display,
    ) -> Self {
        logging::error(event, format!("{} sqlite={error}", context.as_ref()));
        Self::new(
            "database_error",
            "The local database operation failed. See the ostudy diagnostic log for details.",
        )
    }

    pub fn io(error: impl std::fmt::Display) -> Self {
        logging::error("file_error", error.to_string());
        Self::new(
            "file_error",
            "The file operation failed. See the ostudy diagnostic log for details.",
        )
    }

    pub fn serialization(error: impl std::fmt::Display) -> Self {
        logging::error("serialization_error", error.to_string());
        Self::new(
            "serialization_error",
            "The card content could not be read or saved.",
        )
    }

    pub fn invalid_package(message: impl Into<String>) -> Self {
        Self::new("invalid_deck_package", message)
    }
}
