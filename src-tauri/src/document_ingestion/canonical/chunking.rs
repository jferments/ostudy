use std::collections::BTreeSet;

use crate::{
    document_canonical::{
        CanonicalBlock, CanonicalBlockRole, CanonicalSemanticChunk, CanonicalSourceUnit,
    },
    document_domain::{SourceLocator, SourceUnitKind},
    error::CommandError,
};

const MAX_CHUNK_TARGET_CHARS: usize = 24_000;
const MAX_CHUNK_TEXT_CHARS: usize = 1_000_000;

pub(crate) fn build_semantic_chunks(
    unit_kind: SourceUnitKind,
    units: &[CanonicalSourceUnit],
    blocks: &[CanonicalBlock],
) -> Result<Vec<CanonicalSemanticChunk>, CommandError> {
    let mut chunks = Vec::new();
    for unit in units {
        let unit_blocks = blocks
            .iter()
            .filter(|block| block.source_unit_number == unit.unit_number)
            .collect::<Vec<_>>();
        let mut selected = Vec::<&CanonicalBlock>::new();
        let mut selected_chars = 0_usize;
        for block in unit_blocks {
            let heading_boundary =
                block.role == CanonicalBlockRole::Heading && !selected.is_empty();
            let capacity_boundary = !selected.is_empty()
                && selected_chars.saturating_add(block.text.chars().count())
                    > MAX_CHUNK_TARGET_CHARS;
            if heading_boundary || capacity_boundary {
                push_chunk(&mut chunks, unit_kind, unit, &selected)?;
                selected.clear();
                selected_chars = 0;
            }
            selected_chars = selected_chars.saturating_add(block.text.chars().count());
            selected.push(block);
        }
        if !selected.is_empty() {
            push_chunk(&mut chunks, unit_kind, unit, &selected)?;
        }
    }
    Ok(chunks)
}

fn push_chunk(
    chunks: &mut Vec<CanonicalSemanticChunk>,
    unit_kind: SourceUnitKind,
    unit: &CanonicalSourceUnit,
    blocks: &[&CanonicalBlock],
) -> Result<(), CommandError> {
    let asset_ids = blocks
        .iter()
        .flat_map(|block| block.asset_ids.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let element_types = blocks
        .iter()
        .map(|block| block.role.as_str().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut full_text = blocks
        .iter()
        .map(|block| block.text.as_str())
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if full_text.is_empty() && !asset_ids.is_empty() {
        full_text = format!(
            "{} contains embedded visual material with no extractable text. Use the attached source assets as evidence and do not infer unsupported details.",
            unit.label
        );
    }
    if full_text.is_empty() {
        return Ok(());
    }
    if full_text.chars().count() > MAX_CHUNK_TEXT_CHARS
        || full_text
            .chars()
            .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\t'))
    {
        return Err(CommandError::new(
            "document_content_invalid",
            "Canonical evidence exceeds its bound or contains unsupported controls; it was not truncated.",
        ));
    }
    let heading_context = blocks
        .iter()
        .rev()
        .find_map(|block| block.heading_path.last().cloned())
        .or_else(|| unit.title.clone());
    let locator = SourceLocator::with_evidence(
        unit_kind,
        vec![unit.unit_number],
        heading_context.clone().or_else(|| Some(unit.label.clone())),
        asset_ids,
        None,
    )?;
    let token_estimate = full_text.chars().count().div_ceil(4).max(1);
    chunks.push(CanonicalSemanticChunk::new(
        i64::try_from(chunks.len()).unwrap_or(i64::MAX),
        full_text.clone(),
        full_text,
        locator,
        element_types,
        heading_context,
        token_estimate,
        false,
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn source_chunks_preserve_code_bytes_and_refuse_oversized_evidence() {
        let text = "    if x <= 2:\n        return x\n";
        let unit = CanonicalSourceUnit::new(1, SourceUnitKind::Section, "Code", None, text, 1);
        let mut block = CanonicalBlock::new(
            0,
            0,
            1,
            CanonicalBlockRole::CodeBlock,
            text,
            None,
            vec![],
            None,
            None,
            vec![],
            BTreeMap::new(),
        );
        let chunks = build_semantic_chunks(
            SourceUnitKind::Section,
            std::slice::from_ref(&unit),
            std::slice::from_ref(&block),
        )
        .unwrap();
        assert_eq!(chunks[0].full_text, text);
        block.text = format!("{} except when x = 0", "a".repeat(MAX_CHUNK_TEXT_CHARS));
        assert!(build_semantic_chunks(SourceUnitKind::Section, &[unit], &[block]).is_err());
    }
}
