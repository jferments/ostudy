//! LK-05 collection organization is part of complete-library preservation.

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    collections::{self, SourceSelectionInput},
    database::Database,
    error::CommandError,
};

pub(super) fn seed(database: &Database) {
    let document = super::id(500);
    let collection = super::id(520);
    let membership = super::id(521);
    database
        .with_connection(|connection| {
            let source_sha: String = connection
                .query_row(
                    "SELECT sha256 FROM documents WHERE id=?1",
                    [&document],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO document_collections(id,parent_id,name,sort_order,created_at,updated_at,revision)
                     VALUES(?1,NULL,'Week 1',1000,10,10,1)",
                    [&collection],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO document_collection_memberships(id,collection_id,document_id,sort_order,created_at,updated_at,revision)
                     VALUES(?1,?2,?3,1000,10,10,1)",
                    params![membership, collection, document],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO document_collection_ranges(id,membership_id,unit_kind,start_unit,end_unit,source_sha256,created_at)
                     VALUES(?1,?2,'section',1,1,?3,10)",
                    params![super::id(522), membership, source_sha],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE document_collection_organization
                     SET revision=2,updated_at=10 WHERE singleton_id=1",
                    [],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    crate::knowledge::collection_view::refresh_native(database, &collection).unwrap();
    let snapshot = database
        .with_connection(|connection| {
            collections::resolve_selection(
                connection,
                SourceSelectionInput {
                    collection_ids: vec![collection.clone()],
                    documents: vec![],
                    exclusions: vec![],
                },
            )
        })
        .unwrap();
    database
        .with_connection(|connection| {
            let target = super::id(530);
            let operation = super::id(531);
            connection
                .execute(
                    "INSERT INTO study_targets(
                         id,target_kind,collection_id,output_policy_json,
                         active_snapshot_id,active_snapshot_json,created_at,updated_at
                     ) VALUES(?1,'collection',?2,?3,?4,?5,10,10)",
                    params![
                        target,
                        collection,
                        "{\"flashcards\":true,\"studyGuide\":true}",
                        snapshot.snapshot_id,
                        serde_json::to_string(&snapshot).map_err(CommandError::serialization)?
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO study_target_batches(
                         operation_id,target_id,request_sha256,source_snapshot_id,
                         source_snapshot_json,output_policy_json,provider_authorized,
                         state,safe_error_code,safe_error_message,created_at,updated_at,completed_at
                     ) VALUES(?1,?2,?3,?4,?5,?6,0,'failed',
                              'fixture_failure','Preservation fixture',10,10,10)",
                    params![
                        operation,
                        target,
                        "a".repeat(64),
                        snapshot.snapshot_id,
                        serde_json::to_string(&snapshot).map_err(CommandError::serialization)?,
                        "{\"flashcards\":true,\"studyGuide\":true}"
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO study_target_artifact_jobs(
                         batch_id,artifact_kind,state,safe_error_code,safe_error_message,
                         created_at,updated_at,completed_at
                     ) VALUES(?1,'flashcards','failed','fixture_failure',
                              'Preservation fixture',10,10,10)",
                    [operation],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO study_target_suppressions(
                         target_id,artifact_kind,operation_id,reason,created_at
                     ) VALUES(?1,'study_guide',?2,'user_deleted',10)",
                    params![target, super::id(532)],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
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
        return Ok(());
    }
    let before = connection.total_changes();
    let snapshot = collections::resolve_selection(
        connection,
        SourceSelectionInput {
            collection_ids: vec![super::id(520)],
            documents: vec![],
            exclusions: vec![],
        },
    )?;
    assert_eq!(snapshot.members.len(), 1);
    assert_eq!(snapshot.members[0].document_id, document);
    assert_eq!(snapshot.members[0].ranges.len(), 1);
    assert_eq!(snapshot.members[0].ranges[0].start_unit, 1);
    collections::ensure_snapshot_current(connection, &snapshot)?;
    let view =
        crate::knowledge::collection_view::load_in_for_preservation(connection, &super::id(520))?
            .expect("collection synthesis should be retained");
    assert!(!view.stale);
    assert_eq!(view.synthesis.snapshot_id, snapshot.snapshot_id);
    let (stored_snapshot, batch_state, suppression_count): (String, String, i64) = connection
        .query_row(
            "SELECT t.active_snapshot_json,b.state,
                    (SELECT COUNT(*) FROM study_target_suppressions s
                     WHERE s.target_id=t.id AND s.artifact_kind='study_guide')
             FROM study_targets t
             JOIN study_target_batches b ON b.target_id=t.id
             WHERE t.collection_id=?1",
            [super::id(520)],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(CommandError::database)?;
    let target_snapshot: crate::collections::SourceSetSnapshotDto =
        serde_json::from_str(&stored_snapshot).map_err(CommandError::serialization)?;
    collections::ensure_snapshot_current(connection, &target_snapshot)?;
    assert_eq!(target_snapshot.snapshot_id, snapshot.snapshot_id);
    assert_eq!(batch_state, "failed");
    assert_eq!(suppression_count, 1);
    assert_eq!(connection.total_changes(), before);
    Ok(())
}
