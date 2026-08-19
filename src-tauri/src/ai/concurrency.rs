//! Bounded concurrency policy for independent document-provider windows.
//!
//! Concurrency follows the execution topology, not a list of provider brands.
//! Cloud requests can overlap because every request owns a disjoint bounded
//! source window and durable commits remain source-ordered. Local and LAN model
//! servers remain serial so ostudy does not assume that one user-managed
//! inference process was started with multiple decoding slots.

pub(crate) const CLOUD_DOCUMENT_PARALLELISM: usize = 4;

pub(crate) fn document_request_parallelism(network_scope: &str) -> usize {
    match network_scope {
        "cloud" => CLOUD_DOCUMENT_PARALLELISM,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_windows_overlap_while_user_managed_servers_stay_serial() {
        assert_eq!(document_request_parallelism("cloud"), 4);
        assert_eq!(document_request_parallelism("local"), 1);
        assert_eq!(document_request_parallelism("lan"), 1);
    }
}
