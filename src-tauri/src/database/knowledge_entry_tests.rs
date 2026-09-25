use super::*;
use crate::knowledge::{
    analysis_store::Checkpoint, authoring::tests::fixture, contracts::hash,
    evidence::EvidenceCatalog, extraction::decode,
};
use serde_json::json;

#[test]
fn current_and_legacy_knowledge_admission_is_read_only() {
    let (database, source, _, _) = fixture();
    database
        .with_connection(|connection| {
            let changes = connection.total_changes();
            validate(connection).unwrap();
            crate::database::validate_backup_lineage(connection).unwrap();
            assert_eq!(connection.total_changes(), changes);
            Ok(())
        })
        .unwrap();
    let sources = [source];
    let evidence = EvidenceCatalog::build(&sources, false).unwrap();
    let report = decode::initial(
        json!({"units":[],"noLearningSourceIds":[sources[0].id]}),
        &evidence,
        &sources,
    )
    .unwrap();
    Checkpoint::open(&database, &sources, &hash("entry-coverage"))
        .unwrap()
        .save(&report)
        .unwrap();
    database
        .with_connection(|connection| {
            let changes = connection.total_changes();
            validate(connection).unwrap();
            crate::database::validate_backup_lineage(connection).unwrap();
            assert_eq!(connection.total_changes(), changes);
            Ok(())
        })
        .unwrap();
}

#[test]
fn future_typed_content_and_forged_coverage_fail_before_conversion_writes() {
    let (database, source, _, _) = fixture();
    let sources = [source];
    let evidence = EvidenceCatalog::build(&sources, false).unwrap();
    let report = decode::initial(
        json!({"units":[{"title":"Triangle","kind":"fact","statement":"Three sides.",
            "qualifications":[],"topic":"Geometry","importance":"core","evidenceIds":["E0"]}]}),
        &evidence,
        &sources,
    )
    .unwrap();
    Checkpoint::open(&database, &sources, &hash("entry-corruption"))
        .unwrap()
        .save(&report)
        .unwrap();
    database
        .with_connection(|connection| {
            for corruption in [
                "UPDATE knowledge_units SET record_json=json_set(record_json,'$.content.details',json('{\"version\":999,\"shape\":{\"kind\":\"process\",\"steps\":[],\"dependencies\":[]}}'))",
                "UPDATE knowledge_analysis_windows SET report_json=json_set(report_json,'$.coverage.version',999)",
                "UPDATE knowledge_analysis_windows SET report_json=json_set(report_json,'$.coverage.planHash','forged')",
                "UPDATE knowledge_analysis_windows SET report_json=json_set(report_json,'$.coverage.regions[0].state','unreadable')",
                "UPDATE knowledge_analysis_windows SET report_json=json_set(report_json,'$.coverage.regions[0].sourceId','unknown')",
            ] {
                let transaction = connection.transaction().unwrap();
                transaction.execute_batch(corruption).unwrap();
                let changes = transaction.total_changes();
                assert!(crate::database::validate_backup_lineage(&transaction).is_err());
                assert_eq!(transaction.total_changes(), changes);
                transaction.rollback().unwrap();
            }
            validate(connection).unwrap();
            Ok(())
        })
        .unwrap();
}
