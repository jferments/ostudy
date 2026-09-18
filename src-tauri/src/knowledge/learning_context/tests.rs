use super::*;
use crate::{
    database::Database,
    knowledge::{
        analysis_store, authoring::tests::fixture, evidence::EvidenceCatalog, extraction::decode,
    },
};

fn scope(source: &SourceRecord) -> StudyScope {
    StudyScope {
        kind: super::super::contracts::ScopeKind::Document,
        id: source.document_id.clone(),
        members: Vec::new(),
    }
}

fn read(
    database: &Database,
    scope: &StudyScope,
    sources: &[SourceRecord],
    policy: ContextPolicy,
    budget: ContextBudget,
) -> Result<LearningContext, CommandError> {
    database.with_connection(|connection| {
        build(
            connection,
            LearningContextRequest {
                scope,
                include_conversations: false,
                candidates: sources,
                policy,
                target_units: None,
                budget,
            },
        )
    })
}

#[test]
fn consumers_reuse_saved_units_without_writes_and_report_older_coverage_honestly() {
    let (database, source, catalog, _) = fixture();
    let before = database.with_connection(|c| Ok(c.total_changes())).unwrap();
    for _ in 0..3 {
        let result = read(
            &database,
            &scope(&source),
            std::slice::from_ref(&source),
            ContextPolicy::Retrieval,
            ContextBudget::authoring(),
        )
        .unwrap();
        assert_eq!(result.units[0].id, catalog.units[0].id);
        assert_eq!(result.preparation.legacy_coverage_sources, 1);
        assert!(result.preparation.notice().contains("not established"));
        database
            .with_connection(|c| result.stamp.ensure_current(c))
            .unwrap();
        let authored = database
            .with_connection(|c| for_document_chunks(c, &source.owner_id, &[0]))
            .unwrap();
        assert_eq!(authored.units[0].id, result.units[0].id);
    }
    assert_eq!(
        database.with_connection(|c| Ok(c.total_changes())).unwrap(),
        before
    );
}

#[test]
fn scope_filter_runs_before_hydration_and_drops_excluded_images_and_forged_text() {
    let (database, source, _, _) = fixture();
    let general = StudyScope {
        kind: super::super::contracts::ScopeKind::General,
        id: None,
        members: Vec::new(),
    };
    let mut forged = source.clone();
    forged.text = "FORGED_CONTEXT_SENTINEL".into();
    let result = read(
        &database,
        &general,
        &[forged.clone()],
        ContextPolicy::Retrieval,
        ContextBudget::authoring(),
    )
    .unwrap();
    assert!(result.sources.is_empty());
    assert!(result.units.is_empty());
    assert_eq!(result.preparation.omitted_sources, 1);
    let admitted = read(
        &database,
        &scope(&source),
        &[forged],
        ContextPolicy::Retrieval,
        ContextBudget::authoring(),
    )
    .unwrap();
    assert_eq!(admitted.sources[0].text, source.text);
    assert!(!material_prompt(&admitted.sources, &admitted.units)
        .unwrap()
        .to_string()
        .contains("FORGED_CONTEXT_SENTINEL"));
}

