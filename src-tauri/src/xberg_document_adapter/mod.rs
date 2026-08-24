mod adapter;
mod assets;
mod config;
mod detection;
mod errors;
mod metadata;
mod normalization;
mod opc;
mod package_security;
mod runtime;
mod support;
mod tables;

#[cfg(test)]
mod tests;

pub(crate) const XBERG_ADAPTER_BACKEND: &str = "xberg-canonical-adapter-v1";
pub(crate) const XBERG_EXTRACTION_BACKEND: &str = "xberg-1.1.0-git-276cbba4-office-excel-v1";

pub(crate) use adapter::extract;
pub(crate) use detection::{detected_document_format, probe, supports};

#[cfg(test)]
pub(crate) fn extract_for_test(
    paths: &crate::paths::AppPaths,
    document_id: &str,
    source_path: &std::path::Path,
    format: crate::document_domain::DocumentFormat,
    maximum_source_units: usize,
    mut report_units: impl FnMut(usize, usize) -> Result<(), crate::error::CommandError>,
) -> Result<crate::document_canonical::CanonicalIngestion, crate::error::CommandError> {
    struct NeverCanceled;
    impl crate::document_ingestion::contracts::DocumentCancellation for NeverCanceled {
        fn check(&self) -> Result<(), crate::error::CommandError> {
            Ok(())
        }
    }
    let mut progress = |value| match value {
        crate::document_ingestion::contracts::IngestionProgress::Extracting {
            completed_units,
            total_units,
        } => report_units(completed_units, total_units),
        _ => Ok(()),
    };
    adapter::extract(
        paths,
        document_id,
        source_path,
        format,
        maximum_source_units,
        &NeverCanceled,
        &mut progress,
    )
}
