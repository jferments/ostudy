use super::*;

fn table() -> TableStructure {
    TableStructure {
        rows: 2,
        columns: 2,
        cells: vec![
            TableCell {
                row: 0,
                column: 0,
                row_span: 1,
                column_span: 2,
                header: Some(true),
                text: "Velocity (m/s), only while moving".into(),
                bounds: None,
            },
            TableCell {
                row: 1,
                column: 0,
                row_span: 1,
                column_span: 1,
                header: Some(false),
                text: "−2 ≤ v < 3".into(),
                bounds: None,
            },
        ],
        column_headers: None,
    }
}

#[test]
fn typed_table_retains_spans_roles_units_and_qualifications() {
    let table = table();
    table.validate().unwrap();
    let json = serde_json::to_string(&table).unwrap();
    let restored: TableStructure = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, table);
    assert_eq!(restored.cells[0].column_span, 2);
    assert_eq!(restored.cells[0].header, Some(true));
    assert!(restored.text_projection().unwrap().contains("−2 ≤ v < 3"));
    assert!(restored
        .text_projection()
        .unwrap()
        .contains("only while moving"));
}

#[test]
fn inherited_headers_do_not_become_a_fabricated_physical_row() {
    let mut table = table();
    table.column_headers = Some(vec!["Parent header A".into(), "Parent header B".into()]);
    table.validate().unwrap();
    assert!(!table.text_projection().unwrap().contains("Parent header"));
    assert!(serde_json::to_string(&table).unwrap().contains("Parent header"));
}

#[test]
fn invalid_overlapping_and_unbounded_grids_fail_without_truncation() {
    let original = table();
    let mut overlapping = original.clone();
    overlapping.cells.push(original.cells[0].clone());
    assert!(overlapping.validate().is_err());
    let mut invalid = original.clone();
    invalid.cells[0].column_span = 3;
    assert!(invalid.validate().is_err());
    invalid.cells[0].column_span = 0;
    assert!(invalid.validate().is_err());
    invalid.cells[0].column_span = usize::MAX;
    assert!(invalid.validate().is_err());
    let mut oversized = original;
    oversized.cells[0].text = "x".repeat(MAX_TEXT + 1);
    assert!(oversized.text_projection().is_err());
}

#[test]
fn unknown_structure_geometry_and_forged_links_fail_closed() {
    let mut structure = BlockStructure::default();
    structure.version = 999;
    assert!(structure.validate(CanonicalBlockRole::Paragraph).is_err());
    structure.version = 1;
    structure.bounds = Some(SourceBounds {
        x0: 2.0,
        y0: 0.0,
        x1: 1.0,
        y1: 1.0,
    });
    assert!(structure.validate(CanonicalBlockRole::Paragraph).is_err());
    structure.bounds = None;
    structure.links.push(StructureLink {
        target_node: 2,
        kind: StructureLinkKind::Footnote,
    });
    assert!(structure.validate(CanonicalBlockRole::Paragraph).is_err());
    structure.node = Some(1);
    structure.validate(CanonicalBlockRole::Paragraph).unwrap();
    structure.links.push(structure.links[0].clone());
    assert!(structure.validate(CanonicalBlockRole::Paragraph).is_err());
}
