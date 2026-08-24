use xberg::{
    ExtractionConfig, ImageExtractionConfig, OutputFormat, PageConfig, ResultFormat, SecurityLimits,
};

pub(crate) fn extraction_config() -> ExtractionConfig {
    ExtractionConfig {
        use_cache: false,
        enable_quality_processing: false,
        disable_ocr: true,
        force_ocr: false,
        chunking: None,
        token_reduction: None,
        language_detection: None,
        images: Some(ImageExtractionConfig {
            extract_images: true,
            inject_placeholders: false,
            run_ocr_on_images: false,
            include_page_rasters: false,
            include_data_base64: false,
            ..ImageExtractionConfig::default()
        }),
        pages: Some(PageConfig {
            extract_pages: true,
            insert_page_markers: false,
            ..PageConfig::default()
        }),
        result_format: ResultFormat::ElementBased,
        output_format: OutputFormat::Plain,
        include_document_structure: true,
        security_limits: Some(SecurityLimits::default()),
        extraction_timeout_secs: Some(600),
        ..ExtractionConfig::default()
    }
}
