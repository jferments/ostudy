//! Deferred annotation drafts never become evidence for graded components.
//! Recall and explanation retain the independently admitted core statement.
use rusqlite::Connection;

use super::invalid;
use crate::{
    error::CommandError,
    knowledge::{
        authoring::{KnowledgeCatalog, UnitSupport},
        contracts::KnowledgeUnit,
    },
};

pub(super) fn available(unit: &KnowledgeUnit) -> bool {
    unit.content
        .details
        .as_ref()
        .is_none_or(|details| details.deferred_rule().is_none())
}

pub(super) fn allows(catalog: &KnowledgeCatalog, support: &[UnitSupport]) -> bool {
    support.iter().all(|binding| {
        catalog
            .units
            .iter()
            .find(|unit| unit.id == binding.unit_id)
            .is_some_and(available)
    })
}

/// The caller first validates current source/support hashes in the same
/// publication transaction. Recheck annotation capability at this final boundary
/// as well as new-response and cached-response admission.
pub(super) fn validate_stored(
    connection: &Connection,
    support: &[UnitSupport],
) -> Result<(), CommandError> {
    for binding in support {
        let encoded: String = connection
            .query_row(
                "SELECT record_json FROM knowledge_units WHERE id=?1",
                [&binding.unit_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        let unit: KnowledgeUnit =
            serde_json::from_str(&encoded).map_err(CommandError::serialization)?;
        if !available(&unit) {
            return Err(unavailable());
        }
    }
    Ok(())
}

pub(super) fn unavailable() -> CommandError {
    invalid("A deferred knowledge annotation cannot support a graded answer component. Use other admitted evidence or a recall card; do not reconstruct the failed annotation.")
}

#[cfg(test)]
mod tests;
