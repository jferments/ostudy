//! Atomic semantic-window receipts and evidence dependency publication. A partial
//! receipt contains validated units plus bounded rejected drafts for explicit
//! retry; it is never represented as complete coverage.

use std::collections::HashMap;

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::{database::Database, error::CommandError, util::unix_time_millis};

use super::{
    contracts::{hash, invalid, KnowledgeUnit, SourceKind, SourceRecord},
    extraction::decode::AnalysisReport,
    repository,
};

pub(crate) fn window_id(sources: &[SourceRecord], contract: &str) -> Result<String, CommandError> {
    Ok(hash(
        &serde_json::to_string(&(
            "semantic-window-v2",
            contract,
            sources
                .iter()
                .map(|source| (&source.id, &source.content_hash, &source.owner_revision))
                .collect::<Vec<_>>(),
        ))
        .map_err(CommandError::serialization)?,
    )[..32]
        .to_string())
}

pub(crate) fn load(
    database: &Database,
    sources: &[SourceRecord],
    contract: &str,
) -> Result<Option<AnalysisReport>, CommandError> {
    let id = window_id(sources, contract)?;
    database.with_connection(|connection| {
        for source in sources {
            if !repository::source_is_current(connection, source)? {
                return Err(repository::stale_source());
            }
        }
        let serialized: Option<String> = connection.query_row(
            "SELECT report_json FROM knowledge_analysis_windows WHERE id = ?1 AND contract = ?2",
            params![id, contract], |row| row.get(0),
        ).optional().map_err(CommandError::database)?;
        serialized
            .map(|value| serde_json::from_str(&value).map_err(CommandError::serialization))
            .transpose()
    })
}

pub(crate) fn publish(
    database: &Database,
    sources: &[SourceRecord],
    contract: &str,
    report: &AnalysisReport,
) -> Result<HashMap<String, Vec<KnowledgeUnit>>, CommandError> {
    let id = window_id(sources, contract)?;
    let mut checked_report = report.clone();
    checked_report.refresh_coverage(sources);
    if checked_report.missing_sources != report.missing_sources
        || checked_report.no_learning != report.no_learning
        || report.accepted.len() > super::extraction::decode::MAX_WINDOW_UNITS
        || report.rejected.len() > super::extraction::decode::MAX_WINDOW_UNITS
        || report
            .no_learning
            .iter()
            .any(|id| !sources.iter().any(|source| &source.id == id))
    {
        return Err(invalid("The semantic-window receipt has invalid coverage."));
    }
    let serialized = serde_json::to_string(report).map_err(CommandError::serialization)?;
    if serialized.len() > 512_000 {
        return Err(invalid(
            "The semantic-window receipt exceeds its storage budget.",
        ));
    }
    let document_id = sources
        .first()
        .filter(|source| source.kind == SourceKind::Document)
        .and_then(|source| source.document_id.as_deref());
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(CommandError::database)?;
        for source in sources {
            if !repository::source_is_current(&transaction, source)? { return Err(repository::stale_source()); }
        }
        // Every proposed dependency must be in this exact admitted source set.
        // Deserializing a stored/replayed receipt cannot broaden its authority.
        for accepted in &report.accepted {
            let primary = sources.iter().find(|source| source.id == accepted.source_id).ok_or_else(repository::stale_source)?;
            accepted.content.validate(primary)?;
            let grounding = accepted.content.grounding.as_ref().ok_or_else(|| invalid("A v2 analysis unit has no native evidence."))?;
            if !grounding.is_visible_in(sources) { return Err(repository::stale_source()); }
            for reference in &grounding.evidence {
                let dependency = sources.iter().find(|source| source.id == reference.source_id)
                    .ok_or_else(repository::stale_source)?;
                if dependency.kind != primary.kind || dependency.origin != primary.origin
                    || (dependency.id != primary.id && !(primary.kind == SourceKind::Document && dependency.owner_id == primary.owner_id))
                { return Err(invalid("The semantic-window receipt mixes unrelated source authority.")); }
            }
        }
        for source in sources {
            let inputs = report.accepted.iter().filter(|unit| unit.source_id == source.id)
                .map(|unit| unit.content.clone()).collect();
            repository::publish_units_in(&transaction, source, contract, inputs, now)?;
        }
        transaction.execute(
            "INSERT INTO knowledge_analysis_windows (id, document_id, contract, state, accepted_count, rejected_count, report_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
             ON CONFLICT(id) DO UPDATE SET state = excluded.state, accepted_count = excluded.accepted_count,
                 rejected_count = excluded.rejected_count, report_json = excluded.report_json, updated_at = excluded.updated_at",
            params![id, document_id, contract, if report.is_complete() { "complete" } else { "partial" },
                report.accepted.len() as i64, report.rejected_count() as i64, serialized, now],
        ).map_err(CommandError::database)?;
        for source in sources {
            transaction.execute(
                "INSERT INTO knowledge_analysis_sources(window_id, source_id, source_hash) VALUES (?1, ?2, ?3)
                 ON CONFLICT(window_id, source_id) DO UPDATE SET source_hash = excluded.source_hash",
                params![id, source.id, source.content_hash],
            ).map_err(CommandError::database)?;
        }
        let mut result = HashMap::new();
        for source in sources { result.insert(source.id.clone(), repository::load_units(&transaction, &source.id)?); }
        transaction.commit().map_err(CommandError::database)?;
        Ok(result)
    })
}

