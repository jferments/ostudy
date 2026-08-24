use std::{
    collections::HashSet,
    io::{Cursor, Read},
    path::{Component, Path},
};

use roxmltree::Document;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::{document_domain::DocumentFormat, error::CommandError};

use super::support::{bounded_text, media_type_for_format};

const ROOT_RELATIONSHIPS_PATH: &str = "_rels/.rels";
const MAX_ARCHIVE_ENTRIES: usize = 10_000;
const MAX_ARCHIVE_UNCOMPRESSED_BYTES: u128 = 500 * 1024 * 1024;
const MAX_ARCHIVE_ENTRY_BYTES: u64 = 128 * 1024 * 1024;
const MAX_COMPATIBILITY_REWRITE_BYTES: usize = 512 * 1024 * 1024;
const MAX_COMPRESSION_RATIO: u64 = 100;
const MIN_RATIO_CHECK_BYTES: u64 = 1024 * 1024;
const MAX_ROOT_RELATIONSHIPS_BYTES: u64 = 1024 * 1024;
const MAX_MAIN_PART_BYTES: u64 = 16 * 1024 * 1024;

const WORDPROCESSINGML_NAMESPACES: &[&str] = &[
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];
const PRESENTATIONML_NAMESPACES: &[&str] = &[
    "http://schemas.openxmlformats.org/presentationml/2006/main",
    "http://purl.oclc.org/ooxml/presentationml/main",
];
const SPREADSHEETML_NAMESPACES: &[&str] = &[
    "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
    "http://purl.oclc.org/ooxml/spreadsheetml/main",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OoxmlFamily {
    Word,
    Presentation,
    Spreadsheet,
}

impl OoxmlFamily {
    const fn format(self) -> DocumentFormat {
        match self {
            Self::Word => DocumentFormat::Docx,
            Self::Presentation => DocumentFormat::Pptx,
            Self::Spreadsheet => DocumentFormat::Excel,
        }
    }

    const fn standard_root(self) -> &'static str {
        match self {
            Self::Word => "word",
            Self::Presentation => "ppt",
            Self::Spreadsheet => "xl",
        }
    }

    const fn standard_main_part(self) -> &'static str {
        match self {
            Self::Word => "word/document.xml",
            Self::Presentation => "ppt/presentation.xml",
            Self::Spreadsheet => "xl/workbook.xml",
        }
    }

    const fn accepted_namespaces(self) -> &'static [&'static str] {
        match self {
            Self::Word => WORDPROCESSINGML_NAMESPACES,
            Self::Presentation => PRESENTATIONML_NAMESPACES,
            Self::Spreadsheet => SPREADSHEETML_NAMESPACES,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OoxmlNormalizationPlan {
    family: OoxmlFamily,
    source_root: String,
    source_main_part: String,
    source_main_relationships: String,
}

impl OoxmlNormalizationPlan {
    pub(crate) const fn format(&self) -> DocumentFormat {
        self.family.format()
    }

    pub(crate) fn media_type(&self) -> &'static str {
        media_type_for_format(self.format())
    }

    fn map_entry_name(&self, name: &str) -> String {
        if name == self.source_main_part {
            return self.family.standard_main_part().to_string();
        }
        if name == self.source_main_relationships {
            return relationship_part_for(self.family.standard_main_part());
        }

        let source_prefix = format!("{}/", self.source_root);
        if let Some(suffix) = name.strip_prefix(&source_prefix) {
            return format!("{}/{}", self.family.standard_root(), suffix);
        }
        name.to_string()
    }
}

pub(crate) fn inspect_relocated_ooxml(
    bytes: &[u8],
) -> Result<Option<OoxmlNormalizationPlan>, CommandError> {
    if !looks_like_zip(bytes) {
        return Ok(None);
    }

    let mut archive = open_archive(bytes)?;
    if archive.is_empty() || archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(resource_limit(
            "The candidate OOXML package contains an unsupported number of archive entries.",
        ));
    }
    let Some(root_relationships) = read_optional_member(
        &mut archive,
        ROOT_RELATIONSHIPS_PATH,
        MAX_ROOT_RELATIONSHIPS_BYTES,
    )?
    else {
        return Ok(None);
    };
    let Some(main_part) = office_document_target(&root_relationships)? else {
        return Ok(None);
    };
    validate_archive(&mut archive)?;
    let main_xml = read_required_member(&mut archive, &main_part, MAX_MAIN_PART_BYTES)?;
    let family = family_from_main_part(&main_xml)?;

    if main_part == family.standard_main_part() {
        return Ok(None);
    }

    let source_root = part_parent(&main_part);
    if source_root.is_empty() || main_part.split('/').count() != 2 {
        return Err(CommandError::new(
            "unsupported_document_structure",
            "The OOXML package uses a nested or root-level main part that cannot be normalized safely for the current document backend.",
        ));
    }

    let plan = OoxmlNormalizationPlan {
        family,
        source_root,
        source_main_relationships: relationship_part_for(&main_part),
        source_main_part: main_part,
    };
    validate_mapping(&mut archive, &plan)?;
    Ok(Some(plan))
}

