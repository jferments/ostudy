use std::time::Duration;

use xberg::{
    cancellation::CancellationToken, engine::Engine, ExtractInput, ExtractedDocument,
    ExtractionConfig,
};

use crate::{document_ingestion::contracts::DocumentCancellation, error::CommandError};

use super::errors::map_xberg_error;

const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub(crate) fn extract_one(
    bytes: Vec<u8>,
    media_type: String,
    filename: Option<String>,
    mut config: ExtractionConfig,
    cancellation: &dyn DocumentCancellation,
) -> Result<ExtractedDocument, CommandError> {
    cancellation.check()?;
    let caller_token = CancellationToken::new();
    config.cancel_token = Some(caller_token.clone());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| {
            CommandError::new(
                "document_backend_runtime_failed",
                format!("The Xberg extraction runtime could not start: {error}"),
            )
        })?;
    let engine = Engine::new_default();
    let input = ExtractInput::from_bytes(bytes, media_type, filename);

    let (result, cancellation_error) = runtime.block_on(async {
        let extraction = engine.extract(input, &config);
        tokio::pin!(extraction);
        let mut cancellation_error = None;
        let result = loop {
            tokio::select! {
                result = &mut extraction => break result,
                _ = tokio::time::sleep(CANCELLATION_POLL_INTERVAL) => {
                    if cancellation_error.is_none() {
                        if let Err(error) = cancellation.check() {
                            caller_token.cancel();
                            cancellation_error = Some(error);
                        }
                    }
                }
            }
        };
        (result, cancellation_error)
    });

    if let Some(error) = cancellation_error {
        return Err(error);
    }
    let output = result.map_err(map_xberg_error)?;
    if let Some(error) = output.errors.first() {
        return Err(CommandError::new(
            "document_parse_failed",
            format!(
                "Xberg could not extract the managed document safely: {}",
                error.error_type
            ),
        ));
    }
    if output.results.len() != 1 {
        return Err(CommandError::new(
            "document_backend_result_invalid",
            "Xberg returned an invalid number of extraction results for one managed source.",
        ));
    }
    cancellation.check()?;
    output.results.into_iter().next().ok_or_else(|| {
        CommandError::new(
            "document_backend_result_invalid",
            "Xberg did not return an extraction result for the managed source.",
        )
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    struct TestCancellation(AtomicBool);

    impl DocumentCancellation for TestCancellation {
        fn check(&self) -> Result<(), CommandError> {
            if self.0.load(Ordering::SeqCst) {
                Err(CommandError::new(
                    "ai_request_canceled",
                    "The document generation request was canceled.",
                ))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn pre_cancelled_requests_stop_before_xberg_dispatch() {
        let cancellation = TestCancellation(AtomicBool::new(true));
        let error = extract_one(
            b"never parsed".to_vec(),
            "text/plain".to_string(),
            Some("fixture.txt".to_string()),
            ExtractionConfig::default(),
            &cancellation,
        )
        .expect_err("pre-cancelled extraction must stop");
        assert_eq!(error.code, "ai_request_canceled");
    }
}
