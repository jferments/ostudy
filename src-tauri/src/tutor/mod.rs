//! Content-focused conversational tutoring. The tutor owns conversations and
//! fixed conversational quiz rubrics, not practice ratings or learner profiles.

pub(crate) mod cards;
pub(crate) mod commands;
mod context;
mod contracts;
mod conversations;
pub(crate) mod figures;
mod focus;
mod history;
mod memory;
mod organization;
mod output;
mod projects;
mod service;
pub(crate) mod speech;
mod turns;

#[cfg(test)]
mod tests;

pub(crate) fn reconcile_startup(
    database: &crate::database::Database,
) -> Result<(), crate::error::CommandError> {
    turns::reconcile_startup(database)
}

/// Chat sharing is not one of the device-sync consent categories. Complete
/// library backups retain chats; synchronization must never include them,
/// nor the pictures attached to them.
pub(crate) fn sanitize_for_device_transfer(
    connection: &rusqlite::Connection,
) -> Result<(), crate::error::CommandError> {
    connection
        .execute_batch(
            "DELETE FROM tutor_memory_operations;
         DELETE FROM tutor_organization_operations;
         DELETE FROM tutor_turn_sources;
         DELETE FROM tutor_turn_replacements;
         CREATE TEMP TABLE sync_tutor_pictures AS
             SELECT asset_id FROM tutor_turn_attachments WHERE asset_id IS NOT NULL;
         DELETE FROM tutor_turn_attachments;
         DELETE FROM assets WHERE id IN (SELECT asset_id FROM sync_tutor_pictures)
             AND id NOT IN (SELECT asset_id FROM note_assets)
             AND id NOT IN (SELECT asset_id FROM card_template_assets)
             AND id NOT IN (SELECT asset_id FROM question_attempt_assets)
             AND id NOT IN (SELECT asset_id FROM notebook_item_assets)
             AND id NOT IN (SELECT asset_id FROM speech_segments)
             AND id NOT IN (SELECT asset_id FROM speech_voices);
         DROP TABLE sync_tutor_pictures;
         DELETE FROM knowledge_sources WHERE kind = 'conversation';
         DELETE FROM tutor_turns;
         DELETE FROM tutor_conversations;
         DELETE FROM tutor_projects;",
        )
        .map_err(crate::error::CommandError::database)
}
