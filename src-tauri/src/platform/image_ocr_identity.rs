//! Platform OCR compatibility includes the layout implementation and model,
//! not a mutable screenshot position or user-visible crop.

use crate::{
    document_canonical::sha256_hex,
    pdf_ocr_backend::{canceled_ocr_error, OcrOptions, OcrResult, PlatformOcrProvider},
};

impl PlatformOcrProvider {
    pub(crate) fn extraction_contract(&self, options: &OcrOptions) -> OcrResult<Option<String>> {
        if self.is_canceled() {
            return Err(canceled_ocr_error());
        }
        let native = [
            sha256_hex(include_bytes!("../pdf_ocr_backend.rs")),
            sha256_hex(include_bytes!("../pdf_ocr_backend/regions.rs")),
            sha256_hex(include_bytes!("../pdf_ocr_backend/regions/parsers.rs")),
            sha256_hex(include_bytes!("image_ocr_identity.rs")),
            options.language.clone(),
        ]
        .join(":");
        #[cfg(target_os = "android")]
        {
            // Both packaged model/dependency and native recognition changes
            // invalidate reuse. Recognition never downloads a model.
            let gradle = include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../plugins/ostudy-platform/android/build.gradle.kts"
            ));
            let kotlin = include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../plugins/ostudy-platform/android/src/main/java/OstudyDocumentPlugin.kt"
            ));
            Ok(Some(format!(
                "image-ocr-v1:{}:{}:{}",
                sha256_hex(native.as_bytes()),
                sha256_hex(gradle),
                sha256_hex(kotlin)
            )))
        }
        #[cfg(target_os = "ios")]
        {
            let _ = native;
            Ok(None)
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            // Unverifiable custom runtimes remain usable without persistent
            // reuse; they are never reported as compatible cache hits.
            let fingerprint = desktop::fingerprint(self, options);
            if self.is_canceled() {
                return Err(canceled_ocr_error());
            }
            Ok(fingerprint
                .ok()
                .map(|value| format!("image-ocr-v1:{}:{value}", sha256_hex(native.as_bytes()))))
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod desktop {
    use std::{
        fs::{self, File},
        io::Read,
        path::Path,
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };

    use super::*;
    use crate::pdf_ocr_backend::{tesseract_language, OcrError, TempOcrWorkspace};

    pub(super) fn fingerprint(
        provider: &PlatformOcrProvider,
        options: &OcrOptions,
    ) -> OcrResult<String> {
        let workspace = TempOcrWorkspace::create()?;
        let deadline =
            Instant::now() + Duration::from_secs(u64::from(options.timeout_seconds.max(1)));
        let version = metadata(provider, "--version", &workspace.path, deadline)?;
        let languages = metadata(provider, "--list-langs", &workspace.path, deadline)?;
        let root = model_directory(&languages).ok_or_else(unavailable)?;
        let language = tesseract_language(&options.language);
        let mut hashes = vec![sha256_hex(version.as_bytes())];
        for code in language.split('+') {
            if code.is_empty()
                || !code
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            {
                return Err(unavailable());
            }
            let mut bytes = Vec::new();
            File::open(Path::new(root).join(format!("{code}.traineddata")))
                .map_err(|_| unavailable())?
                .take(64 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| unavailable())?;
            if bytes.is_empty()
                || bytes.len() > 64 * 1024 * 1024
                || provider.is_canceled()
                || Instant::now() >= deadline
            {
                return Err(unavailable());
            }
            hashes.push(sha256_hex(&bytes));
        }
        Ok(sha256_hex(hashes.join(":").as_bytes()))
    }

    fn metadata(
        provider: &PlatformOcrProvider,
        argument: &str,
        directory: &Path,
        deadline: Instant,
    ) -> OcrResult<String> {
        let path = directory.join("identity.txt");
        let output = File::create(&path).map_err(|_| unavailable())?;
        let mut child = Command::new(&provider.executable)
            .arg(argument)
            .stdout(Stdio::from(output.try_clone().map_err(|_| unavailable())?))
            .stderr(Stdio::from(output))
            .spawn()
            .map_err(|_| unavailable())?;
        loop {
            if provider.is_canceled()
                || Instant::now() >= deadline
                || fs::metadata(&path).is_ok_and(|value| value.len() > 32 * 1024)
            {
                let _ = child.kill();
                let _ = child.wait();
                return Err(unavailable());
            }
            match child.try_wait() {
                Ok(Some(status)) if status.success() => break,
                Ok(Some(_)) => return Err(unavailable()),
                Ok(None) => thread::sleep(Duration::from_millis(25)),
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(unavailable());
                }
            }
        }
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(|_| unavailable())?
            .take(32 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| unavailable())?;
        if bytes.len() > 32 * 1024 {
            return Err(unavailable());
        }
        String::from_utf8(bytes).map_err(|_| unavailable())
    }

    fn model_directory(output: &str) -> Option<&str> {
        output.lines().find_map(|line| {
            let path = line.strip_prefix("List of available languages in \"")?;
            let (path, _) = path.split_once('"')?;
            (!path.is_empty()).then_some(path)
        })
    }

    fn unavailable() -> OcrError {
        OcrError::Configuration("The local OCR runtime identity could not be verified.".to_string())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn model_identity_uses_only_the_runtime_reported_directory() {
            assert_eq!(
                model_directory("List of available languages in \"/models/tessdata/\" (1):\neng\n"),
                Some("/models/tessdata/")
            );
            assert_eq!(
                model_directory("List of available languages (1):\neng\n"),
                None
            );
            assert_eq!(model_directory("/guessed/tessdata\neng"), None);
            assert_eq!(
                model_directory("List of available languages in \"\" (0):"),
                None
            );
        }
    }
}
