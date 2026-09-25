//! Include the LK-03 graph in the existing typed-row/file preservation receipt.
//! Native admission and context readers verify the retained representation; no
//! provider, user data, inferred table structure or weakened assertion is used.
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use crate::{
    database::Database,
    document_canonical::{
        structure::{BlockStructure, TableCell, TableStructure},
        CanonicalBlock, CanonicalBlockRole,
    },
    document_canonical_store,
    error::CommandError,
    knowledge::{
        adapters::{materialize_source, SourceSeed},
        analysis_store::Checkpoint,
        contracts::{hash, SourceKind},
        evidence::EvidenceCatalog,
        extraction::decode,
        learning_context,
    },
};

pub(super) fn seed(database: &Database) {
    let document = super::id(500);
    let table = TableStructure {
        rows: 1,
        columns: 2,
        cells: ["Ordering", "One precedes two"]
            .into_iter()
            .enumerate()
            .map(|(column, text)| TableCell {
                row: 0,
                column,
                row_span: 1,
                column_span: 1,
                header: Some(false),
                text: text.into(),
                bounds: None,
            })
            .collect(),
        column_headers: Some(vec!["Topic".into(), "Rule".into()]),
    };
    let text = table.text_projection().unwrap();
    let mut block = CanonicalBlock::new(
        0,
        0,
        1,
        CanonicalBlockRole::Table,
        &text,
        None,
        vec![],
        None,
        None,
        vec![],
        Default::default(),
    );
    block.structure = Some(BlockStructure {
        table: Some(table),
        ..Default::default()
    });
    database
        .with_connection(|connection| {
            let transaction = connection.transaction().map_err(CommandError::database)?;
            transaction
                .execute(
                    "INSERT INTO document_source_blocks(document_id,block_index,block_ordinal,
                 source_unit_number,role,text,parent_heading,heading_path_json,geometry_json,
                 confidence,asset_ids_json,attributes_json,content_sha256,created_at)
                 VALUES(?1,0,0,1,'table',?2,NULL,'[]',NULL,NULL,'[]','{}',?3,10)",
                    params![document, text, block.content_sha256],
                )
                .map_err(CommandError::database)?;
            transaction
                .execute(
                    "UPDATE document_semantic_chunk_sources SET text=?1,full_text=?1,content_sha256=?2
                 WHERE document_id=?3 AND chunk_index=0",
                    params![text, hash(&text), document],
                )
                .map_err(CommandError::database)?;
            document_canonical_store::evidence::publish(&transaction, &document, &[block])?;
            document_canonical_store::evidence::validate(&transaction)?;
            transaction.commit().map_err(CommandError::database)
        })
        .unwrap();
    let sources = materialize_source(
        database,
        &SourceSeed {
            kind: SourceKind::Document,
            owner_id: document,
            parent_key: "0".into(),
        },
    )
    .unwrap();
    let evidence = EvidenceCatalog::build(&sources, false).unwrap();
    let good = json!({
        "title":"Precedence", "kind":"process", "statement":"One precedes two.",
        "qualifications":[], "topic":"Ordering", "importance":"core", "evidenceIds":["E0"],
        "details":{"kind":"process", "steps":[
            {"text":"One","occurrence":0}, {"text":"two","occurrence":0}
        ],"dependencies":[{"before":0,"after":1}]}
    });
    let mut bad = good.clone();
    bad["evidenceIds"] = json!(["E404"]);
    let report = decode::initial(json!({"units":[good,bad]}), &evidence, &sources).unwrap();
    assert_eq!(report.accepted.len(), 1);
    assert_eq!(report.rejected.len(), 1);
    assert!(!report.is_complete());
    Checkpoint::open(database, &sources, &hash("lk03-preservation"))
        .unwrap()
        .save(&report)
        .unwrap();
    database
        .with_connection(|connection| verify(connection))
        .unwrap();
}

pub(super) fn verify(connection: &Connection) -> Result<(), CommandError> {
    let document = super::id(500);
    let present = connection
        .query_row("SELECT id FROM documents WHERE id=?1", [&document], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map_err(CommandError::database)?;
    if present.is_none() {
        // The restore target's original empty library is verified by the same
        // manifest, including after rollback. There is no graph to decode there.
        return Ok(());
    }
    let before = connection.total_changes();
    document_canonical_store::evidence::validate(connection)?;
    let context = learning_context::for_document_chunks(connection, &document, &[0])?;
    assert_eq!(context.sources.len(), 1);
    assert_eq!(context.units.len(), 1);
    assert_eq!(context.preparation.rejected_proposals, 1);
    assert_eq!(context.preparation.legacy_coverage_sources, 0);
    assert_eq!(context.preparation.evidence_structures.len(), 1);
    let payload = learning_context::material_prompt(&context.sources, &context.units)?;
    assert_eq!(
        payload["units"][0]["details"]["steps"],
        json!(["One", "two"])
    );
    assert_eq!(
        payload["units"][0]["details"]["dependencies"],
        json!([{"before":0,"after":1}])
    );
    context.stamp.ensure_current(connection)?;
    assert_eq!(connection.total_changes(), before);
    Ok(())
}
