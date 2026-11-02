//! The certificate authorities Android trusts, as PEM files.
//!
//! Desktop HTTPS clients verify servers through `rustls-platform-verifier`.
//! Its Android half is a Kotlin component reached over JNI, which the app
//! neither packages nor initializes, so every TLS handshake failed on a phone
//! and OpenRouter setup reported that it could not connect. Android keeps its
//! trusted authorities as one PEM file each, so the Android build reads them
//! and the AI gateway trusts exactly that set.

use std::path::Path;

/// The updatable Conscrypt store (Android 14 and later) wins over the copy in
/// the system image, which older releases use.
pub(super) const ANDROID_CA_DIRECTORIES: [&str; 2] = [
    "/apex/com.android.conscrypt/cacerts",
    "/system/etc/security/cacerts",
];

const CERTIFICATE_MARKER: &[u8] = b"-----BEGIN CERTIFICATE-----";

/// The PEM certificate files in the first directory that holds any. Android
/// writes a text dump after each certificate, which PEM parsing skips; files
/// without a certificate are left out.
pub(super) fn read_ca_directories(directories: &[impl AsRef<Path>]) -> Vec<Vec<u8>> {
    for directory in directories {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        let certificates: Vec<Vec<u8>> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| std::fs::read(entry.path()).ok())
            .filter(|bytes| {
                bytes
                    .windows(CERTIFICATE_MARKER.len())
                    .any(|window| window == CERTIFICATE_MARKER)
            })
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
            std::env::temp_dir().join(format!("ostudy-system-ca-{name}-{}", std::process::id()));
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
        assert_eq!(read_ca_directories(&[&missing, &store]).len(), 1);
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

        assert_eq!(read_ca_directories(&[&empty, &apex, &system]).len(), 1);
        assert!(read_ca_directories(&[root.join("missing")]).is_empty());
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
