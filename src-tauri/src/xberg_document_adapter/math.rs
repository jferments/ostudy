//! Normalize only equations explicitly identified by the source adapter.
//! This runs before canonical source units/hashes are created, never as a repair
//! of stored evidence. Original document bytes and locators remain authoritative.

use sha2::{Digest, Sha256};
use crate::{cards::math::mathml, document_canonical::CanonicalBlockRole};
use super::normalization::NormalizedDocument;

pub(super) fn normalize_identified_math(document: &mut NormalizedDocument) -> usize {
    let mut unsupported = 0;
    for unit in &mut document.units {
        for block in &mut unit.blocks {
            if block.role != CanonicalBlockRole::Equation || !mathml::starts_mathml(block.text.trim()) {
                continue;
            }
            // Reserve application-owned provenance names; never trust a donor
            // attribute claiming that its formula has already been validated.
            for key in ["mathInputFormat", "mathInputSha256", "mathNormalization", "mathInputSource"] {
                block.attributes.remove(key);
            }
            block.attributes.insert("mathInputFormat".into(), "mathml".into());
            block.attributes.insert("mathInputSha256".into(), format!("{:x}", Sha256::digest(block.text.as_bytes())));
            match mathml::parse(&block.text) {
                Ok(formula) => {
                    if block.text.chars().count() <= 2000 {
                        block.attributes.insert("mathInputSource".into(), block.text.clone());
                    }
                    block.attributes.insert("mathNormalization".into(), "presentation-mathml-to-latex-v1".into());
                    block.text = formula.delimited();
                }
                Err(_) => {
                    // Keep the entire extracted source, not flattened child text
                    // or a guessed equation. The extraction report is degraded.
                    block.attributes.insert("mathNormalization".into(), "unsupported-source-retained".into());
                    unsupported += 1;
                }
            }
        }
        unit.text_chars = unit.blocks.iter().map(|block| block.text.chars().count()).sum();
    }
    unsupported
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use xberg::{ExtractedDocument, types::document_structure::{ContentLayer, DocumentNode, DocumentStructure, NodeContent}};
    use crate::{document_domain::DocumentFormat, document_ingestion::canonical::unit_text_projection};

    #[test]
    fn document_mathml_normalizes_identified_equations_before_source_hashing() {
        let source = "<math><mfrac><mi>a</mi><mi>b</mi></mfrac></math>";
        let mut extracted = ExtractedDocument::default();
        extracted.document = Some(DocumentStructure { nodes: vec![DocumentNode {
            id: "formula".into(), content: NodeContent::Formula { text: source.into() },
            parent: None, children: vec![], content_layer: ContentLayer::Body,
            page: None, page_end: None, bbox: None, annotations: vec![], attributes: None,
        }], source_format: Some("html".into()), relationships: vec![], node_types: vec![] });
        let mut normalized = super::super::normalization::normalize_document(&extracted, DocumentFormat::Html, &HashMap::new()).unwrap();
        assert_eq!(normalize_identified_math(&mut normalized), 0);
        let block = &normalized.units[0].blocks[0];
        assert_eq!(block.text, r"\(\frac{a}{b}\)");
        assert_eq!(block.attributes["mathInputSource"], source);
        assert_eq!(block.attributes["mathInputSha256"], format!("{:x}", Sha256::digest(source.as_bytes())));
        assert!(unit_text_projection(&normalized.units[0], "Section 1").contains(r"\frac{a}{b}"));
        let before = block.text.clone();
        normalize_identified_math(&mut normalized);
        assert_eq!(normalized.units[0].blocks[0].text, before);
    }

    #[test]
    fn document_mathml_unsupported_input_stays_source_not_a_false_equation() {
        use crate::document_ingestion::canonical::{NormalizedBlock, UnitDraft};
        let source = "<math><mfrac><mi>a</mi></mfrac></math>";
        let block = |role| NormalizedBlock { role, text: source.into(), heading_level: None,
            parent_heading: None, heading_path: vec![], asset_ids: vec![], attributes: Default::default() };
        let mut normalized = NormalizedDocument { units: vec![UnitDraft { blocks: vec![block(CanonicalBlockRole::Equation), block(CanonicalBlockRole::CodeBlock)], ..Default::default() }], exact_unit_boundaries: false, skipped_node_count: 0, external_image_reference_count: 0 };
        assert_eq!(normalize_identified_math(&mut normalized), 1);
        assert_eq!(normalized.units[0].blocks[0].text, source);
        assert_eq!(normalized.units[0].blocks[0].attributes["mathNormalization"], "unsupported-source-retained");
        assert!(normalized.units[0].blocks[1].attributes.is_empty());
    }
}
