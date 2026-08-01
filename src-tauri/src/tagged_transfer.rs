//! Backward-compatible tag preservation for `.studydeck` packages.
//!
//! The original package format is still readable by older ostudy builds. New
//! exports add optional `tags` and `note_tags` tables that old readers ignore;
//! this wrapper restores those optional records after the core package importer
//! has validated and inserted the deck content.

include!("tagged_transfer/types_and_commands.rs");
include!("tagged_transfer/export.rs");
include!("tagged_transfer/import.rs");
include!("tagged_transfer/tests.rs");
