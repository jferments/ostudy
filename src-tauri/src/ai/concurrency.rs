//! Bounded concurrency policy for independent document-provider windows.
//!
//! Remote API windows can execute concurrently because each request owns a
//! disjoint bounded source window and final ordering/checkpoint commits remain
//! deterministic. Local llama.cpp remains serial so multiple requests do not
//! contend for the same local inference engine.

pub(crate) const OPENROUTER_DOCUMENT_PARALLELISM: usize = 4;
pub(crate) const DEEPSEEK_DOCUMENT_PARALLELISM: usize = 2;

pub(crate) fn document_request_parallelism(provider_kind: &str) -> usize {
    match provider_kind {
        "openrouter" => OPENROUTER_DOCUMENT_PARALLELISM,
        "deepseek" => DEEPSEEK_DOCUMENT_PARALLELISM,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_windows_are_bounded_and_local_inference_stays_serial() {
        assert_eq!(document_request_parallelism("openrouter"), 4);
        assert_eq!(document_request_parallelism("deepseek"), 2);
        assert_eq!(document_request_parallelism("llama_cpp"), 1);
    }
}
