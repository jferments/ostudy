use crate::{document_canonical::CanonicalBlockRole, document_domain::DocumentFormat};

use super::{
    model::{NormalizedBlock, UnitDraft},
    support::{bounded_text, MAX_UNIT_TEXT_CHARS},
};

const MAX_UNIT_TARGET_CHARS: usize = 60_000;
const MAX_UNIT_BLOCKS: usize = 160;

pub(crate) fn segment_units(
    format: DocumentFormat,
    blocks: Vec<NormalizedBlock>,
) -> Vec<UnitDraft> {
    let mut units = Vec::new();
    let mut current = UnitDraft::default();
    for block in blocks {
        let heading_boundary = block.heading_level == Some(1) && !current.is_empty();
        let table_boundary = matches!(
            format,
            DocumentFormat::Excel | DocumentFormat::Ods | DocumentFormat::Csv
        ) && block.role == CanonicalBlockRole::Table
            && current
                .blocks
                .iter()
                .any(|existing| existing.role == CanonicalBlockRole::Table);
        let capacity_boundary = !current.is_empty()
            && (current.blocks.len() >= MAX_UNIT_BLOCKS
                || current
                    .text_chars
                    .saturating_add(block.text.chars().count())
                    > MAX_UNIT_TARGET_CHARS);
        if heading_boundary || table_boundary || capacity_boundary {
            units.push(current);
            current = UnitDraft::default();
        }
        current.push(block);
    }
    if !current.is_empty() {
        units.push(current);
    }
    units
}

pub(crate) fn unit_text_projection(unit: &UnitDraft, label: &str) -> String {
    let text = unit
        .blocks
        .iter()
        .map(|block| block.text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if text.is_empty() && unit.blocks.iter().any(|block| !block.asset_ids.is_empty()) {
        format!(
            "{label} contains embedded visual material with no extractable text. Use the attached source assets as evidence and do not infer unsupported details."
        )
    } else {
        bounded_text(&text, MAX_UNIT_TEXT_CHARS)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn block(role: CanonicalBlockRole, text: &str, heading_level: Option<u8>) -> NormalizedBlock {
        NormalizedBlock {
            role,
            text: text.to_string(),
            heading_level,
            parent_heading: None,
            heading_path: Vec::new(),
            asset_ids: Vec::new(),
            attributes: BTreeMap::new(),
        }
    }

    #[test]
    fn heading_boundaries_create_stable_units() {
        let units = segment_units(
            DocumentFormat::Docx,
            vec![
                block(CanonicalBlockRole::Title, "First", Some(1)),
                block(CanonicalBlockRole::Paragraph, "Body", None),
                block(CanonicalBlockRole::Heading, "Second", Some(1)),
            ],
        );
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].title.as_deref(), Some("First"));
        assert_eq!(units[1].title.as_deref(), Some("Second"));
    }
}
