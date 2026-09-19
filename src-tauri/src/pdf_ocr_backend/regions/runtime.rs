//! One platform invocation supplies both the legacy text and the image regions.

use super::RegionOcrResult;
use crate::pdf_ocr_backend::{
    canceled_ocr_error, OcrOptions, OcrProvider, OcrResult, PlatformOcrProvider,
};

impl PlatformOcrProvider {
    pub(crate) fn process_regions(
        &self,
        image_data: &[u8],
        options: &OcrOptions,
    ) -> OcrResult<RegionOcrResult> {
        self.validate_image_data(image_data)?;
        if self.is_canceled() {
            return Err(canceled_ocr_error());
        }
        let result = self.recognize_regions(image_data, options)?;
        result.validate()?;
        Ok(result)
    }

    fn recognize_regions(
        &self,
        image_data: &[u8],
        options: &OcrOptions,
    ) -> OcrResult<RegionOcrResult> {
        #[cfg(target_os = "android")]
        {
            self.ensure_available()?;
            self.process_android_ocr(image_data, options)
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            self.process_desktop_tesseract(image_data, options)
        }
        #[cfg(target_os = "ios")]
        {
            let _ = (image_data, options);
            self.ensure_available()?;
            Err(oxidize_pdf::OcrError::ProviderNotAvailable(
                "Image OCR is unavailable on this platform.".to_string(),
            ))
        }
    }
}
