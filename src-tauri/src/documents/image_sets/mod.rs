//! Ordered screenshot-source use cases. Selection inspection is read-only;
//! retaining a source and publishing its canonical evidence are separate steps.

pub(crate) mod admission;
pub(crate) mod backup;
pub(crate) mod cache;
pub(crate) mod files;
pub(crate) mod import;
pub(crate) mod manifest;
pub(crate) mod repository;
pub(crate) mod raster;

#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
mod tests;
