//! The certificate authorities Android trusts, for the HTTPS clients.
//!
//! Desktop builds verify servers through `rustls-platform-verifier`. On
//! Android that verifier calls a Kotlin component over JNI, which the app
//! neither packages nor initializes, so every TLS handshake failed there and
//! the AI gateway reported that it could not connect. Android keeps its
//! trusted authorities as PEM files, so the Android build reads them and
//! verifies against exactly that set.

use std::path::Path;

use reqwest::Certificate;

/// The updatable Conscrypt store (Android 14 and later) wins over the copy in
/// the system image, which older releases use.
pub(super) const ANDROID_CA_DIRECTORIES: [&str; 2] = [
    "/apex/com.android.conscrypt/cacerts",
    "/system/etc/security/cacerts",
];

/// The certificates in the first directory that holds any. Each file is one
/// PEM certificate followed by its text dump; unreadable files are skipped.
pub(super) fn system_roots(directories: &[impl AsRef<Path>]) -> Vec<Certificate> {
    for directory in directories {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        let certificates: Vec<Certificate> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| std::fs::read(entry.path()).ok())
            .filter_map(|bytes| Certificate::from_pem(&bytes).ok())
            .collect();
        if !certificates.is_empty() {
            return certificates;
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_ROOT: &str = "-----BEGIN CERTIFICATE-----
MIIBjDCCATOgAwIBAgIUdQJxFSVTNT3gmhS2eHVbULlFxrIwCgYIKoZIzj0EAwIw
GzEZMBcGA1UEAwwQb1N0dWR5IHRlc3Qgcm9vdDAgFw0yNjExMDIwMDI0MjFaGA8y
MTI2MTAwOTAwMjQyMVowGzEZMBcGA1UEAwwQb1N0dWR5IHRlc3Qgcm9vdDBZMBMG
ByqGSM49AgEGCCqGSM49AwEHA0IABGk177o8TonTu58REnVDmc+3yVSa4r+jMvS9
wjXl/jrJiyJIjohfu127V45CJRC/uYMGWB6hPMaFF5/fml90Eb2jUzBRMB0GA1Ud
DgQWBBQlN1T+vaNmxWD8QGaqagsjuHjkVzAfBgNVHSMEGDAWgBQlN1T+vaNmxWD8
QGaqagsjuHjkVzAPBgNVHRMBAf8EBTADAQH/MAoGCCqGSM49BAMCA0cAMEQCIDH7
RDqaoWUJ8JqMDvIsWodJyfcve+2xHhsvUzlkAdosAiASpz+9L6d8KVEt/BEVjd7H
wJD45/S4gBc2etf7VpvgMA==
-----END CERTIFICATE-----
";

    fn scratch(name: &str) -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("ostudy-https-client-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn android_ca_files_with_a_text_dump_after_the_pem_are_read() {
        let root = scratch("read");
        let store = root.join("cacerts");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(
            store.join("1a2b3c4d.0"),
            format!("{TEST_ROOT}Certificate:\n    Data:\n        Version: 3 (0x2)\n"),
        )
        .unwrap();
        std::fs::write(store.join("broken.0"), "not a certificate").unwrap();

        let missing = root.join("missing");
        assert_eq!(system_roots(&[&missing, &store]).len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_first_directory_with_certificates_wins() {
        let root = scratch("order");
        let (empty, apex, system) = (root.join("empty"), root.join("apex"), root.join("system"));
        for directory in [&empty, &apex, &system] {
            std::fs::create_dir_all(directory).unwrap();
        }
        std::fs::write(apex.join("a.0"), TEST_ROOT).unwrap();
        std::fs::write(system.join("a.0"), TEST_ROOT).unwrap();
        std::fs::write(system.join("b.0"), TEST_ROOT).unwrap();

        assert_eq!(system_roots(&[&empty, &apex, &system]).len(), 1);
        assert!(system_roots(&[root.join("missing")]).is_empty());
        assert_eq!(
            ANDROID_CA_DIRECTORIES[0],
            "/apex/com.android.conscrypt/cacerts"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_client_builds_from_the_shared_builder() {
        super::super::client::https_client_builder()
            .build()
            .unwrap();
    }
}
