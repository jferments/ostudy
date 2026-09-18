//! Atomic semantic-window receipts. Successful siblings are durable before
//! repair; optimistic revisions prevent concurrent or obsolete interpretation
//! work from overwriting newer accepted results. Reuse is read-only.
use super::{
    contracts::{hash, invalid, is_hash, KnowledgeUnit, SourceKind, SourceRecord},
    extraction::decode::AnalysisReport,
    repository,
};
use crate::{database::Database, error::CommandError, util::unix_time_millis};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

pub(crate) fn window_id(sources: &[SourceRecord], contract: &str) -> Result<String, CommandError> {
    Ok(hash(
        &serde_json::to_string(&(
            "semantic-window-v2",
            contract,
            sources
                .iter()
                .map(|s| (&s.id, &s.content_hash, &s.owner_revision))
                .collect::<Vec<_>>(),
        ))
        .map_err(CommandError::serialization)?,
    )[..32]
        .to_string())
}

/// Reuse a producer receipt only if the complete ordered source window is
/// current. Consumer model changes do not replace source interpretations.
pub(crate) fn compatible_contract(
    database: &Database,
    sources: &[SourceRecord],
) -> Result<Option<String>, CommandError> {
    let Some(first) = sources.first() else {
        return Ok(None);
    };
    database.with_connection(|connection| {
        let contract: Option<String> = connection
            .query_row(
                "SELECT extraction_contract FROM knowledge_sources WHERE id=?1 AND active=1",
                [&first.id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some(contract) = contract.filter(|value| is_hash(value)) else {
            return Ok(None);
        };
        Ok(load_in(connection, sources, &contract)?.map(|_| contract))
    })
}

pub(crate) struct Checkpoint<'a> {
    database: &'a Database,
    sources: &'a [SourceRecord],
    contract: &'a str,
    revision: String,
}
impl<'a> Checkpoint<'a> {
    pub(crate) fn open(
        database: &'a Database,
        sources: &'a [SourceRecord],
        contract: &'a str,
    ) -> Result<Self, CommandError> {
        if sources.is_empty()
            || sources.len() > super::extraction::EXTRACTION_BATCH_SOURCES
            || !is_hash(contract)
            || sources.iter().map(|s| &s.id).collect::<HashSet<_>>().len() != sources.len()
        {
            return Err(invalid(
                "The semantic checkpoint scope or contract is invalid.",
            ));
        }
        let revision = database.with_connection(|c| revision_at(c, sources))?;
        Ok(Self {
            database,
            sources,
            contract,
            revision,
        })
    }
    pub(crate) fn load(&self) -> Result<Option<AnalysisReport>, CommandError> {
        self.database.with_connection(|c| {
            self.check(c)?;
            load_in(c, self.sources, self.contract)
        })
    }
    pub(crate) fn units(&self) -> Result<HashMap<String, Vec<KnowledgeUnit>>, CommandError> {
        self.database.with_connection(|c| {
            self.check(c)?;
            load_units(c, self.sources)
        })
    }
    pub(crate) fn save(
        &mut self,
        report: &AnalysisReport,
    ) -> Result<HashMap<String, Vec<KnowledgeUnit>>, CommandError> {
        let (outcome, revision) = self.database.with_connection(|c| {
            let tx = c
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(CommandError::database)?;
            self.check(&tx)?;
            publish_in(&tx, self.sources, self.contract, report)?;
            let outcome = load_units(&tx, self.sources)?;
            let revision = revision_at(&tx, self.sources)?;
            tx.commit().map_err(CommandError::database)?;
            Ok((outcome, revision))
        })?;
        self.revision = revision;
        Ok(outcome)
    }
    fn check(&self, c: &Connection) -> Result<(), CommandError> {
        if revision_at(c, self.sources)? != self.revision {
            return Err(repository::stale_source());
        }
        Ok(())
    }
}

fn revision_at(c: &Connection, sources: &[SourceRecord]) -> Result<String, CommandError> {
    for source in sources {
        source.validate()?;
        if !repository::source_is_current(c, source)? {
            return Err(repository::stale_source());
        }
    }
    let ids = serde_json::to_string(&sources.iter().map(|s| &s.id).collect::<Vec<_>>())
        .map_err(CommandError::serialization)?;
    let mut digest = Sha256::new();
    for query in [
        "SELECT json_array(id,content_hash,owner_revision,active,extracted_hash,extraction_contract)
         FROM knowledge_sources WHERE id IN (SELECT value FROM json_each(?1)) ORDER BY id LIMIT 2049",
        "SELECT json_array(id,source_id,source_hash,content_hash,extraction_contract)
         FROM knowledge_units WHERE source_id IN (SELECT value FROM json_each(?1))
            OR id IN (SELECT unit_id FROM knowledge_unit_evidence WHERE source_id IN (SELECT value FROM json_each(?1)))
         ORDER BY id LIMIT 2049",
        "SELECT json_array(id,contract,state,report_json) FROM knowledge_analysis_windows
         WHERE id IN (SELECT window_id FROM knowledge_analysis_sources WHERE source_id IN (SELECT value FROM json_each(?1)))
         ORDER BY id LIMIT 2049",
    ] {
        digest.update(query.as_bytes());
        let mut statement=c.prepare(query).map_err(CommandError::database)?;
        let mut rows=statement.query([&ids]).map_err(CommandError::database)?;
        let mut count=0u64;
        while let Some(row)=rows.next().map_err(CommandError::database)? {
            count+=1;if count>2048{return Err(invalid("The semantic checkpoint exceeds its dependency budget."));}
            let value:String=row.get(0).map_err(CommandError::database)?;
            digest.update((value.len()as u64).to_le_bytes());digest.update(value.as_bytes());
        }
        digest.update(count.to_le_bytes());
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
pub(crate) fn load(
    database: &Database,
    sources: &[SourceRecord],
    contract: &str,
) -> Result<Option<AnalysisReport>, CommandError> {
    database.with_connection(|c| load_in(c, sources, contract))
}
fn load_in(
    c: &Connection,
    sources: &[SourceRecord],
    contract: &str,
) -> Result<Option<AnalysisReport>, CommandError> {
    for source in sources {
        if !repository::source_is_current(c, source)? {
            return Err(repository::stale_source());
        }
        if !repository::extraction_is_current(c, source, contract)? {
            return Ok(None);
        }
    }
    let id = window_id(sources, contract)?;
    let serialized: Option<String> = c
        .query_row(
            "SELECT report_json FROM knowledge_analysis_windows WHERE id=?1 AND contract=?2
         AND (SELECT COUNT(*) FROM knowledge_analysis_sources WHERE window_id=?1)=?3",
            params![id, contract, sources.len() as i64],
            |r| r.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    serialized
        .map(|value| {
            let report: AnalysisReport =
                serde_json::from_str(&value).map_err(CommandError::serialization)?;
            validate_report(sources, &report)?;
            Ok(report)
        })
        .transpose()
}
fn load_units(
    c: &Connection,
    sources: &[SourceRecord],
) -> Result<HashMap<String, Vec<KnowledgeUnit>>, CommandError> {
    sources
        .iter()
        .map(|source| Ok((source.id.clone(), repository::load_units(c, &source.id)?)))
        .collect()
}

pub(crate) fn validate_report(sources: &[SourceRecord], report: &AnalysisReport) -> Result<(), CommandError> {
    if let Some(coverage) = &report.coverage {
        coverage.validate(sources, &report.accepted, &report.no_learning)?;
    }
    let mut checked = report.clone();
    checked.refresh_coverage(sources);
    if checked.missing_sources != report.missing_sources
        || checked.no_learning != report.no_learning
        || report.accepted.len() > super::extraction::decode::MAX_WINDOW_UNITS
        || report.rejected.len() > super::extraction::decode::MAX_WINDOW_UNITS
        || report.no_learning.iter().collect::<HashSet<_>>().len() != report.no_learning.len()
        || report
            .rejected
            .iter()
            .map(|r| r.slot)
            .collect::<HashSet<_>>()
            .len()
            != report.rejected.len()
        || report
            .no_learning
            .iter()
            .any(|id| !sources.iter().any(|s| &s.id == id))
    {
        return Err(invalid("The semantic-window receipt has invalid coverage."));
    }
    for accepted in &report.accepted {
        let primary = sources
            .iter()
            .find(|s| s.id == accepted.source_id)
            .ok_or_else(repository::stale_source)?;
        accepted.content.validate(primary)?;
        let grounding = accepted
            .content
            .grounding
            .as_ref()
            .ok_or_else(|| invalid("A v2 analysis unit has no native evidence."))?;
        if !grounding.is_visible_in(sources) {
            return Err(repository::stale_source());
        }
        for reference in &grounding.evidence {
            let dependency = sources
                .iter()
                .find(|s| s.id == reference.source_id)
                .ok_or_else(repository::stale_source)?;
            if dependency.kind != primary.kind
                || dependency.origin != primary.origin
                || (dependency.id != primary.id
                    && !(primary.kind == SourceKind::Document
                        && dependency.owner_id == primary.owner_id))
            {
                return Err(invalid(
                    "The semantic-window receipt mixes unrelated source authority.",
                ));
            }
        }
    }
    Ok(())
}
fn publish_in(
    c: &Connection,
    sources: &[SourceRecord],
    contract: &str,
    report: &AnalysisReport,
) -> Result<(), CommandError> {
    validate_report(sources, report)?;
    let serialized = serde_json::to_string(report).map_err(CommandError::serialization)?;
    if serialized.len() > 512000 {
        return Err(invalid(
            "The semantic-window receipt exceeds its storage budget.",
        ));
    }
    let id = window_id(sources, contract)?;
    let ids = serde_json::to_string(&sources.iter().map(|s| &s.id).collect::<Vec<_>>())
        .map_err(CommandError::serialization)?;
    // Overlapping old windows must not remain replayable after regrouping or a
    // contract change. Their untouched siblings need fresh analysis; canonical
    // material and already authored cards/guides are never deleted here.
    c.execute(
        "UPDATE knowledge_sources SET extraction_contract='',extracted_hash=''
         WHERE id NOT IN (SELECT value FROM json_each(?1)) AND id IN (
            SELECT source_id FROM knowledge_analysis_sources WHERE window_id<>?2 AND window_id IN (
                SELECT window_id FROM knowledge_analysis_sources WHERE source_id IN (SELECT value FROM json_each(?1))))",
        params![ids,id],
    ).map_err(CommandError::database)?;
    c.execute(
        "DELETE FROM knowledge_analysis_windows WHERE id<>?2 AND id IN (
            SELECT window_id FROM knowledge_analysis_sources WHERE source_id IN (SELECT value FROM json_each(?1)))",
        params![ids,id],
    ).map_err(CommandError::database)?;
    let now = unix_time_millis()?;
    for source in sources {
        let inputs = report
            .accepted
            .iter()
            .filter(|u| u.source_id == source.id)
            .map(|u| u.content.clone())
            .collect();
        repository::publish_units_in(c, source, contract, inputs, now)?;
    }
    let document_id = sources
        .first()
        .filter(|s| s.kind == SourceKind::Document)
        .and_then(|s| s.document_id.as_deref());
    c.execute(
        "INSERT INTO knowledge_analysis_windows(id,document_id,contract,state,accepted_count,rejected_count,report_json,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8)
         ON CONFLICT(id) DO UPDATE SET state=excluded.state,accepted_count=excluded.accepted_count,
            rejected_count=excluded.rejected_count,report_json=excluded.report_json,updated_at=excluded.updated_at",
        params![id,document_id,contract,if report.is_complete(){"complete"}else{"partial"},report.accepted.len()as i64,
            report.rejected_count()as i64,serialized,now],
    ).map_err(CommandError::database)?;
    for source in sources {
        c.execute("INSERT INTO knowledge_analysis_sources(window_id,source_id,source_hash) VALUES(?1,?2,?3)
            ON CONFLICT(window_id,source_id) DO UPDATE SET source_hash=excluded.source_hash",
            params![id,source.id,source.content_hash]).map_err(CommandError::database)?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn publish(
    database: &Database,
    sources: &[SourceRecord],
    contract: &str,
    report: &AnalysisReport,
) -> Result<HashMap<String, Vec<KnowledgeUnit>>, CommandError> {
    Checkpoint::open(database, sources, contract)?.save(report)
}
pub(crate) fn pending_for_sources(
    database: &Database,
    ids: &[String],
) -> Result<usize, CommandError> {
    let serialized = serde_json::to_string(ids).map_err(CommandError::serialization)?;
    database.with_connection(|c|c.query_row(
        "SELECT COALESCE(SUM(w.rejected_count),0) FROM knowledge_analysis_windows w WHERE w.state='partial'
         AND EXISTS(SELECT 1 FROM knowledge_analysis_sources d JOIN knowledge_sources s ON s.id=d.source_id
            WHERE d.window_id=w.id AND s.active=1 AND s.content_hash=d.source_hash AND s.extraction_contract=w.contract
                AND s.id IN(SELECT value FROM json_each(?1)))",
        [serialized],|r|r.get::<_,u32>(0).map(|n|n as usize),
    ).map_err(CommandError::database))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::{
        authoring::tests::fixture, evidence::EvidenceCatalog, extraction::decode,
    };
    use serde_json::json;
    fn report(sources: &[SourceRecord], ids: serde_json::Value) -> AnalysisReport {
        let catalog = EvidenceCatalog::build(sources, false).unwrap();
        decode::initial(json!({"units":[{"title":"Geometry","kind":"relationship","statement":"Triangles have three sides and three angles.",
            "qualifications":[],"topic":"Geometry","importance":"core","evidenceIds":ids}],"noLearningSourceIds":[]}),&catalog,sources).unwrap()
    }
    #[test]
    fn changing_secondary_evidence_invalidates_unit_and_resume_receipt() {
        let (db, first, _, _) = fixture();
        let mut second = first.clone();
        second.segment_key = "1:0:0".into();
        second.locator = Some(
            crate::document_domain::SourceLocator::new(
                crate::document_domain::SourceUnitKind::Page,
                vec![2],
                None,
            )
            .unwrap(),
        );
        second.text = "Triangles have three angles.".into();
        second.refresh_identity().unwrap();
        db.with_connection(|c| repository::publish_source(c, &second, "1", 2))
            .unwrap();
        let sources = [first.clone(), second.clone()];
        let contract = hash("v2-test");
        let published = publish(
            &db,
            &sources,
            &contract,
            &report(&sources, json!(["E0", "E1"])),
        )
        .unwrap();
        assert_eq!(published[&second.id].len(), 1);
        let catalog = crate::knowledge::authoring::KnowledgeCatalog {
            units: published[&second.id].clone(),
            document_core_units: Default::default(),
            preparation: Default::default(),
        };
        let support = catalog.select(&[catalog.units[0].id.clone()], 2).unwrap();
        db.with_connection(|c| {
            let locator = crate::knowledge::authoring::support_locator(
                c,
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
        let sources = [first.clone()];
        let contract = hash("stale");
        let mut checkpoint = Checkpoint::open(&db, &sources, &contract).unwrap();
        let result = report(&sources, json!(["E0"]));
        let mut changed = first;
        changed.text.push_str(" Changed");
        changed.refresh_identity().unwrap();
        db.with_connection(|c| repository::publish_source(c, &changed, "0", 3))
            .unwrap();
        assert!(checkpoint.save(&result).is_err());
        db.with_connection(|c| {
            let count: i64 = c
                .query_row("SELECT COUNT(*) FROM knowledge_analysis_windows", [], |r| {
                    r.get(0)
                })
                .map_err(CommandError::database)?;
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn concurrent_interpretation_cannot_overwrite_a_newer_checkpoint() {
        let (db, source, _, _) = fixture();
        let sources = [source];
        let contract = hash("race");
        let mut first = Checkpoint::open(&db, &sources, &contract).unwrap();
        let mut second = Checkpoint::open(&db, &sources, &contract).unwrap();
        let initial = report(&sources, json!(["E0"]));
        first.save(&initial).unwrap();
        assert!(second.save(&initial).is_err());
        let revision = first.revision.clone();
        first.units().unwrap();
        assert_eq!(first.revision, revision);
        assert!(first.load().unwrap().unwrap().is_complete());
    }
    #[test]
    fn regrouping_retires_old_receipts_without_deleting_canonical_sources() {
        let (db, first, _, _) = fixture();
        let mut second = first.clone();
        second.segment_key = "1:0:0".into();
        second.refresh_identity().unwrap();
        db.with_connection(|c| repository::publish_source(c, &second, "1", 2))
            .unwrap();
        let sources = [first.clone(), second.clone()];
        let contract = hash("regroup");
        publish(
            &db,
            &sources,
            &contract,
            &report(&sources, json!(["E0", "E1"])),
        )
        .unwrap();
        let subset = std::slice::from_ref(&first);
        publish(&db, subset, &contract, &report(subset, json!(["E0"]))).unwrap();
        assert!(load(&db, &sources, &contract).unwrap().is_none());
        db.with_connection(|c| {
            assert!(repository::load_source(c, &second.id)?.is_some());
            assert!(!repository::extraction_is_current(c, &second, &contract)?);
            Ok(())
        })
        .unwrap();
    }
}

#[cfg(test)]
mod consumer_reuse_tests {
    use super::*;
    #[test]
    fn new_consumer_model_reuses_original_receipt_but_edits_never_do() {
        let (db, source, _, _) = crate::knowledge::authoring::tests::fixture();
        let sources = [source.clone()];
        let producer = hash("producer-flash");
        let report = AnalysisReport {
            no_learning: vec![source.id.clone()],
            ..Default::default()
        };
        publish(&db, &sources, &producer, &report).unwrap();
        assert_eq!(
            compatible_contract(&db, &sources).unwrap(),
            Some(producer.clone())
        );
        db.with_connection(|c| {
            assert!(repository::extraction_available(c, &source)?);
            assert!(!repository::extraction_is_current(
                c,
                &source,
                &hash("consumer-pro")
            )?);
            Ok(())
        })
        .unwrap();
        let mut changed = source;
        changed.text.push_str(" edited");
        changed.refresh_identity().unwrap();
        db.with_connection(|c| repository::publish_source(c, &changed, "0", 3))
            .unwrap();
        assert_eq!(compatible_contract(&db, &[changed]).unwrap(), None);
    }
}