#[test]
fn all_dependencies_travel_with_authoring_but_retrieval_eviction_drops_the_entire_claim() {
    let (database, first, _, _) = fixture();
    let mut second = first.clone();
    second.segment_key = "1:0:0".into();
    second.text = "Except in the explicitly stated case.".into();
    second.refresh_identity().unwrap();
    database
        .with_connection(|c| repository::publish_source(c, &second, "1", 2))
        .unwrap();
    let sources = [first.clone(), second];
    let evidence = EvidenceCatalog::build(&sources, false).unwrap();
    let report = decode::initial(json!({"units":[{"title":"Qualified relation","kind":"relationship",
        "statement":"The relation holds except in the stated case.","qualifications":[],"topic":"Geometry",
        "importance":"core","evidenceIds":["E0","E1"]}]}), &evidence, &sources).unwrap();
    analysis_store::publish(&database, &sources, &hash("context-relations"), &report).unwrap();
    let authored = database
        .with_connection(|c| for_document_chunks(c, &first.owner_id, &[0]))
        .unwrap();
    assert_eq!(authored.sources.len(), 2);
    assert_eq!(authored.units.len(), 1);
    let retrieved = read(
        &database,
        &scope(&first),
        &sources,
        ContextPolicy::Retrieval,
        ContextBudget {
            sources: 1,
            ..ContextBudget::authoring()
        },
    )
    .unwrap();
    assert_eq!(retrieved.sources.len(), 1);
    assert!(retrieved.units.is_empty());
    assert_eq!(retrieved.preparation.omitted_units, 1);
    assert!(read(
        &database,
        &scope(&first),
        &sources,
        ContextPolicy::Coverage,
        ContextBudget {
            sources: 1,
            ..ContextBudget::authoring()
        }
    )
    .is_err());
    assert!(
        material_prompt(&authored.sources[..1], &authored.units).unwrap()["units"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn late_interpretation_replacement_is_rejected_even_when_source_bytes_did_not_change() {
    let (database, source, _, _) = fixture();
    let result = read(
        &database,
        &scope(&source),
        std::slice::from_ref(&source),
        ContextPolicy::Retrieval,
        ContextBudget::authoring(),
    )
    .unwrap();
    repository::publish_units(&database, &source, &hash("new-empty-result"), Vec::new()).unwrap();
    assert!(database
        .with_connection(|c| result.stamp.ensure_current(c))
        .is_err());
    let empty = read(
        &database,
        &scope(&source),
        std::slice::from_ref(&source),
        ContextPolicy::Retrieval,
        ContextBudget::authoring(),
    )
    .unwrap();
    assert!(empty.units.is_empty());
    database
        .with_connection(|c| {
            c.execute(
                "UPDATE knowledge_sources SET active=0 WHERE id=?1",
                [&source.id],
            )
            .map_err(CommandError::database)?;
            assert!(empty.stamp.ensure_current(c).is_err());
            Ok(())
        })
        .unwrap();
}

#[test]
fn unreadable_and_unprepared_inputs_are_distinct_from_exhaustive_analysis() {
    let (database, mut source, _, _) = fixture();
    let evidence = EvidenceCatalog::build(std::slice::from_ref(&source), false).unwrap();
    let report = decode::initial(
        json!({"units":[],"regionDispositions":[
            {"evidenceId":"E0","disposition":"unreadable"}]}),
        &evidence,
        std::slice::from_ref(&source),
    )
    .unwrap();
    analysis_store::publish(
        &database,
        std::slice::from_ref(&source),
        &hash("unreadable"),
        &report,
    )
    .unwrap();
    let result = read(
        &database,
        &scope(&source),
        std::slice::from_ref(&source),
        ContextPolicy::Retrieval,
        ContextBudget::authoring(),
    )
    .unwrap();
    assert_eq!(result.preparation.unreadable_regions, 1);
    assert_eq!(result.preparation.legacy_coverage_sources, 0);
    assert!(result.preparation.notice().contains("not established"));
    source.text.push_str(" changed");
    source.refresh_identity().unwrap();
    database
        .with_connection(|c| repository::publish_source(c, &source, "0", 3))
        .unwrap();
    let result = read(
        &database,
        &scope(&source),
        std::slice::from_ref(&source),
        ContextPolicy::Retrieval,
        ContextBudget::authoring(),
    )
    .unwrap();
    assert_eq!(result.preparation.unprepared_sources, 1);
    assert!(result.units.is_empty());
    assert!(database
        .with_connection(|c| for_document_chunks(c, &source.owner_id, &[0]))
        .is_err());
}

#[test]
fn byte_budgets_never_clip_statements_or_hide_a_coverage_omission() {
    let (database, source, _, _) = fixture();
    let size = serde_json::to_vec(&source).unwrap().len();
    let budget = ContextBudget {
        bytes: size + 1,
        ..ContextBudget::authoring()
    };
    let result = read(
        &database,
        &scope(&source),
        std::slice::from_ref(&source),
        ContextPolicy::Retrieval,
        budget,
    )
    .unwrap();
    assert_eq!(result.sources[0].text, source.text);
    assert!(result.units.is_empty());
    assert_eq!(result.preparation.omitted_units, 1);
    assert!(read(
        &database,
        &scope(&source),
        &[source],
        ContextPolicy::Coverage,
        budget
    )
    .is_err());
}

#[test]
fn context_deduplicates_native_evidence_and_rejects_a_missing_requested_chunk() {
    let (database, source, _, _) = fixture();
    assert!(database
        .with_connection(|c| for_document_chunks(c, &source.owner_id, &[0, 99]))
        .is_err());
    let sources = [source.clone(), source.clone()];
    let result = read(
        &database,
        &scope(&source),
        &sources,
        ContextPolicy::Coverage,
        ContextBudget::authoring(),
    )
    .unwrap();
    assert_eq!(result.sources.len(), 1);
    assert_eq!(result.units.len(), 1);
    let prompt = material_prompt(&result.sources, &result.units).unwrap();
    assert_eq!(prompt["sources"].as_array().unwrap().len(), 1);
    assert_eq!(prompt["units"].as_array().unwrap().len(), 1);
}

#[test]
fn canonical_tables_enter_context_only_as_complete_admitted_blocks_and_changes_invalidate() {
    use crate::{
        document_canonical::{
            structure::{BlockStructure, TableCell, TableStructure},
            CanonicalBlock, CanonicalBlockRole, CanonicalDocument, CanonicalExtractionReport,
            CanonicalIngestion, CanonicalSourceFile, CanonicalSourceUnit, DocumentMetadata,
        },
        document_canonical_store,
        document_domain::{DocumentSourceDescriptor, SourceUnitKind},
    };
    let (database, mut source, _, _) = fixture();
    let table = TableStructure {
        rows: 1,
        columns: 1,
        cells: vec![TableCell {
            row: 0,
            column: 0,
            row_span: 1,
            column_span: 1,
            header: Some(false),
            text: "−2 ≤ v < 3".into(),
            bounds: None,
        }],
        column_headers: Some(vec!["Velocity (m/s), only while moving".into()]),
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
        BTreeMap::new(),
    );
    block.structure = Some(BlockStructure {
        table: Some(table.clone()),
        ..Default::default()
    });
    let ingestion = CanonicalIngestion {
        document: CanonicalDocument {
            schema_version: 1,
            source: DocumentSourceDescriptor::pdf(1),
            source_files: vec![CanonicalSourceFile {
                file_ordinal: 1,
                original_filename: "table.pdf".into(),
                media_type: "application/pdf".into(),
                relative_path: "documents/geometry.pdf".into(),
                byte_size: 1,
                sha256: "2".repeat(64),
            }],
            metadata: DocumentMetadata::default(),
            units: vec![CanonicalSourceUnit::new(
                1,
                SourceUnitKind::Page,
                "Page 1",
                None,
                &text,
                1,
            )],
            blocks: vec![block.clone()],
            assets: vec![],
            extraction: CanonicalExtractionReport {
                adapter_backend: "fixture".into(),
                extraction_backend: "fixture".into(),
                render_backend: None,
                extraction_mode: "native".into(),
                semantic_analysis_degraded: false,
                ocr_unit_count: 0,
                ocr_mean_confidence: None,
                ocr_engine: None,
            },
        },
        semantic_chunks: vec![],
    };
    database
        .with_connection(|c| {
            let transaction = c.transaction().map_err(CommandError::database)?;
            document_canonical_store::persist_canonical_ingestion(
                &transaction,
                &source.owner_id,
                &ingestion,
                2,
            )?;
            transaction.commit().map_err(CommandError::database)
        })
        .unwrap();
    source.text = text;
    source.refresh_identity().unwrap();
    database
        .with_connection(|c| repository::publish_source(c, &source, "0", 3))
        .unwrap();
    repository::publish_units(&database, &source, &hash("table-context"), vec![]).unwrap();
    let ctx = read(
        &database,
        &scope(&source),
        std::slice::from_ref(&source),
        ContextPolicy::Coverage,
        ContextBudget::authoring(),
    )
    .unwrap();
    assert_eq!(ctx.preparation.evidence_structures.len(), 1);
    assert_eq!(
        ctx.preparation.evidence_structures[0]["payload"]["table"],
        serde_json::to_value(table).unwrap()
    );
    block
        .structure
        .as_mut()
        .unwrap()
        .table
        .as_mut()
        .unwrap()
        .cells[0]
        .header = Some(true);
    database
        .with_connection(|c| {
            let transaction = c.transaction().map_err(CommandError::database)?;
            document_canonical_store::evidence::publish(&transaction, &source.owner_id, &[block])?;
            transaction.commit().map_err(CommandError::database)?;
            assert!(ctx.stamp.ensure_current(c).is_err());
            Ok(())
        })
        .unwrap();
}
