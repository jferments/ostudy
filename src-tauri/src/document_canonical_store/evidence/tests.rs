use super::*;
use crate::{
    database::Database,
    document_canonical::{CanonicalDocument, CanonicalExtractionReport, CanonicalIngestion,
        CanonicalSourceFile, CanonicalSourceUnit, DocumentMetadata, structure::{StructureLink, StructureLinkKind, TableCell}},
    document_domain::{DocumentSourceDescriptor, SourceUnitKind},
    document_canonical_store::{clear_canonical_derivatives, persist_canonical_ingestion},
};

fn block(index: i64, text: &str) -> CanonicalBlock {
    let mut block = CanonicalBlock::new(index, index, 1, CanonicalBlockRole::Paragraph,
        text, None, vec![], None, None, vec![], BTreeMap::new());
    block.structure = Some(BlockStructure { node: Some(index as u32), ..BlockStructure::default() });
    block
}

fn fixture(document: &str) -> CanonicalIngestion {
    CanonicalIngestion {
        document: CanonicalDocument {
            schema_version: 1,
            source: DocumentSourceDescriptor::pdf(1),
            source_files: vec![CanonicalSourceFile {
                file_ordinal: 1, original_filename: "authored.pdf".into(), media_type: "application/pdf".into(),
                relative_path: format!("documents/{document}/source.pdf"), byte_size: 1, sha256: sha256_hex(b"x"),
            }],
            metadata: DocumentMetadata::default(),
            units: vec![CanonicalSourceUnit::new(1, SourceUnitKind::Page, "Page 1", None,
                "Keep the conditions.\n    return x <= 2", 1)],
            blocks: vec![block(0, "Keep the conditions."), block(1, "    return x <= 2")],
            assets: vec![],
            extraction: CanonicalExtractionReport {
                adapter_backend: "synthetic-v1".into(), extraction_backend: "synthetic".into(),
                render_backend: None, extraction_mode: "native".into(), semantic_analysis_degraded: false,
                ocr_unit_count: 0, ocr_mean_confidence: None, ocr_engine: None,
            },
        },
        semantic_chunks: vec![],
    }
}

fn create_document(connection: &Connection, document: &str) {
    connection.execute(
        "INSERT INTO documents (id, title, original_filename, mime_type, byte_size,
         sha256, source_relative_path, lifecycle_state, created_at, updated_at,
         source_unit_count, extracted_source_unit_count)
         VALUES (?1, 'Authored fixture', 'authored.pdf', 'application/pdf', 1, ?2,
         ?3, 'failed', 1, 1, 1, 1)",
        params![document, sha256_hex(b"x"), format!("documents/{document}/source.pdf")],
    ).unwrap();
}

fn persist(connection: &mut Connection, document: &str, fixture: &CanonicalIngestion) {
    let transaction = connection.transaction().unwrap();
    clear_canonical_derivatives(&transaction, document).unwrap();
    persist_canonical_ingestion(&transaction, document, fixture, 1).unwrap();
    validate(&transaction).unwrap();
    transaction.commit().unwrap();
}

fn identities(connection: &Connection, document: &str) -> BTreeMap<String, String> {
    let mut statement = connection.prepare("SELECT payload_sha256, id FROM document_evidence_occurrences WHERE document_id=?1").unwrap();
    statement.query_map([document], |row| Ok((row.get(0)?, row.get(1)?))).unwrap()
        .collect::<Result<_, _>>().unwrap()
}

#[test]
fn occurrence_identity_survives_reordering_while_payloads_remain_immutable() {
    let database = Database::open_in_memory().unwrap();
    database.with_connection(|connection| {
        let document = "a".repeat(32);
        create_document(connection, &document);
        let mut fixture = fixture(&document);
        persist(connection, &document, &fixture);
        let before = identities(connection, &document);
        fixture.document.blocks.reverse();
        for (index, old) in fixture.document.blocks.iter_mut().enumerate() {
            let mut current = CanonicalBlock::new(index as i64, index as i64, 1, old.role,
                old.text.clone(), old.parent_heading.clone(), old.heading_path.clone(), old.geometry,
                old.confidence, old.asset_ids.clone(), old.attributes.clone());
            current.structure = old.structure.clone();
            *old = current;
        }
        persist(connection, &document, &fixture);
        assert_eq!(identities(connection, &document), before);
        assert!(connection.execute("UPDATE document_evidence_payloads SET payload_json='{}'", []).is_err());
        let text: String = connection.query_row("SELECT text FROM document_source_blocks WHERE block_index=0", [], |row| row.get(0)).unwrap();
        assert_eq!(text, "    return x <= 2");
        Ok(())
    }).unwrap();
}

