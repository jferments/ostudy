//! Durable staged image ingest and conservative filesystem reconciliation.
//!
//! SQLite is the authoritative asset ledger. Files are first copied into a
//! draft-owned staging path, flushed, and then promoted by the same transaction
//! that creates a note, template, or imported deck. Every filesystem boundary is
//! represented in the ledger so startup can repair a crash without guessing or
//! deleting a referenced image.

pub(crate) mod occlusion;

include!("asset_lifecycle/core_a.rs");
include!("asset_lifecycle/core_b.rs");
include!("asset_lifecycle/core_c.rs");
include!("asset_lifecycle/reconcile_a1.rs");
include!("asset_lifecycle/reconcile_a2.rs");
include!("asset_lifecycle/reconcile_b1.rs");
include!("asset_lifecycle/reconcile_b2.rs");
include!("asset_lifecycle/reconcile_b3.rs");

/// Stages a small PNG into a new draft of `purpose`, as an editor's image
/// import does, for owners' tests. Returns the draft and the staged asset.
#[cfg(test)]
pub(crate) fn stage_test_image(
    connection: &mut Connection,
    paths: &AppPaths,
    purpose: &str,
) -> Result<(AssetDraftDto, AssetDto), CommandError> {
    let draft = begin_draft_in_connection(connection, purpose, None)?;
    let source = paths
        .app_data_dir()
        .join(format!("test-source-{}.png", generate_id(connection)?));
    fs::write(
        &source,
        [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0],
    )
    .map_err(CommandError::io)?;
    let asset = stage_source_in_connection(
        connection,
        paths,
        &source,
        &draft.id,
        IngestFaults::default(),
    )?;
    Ok((draft, asset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cards::validate_content,
        decks::{create_deck_in_connection, CreateDeckInput},
        scheduler::insert_new_schedule,
    };
    use serde_json::json;

    include!("asset_lifecycle/tests_a.rs");
    include!("asset_lifecycle/tests_b.rs");
    include!("asset_lifecycle/tests_c.rs");
    include!("asset_lifecycle/tests_d.rs");
}
