//! Card organization, generated card types, safe templates, search, and bulk actions.
//!
//! Rust owns validation, deterministic rendering, stable reverse/cloze card
//! identities, and transactional persistence. Saved templates are data-only
//! bulk-entry forms; generated cards are ordinary Basic cards with no template
//! ownership link.

include!("authoring/types.rs");
include!("authoring/commands.rs");
include!("authoring/search_and_tags.rs");
include!("authoring/writes_and_transfer.rs");
include!("authoring/stage3/types_and_commands.rs");
include!("authoring/stage3/generated_writes.rs");
include!("authoring/stage3/generated_rendering.rs");
include!("authoring/stage3/cloze.rs");
include!("authoring/stage3/template_crud.rs");
include!("authoring/stage3/template_validation.rs");
include!("authoring/stage3/template_generation.rs");
include!("authoring/stage3/transfer.rs");
include!("authoring/normalization.rs");
include!("authoring/tests.rs");
