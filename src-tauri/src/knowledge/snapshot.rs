//! Whole-document optimistic read boundary for derived learning views. A source
//! with zero extracted units is still an input. Hash ordered, length-framed rows
//! without holding a SQLite lock across provider calls or recording source text
//! in logs. Display names and progress timestamps are deliberately excluded.
use rusqlite::Connection;
use sha2::{Digest, Sha256};

use super::repository;
use crate::{error::CommandError, util::validate_entity_id};

#[derive(Clone, Debug)]
pub(crate) struct DocumentSnapshot {
    document_id: String,
    fingerprint: String,
}

impl DocumentSnapshot {
    pub(crate) fn capture(
        connection: &Connection,
        document_id: &str,
    ) -> Result<Self, CommandError> {
        validate_entity_id(document_id, "knowledge document")?;
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM documents WHERE id = ?1)",
                [document_id],
                |r| r.get(0),
            )
            .map_err(CommandError::database)?;
        if !exists {
            return Err(repository::stale_source());
        }
        let mut digest = Sha256::new();
        digest.update(b"document-analysis-snapshot-v1");
        for query in [
            "SELECT json_array(sha256, source_format, source_unit_kind, source_unit_count,
                extraction_backend, adapter_backend, canonical_schema_version, archived_at)
             FROM documents WHERE id = ?1",
            "SELECT json_array(chunk_index, content_sha256, full_text, source_locator_json,
                visual_asset_ids_json, heading_context, canonical_schema_version)
             FROM document_semantic_chunk_sources WHERE document_id = ?1 ORDER BY chunk_index LIMIT 100001",
            "SELECT json_array(asset_id, sha256, media_type, byte_size, relative_path, role,
                source_unit_number, block_index, width, height, alt_text)
             FROM document_source_assets WHERE document_id = ?1 ORDER BY asset_id LIMIT 100001",
            "SELECT json_array(id, owner_revision, content_hash, active, extracted_hash, extraction_contract)
             FROM knowledge_sources WHERE document_id = ?1 AND kind = 'document' ORDER BY id LIMIT 100001",
            "SELECT json_array(u.id, u.source_id, u.source_hash, u.content_hash, u.extraction_contract)
             FROM knowledge_units u JOIN knowledge_sources s ON s.id = u.source_id
             WHERE s.document_id = ?1 AND s.kind = 'document' ORDER BY u.id LIMIT 1600001",
            "SELECT json_array(id, contract, state, accepted_count, rejected_count, report_json)
             FROM knowledge_analysis_windows WHERE document_id = ?1 ORDER BY id LIMIT 100001",
        ] {
            digest.update((query.len() as u64).to_le_bytes());
            digest.update(query.as_bytes());
            let mut statement = connection.prepare(query).map_err(CommandError::database)?;
            let mut rows = statement.query([document_id]).map_err(CommandError::database)?;
            let mut count = 0_usize;
            let limit = if query.contains("FROM knowledge_units") { 1_600_000 } else { 100_000 };
            while let Some(row) = rows.next().map_err(CommandError::database)? {
                count += 1;
                if count > limit {
                    return Err(super::contracts::invalid("The document analysis snapshot exceeds its supported record limit."));
                }
                let value: String = row.get(0).map_err(CommandError::database)?;
                digest.update((value.len() as u64).to_le_bytes());
                digest.update(value.as_bytes());
            }
            digest.update((count as u64).to_le_bytes());
        }
        Ok(Self {
            document_id: document_id.to_string(),
            fingerprint: format!("{:x}", digest.finalize()),
        })
    }

    pub(crate) fn identity(&self) -> &str {
        &self.fingerprint
    }

    /// Call inside the same short write transaction as derived publication.
    pub(crate) fn ensure_current(&self, connection: &Connection) -> Result<(), CommandError> {
        if Self::capture(connection, &self.document_id)?.fingerprint != self.fingerprint {
            return Err(repository::stale_source());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::{authoring::tests::fixture, contracts::hash};

    #[test]
    fn an_empty_extraction_remains_a_revision_dependency() {
        let (db, mut source, _, _) = fixture();
        repository::publish_units(&db, &source, &hash("empty"), vec![]).unwrap();
        let snapshot = db
            .with_connection(|c| DocumentSnapshot::capture(c, &source.owner_id))
            .unwrap();
        source.text.push_str(" New material");
        source.refresh_identity().unwrap();
        db.with_connection(|c| repository::publish_source(c, &source, "0", 2))
            .unwrap();
        assert_eq!(
            db.with_connection(|c| snapshot.ensure_current(c))
                .unwrap_err()
                .code,
            "semantic_source_changed"
        );
    }

    #[test]
    fn added_removed_and_reinterpreted_sources_are_detected() {
        let (db, source, _, _) = fixture();
        let snapshot = db
            .with_connection(|c| DocumentSnapshot::capture(c, &source.owner_id))
            .unwrap();
        let mut another = source.clone();
        another.segment_key = "1:0:0".into();
        another.refresh_identity().unwrap();
        db.with_connection(|c| repository::publish_source(c, &another, "1", 2))
            .unwrap();
        assert!(db.with_connection(|c| snapshot.ensure_current(c)).is_err());
        let snapshot = db
            .with_connection(|c| DocumentSnapshot::capture(c, &source.owner_id))
            .unwrap();
        db.with_connection(|c| {
            c.execute("DELETE FROM knowledge_sources WHERE id = ?1", [&another.id])
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
        assert!(db.with_connection(|c| snapshot.ensure_current(c)).is_err());
        let snapshot = db
            .with_connection(|c| DocumentSnapshot::capture(c, &source.owner_id))
            .unwrap();
        repository::publish_units(&db, &source, &hash("different interpretation"), vec![]).unwrap();
        assert!(db.with_connection(|c| snapshot.ensure_current(c)).is_err());
    }

    #[test]
    fn display_rename_and_progress_do_not_invalidate_canonical_v2_evidence() {
        let (db, mut source, _, _) = fixture();
        source.identity_version = 2;
        source.refresh_identity().unwrap();
        db.with_connection(|c| repository::publish_source(c, &source, "0", 2))
            .unwrap();
        let snapshot = db
            .with_connection(|c| DocumentSnapshot::capture(c, &source.owner_id))
            .unwrap();
        db.with_connection(|c| {
            c.execute("UPDATE documents SET title = 'Display title', updated_at = 500, revision = revision + 1 WHERE id = ?1", [&source.owner_id]).map_err(CommandError::database)?;
            snapshot.ensure_current(c)
        }).unwrap();
    }

    #[test]
    fn stale_empty_input_cannot_commit_a_derived_view() {
        let (db, source, _, _) = fixture();
        repository::publish_units(&db, &source, &hash("empty"), vec![]).unwrap();
        let snapshot = db
            .with_connection(|c| DocumentSnapshot::capture(c, &source.owner_id))
            .unwrap();
        db.with_connection(|c| {
            c.execute(
                "UPDATE knowledge_sources SET active = 0 WHERE id = ?1",
                [&source.id],
            )
            .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
        let result = db.with_connection(|c| {
            let tx = c.transaction().map_err(CommandError::database)?;
            snapshot.ensure_current(&tx)?;
            tx.execute(
                "UPDATE documents SET title = 'Stale derived title' WHERE id = ?1",
                [&source.owner_id],
            )
            .map_err(CommandError::database)?;
            tx.commit().map_err(CommandError::database)
        });
        assert!(result.is_err());
        db.with_connection(|c| {
            let title: String = c
                .query_row(
                    "SELECT title FROM documents WHERE id = ?1",
                    [&source.owner_id],
                    |r| r.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(title, "Geometry");
            Ok(())
        })
        .unwrap();
    }
}
