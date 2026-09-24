use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension};

use super::{invalid, GenerationPolicy, GenerationReport, MAX_COUNT};
use crate::{database::Database, error::CommandError, util::unix_time_millis};

pub(super) struct PendingReport {
    pub policy: GenerationPolicy,
    pub candidate_count: Option<usize>,
    pub available_knowledge_units: usize,
}

pub(crate) fn begin(
    database: &Database,
    document_id: &str,
    job_id: &str,
    policy: &GenerationPolicy,
    available_knowledge_units: usize,
) -> Result<(), CommandError> {
    policy.validate()?;
    if available_knowledge_units > MAX_COUNT {
        return Err(invalid());
    }
    let encoded = serde_json::to_string(policy).map_err(CommandError::serialization)?;
    database.with_connection(|connection| {
        connection
            .execute(
                "INSERT INTO question_generation_reports
                    (job_id, document_id, policy_json, available_knowledge_units, created_at)
                 SELECT id, document_id, ?3, ?4, ?5 FROM document_generation_jobs
                 WHERE id = ?1 AND document_id = ?2
                 ON CONFLICT(job_id) DO NOTHING",
                params![
                    job_id,
                    document_id,
                    encoded,
                    i64::try_from(available_knowledge_units).map_err(|_| invalid())?,
                    unix_time_millis()?
                ],
            )
            .map_err(CommandError::database)?;
        let stored = pending(connection, document_id, job_id)?.ok_or_else(invalid)?;
        if stored.policy.fingerprint()? != policy.fingerprint()?
            || stored.available_knowledge_units != available_knowledge_units
        {
            return Err(invalid());
        }
        Ok(())
    })
}

pub(crate) fn record_candidates(
    database: &Database,
    job_id: &str,
    count: usize,
) -> Result<(), CommandError> {
    if count > 10_000 {
        return Err(invalid());
    }
    let count = i64::try_from(count).map_err(|_| invalid())?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE question_generation_reports SET candidate_count = ?1
                 WHERE job_id = ?2 AND report_json IS NULL
                   AND (candidate_count IS NULL OR candidate_count = ?1)",
                params![count, job_id],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(invalid());
        }
        Ok(())
    })
}

pub(super) fn pending(
    connection: &Connection,
    document_id: &str,
    job_id: &str,
) -> Result<Option<PendingReport>, CommandError> {
    let row: Option<(String, i64, Option<i64>)> = connection
        .query_row(
            "SELECT policy_json, available_knowledge_units, candidate_count
             FROM question_generation_reports
             WHERE document_id = ?1 AND job_id = ?2 AND report_json IS NULL",
            params![document_id, job_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    row.map(|(policy, available_knowledge_units, candidate_count)| {
        Ok(PendingReport {
            policy: serde_json::from_str(&policy).map_err(CommandError::serialization)?,
            available_knowledge_units: usize::try_from(available_knowledge_units)
                .map_err(|_| invalid())?,
            candidate_count: candidate_count
                .map(|count| usize::try_from(count).map_err(|_| invalid()))
                .transpose()?,
        })
    })
    .transpose()
}

pub(super) fn complete(
    connection: &Connection,
    document_id: &str,
    job_id: &str,
    report: &GenerationReport,
) -> Result<(), CommandError> {
    let json = serde_json::to_string(report).map_err(CommandError::serialization)?;
    let changed = connection
        .execute(
            "UPDATE question_generation_reports SET report_json = ?1, created_at = ?2
             WHERE document_id = ?3 AND job_id = ?4 AND report_json IS NULL",
            params![json, report.generated_at, document_id, job_id],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn load_for_job(
    connection: &Connection,
    document_id: &str,
    job_id: &str,
) -> Result<Option<GenerationReport>, CommandError> {
    let json: Option<String> = connection
        .query_row(
            "SELECT report.report_json
             FROM question_generation_reports report
             JOIN document_generation_jobs job ON job.id = report.job_id
             WHERE report.document_id = ?1 AND report.job_id = ?2
               AND report.report_json IS NOT NULL AND job.state = 'succeeded'",
            params![document_id, job_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    json.map(|json| {
        if json.len() > 65_536 {
            return Err(invalid());
        }
        let report: GenerationReport =
            serde_json::from_str(&json).map_err(|_| invalid())?;
        report.validate()?;
        Ok(report)
    })
    .transpose()
}

pub(crate) fn load_latest(
    connection: &Connection,
    document_id: &str,
) -> Result<Option<GenerationReport>, CommandError> {
    Ok(load_latest_many(connection, &[document_id])?.remove(document_id))
}

/// One bounded query for a library page, not a new query for each document row.
pub(crate) fn load_latest_many(
    connection: &Connection,
    document_ids: &[&str],
) -> Result<BTreeMap<String, GenerationReport>, CommandError> {
    if document_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let ids = serde_json::to_string(document_ids).map_err(CommandError::serialization)?;
    let mut statement = connection
        .prepare(
            "SELECT requested.value, report.report_json FROM json_each(?1) requested
             JOIN question_generation_reports report ON report.job_id = (
                 SELECT candidate.job_id FROM question_generation_reports candidate
                 JOIN document_generation_jobs job ON job.id = candidate.job_id
                 WHERE candidate.document_id = requested.value
                   AND candidate.report_json IS NOT NULL AND job.state = 'succeeded'
                 ORDER BY candidate.created_at DESC, candidate.job_id DESC LIMIT 1)",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([ids], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?;
    let mut reports = BTreeMap::new();
    for row in rows {
        let (document_id, json) = row.map_err(CommandError::database)?;
        if json.len() > 65_536 {
            return Err(invalid());
        }
        let report: GenerationReport = serde_json::from_str(&json).map_err(|_| invalid())?;
        report.validate()?;
        reports.insert(document_id, report);
    }
    Ok(reports)
}
