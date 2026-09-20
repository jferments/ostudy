//! Ordered image sources enter the same canonical document pipeline as other
//! formats. Original ownership and reusable OCR remain below this adapter.

use std::{fs::File, io::Read, path::Path};

use oxidize_pdf::OcrOptions;

use crate::{
    database::Database,
    document_domain::DocumentFormat,
    documents::image_sets::{
        cache,
        manifest::{ImageManifest, MAX_MANIFEST_BYTES},
        raster, storage,
    },
    error::CommandError,
    pdf_ocr_backend::{is_cancellation_error, regions::RegionOcrResult, PlatformOcrProvider},
};

use super::contracts::{
    AdapterExtraction, AdapterRequest, DocumentAdapter, DocumentAdapterIdentity, IngestionProgress,
    IngestionProgressSink, IngestionStage,
};

mod overlap;
mod passages;
mod projection;
mod regions;

pub(crate) const IDENTITY: DocumentAdapterIdentity = DocumentAdapterIdentity {
    adapter_backend: "image-set-canonical-v3-filter",
    extraction_backend: "image-set-region-ocr-v1",
    render_backend: Some("image-set-inclusion-render-v3-filter"),
};

pub(crate) struct ImageSetAdapter;

impl ImageSetAdapter {
    pub(crate) fn extract_cached(
        &self,
        database: &Database,
        request: AdapterRequest<'_>,
        progress: &mut dyn IngestionProgressSink,
    ) -> Result<AdapterExtraction, CommandError> {
        extract_native(Some(database), request, progress)
    }
}

impl DocumentAdapter for ImageSetAdapter {
    fn supports(&self, format: DocumentFormat) -> bool {
        format == DocumentFormat::ImageBundle
    }

    fn identity(&self, format: DocumentFormat) -> Result<DocumentAdapterIdentity, CommandError> {
        if !self.supports(format) {
            return Err(invalid(
                "This adapter accepts native ordered image sets only.",
            ));
        }
        Ok(IDENTITY)
    }

    fn probe(&self, format: DocumentFormat, source_path: &Path) -> Result<bool, CommandError> {
        if !self.supports(format) {
            return Ok(false);
        }
        read_manifest(source_path).map(|_| true)
    }

    fn extract(
        &self,
        request: AdapterRequest<'_>,
        progress: &mut dyn IngestionProgressSink,
    ) -> Result<AdapterExtraction, CommandError> {
        extract_native(None, request, progress)
    }
}

fn read_manifest(path: &Path) -> Result<(ImageManifest, Vec<u8>), CommandError> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(CommandError::io)?
        .take(MAX_MANIFEST_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(CommandError::io)?;
    Ok((ImageManifest::decode(&bytes)?, bytes))
}

fn extract_native(
    database: Option<&Database>,
    request: AdapterRequest<'_>,
    progress: &mut dyn IngestionProgressSink,
) -> Result<AdapterExtraction, CommandError> {
    let provider = PlatformOcrProvider::for_document(request.document_id);
    let options = OcrOptions::default();
    let contract = provider.extraction_contract(&options).map_err(ocr_error)?;
    extract_with(database, request, progress, contract.as_deref(), |png| {
        provider.process_regions(png, &options).map_err(ocr_error)
    })
}

/// Native test seam: no providers, user paths, or database lock cross the OCR
/// callback. Cached siblings survive a later extraction or publication failure.
fn extract_with(
    database: Option<&Database>,
    request: AdapterRequest<'_>,
    progress: &mut dyn IngestionProgressSink,
    contract: Option<&str>,
    mut recognize: impl FnMut(&[u8]) -> Result<RegionOcrResult, CommandError>,
) -> Result<AdapterExtraction, CommandError> {
    request.cancellation.check()?;
    progress.report(IngestionProgress::Stage(IngestionStage::DetectingFormat))?;
    let (manifest, bytes) = read_manifest(request.source_path)?;
    if request.format != DocumentFormat::ImageBundle
        || manifest.document_id != request.document_id
        || manifest.members.len() > request.maximum_source_units
    {
        return Err(invalid("The image-set identity or source count changed."));
    }
    let settings = database
        .map(|database| {
            database.with_connection(|connection| storage::settings(connection, request.document_id))
        })
        .transpose()?
        .unwrap_or_default();
    let mut output = projection::Projection::new(&manifest, &bytes, settings)?;
    for (index, member) in manifest.members.iter().enumerate() {
        request.cancellation.check()?;
        let image = raster::Raster::read(request.paths, member)?;
        let inclusion = raster::inclusion(member)?;
        let extraction = cache::get_or_extract(database, member, contract, || {
            request.cancellation.check()?;
            let result = recognize(&image.png()?)?;
            request.cancellation.check()?;
            Ok(result)
        });
        let (result, reused) = match extraction {
            Ok(extraction) => (Some(extraction.result), extraction.reused),
            Err(error) if error.code == "image_ocr_unavailable" => (None, false),
            Err(error) => return Err(error),
        };
        request.cancellation.check()?;
        output.add(
            request.paths,
            member,
            index,
            &image,
            inclusion,
            result.as_ref(),
            reused,
        )?;
        progress.report(IngestionProgress::Ocr {
            completed_units: index + 1,
            total_units: manifest.members.len(),
        })?;
    }
    request.cancellation.check()?;
    progress.report(IngestionProgress::Stage(IngestionStage::ValidatingResult))?;
    output.finish()
}

fn ocr_error(error: oxidize_pdf::OcrError) -> CommandError {
    if is_cancellation_error(&error) {
        CommandError::new("ai_request_canceled", "Image extraction was canceled.")
    } else {
        CommandError::new(
            "image_ocr_unavailable",
            "Local OCR could not read this image. Its original remains available for inspection.",
        )
    }
}

fn invalid(message: &str) -> CommandError {
    CommandError::new("invalid_image_set", message)
}

#[cfg(test)]
mod filter_tests;
#[cfg(test)]
mod overlap_tests;
#[cfg(test)]
mod tests;
