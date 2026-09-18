use super::*;
use crate::{document_canonical::CanonicalBlockRole, document_domain::DocumentFormat};
use xberg::types::document_structure::{
    ContentLayer, DocumentNode, DocumentRelationship, DocumentStructure, GridCell, NodeIndex,
    TableGrid,
};

fn node(content: NodeContent) -> DocumentNode {
    DocumentNode {
        id: "extractor-local".to_string(),
        content,
        parent: None,
        children: Vec::new(),
        content_layer: ContentLayer::Body,
        page: None,
        page_end: None,
        bbox: None,
        annotations: Vec::new(),
        attributes: None,
    }
}

fn document(nodes: Vec<DocumentNode>) -> ExtractedDocument {
    ExtractedDocument {
        document: Some(DocumentStructure {
            nodes,
            source_format: Some("docx".to_string()),
            relationships: Vec::new(),
            node_types: Vec::new(),
        }),
        ..ExtractedDocument::default()
    }
}

#[test]
fn native_normalization_retains_distinct_occurrences_and_exact_code() {
    let text = "Do not change this condition.";
    let code = "    if x <= 2:\n        return x\n";
    let mut first = node(NodeContent::Paragraph { text: text.into() });
    first.attributes = Some(HashMap::from([(NODE.to_string(), "999".to_string())]));
    let input = document(vec![
        first,
        node(NodeContent::Paragraph { text: text.into() }),
        node(NodeContent::Code {
            text: code.into(),
            language: Some("python".into()),
        }),
    ]);
    let normalized =
        super::super::normalize_document(&input, DocumentFormat::Docx, &HashMap::new()).unwrap();
    let catalog = StructureCatalog::new(&input).unwrap();
    let blocks = normalized
        .units
        .iter()
        .flat_map(|unit| &unit.blocks)
        .collect::<Vec<_>>();
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].text, blocks[1].text);
    assert_eq!(
        catalog.resolve(&blocks[0].attributes).unwrap().unwrap().node,
        Some(0)
    );
    assert_eq!(
        catalog.resolve(&blocks[1].attributes).unwrap().unwrap().node,
        Some(1)
    );
    assert_eq!(blocks[2].role, CanonicalBlockRole::CodeBlock);
    assert_eq!(blocks[2].text, code);
    assert!(literal_text(&"x".repeat(250_001)).is_err());
    assert!(literal_text("not\0valid").is_err());
}

#[test]
fn table_and_footnote_relationship_are_retained_without_invented_coordinates() {
    let table = TableGrid {
        rows: 2,
        cols: 2,
        cells: vec![
            GridCell {
                row: 0,
                col: 0,
                row_span: 1,
                col_span: 2,
                is_header: true,
                content: "Velocity (m/s), only while moving".into(),
                bbox: None,
            },
            GridCell {
                row: 1,
                col: 0,
                row_span: 1,
                col_span: 2,
                is_header: false,
                content: "−2 ≤ v < 3".into(),
                bbox: None,
            },
        ],
    };
    let mut input = document(vec![
        node(NodeContent::Table { grid: table }),
        node(NodeContent::Footnote {
            text: "Not applicable at rest.".into(),
        }),
    ]);
    input.document.as_mut().unwrap().relationships = vec![
        DocumentRelationship {
            source: NodeIndex(0),
            target: NodeIndex(1),
            kind: RelationshipKind::FootnoteReference,
        },
        DocumentRelationship {
            source: NodeIndex(0),
            target: NodeIndex(99),
            kind: RelationshipKind::Caption,
        },
    ];
    let normalized =
        super::super::normalize_document(&input, DocumentFormat::Docx, &HashMap::new()).unwrap();
    let catalog = StructureCatalog::new(&input).unwrap();
    let block = normalized
        .units
        .iter()
        .flat_map(|unit| &unit.blocks)
        .find(|block| block.role == CanonicalBlockRole::Table)
        .unwrap();
    let structure = catalog.resolve(&block.attributes).unwrap().unwrap();
    structure.validate(block.role).unwrap();
    assert_eq!(structure.links.len(), 2);
    assert!(structure.bounds.is_none());
    let table = structure.table.unwrap();
    assert_eq!(table.cells[0].column_span, 2);
    assert_eq!(table.cells[0].header, Some(true));
    assert_eq!(table.cells[1].text, "−2 ≤ v < 3");
    assert_eq!(table.text_projection().unwrap(), block.text);
    assert_eq!(structure.links[1].target_node, 99);
}

#[test]
fn equal_flat_text_does_not_discard_different_table_headers() {
    let table = |header: &str| xberg::Table {
        cells: vec![vec!["2".into()]],
        markdown: "2".into(),
        page_number: 1,
        bounding_box: None,
        table_id: None,
        columns: Some(vec![header.into()]),
    };
    let input = ExtractedDocument {
        tables: vec![table("Velocity (m/s)"), table("Acceleration (m/s²)")],
        ..ExtractedDocument::default()
    };
    let normalized =
        super::super::normalize_document(&input, DocumentFormat::Docx, &HashMap::new()).unwrap();
    let catalog = StructureCatalog::new(&input).unwrap();
    let tables = normalized
        .units
        .iter()
        .flat_map(|unit| &unit.blocks)
        .filter(|block| block.role == CanonicalBlockRole::Table)
        .map(|block| {
            catalog
                .resolve(&block.attributes)
                .unwrap()
                .unwrap()
                .table
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(tables.len(), 2);
    assert_ne!(tables[0].column_headers, tables[1].column_headers);
    assert!(tables.iter().all(|table| table.cells[0].header.is_none()));
}

#[test]
fn code_layer_retains_intrinsic_kind_and_separate_source_role() {
    let mut code = node(NodeContent::Code {
        text: "    x = 2".into(),
        language: None,
    });
    code.content_layer = ContentLayer::Footnote;
    let input = document(vec![code]);
    let normalized =
        super::super::normalize_document(&input, DocumentFormat::Docx, &HashMap::new()).unwrap();
    let block = &normalized.units[0].blocks[0];
    assert_eq!(block.role, CanonicalBlockRole::CodeBlock);
    assert_eq!(block.attributes["sourceLayer"], "footnote");
    assert_eq!(block.text, "    x = 2");
}
