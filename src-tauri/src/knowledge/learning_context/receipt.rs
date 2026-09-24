use super::*;
use sha2::{Digest, Sha256};

mod source_range;

const MAX_ROWS: usize = 16384;

pub(super) fn fingerprint(
    connection: &Connection,
    sources: &[SourceRecord],
) -> Result<String, CommandError> {
    let ids = serde_json::to_string(
        &sources
            .iter()
            .map(|source| &source.id)
            .collect::<BTreeSet<_>>(),
    )
    .map_err(CommandError::serialization)?;
    let mut digest = Sha256::new();
    digest.update(b"learning-context-v1");
    for query in [
        "SELECT json_array(id,owner_revision,content_hash,active,extracted_hash,extraction_contract)
         FROM knowledge_sources WHERE id IN(SELECT value FROM json_each(?1)) ORDER BY id LIMIT 16385",
        "SELECT json_array(id,source_id,source_hash,content_hash,extraction_contract,record_json)
         FROM knowledge_units WHERE source_id IN(SELECT value FROM json_each(?1))
         OR id IN(SELECT unit_id FROM knowledge_unit_evidence WHERE source_id IN(SELECT value FROM json_each(?1)))
         ORDER BY id LIMIT 16385",
        "SELECT json_array(id,contract,state,report_json) FROM knowledge_analysis_windows
         WHERE id IN(SELECT window_id FROM knowledge_analysis_sources WHERE source_id IN(SELECT value FROM json_each(?1)))
         ORDER BY id LIMIT 16385",
    ] {
        digest.update(query.as_bytes());
        let mut statement = connection.prepare(query).map_err(CommandError::database)?;
        let mut rows = statement.query([&ids]).map_err(CommandError::database)?;
        let mut count = 0;
        while let Some(row) = rows.next().map_err(CommandError::database)? {
            count += 1;
            if count > MAX_ROWS {
                return Err(invalid(
                    "The learning-context dependency budget was exceeded.",
                ));
            }
            let value: String = row.get(0).map_err(CommandError::database)?;
            digest.update((value.len() as u64).to_le_bytes());
            digest.update(value.as_bytes());
        }
        digest.update((count as u64).to_le_bytes());
    }
    // Captures structured evidence even when its plain-text projection did not
    // change. Bound to admitted literal blocks; no display titles participate.
    for value in structures(connection, sources, 16 * 1024 * 1024)? {
        let bytes = serde_json::to_vec(&value).map_err(CommandError::serialization)?;
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub(super) fn summarize(
    connection: &Connection,
    sources: &[SourceRecord],
) -> Result<PreparationSummary, CommandError> {
    let ids = sources
        .iter()
        .filter(|source| source.kind != SourceKind::Conversation)
        .map(|source| source.id.clone())
        .collect::<HashSet<_>>();
    let mut summary = PreparationSummary::default();
    let mut described = HashSet::new();
    for source in sources
        .iter()
        .filter(|source| source.kind != SourceKind::Conversation)
    {
        if !repository::extraction_available(connection, source)? {
            summary.unprepared_sources += 1;
        }
    }
    let serialized = serde_json::to_string(&ids).map_err(CommandError::serialization)?;
    let mut statement = connection
        .prepare(
            "SELECT w.report_json FROM knowledge_analysis_windows w WHERE
         EXISTS(SELECT 1 FROM knowledge_analysis_sources d WHERE d.window_id=w.id
           AND d.source_id IN(SELECT value FROM json_each(?1)))
         AND NOT EXISTS(SELECT 1 FROM knowledge_analysis_sources d
           LEFT JOIN knowledge_sources s ON s.id=d.source_id
           WHERE d.window_id=w.id AND (s.id IS NULL OR s.active<>1 OR s.content_hash<>d.source_hash
             OR s.extraction_contract<>w.contract)) ORDER BY w.id LIMIT 16385",
        )
        .map_err(CommandError::database)?;
    let mut rows = statement
        .query([serialized])
        .map_err(CommandError::database)?;
    let mut count = 0;
    while let Some(row) = rows.next().map_err(CommandError::database)? {
        count += 1;
        if count > MAX_ROWS {
            return Err(invalid("The coverage-summary budget was exceeded."));
        }
        let text: String = row.get(0).map_err(CommandError::database)?;
        if text.len() > 512000 {
            return Err(invalid("The retained coverage receipt exceeds its limit."));
        }
        let report: super::super::extraction::decode::AnalysisReport =
            serde_json::from_str(&text).map_err(CommandError::serialization)?;
        summary.rejected_proposals += report.rejected.len();
        if let Some(coverage) = &report.coverage {
            let value = serde_json::to_value(coverage).map_err(CommandError::serialization)?;
            if let Some(regions) = value.get("regions").and_then(Value::as_array) {
                let mut dependency_ids = HashSet::new();
                let mut dependencies = Vec::new();
                for region in regions {
                    let id = region
                        .get("sourceId")
                        .and_then(Value::as_str)
                        .ok_or_else(|| invalid("A coverage region has no source identity."))?;
                    if dependency_ids.insert(id.to_string()) {
                        dependencies.push(
                            repository::load_source(connection, id)?
                                .ok_or_else(repository::stale_source)?,
                        );
                    }
                }
                super::super::analysis_store::validate_report(&dependencies, &report)?;
                for region in regions {
                    let Some(id) = region.get("sourceId").and_then(Value::as_str) else {
                        continue;
                    };
                    if !ids.contains(id) {
                        continue;
                    }
                    described.insert(id.to_string());
                    match region.get("state").and_then(Value::as_str) {
                        Some("unresolved") => summary.unresolved_regions += 1,
                        Some("unreadable") => summary.unreadable_regions += 1,
                        Some("interpreted" | "retained_metadata" | "no_learning") => {}
                        _ => return Err(invalid("A retained region has an invalid disposition.")),
                    }
                }
            }
        }
    }
    for source in sources
        .iter()
        .filter(|source| source.kind != SourceKind::Conversation)
    {
        if !described.contains(&source.id) && repository::extraction_available(connection, source)?
        {
            summary.legacy_coverage_sources += 1;
        }
    }
    Ok(summary)
}

pub(super) fn structures(
    connection: &Connection,
    sources: &[SourceRecord],
    budget: usize,
) -> Result<Vec<Value>, CommandError> {
    let mut result = Vec::new();
    let mut used = 0;
    let mut seen = HashSet::new();
    for source in sources
        .iter()
        .filter(|source| source.kind == SourceKind::Document)
    {
        let Some(locator) = &source.locator else {
            continue;
        };
        let numbers =
            serde_json::to_string(&locator.unit_numbers).map_err(CommandError::serialization)?;
        let mut statement = connection
            .prepare(
                "SELECT o.id,o.payload_sha256,p.payload_json,o.location_json,b.text,
             (SELECT json_group_array(json_object('kind',l.kind,'targetNode',l.target_node,
               'targetOccurrenceId',l.target_occurrence_id)) FROM document_evidence_links l WHERE l.occurrence_id=o.id)
             FROM document_evidence_occurrences o JOIN document_evidence_payloads p ON p.sha256=o.payload_sha256
             JOIN document_source_blocks b ON b.document_id=o.document_id AND b.block_index=o.block_index
             WHERE o.document_id=?1 AND o.source_unit_number IN(SELECT value FROM json_each(?2))
             ORDER BY o.id LIMIT 16385",
            )
            .map_err(CommandError::database)?;
        let mut rows = statement
            .query(params![source.document_id, numbers])
            .map_err(CommandError::database)?;
        let mut count = 0;
        while let Some(row) = rows.next().map_err(CommandError::database)? {
            count += 1;
            if count > MAX_ROWS {
                return Err(invalid("The evidence occurrence budget was exceeded."));
            }
            let block_text: String = row.get(4).map_err(CommandError::database)?;
            if block_text.is_empty() || !source.text.contains(&block_text) {
                continue;
            }
            let id: String = row.get(0).map_err(CommandError::database)?;
            if !seen.insert(id.clone()) {
                continue;
            }
            let sha: String = row.get(1).map_err(CommandError::database)?;
            let text: String = row.get(2).map_err(CommandError::database)?;
            if hash(&text) != sha {
                return Err(invalid(
                    "The canonical evidence payload hash is inconsistent.",
                ));
            }
            let payload: Value =
                serde_json::from_str(&text).map_err(CommandError::serialization)?;
            if payload.get("version").and_then(Value::as_u64) != Some(1) {
                return Err(invalid(
                    "The canonical evidence payload version is unsupported.",
                ));
            }
            if let Some(table) = payload.get("table").filter(|value| !value.is_null()) {
                let table: crate::document_canonical::structure::TableStructure =
                    serde_json::from_value(table.clone()).map_err(CommandError::serialization)?;
                table.validate()?;
            }
            let location: String = row.get(3).map_err(CommandError::database)?;
            let location: Value =
                serde_json::from_str(&location).map_err(CommandError::serialization)?;
            let links: String = row.get(5).map_err(CommandError::database)?;
            let links: Value = serde_json::from_str(&links).map_err(CommandError::serialization)?;
            // Native, revision-bound block locations are planning metadata, not
            // model-authored evidence. Repeated text has no unique range.
            let source_range = source_range::unique(&source.text, &block_text).map(
                |(start, end)| json!({"version":1,"sourceHash":source.content_hash,
                    "startByte":start,"endByte":end}),
            );
            let value = json!({"sourceId":source.id,"occurrenceId":id,"payloadHash":sha,
                "payload":payload,"location":location,"links":links,
                "sourceRange":source_range});
            let size = serde_json::to_vec(&value)
                .map_err(CommandError::serialization)?
                .len();
            if size > budget.saturating_sub(used) {
                continue;
            }
            used += size;
            result.push(value);
        }
    }
    Ok(result)
}
