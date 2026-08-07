//! Card organization, generated card types, safe templates, search, and bulk actions.
//!
//! Rust owns validation, deterministic rendering, stable reverse card
//! identities, single-schedule rotating cloze presentations, and transactional
//! persistence. Saved templates are data-only bulk-entry forms; generated cards
//! are ordinary Basic cards with no template ownership link.
//!
//! The implementation is currently split with `include!` for file-size and
//! reviewability reasons. This is a lexical split, not a Rust module boundary:
//! every included file shares this module's imports, private helpers, and name
//! space. Code in these files must therefore be treated as one coupled module
//! until a later refactor introduces ordinary `mod` declarations and explicit
//! visibility without changing command names, validation order, or transaction
//! semantics.

include!("authoring/types.rs");
include!("authoring/commands.rs");
include!("authoring/search_and_tags.rs");
include!("authoring/writes_and_transfer.rs");
include!("authoring/stage3_v2.rs");
include!("authoring/stage3/transfer.rs");
include!("authoring/generated_import.rs");
include!("authoring/normalization.rs");
include!("authoring/tests.rs");
include!("authoring/stage36_38_tests.rs");
