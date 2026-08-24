use std::io::{Cursor, Read};

use roxmltree::Document;
use zip::ZipArchive;

use crate::{document_domain::DocumentFormat, error::CommandError};

use super::support::bounded_text;

const ODF_MANIFEST_PATH: &str = "META-INF/manifest.xml";
const MAX_ODF_MANIFEST_BYTES: u64 = 2 * 1024 * 1024;

pub(crate) fn reject_encrypted_package(
    bytes: &[u8],
    format: DocumentFormat,
) -> Result<(), CommandError> {
    if !matches!(
        format,
        DocumentFormat::Odt | DocumentFormat::Odp | DocumentFormat::Ods
    ) {
        return Ok(());
    }

    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|error| {
        malformed(&format!(
            "The OpenDocument package archive is malformed: {}",
            bounded_text(&error.to_string(), 300)
        ))
    })?;
    let mut manifest = match archive.by_name(ODF_MANIFEST_PATH) {
        Ok(manifest) => manifest,
        Err(zip::result::ZipError::FileNotFound) => return Ok(()),
        Err(error) => {
            return Err(malformed(&format!(
                "The OpenDocument package manifest could not be read: {}",
                bounded_text(&error.to_string(), 300)
            )))
        }
    };

    if manifest.size() > MAX_ODF_MANIFEST_BYTES {
        return Err(CommandError::new(
            "document_resource_limit",
            "The OpenDocument package manifest exceeds oStudy's bounded metadata limit.",
        ));
    }

    let mut bytes = Vec::with_capacity(usize::try_from(manifest.size()).unwrap_or(0));
    manifest
        .by_ref()
        .take(MAX_ODF_MANIFEST_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(CommandError::io)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_ODF_MANIFEST_BYTES {
        return Err(CommandError::new(
            "document_resource_limit",
            "The OpenDocument package manifest exceeds oStudy's bounded metadata limit.",
        ));
    }

    let xml = std::str::from_utf8(&bytes)
        .map_err(|_| malformed("The OpenDocument package manifest is not valid UTF-8 XML."))?;
    let document = Document::parse(xml).map_err(|error| {
        malformed(&format!(
            "The OpenDocument package manifest is malformed: {}",
            bounded_text(&error.to_string(), 300)
        ))
    })?;
    if document.descendants().any(|node| {
        node.is_element() && matches!(node.tag_name().name(), "encryption-data" | "encrypted-key")
    }) {
        return Err(CommandError::new(
            "document_encrypted",
            "Encrypted or password-protected documents cannot be processed.",
        ));
    }

    Ok(())
}

fn malformed(message: &str) -> CommandError {
    CommandError::new("document_malformed", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_odf_manifest_is_rejected_before_parser_dispatch() {
        let encrypted = include_bytes!("../../tests/fixtures/anydoc-v0.2.3/encrypted--errors.odt");
        assert_eq!(
            reject_encrypted_package(encrypted, DocumentFormat::Odt)
                .unwrap_err()
                .code,
            "document_encrypted"
        );
    }

    #[test]
    fn ordinary_odf_manifest_is_accepted() {
        let ordinary = include_bytes!("../../tests/fixtures/anydoc-v0.2.3/handmade-lists.odt");
        reject_encrypted_package(ordinary, DocumentFormat::Odt).unwrap();
    }
}