#[test]
fn identical_payloads_never_merge_document_ownership_or_distinct_occurrences() {
    let database = Database::open_in_memory().unwrap();
    database.with_connection(|connection| {
        for document in ["a".repeat(32), "b".repeat(32)] {
            create_document(connection, &document);
            persist(connection, &document, &fixture(&document));
        }
        let a = identities(connection, &"a".repeat(32));
        let b = identities(connection, &"b".repeat(32));
        assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
        assert_ne!(a.values().collect::<Vec<_>>(), b.values().collect::<Vec<_>>());
        connection.execute("DELETE FROM documents WHERE id=?1", ["a".repeat(32)]).unwrap();
        assert_eq!(identities(connection, &"b".repeat(32)), b);
        let count: i64 = connection.query_row("SELECT COUNT(*) FROM document_evidence_payloads", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 2);
        connection.execute("DELETE FROM documents WHERE id=?1", ["b".repeat(32)]).unwrap();
        let count: i64 = connection.query_row("SELECT COUNT(*) FROM document_evidence_payloads", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 0);
        Ok(())
    }).unwrap();
}

#[test]
fn table_values_headers_spans_and_resolved_or_missing_links_survive_persistence() {
    let database = Database::open_in_memory().unwrap();
    database.with_connection(|connection| {
        let document = "a".repeat(32);
        create_document(connection, &document);
        let mut fixture = fixture(&document);
        let table = TableStructure {
            rows: 1, columns: 2,
            cells: vec![TableCell { row: 0, column: 0, row_span: 1, column_span: 2,
                header: Some(false), text: "−2 ≤ v < 3".into(), bounds: None }],
            column_headers: Some(vec!["Velocity (m/s)".into(), "Only while moving".into()]),
        };
        let mut value = CanonicalBlock::new(0, 0, 1, CanonicalBlockRole::Table,
            table.text_projection().unwrap(), None, vec![], None, None, vec![], BTreeMap::new());
        value.structure = Some(BlockStructure { node: Some(0), table: Some(table.clone()),
            links: vec![StructureLink { target_node: 1, kind: StructureLinkKind::Footnote },
                StructureLink { target_node: 99, kind: StructureLinkKind::Caption }],
            ..BlockStructure::default() });
        fixture.document.blocks[0] = value;
        persist(connection, &document, &fixture);
        let json: String = connection.query_row("SELECT payload_json FROM document_evidence_payloads WHERE json_extract(payload_json, '$.role')='table'", [], |row| row.get(0)).unwrap();
        let restored: Payload = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.table, Some(table));
        assert_eq!(restored.text, None);
        let resolved: i64 = connection.query_row("SELECT COUNT(*) FROM document_evidence_links WHERE target_occurrence_id IS NOT NULL", [], |row| row.get(0)).unwrap();
        let unresolved: i64 = connection.query_row("SELECT COUNT(*) FROM document_evidence_links WHERE target_occurrence_id IS NULL", [], |row| row.get(0)).unwrap();
        assert_eq!((resolved, unresolved), (1, 1));
        Ok(())
    }).unwrap();
}

#[test]
fn backfill_retains_exact_text_and_rejects_future_payloads_read_only() {
    let database = Database::open_in_memory().unwrap();
    database.with_connection(|connection| {
        let document = "a".repeat(32);
        create_document(connection, &document);
        persist(connection, &document, &fixture(&document));
        connection.execute("DELETE FROM document_evidence_occurrences", []).unwrap();
        assert_eq!(backfill(connection).unwrap(), 2);
        validate(connection).unwrap();
        let transaction = connection.transaction().unwrap();
        let json = r#"{"version":999,"role":"paragraph","text":"Unchanged","table":null}"#;
        transaction.execute("INSERT INTO document_evidence_payloads VALUES (?1, ?2)", params![sha256_hex(json.as_bytes()), json]).unwrap();
        let changes = transaction.total_changes();
        assert!(validate(&transaction).is_err());
        assert_eq!(transaction.total_changes(), changes);
        transaction.rollback().unwrap();
        validate(connection).unwrap();
        Ok(())
    }).unwrap();
}
