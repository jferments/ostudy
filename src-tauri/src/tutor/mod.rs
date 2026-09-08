//! Content-focused conversational tutoring. The tutor owns conversations and
//! fixed conversational quiz rubrics, not practice ratings or learner profiles.

pub(crate) mod commands;
mod context;
mod contracts;
mod conversations;
mod focus;
mod history;
mod memory;
mod output;
mod service;
mod turns;

#[cfg(test)]
mod tests;

pub(crate) fn reconcile_startup(
    database: &crate::database::Database,
) -> Result<(), crate::error::CommandError> {
    turns::reconcile_startup(database)
}

/// Chat sharing is not one of the device-sync consent categories. Complete
/// library backups retain chats; synchronization must never include them.
pub(crate) fn sanitize_for_device_transfer(
    connection: &rusqlite::Connection,
) -> Result<(), crate::error::CommandError> {
    connection
        .execute_batch(
            "DELETE FROM tutor_memory_operations;
         DELETE FROM tutor_turn_sources;
         DELETE FROM knowledge_sources WHERE kind = 'conversation';
         DELETE FROM tutor_turns;
         DELETE FROM tutor_conversations;",
        )
        .map_err(crate::error::CommandError::database)
}
