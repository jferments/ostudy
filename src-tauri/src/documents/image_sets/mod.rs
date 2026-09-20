//! Ordered screenshot-source use cases. Selection inspection is read-only;
//! retaining a source and publishing its canonical evidence are separate steps.

pub(crate) mod admission;
pub(crate) mod backup;
pub(crate) mod cache;
pub(crate) mod files;
pub(crate) mod filter;
pub(crate) mod filter_commands;
pub(crate) mod import;
pub(crate) mod manifest;
pub(crate) mod operations;
pub(crate) mod prepare;
pub(crate) mod raster;
pub(crate) mod receipt;
pub(crate) mod repository;
pub(crate) mod storage;
pub(crate) mod view;

#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
mod tests;
