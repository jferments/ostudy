//! Admit LK-03 payloads before migrations, repairs, or journal-mode changes.
//! Older unstructured knowledge remains valid. No missing structure is inferred.
use std::collections::HashSet;

use rusqlite::Connection;

use crate::knowledge::{
    analysis_store, contracts::KnowledgeUnit, extraction::decode::AnalysisReport, repository,
};

pub(super) fn validate(connection: &Connection) -> Result<(), String> {
    for (table, column) in [
        ("knowledge_units", "record_json"),
        ("knowledge_analysis_windows", "report_json"),
    ] {
        let mut statement = connection
            .prepare(&format!("SELECT {column} FROM {table}"))
            .map_err(|_| damaged())?;
        let mut rows = statement.query([]).map_err(|_| damaged())?;
        while let Some(row) = rows.next().map_err(|_| damaged())? {
            let text: String = row.get(0).map_err(|_| damaged())?;
            if text.len() > 512_000 {
                return Err(damaged());
            }
            if table == "knowledge_units" {
                let unit: KnowledgeUnit = serde_json::from_str(&text).map_err(|_| damaged())?;
                if let Some(details) = &unit.content.details {
                    details
                        .validate(&unit.content.statement, unit.content.kind)
                        .map_err(|_| damaged())?;
                }
            } else {
                let report: AnalysisReport = serde_json::from_str(&text).map_err(|_| damaged())?;
                for unit in &report.accepted {
                    if let Some(details) = &unit.content.details {
                        details
                            .validate(&unit.content.statement, unit.content.kind)
                            .map_err(|_| damaged())?;
                    }
                }
                if let Some(coverage) = &report.coverage {
                    let value = serde_json::to_value(coverage).map_err(|_| damaged())?;
                    let regions = value["regions"].as_array().ok_or_else(damaged)?;
                    let mut seen = HashSet::new();
                    let mut sources = Vec::new();
                    for region in regions {
                        let id = region["sourceId"].as_str().ok_or_else(damaged)?;
                        if seen.insert(id) {
                            sources.push(
                                repository::load_retained_source(connection, id)
                                    .map_err(|_| damaged())?
                                    .ok_or_else(damaged)?,
                            );
                        }
                    }
                    analysis_store::validate_report(&sources, &report).map_err(|_| damaged())?;
                }
            }
        }
    }
    Ok(())
}

fn damaged() -> String {
    "Stored knowledge or coverage has unsupported or corrupt content. The database was not modified."
        .to_string()
}

#[cfg(test)]
#[path = "knowledge_entry_tests.rs"]
mod current_format_tests;
