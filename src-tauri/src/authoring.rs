//! Card organization, tags, full-text search, bulk entry, and move/copy.
//!
//! This module owns the Phase 2 Stage 2 authoring boundary. All user-controlled
//! values are validated before they reach SQLite, all writes use transactions,
//! and FTS queries are built from quoted tokens rather than accepting raw FTS5
//! syntax from the interface.

include!("authoring/types.rs");
include!("authoring/commands.rs");
include!("authoring/search_and_tags.rs");
include!("authoring/writes_and_transfer.rs");
include!("authoring/normalization.rs");
include!("authoring/tests.rs");