pub(crate) fn pending_for_sources(
    database: &Database,
    ids: &[String],
) -> Result<usize, CommandError> {
    let serialized = serde_json::to_string(ids).map_err(CommandError::serialization)?;
    database.with_connection(|connection| {
        connection.query_row(
            "SELECT COALESCE(SUM(w.rejected_count), 0) FROM knowledge_analysis_windows w WHERE w.state = 'partial'
             AND EXISTS(SELECT 1 FROM knowledge_analysis_sources d
                JOIN knowledge_sources s ON s.id = d.source_id
                WHERE d.window_id = w.id AND s.active = 1 AND s.content_hash = d.source_hash
                  AND s.extraction_contract = w.contract
                  AND s.id IN (SELECT value FROM json_each(?1)))",
            [serialized], |row| row.get::<_, u32>(0).map(|count| count as usize),
        ).map_err(CommandError::database)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::{
        authoring::tests::fixture, evidence::EvidenceCatalog, extraction::decode,
    };
    use serde_json::json;

    fn cross_page_report(sources: &[SourceRecord]) -> AnalysisReport {
        let catalog = EvidenceCatalog::build(sources, false).unwrap();
        decode::initial(json!({"units":[{"title":"Geometry","kind":"relationship",
            "statement":"Triangles have three sides and three angles.","qualifications":[],"topic":"Geometry",
            "importance":"core","evidenceIds":["E0","E1"]}],"noLearningSourceIds":[]}), &catalog, sources).unwrap()
    }

    #[test]
    fn changing_secondary_evidence_invalidates_unit_and_resume_receipt() {
        let (db, first, _, _) = fixture();
        let mut second = first.clone();
        second.segment_key = "1:0:0".to_string();
        second.locator = Some(
            crate::document_domain::SourceLocator::new(
                crate::document_domain::SourceUnitKind::Page,
                vec![2],
                None,
            )
            .unwrap(),
        );
        second.text = "Triangles have three angles.".to_string();
        second.refresh_identity().unwrap();
        db.with_connection(|c| repository::publish_source(c, &second, "1", 2))
            .unwrap();
        let sources = [first.clone(), second.clone()];
        let contract = hash("v2-test");
        let report = cross_page_report(&sources);
        let published = publish(&db, &sources, &contract, &report).unwrap();
        assert_eq!(published[&second.id].len(), 1);
        let catalog = crate::knowledge::authoring::KnowledgeCatalog {
            units: published[&second.id].clone(),
            document_core_units: Default::default(),
        };
        let support = catalog.select(&[catalog.units[0].id.clone()], 2).unwrap();
        db.with_connection(|connection| {
            let locator = crate::knowledge::authoring::support_locator(
                connection,
                &support,
                first.locator.as_ref().unwrap(),
            )?;
            assert_eq!(locator.unit_numbers, vec![1, 2]);
            assert!(!catalog.units[0]
                .content
                .grounding
                .as_ref()
                .unwrap()
                .is_visible_in(std::slice::from_ref(&first)));
            Ok(())
        })
        .unwrap();
        assert!(load(&db, &sources, &contract)
            .unwrap()
            .unwrap()
            .is_complete());
        second.text.push_str(" edited");
        second.refresh_identity().unwrap();
        db.with_connection(|c| repository::publish_source(c, &second, "1", 3))
            .unwrap();
        db.with_connection(|c| {
            assert!(repository::load_units(c, &first.id)?.is_empty());
            assert!(!repository::extraction_is_current(c, &first, &contract)?);
            Ok(())
        })
        .unwrap();
        assert!(load(&db, &[first, second], &contract).unwrap().is_none());
    }

    #[test]
    fn stale_publication_is_atomic_and_cannot_claim_complete_coverage() {
        let (db, first, _, _) = fixture();
        let mut second = first.clone();
        second.segment_key = "1:0:0".to_string();
        second.refresh_identity().unwrap();
        db.with_connection(|c| repository::publish_source(c, &second, "1", 2))
            .unwrap();
        let sources = [first, second.clone()];
        let report = cross_page_report(&sources);
        second.text.push_str(" changed");
        second.refresh_identity().unwrap();
        db.with_connection(|c| repository::publish_source(c, &second, "1", 3))
            .unwrap();
        assert!(publish(&db, &sources, &hash("new-contract"), &report).is_err());
        db.with_connection(|c| {
            let count: i64 = c
                .query_row(
                    "SELECT COUNT(*) FROM knowledge_analysis_windows",
                    [],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
    }
}
