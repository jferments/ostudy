//! AI provider profile aggregate, task defaults, credential metadata, and
//! write-only credential coordination.

pub(crate) mod commands;
mod contracts;
mod defaults;
mod policy;
mod projections;
mod repository;
mod service;

pub(crate) use contracts::{ProfileRequestSnapshot, ProfileSnapshot};
pub(crate) use repository::load_profile_snapshot;
pub(crate) use service::{load_default_profile_request, load_profile_request};

pub(crate) fn is_device_transfer_table(table: &str) -> bool {
    repository::is_device_transfer_table(table) || defaults::is_device_transfer_table(table)
}

pub(crate) fn sanitize_for_device_transfer(
    connection: &rusqlite::Connection,
    include_profiles: bool,
) -> Result<(), crate::error::CommandError> {
    defaults::sanitize_for_device_transfer(connection, include_profiles)?;
    repository::sanitize_for_device_transfer(connection, include_profiles)
}

#[cfg(test)]
mod tests;
