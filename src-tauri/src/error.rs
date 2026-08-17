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
            "database_operation_failed",
            format!("{}:{}", caller.file(), caller.line()),
            error,
        )
    }

    pub fn database_at(
        event: &str,
        context: impl AsRef<str>,
        error: impl std::fmt::Display,
    ) -> Self {
        logging::error_target(
            "database.operation",
            event,
            "A local SQLite operation failed.",
            serde_json::json!({
                "context": context.as_ref(),
                "sqliteError": error.to_string(),
            }),
        );
        Self::new(
            "database_error",
            "The local database operation failed. See the ostudy diagnostic log for details.",
        )
    }

    pub fn io(error: impl std::fmt::Display) -> Self {
        logging::error_target(
            "filesystem.operation",
            "file_operation_failed",
            "A local file operation failed.",
            serde_json::json!({ "ioError": error.to_string() }),
        );
        Self::new(
            "file_error",
            "The file operation failed. See the ostudy diagnostic log for details.",
        )
    }

    pub fn serialization(error: impl std::fmt::Display) -> Self {
        logging::error_target(
            "serialization",
            "serialization_failed",
            "Structured application data could not be encoded or decoded.",
            serde_json::json!({ "serializationError": error.to_string() }),
        );
        Self::new(
            "serialization_error",
            "The card content could not be read or saved.",
        )
    }

    pub fn invalid_package(message: impl Into<String>) -> Self {
        Self::new("invalid_deck_package", message)
    }
}
