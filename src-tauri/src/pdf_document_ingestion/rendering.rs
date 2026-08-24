use std::{fs, path::Path};

use crate::{
    error::CommandError,
    paths::AppPaths,
    pdf_render_backend::{self, PdfRenderRuntime, RenderProcessError},
};

use super::{
    contracts::IngestionProgress,
    errors::map_pdf_render_error,
    files::{sha256_hex, sync_directory, write_replacing_file},
    validation::ensure_document_not_canceled,
};

const MAX_RENDER_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct RenderedPage {
    pub(crate) page_number: usize,
    pub(crate) relative_path: String,
    pub(crate) byte_size: i64,
    pub(crate) sha256: String,
}

pub(crate) fn render_standard_pages<F>(
    paths: &AppPaths,
    render_runtime: &PdfRenderRuntime,
    document_id: &str,
    source_path: &Path,
    page_count: usize,
    report_progress: &mut F,
) -> Result<Vec<RenderedPage>, CommandError>
where
    F: FnMut(IngestionProgress) -> Result<(), CommandError>,
{
    report_progress(IngestionProgress::Rendering {
        completed_units: 0,
        total_units: page_count,
    })?;
    let page_directory = paths.document_pages_dir(document_id);
    fs::create_dir_all(&page_directory).map_err(CommandError::io)?;
    let mut rendered = Vec::new();
    let render_result = pdf_render_backend::render_pages(
        render_runtime,
        source_path,
        |page_number, png| {
            ensure_document_not_canceled(document_id)?;
            if png.is_empty() || png.len() > MAX_RENDER_BYTES {
                return Err(CommandError::new(
                    "document_render_too_large",
                    format!(
                        "The rendered image for source page {page_number} exceeds ostudy's image limit."
                    ),
                ));
            }
            let relative = AppPaths::document_page_relative_path(
                document_id,
                i64::try_from(page_number).unwrap_or(i64::MAX),
            );
            write_replacing_file(&paths.resolve_relative(&relative), &png)?;
            rendered.push(RenderedPage {
                page_number,
                relative_path: relative,
                byte_size: i64::try_from(png.len()).unwrap_or(i64::MAX),
                sha256: sha256_hex(&png),
            });
            report_progress(IngestionProgress::Rendering {
                completed_units: rendered.len(),
                total_units: page_count,
            })?;
            Ok(())
        },
    );
    let rendered_count = match render_result {
        Ok(count) => count,
        Err(RenderProcessError::Visitor(error)) => return Err(error),
        Err(RenderProcessError::Backend(error)) => return Err(map_pdf_render_error(error)),
    };
    if rendered_count != page_count {
        return Err(CommandError::new(
            "document_backend_unit_count_mismatch",
            "The extraction and rendering adapters reported different source-unit counts.",
        ));
    }
    sync_directory(&page_directory)?;
    Ok(rendered)
}
