//! Native completion summaries describe one generation pass, not current deck
//! inventory, factual certification or learner mastery. No source text is kept.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use super::SourceGroundedGeneratedCard;
use crate::{
    authoring::card_types_and_templates::generated_import::{
        question::GeneratedQuestion, GeneratedImportCommitContext,
    },
    cards::questions::QuestionBody,
    error::CommandError,
    knowledge::assessment::policy::{GenerationPolicy, QuestionFamily},
};

mod storage;
pub(crate) use storage::{begin, load_latest, load_latest_many, record_candidates};

const MAX_COUNT: usize = 10_000_000;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerationTypeSummary {
    pub family: QuestionFamily,
    pub note_count: usize,
    pub card_count: usize,
    pub item_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerationReport {
    pub schema_version: u8,
    pub generated_at: i64,
    pub requested_policy: GenerationPolicy,
    pub candidate_note_count: usize,
    pub selected_note_count: usize,
    pub curation_removed_count: usize,
    pub saved_note_count: usize,
    pub saved_card_count: usize,
    pub skipped_duplicate_count: usize,
    pub available_knowledge_units: usize,
    pub linked_knowledge_units: usize,
    pub evidence_binding_count: usize,
    pub item_count: usize,
    pub types: Vec<GenerationTypeSummary>,
}

impl GenerationReport {
    fn validate(&self) -> Result<(), CommandError> {
        self.requested_policy.validate()?;
        let counts = [
            self.candidate_note_count,
            self.selected_note_count,
            self.curation_removed_count,
            self.saved_note_count,
            self.saved_card_count,
            self.skipped_duplicate_count,
            self.available_knowledge_units,
            self.linked_knowledge_units,
            self.evidence_binding_count,
            self.item_count,
        ];
        if self.schema_version != 1
            || self.generated_at < 0
            || counts.iter().any(|count| *count > MAX_COUNT)
            || self.types.len() > 7
            || self.types.iter().any(|row| {
                row.note_count == 0
                    || row.note_count > row.card_count
                    || row.card_count > row.item_count
                    || row.item_count > MAX_COUNT
                    || !self.requested_policy.allowed_types.contains(&row.family)
            })
        {
            return Err(invalid());
        }
        let families = self
            .types
            .iter()
            .map(|row| row.family)
            .collect::<BTreeSet<_>>();
        if families.len() != self.types.len()
            || self.selected_note_count + self.curation_removed_count != self.candidate_note_count
            || self.saved_note_count + self.skipped_duplicate_count != self.selected_note_count
            || self.linked_knowledge_units > self.available_knowledge_units
            || self.types.iter().map(|row| row.note_count).sum::<usize>() != self.saved_note_count
            || self.types.iter().map(|row| row.card_count).sum::<usize>() != self.saved_card_count
            || self.types.iter().map(|row| row.item_count).sum::<usize>() != self.item_count
        {
            return Err(invalid());
        }
        Ok(())
    }
}

/// The caller already owns the canonical authoring transaction. Source-index
/// receipts identify only notes actually inserted, after duplicate filtering.
pub(crate) fn publish(
    connection: &Connection,
    document_id: &str,
    job_id: &str,
    cards: &[SourceGroundedGeneratedCard],
    committed: &GeneratedImportCommitContext,
    now: i64,
) -> Result<(), CommandError> {
    let Some(pending) = storage::pending(connection, document_id, job_id)? else {
        // Previously completed jobs and legacy callers have no captured inputs.
        // Never manufacture their requested policy or retrospective coverage.
        return Ok(());
    };
    let candidate_count = pending.candidate_count.ok_or_else(invalid)?;
    if cards.len() > candidate_count || committed.created_notes.len() > cards.len() {
        return Err(invalid());
    }
    let mut types = BTreeMap::<QuestionFamily, GenerationTypeSummary>::new();
    let mut indices = BTreeSet::new();
    let mut units = BTreeSet::new();
    let mut bindings = 0;
    for created in &committed.created_notes {
        if !indices.insert(created.source_index) {
            return Err(invalid());
        }
        let card = cards.get(created.source_index).ok_or_else(invalid)?;
        let card_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM cards WHERE note_id = ?1 AND deleted_at IS NULL",
                [&created.note_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        let card_count = usize::try_from(card_count).map_err(|_| invalid())?;
        if card_count == 0 {
            return Err(invalid());
        }
        let (family, items) = workload(card, card_count)?;
        let entry = types.entry(family).or_insert(GenerationTypeSummary {
            family,
            note_count: 0,
            card_count: 0,
            item_count: 0,
        });
        entry.note_count += 1;
        entry.card_count += card_count;
        entry.item_count += items;
        units.extend(card.knowledge_support.iter().map(|unit| &unit.unit_id));
        bindings += card
            .assessment
            .as_ref()
            .map_or(card.knowledge_support.len(), |record| {
                record
                    .components
                    .iter()
                    .map(|component| component.units.len())
                    .sum()
            });
    }
    let report = GenerationReport {
        schema_version: 1,
        generated_at: now,
        requested_policy: pending.policy,
        candidate_note_count: candidate_count,
        selected_note_count: cards.len(),
        curation_removed_count: candidate_count - cards.len(),
        saved_note_count: committed.created_notes.len(),
        saved_card_count: types.values().map(|row| row.card_count).sum(),
        skipped_duplicate_count: cards.len() - committed.created_notes.len(),
        available_knowledge_units: pending.available_knowledge_units,
        linked_knowledge_units: units.len(),
        evidence_binding_count: bindings,
        item_count: types.values().map(|row| row.item_count).sum(),
        types: types.into_values().collect(),
    };
    report.validate()?;
    storage::complete(connection, document_id, job_id, &report)
}

fn workload(
    card: &SourceGroundedGeneratedCard,
    card_count: usize,
) -> Result<(QuestionFamily, usize), CommandError> {
    if card.card["type"] != "interactive" {
        return match card.card["type"].as_str() {
            Some("basic" | "cloze") => Ok((QuestionFamily::Recall, card_count)),
            _ => Err(invalid()),
        };
    }
    if card_count != 1 {
        return Err(invalid());
    }
    let question: GeneratedQuestion = serde_json::from_value(card.card["question"].clone())
        .map_err(CommandError::serialization)?;
    let question = question.admit()?;
    let items = match &question.definition().question {
        QuestionBody::SingleChoice { options, .. } | QuestionBody::SelectAll { options, .. } => {
            options.len()
        }
        QuestionBody::TrueFalse { .. } => 1,
        QuestionBody::Matching { pairs, .. } => pairs.len(),
        QuestionBody::Grouping { items, .. } => items.len(),
        QuestionBody::Ordering { items, .. } => items.len(),
        QuestionBody::ImageOcclusion { .. } => return Err(invalid()),
    };
    Ok((QuestionFamily::from_kind(question.kind().as_str())?, items))
}

fn invalid() -> CommandError {
    CommandError::new(
        "generation_report_invalid",
        "The generation summary does not match its captured inputs or committed cards.",
    )
}

#[cfg(test)]
mod tests;
