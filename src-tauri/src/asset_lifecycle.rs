//! Durable staged image ingest and conservative filesystem reconciliation.
//!
//! SQLite is the authoritative asset ledger. Files are first copied into a
//! draft-owned staging path, flushed, and then promoted by the same transaction
//! that creates a note, template, or imported deck. Every filesystem boundary is
//! represented in the ledger so startup can repair a crash without guessing or
//! deleting a referenced image.

include!("asset_lifecycle/core_a.rs");
include!("asset_lifecycle/core_b.rs");
include!("asset_lifecycle/core_c.rs");
include!("asset_lifecycle/reconcile_a1.rs");
include!("asset_lifecycle/reconcile_a2.rs");
include!("asset_lifecycle/reconcile_b1.rs");
include!("asset_lifecycle/reconcile_b2.rs");
include!("asset_lifecycle/reconcile_b3.rs");

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
