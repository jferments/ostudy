//! Card organization, generated card types, safe templates, search, and bulk actions.
//!
//! Rust owns validation, deterministic rendering, stable reverse card
//! identities, single-schedule rotating cloze presentations, and transactional
//! persistence. Saved templates are data-only bulk-entry forms; generated cards
//! are ordinary Basic cards with no template ownership link.

include!("authoring/types.rs");
include!("authoring/commands.rs");
include!("authoring/search_and_tags.rs");
include!("authoring/writes_and_transfer.rs");
include!("authoring/stage3_v2.rs");
include!("authoring/stage3/transfer.rs");
include!("authoring/generated_import_v2.rs");
include!("authoring/normalization.rs");
include!("authoring/tests.rs");
