//! Card organization, discovery, and high-volume entry workflows.
//!
//! This module owns Stage 2 metadata and multi-card transactions. Rich card
//! documents continue to pass through the validator in `cards`; search indexes
//! contain only projections of already validated durable text.

mod browse;
mod bulk;
mod duplicates;
mod save;
mod tags;
mod transfer;

use rusqlite::{Connection, OptionalExtension};

use crate::error::CommandError;

pub use browse::browse_cards;
pub use bulk::bulk_create_cards;
pub use duplicates::check_card_duplicates;
pub use save::{create_organized_card, update_organized_card};
pub use tags::set_card_tags;
pub use transfer::transfer_cards;

pub(crate) fn ensure_active_deck(connection: &Connection, deck_id: &str) -> Result<(), CommandError> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(\n\
                 SELECT 1 FROM decks\n\
                 WHERE id = ?1 AND deleted_at IS NULL AND archived_at IS NULL\n\
             )",
            [deck_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if exists == 0 {
        return Err(CommandError::new(
            "deck_not_found",
            "The destination deck no longer exists.",
        ));
    }
    Ok(())
}

pub(crate) fn ensure_active_note(connection: &Connection, note_id: &str) -> Result<(), CommandError> {
    let exists = connection
        .query_row(
            "SELECT 1\n\
             FROM notes n\n\
             JOIN decks d ON d.id = n.deck_id\n\
             WHERE n.id = ?1\n\
               AND n.deleted_at IS NULL\n\
               AND d.deleted_at IS NULL\n\
               AND d.archived_at IS NULL",
            [note_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    if exists.is_none() {
        return Err(CommandError::new(
            "card_not_found",
            "That flashcard no longer exists.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