pub(crate) fn normalize_for_xberg(
    bytes: &[u8],
    plan: &OoxmlNormalizationPlan,
) -> Result<Vec<u8>, CommandError> {
    let mut archive = open_archive(bytes)?;
    validate_archive(&mut archive)?;
    validate_mapping(&mut archive, plan)?;

    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(map_zip_error)?;
        if file.is_dir() {
            continue;
        }
        let source_name = safe_entry_name(&file)?;
        let target_name = plan.map_entry_name(&source_name);
        writer
            .raw_copy_file_rename(file, target_name)
            .map_err(map_zip_error)?;
    }
    let output = writer.finish().map_err(map_zip_error)?.into_inner();
    if output.is_empty() || output.len() > MAX_COMPATIBILITY_REWRITE_BYTES {
        return Err(resource_limit(
            "The normalized OOXML package exceeds oStudy's compatibility-processing limit.",
        ));
    }
    Ok(output)
}

fn open_archive(bytes: &[u8]) -> Result<ZipArchive<Cursor<&[u8]>>, CommandError> {
    ZipArchive::new(Cursor::new(bytes)).map_err(map_zip_error)
}

fn validate_archive(archive: &mut ZipArchive<Cursor<&[u8]>>) -> Result<(), CommandError> {
    if archive.is_empty() || archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(resource_limit(
            "The OOXML package contains an unsupported number of archive entries.",
        ));
    }
    if archive
        .decompressed_size()
        .is_some_and(|size| size > MAX_ARCHIVE_UNCOMPRESSED_BYTES)
    {
        return Err(resource_limit(
            "The OOXML package exceeds oStudy's 500 MB expanded archive limit.",
        ));
    }

    let mut names = HashSet::new();
    let mut total_uncompressed = 0_u128;
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(map_zip_error)?;
        let name = safe_entry_name(&file)?;
        if !names.insert(name) {
            return Err(malformed(
                "The OOXML package contains duplicate normalized archive paths.",
            ));
        }
        if file.encrypted() {
            return Err(CommandError::new(
                "document_encrypted",
                "Encrypted or password-protected documents cannot be processed.",
            ));
        }
        if file.is_symlink() {
            return Err(malformed(
                "The OOXML package contains a symbolic-link archive entry.",
            ));
        }
        if file.is_dir() {
            continue;
        }
        if !matches!(
            file.compression(),
            CompressionMethod::Stored | CompressionMethod::Deflated
        ) {
            return Err(CommandError::new(
                "unsupported_document_compression",
                "The OOXML package uses an unsupported archive compression method.",
            ));
        }

        let uncompressed = file.size();
        let compressed = file.compressed_size();
        if uncompressed > MAX_ARCHIVE_ENTRY_BYTES {
            return Err(resource_limit(
                "An OOXML package entry exceeds oStudy's 128 MB expanded-entry limit.",
            ));
        }
        if uncompressed >= MIN_RATIO_CHECK_BYTES
            && (compressed == 0 || uncompressed > compressed.saturating_mul(MAX_COMPRESSION_RATIO))
        {
            return Err(resource_limit(
                "The OOXML package exceeds oStudy's 100× compression-ratio limit.",
            ));
        }
        total_uncompressed = total_uncompressed.saturating_add(u128::from(uncompressed));
        if total_uncompressed > MAX_ARCHIVE_UNCOMPRESSED_BYTES {
            return Err(resource_limit(
                "The OOXML package exceeds oStudy's 500 MB expanded archive limit.",
            ));
        }
    }
    Ok(())
}

