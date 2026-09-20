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
                    "INSERT INTO document_collections(id,parent_id,name,sort_order,archived_at,created_at,updated_at,revision)
                     VALUES(?1,NULL,'Week 1',1000,NULL,10,10,1)",
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
    let view = crate::knowledge::collection_view::load_in_for_preservation(connection, &super::id(520))?
        .expect("collection synthesis should be retained");
    assert!(!view.stale);
    assert_eq!(view.synthesis.snapshot_id, snapshot.snapshot_id);
    assert_eq!(connection.total_changes(), before);
    Ok(())
}
