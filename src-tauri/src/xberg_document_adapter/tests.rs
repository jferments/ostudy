use std::collections::HashMap;

use xberg::types::document_structure::{
    ContentLayer, DocumentNode, DocumentStructure, NodeContent,
};
use xberg::{ExtractedDocument, PageContent};

use crate::{document_canonical::CanonicalBlockRole, document_domain::DocumentFormat};

use super::normalization::normalize_document;

fn page(page_number: u32, content: &str) -> PageContent {
    PageContent {
        page_number,
        content: content.to_string(),
        tables: Vec::new(),
        image_indices: Vec::new(),
        hierarchy: None,
        is_blank: Some(false),
        layout_regions: None,
        speaker_notes: None,
        section_name: None,
        sheet_name: None,
    }
}

#[test]
fn pptx_pages_preserve_exact_boundaries_and_speaker_notes() {
    let mut first = page(1, "Cardiac output");
    first.section_name = Some("Hemodynamics".to_string());
    first.speaker_notes = Some("Cardiac output equals heart rate times stroke volume.".to_string());
    let mut document = ExtractedDocument::default();
    document.pages = Some(vec![first, page(2, "Systemic vascular resistance")]);

    let normalized = normalize_document(&document, DocumentFormat::Pptx, &HashMap::new())
        .expect("PPTX pages should normalize");

    assert!(normalized.exact_unit_boundaries);
    assert_eq!(normalized.units.len(), 2);
    assert_eq!(normalized.units[0].label.as_deref(), Some("Slide 1"));
    assert_eq!(normalized.units[0].title.as_deref(), Some("Hemodynamics"));
    assert!(normalized.units[0]
        .blocks
        .iter()
        .any(|block| block.role == CanonicalBlockRole::SpeakerNote));
}

#[test]
fn spreadsheet_pages_keep_sheet_names_as_section_labels() {
    let mut sheet = page(1, "Medication\tDose\nHeparin\t5000 units");
    sheet.sheet_name = Some("Anticoagulants".to_string());
    let mut document = ExtractedDocument::default();
    document.pages = Some(vec![sheet]);

    let normalized = normalize_document(&document, DocumentFormat::Excel, &HashMap::new())
        .expect("spreadsheet pages should normalize");

    assert!(normalized.exact_unit_boundaries);
    assert_eq!(normalized.units.len(), 1);
    assert_eq!(normalized.units[0].label.as_deref(), Some("Anticoagulants"));
    assert_eq!(normalized.units[0].title.as_deref(), Some("Anticoagulants"));
}

#[test]
fn flow_documents_split_on_level_one_headings() {
    let mut document = ExtractedDocument::default();
    document.document = Some(DocumentStructure {
        nodes: vec![
            DocumentNode {
                id: "heading-1".to_string(),
                content: NodeContent::Heading {
                    level: 1,
                    text: "Respiratory failure".to_string(),
                },
                parent: None,
                children: Vec::new(),
                content_layer: ContentLayer::Body,
                page: None,
                page_end: None,
                bbox: None,
                annotations: Vec::new(),
                attributes: None,
            },
            DocumentNode {
                id: "paragraph-1".to_string(),
                content: NodeContent::Paragraph {
                    text: "Hypoxemia can occur when ventilation and perfusion are mismatched."
                        .to_string(),
                },
                parent: None,
                children: Vec::new(),
                content_layer: ContentLayer::Body,
                page: None,
                page_end: None,
                bbox: None,
                annotations: Vec::new(),
                attributes: None,
            },
            DocumentNode {
                id: "heading-2".to_string(),
                content: NodeContent::Heading {
                    level: 1,
                    text: "Ventilatory failure".to_string(),
                },
                parent: None,
                children: Vec::new(),
                content_layer: ContentLayer::Body,
                page: None,
                page_end: None,
                bbox: None,
                annotations: Vec::new(),
                attributes: None,
            },
        ],
        source_format: Some("docx".to_string()),
        relationships: Vec::new(),
        node_types: Vec::new(),
    });

    let normalized = normalize_document(&document, DocumentFormat::Docx, &HashMap::new())
        .expect("DOCX structure should normalize");

    assert!(!normalized.exact_unit_boundaries);
    assert_eq!(normalized.units.len(), 2);
    assert_eq!(
        normalized.units[0].title.as_deref(),
        Some("Respiratory failure")
    );
    assert_eq!(
        normalized.units[1].title.as_deref(),
        Some("Ventilatory failure")
    );
}