fn validate_mapping(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    plan: &OoxmlNormalizationPlan,
) -> Result<(), CommandError> {
    let source_prefix = format!("{}/", plan.source_root);
    let mut mapped_names = HashSet::new();
    let mut mapped_source_entries = 0_usize;
    let mut found_main_part = false;

    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(map_zip_error)?;
        if file.is_dir() {
            continue;
        }
        let source_name = safe_entry_name(&file)?;
        if source_name == plan.source_main_part {
            found_main_part = true;
        }
        if source_name.starts_with(&source_prefix) {
            mapped_source_entries = mapped_source_entries.saturating_add(1);
        }
        let target_name = plan.map_entry_name(&source_name);
        if !mapped_names.insert(target_name) {
            return Err(malformed(
                "The relocated OOXML package would create conflicting normalized archive paths.",
            ));
        }
    }

    if !found_main_part || mapped_source_entries == 0 {
        return Err(malformed(
            "The OOXML package does not contain the main document subtree declared by its root relationship.",
        ));
    }
    Ok(())
}

fn read_optional_member(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    name: &str,
    maximum_bytes: u64,
) -> Result<Option<Vec<u8>>, CommandError> {
    match archive.by_name(name) {
        Ok(file) => read_member(file, maximum_bytes).map(Some),
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(error) => Err(map_zip_error(error)),
    }
}

fn read_required_member(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    name: &str,
    maximum_bytes: u64,
) -> Result<Vec<u8>, CommandError> {
    let file = archive.by_name(name).map_err(|error| match error {
        zip::result::ZipError::FileNotFound => malformed(
            "The OOXML package root relationship references a missing main document part.",
        ),
        other => map_zip_error(other),
    })?;
    read_member(file, maximum_bytes)
}

fn read_member<R: Read>(
    mut file: zip::read::ZipFile<'_, R>,
    maximum_bytes: u64,
) -> Result<Vec<u8>, CommandError> {
    if file.size() > maximum_bytes {
        return Err(resource_limit(
            "An OOXML package metadata part exceeds oStudy's bounded XML limit.",
        ));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(file.size()).unwrap_or(0));
    file.by_ref()
        .take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(CommandError::io)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_bytes {
        return Err(resource_limit(
            "An OOXML package metadata part exceeds oStudy's bounded XML limit.",
        ));
    }
    Ok(bytes)
}

fn office_document_target(bytes: &[u8]) -> Result<Option<String>, CommandError> {
    let xml = std::str::from_utf8(bytes)
        .map_err(|_| malformed("The OOXML root relationships part is not valid UTF-8 XML."))?;
    let document = Document::parse(xml).map_err(|error| {
        malformed(&format!(
            "The OOXML root relationships part is malformed: {}",
            bounded_text(&error.to_string(), 300)
        ))
    })?;

    let mut target = None;
    for relationship in document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "Relationship")
    {
        let Some(relationship_type) = relationship.attribute("Type") else {
            continue;
        };
        if !relationship_type.ends_with("/officeDocument") {
            continue;
        }
        if relationship
            .attribute("TargetMode")
            .is_some_and(|mode| mode.eq_ignore_ascii_case("external"))
        {
            return Err(CommandError::new(
                "invalid_document_source",
                "The OOXML package declares an external main-document relationship.",
            ));
        }
        let candidate = relationship
            .attribute("Target")
            .ok_or_else(|| malformed("The OOXML main-document relationship has no target."))?;
        let candidate = normalize_part_name(candidate)?;
        if target.replace(candidate).is_some() {
            return Err(malformed(
                "The OOXML package declares more than one main document relationship.",
            ));
        }
    }
    Ok(target)
}

fn family_from_main_part(bytes: &[u8]) -> Result<OoxmlFamily, CommandError> {
    let xml = std::str::from_utf8(bytes)
        .map_err(|_| malformed("The OOXML main document part is not valid UTF-8 XML."))?;
    let document = Document::parse(xml).map_err(|error| {
        malformed(&format!(
            "The OOXML main document part is malformed: {}",
            bounded_text(&error.to_string(), 300)
        ))
    })?;
    let namespace = document
        .root_element()
        .tag_name()
        .namespace()
        .unwrap_or_default();
    for family in [
        OoxmlFamily::Word,
        OoxmlFamily::Presentation,
        OoxmlFamily::Spreadsheet,
    ] {
        if family.accepted_namespaces().contains(&namespace) {
            return Ok(family);
        }
    }
    Err(CommandError::new(
        "unsupported_document_format",
        "The OOXML main document part is not a supported Word, PowerPoint, or spreadsheet document.",
    ))
}

fn normalize_part_name(value: &str) -> Result<String, CommandError> {
    let value = value.trim();
    if value.is_empty()
        || value.contains('\0')
        || value.contains('\\')
        || value.contains('?')
        || value.contains('#')
    {
        return Err(malformed(
            "The OOXML main-document relationship contains an invalid package path.",
        ));
    }

    let relative = value.trim_start_matches('/');
    let mut components = Vec::new();
    for component in relative.split('/') {
        match component {
            "" | ".." => {
                return Err(malformed(
                    "The OOXML main-document relationship contains an unsafe package path.",
                ));
            }
            "." => {}
            safe => components.push(safe),
        }
    }
    if components.is_empty() {
        return Err(malformed(
            "The OOXML main-document relationship contains an empty package path.",
        ));
    }
    Ok(components.join("/"))
}

