//! Narrow SQL projections shared by workload, library, practice, and analytics.
//!
//! This is deliberately not a generic ORM. Each child module encodes one bounded
//! read contract and keeps statement count independent of returned row count.

pub mod cards;
pub mod decks;
pub mod learning_signals;
pub mod practice;
pub mod scope;
