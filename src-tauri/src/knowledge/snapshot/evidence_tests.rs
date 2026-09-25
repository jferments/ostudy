//! Canonical payload and occurrence changes are publication dependencies even
//! when their text projection, semantic units and consumer model are unchanged.
use super::*;
use crate::knowledge::{authoring::tests::fixture, contracts::hash};
use rusqlite::params;
use serde_json::json;

#[test]
fn structure_only_changes_and_occurrence_links_invalidate_document_publication() {
    let (database, source, _, _) = fixture();
    database
        .with_connection(|connection| {
            let empty = DocumentSnapshot::capture(connection, &source.owner_id)?;
            let payload = |header| {
                json!({"version":1,"role":"table","text":null,"table":{
                    "rows":1,"columns":1,"columnHeaders":["Velocity (m/s)"],
                    "cells":[{"row":0,"column":0,"rowSpan":1,"columnSpan":1,
                        "header":header,"text":"−2 ≤ v < 3","bounds":null}]
                }})
                .to_string()
            };
            let original = payload(false);
            connection
                .execute(
                    "INSERT INTO document_evidence_payloads(sha256,payload_json) VALUES(?1,?2)",
                    params![hash(&original), original],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO document_evidence_occurrences(id,document_id,block_index,
                     source_file_sha256,source_unit_number,payload_sha256,location_json,identity_state)
                     VALUES(?1,?2,NULL,?3,1,?4,'{}','exact')",
                    params!["a".repeat(32), source.owner_id, "2".repeat(64), hash(&original)],
                )
                .map_err(CommandError::database)?;
            assert!(empty.ensure_current(connection).is_err());
            let before = DocumentSnapshot::capture(connection, &source.owner_id)?;
            let changes = connection.total_changes();
            before.ensure_current(connection)?;
            assert_eq!(connection.total_changes(), changes);

            let successor = payload(true);
            connection
                .execute(
                    "INSERT INTO document_evidence_payloads(sha256,payload_json) VALUES(?1,?2)",
                    params![hash(&successor), successor],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE document_evidence_occurrences SET payload_sha256=?1 WHERE id=?2",
                    params![hash(&successor), "a".repeat(32)],
                )
                .map_err(CommandError::database)?;
            assert!(repository::source_is_current(connection, &source)?);
            assert!(before.ensure_current(connection).is_err());

            let before = DocumentSnapshot::capture(connection, &source.owner_id)?;
            connection
                .execute(
                    "INSERT INTO document_evidence_links(occurrence_id,target_node,kind)
                     VALUES(?1,99,'footnote')",
                    ["a".repeat(32)],
                )
                .map_err(CommandError::database)?;
            assert!(before.ensure_current(connection).is_err());
            let before = DocumentSnapshot::capture(connection, &source.owner_id)?;
            connection
                .execute(
                    "DELETE FROM document_evidence_links WHERE occurrence_id=?1",
                    ["a".repeat(32)],
                )
                .map_err(CommandError::database)?;
            assert!(before.ensure_current(connection).is_err());
            let before = DocumentSnapshot::capture(connection, &source.owner_id)?;
            connection
                .execute(
                    "DELETE FROM document_evidence_occurrences WHERE id=?1",
                    ["a".repeat(32)],
                )
                .map_err(CommandError::database)?;
            assert!(before.ensure_current(connection).is_err());
            Ok(())
        })
        .unwrap();
}