fn safe_entry_name<R: Read>(file: &zip::read::ZipFile<'_, R>) -> Result<String, CommandError> {
    if file.name().contains('\0') || file.name().contains('\\') {
        return Err(malformed(
            "The OOXML package contains an unsafe archive entry path.",
        ));
    }
    let path = file.enclosed_name().ok_or_else(|| {
        malformed("The OOXML package contains an archive entry outside the package root.")
    })?;
    path_components(&path)
}

fn path_components(path: &Path) -> Result<String, CommandError> {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let value = value.to_str().ok_or_else(|| {
                    malformed("The OOXML package contains a non-UTF-8 archive entry path.")
                })?;
                components.push(value);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(malformed(
                    "The OOXML package contains an unsafe archive entry path.",
                ));
            }
        }
    }
    if components.is_empty() {
        return Err(malformed(
            "The OOXML package contains an empty archive entry path.",
        ));
    }
    Ok(components.join("/"))
}

fn part_parent(part: &str) -> String {
    part.rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_default()
}

fn relationship_part_for(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((parent, filename)) => format!("{parent}/_rels/{filename}.rels"),
        None => format!("_rels/{part}.rels"),
    }
}

fn looks_like_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
        || bytes.starts_with(b"PK\x05\x06")
        || bytes.starts_with(b"PK\x07\x08")
}

fn map_zip_error(error: zip::result::ZipError) -> CommandError {
    let detail = bounded_text(&error.to_string(), 300);
    if detail.to_ascii_lowercase().contains("password")
        || detail.to_ascii_lowercase().contains("encrypted")
    {
        return CommandError::new(
            "document_encrypted",
            "Encrypted or password-protected documents cannot be processed.",
        );
    }
    malformed(&format!(
        "The OOXML package could not be read safely: {detail}"
    ))
}

fn malformed(message: &str) -> CommandError {
    CommandError::new("document_malformed", message)
}

fn resource_limit(message: &str) -> CommandError {
    CommandError::new("document_resource_limit", message)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::SimpleFileOptions;

    use super::*;

    #[test]
    fn relocated_presentation_is_detected_and_rewritten_for_xberg() {
        let bytes = include_bytes!("../../tests/fixtures/anydoc-v0.2.3/handmade-altpath.pptx");
        let plan = inspect_relocated_ooxml(bytes)
            .expect("relocated fixture inspection should succeed")
            .expect("relocated fixture should require normalization");
        assert_eq!(plan.format(), DocumentFormat::Pptx);

        let normalized = normalize_for_xberg(bytes, &plan)
            .expect("relocated fixture should normalize for Xberg");
        assert_eq!(
            xberg::core::mime::detect_mime_type_from_bytes(&normalized)
                .expect("Xberg should detect the normalized package"),
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        );
        let mut archive = ZipArchive::new(Cursor::new(normalized)).unwrap();
        assert!(archive.by_name("ppt/presentation.xml").is_ok());
        assert!(archive.by_name("ppt/_rels/presentation.xml.rels").is_ok());
        assert!(archive.by_name("ppt/s/one.xml").is_ok());
        assert!(archive.by_name("deck/pres.xml").is_err());
    }

    #[test]
    fn external_or_traversing_main_relationships_are_rejected() {
        for target in ["https://example.test/presentation.xml", "../deck/pres.xml"] {
            let target_mode = if target.starts_with("https://") {
                " TargetMode=\"External\""
            } else {
                ""
            };
            let relationships = format!(
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="{target}"{target_mode}/></Relationships>"#
            );
            let bytes = package(&[
                (ROOT_RELATIONSHIPS_PATH, relationships.as_bytes()),
                (
                    "deck/pres.xml",
                    br#"<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>"#,
                ),
            ]);
            assert_eq!(
                inspect_relocated_ooxml(&bytes).unwrap_err().code,
                if target.starts_with("https://") {
                    "invalid_document_source"
                } else {
                    "document_malformed"
                }
            );
        }
    }

    #[test]
    fn generic_zip_without_an_office_main_relationship_is_not_promoted() {
        let bytes = package(&[("notes.txt", b"not an OOXML package")]);
        assert!(inspect_relocated_ooxml(&bytes).unwrap().is_none());
    }

    fn package(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, bytes) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
}
