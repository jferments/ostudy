use std::collections::{BTreeMap, BTreeSet};

use xberg::Metadata;

use crate::document_canonical::DocumentMetadata;

use super::support::{bounded_text, MAX_ATTRIBUTE_VALUE_CHARS, MAX_HEADING_CHARS};

const MAX_AUTHORS: usize = 128;
const MAX_AUTHOR_CHARS: usize = 500;
const MAX_KEYWORDS: usize = 128;

pub(crate) fn map_metadata(
    metadata: &Metadata,
    fallback_title: Option<String>,
) -> DocumentMetadata {
    let title = metadata
        .title
        .as_deref()
        .map(|value| bounded_text(value, 1_000))
        .filter(|value| !value.is_empty())
        .or(fallback_title);
    let authors = metadata
        .authors
        .as_deref()
        .unwrap_or_default()
        .iter()
        .take(MAX_AUTHORS)
        .map(|value| bounded_text(value, MAX_AUTHOR_CHARS))
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let language = metadata
        .language
        .as_deref()
        .map(|value| bounded_text(value, 80))
        .filter(|value| !value.is_empty());
    let subject = metadata
        .subject
        .as_deref()
        .map(|value| bounded_text(value, 1_000))
        .filter(|value| !value.is_empty());

    let mut attributes = BTreeMap::new();
    insert_attribute(&mut attributes, "createdAt", metadata.created_at.as_deref());
    insert_attribute(
        &mut attributes,
        "modifiedAt",
        metadata.modified_at.as_deref(),
    );
    insert_attribute(&mut attributes, "createdBy", metadata.created_by.as_deref());
    insert_attribute(
        &mut attributes,
        "modifiedBy",
        metadata.modified_by.as_deref(),
    );
    if let Some(format) = metadata.format.as_ref() {
        attributes.insert(
            "xbergFormatMetadata".to_string(),
            bounded_text(&format.to_string(), MAX_ATTRIBUTE_VALUE_CHARS),
        );
    }
    let keywords = metadata
        .keywords
        .as_deref()
        .unwrap_or_default()
        .iter()
        .take(MAX_KEYWORDS)
        .map(|value| bounded_text(value, MAX_HEADING_CHARS))
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    if !keywords.is_empty() {
        attributes.insert(
            "keywords".to_string(),
            bounded_text(&keywords, MAX_ATTRIBUTE_VALUE_CHARS),
        );
    }

    DocumentMetadata {
        title,
        authors,
        language,
        subject,
        attributes,
    }
}

fn insert_attribute(attributes: &mut BTreeMap<String, String>, key: &str, value: Option<&str>) {
    if let Some(value) = value
        .map(|value| bounded_text(value, MAX_ATTRIBUTE_VALUE_CHARS))
        .filter(|value| !value.is_empty())
    {
        attributes.insert(key.to_string(), value);
    }
}
