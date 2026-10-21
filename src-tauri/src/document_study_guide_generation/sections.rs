//! A guide's sections follow its key ideas, not the learning plan's windows.
//!
//! The planner groups consecutive learning topics; here each group becomes one
//! guide topic that the rest of generation treats like any other: its id is
//! its first member's (a real learning topic, so storage, notes and links keep
//! working), and its evidence, source pages and knowledge are its members'
//! together. The shared learning plan itself never changes. Without a plan,
//! every topic stays its own section, which is the guide as it was before.

use std::collections::BTreeSet;

use crate::{
    ai::document_intelligence::{validate_section_plan, GuideTopicBrief, PlannedGuideSection},
    database::Database,
    document_domain::{SourceLocator, TimeRangeMs},
    document_learning::{DocumentLearningPlan, LearningTopic},
    error::CommandError,
    knowledge::authoring::KnowledgeCatalog,
    util::unix_time_millis,
};

use super::{hash_text, StudyGuideTopicEvidence};

const MAX_LOCATOR_ASSETS: usize = 256;
const MAX_MERGED_SUMMARY_CHARS: usize = 1_200;
const MAX_MERGED_LIST: usize = 12;

/// Fewer topics than this are their own sections without asking the model.
pub(super) const MIN_TOPICS_TO_PLAN: usize = 3;

pub(super) fn contract_key(topic_contract: &str) -> String {
    hash_text(&format!("guide-section-plan-v1|{topic_contract}"))
}

/// Every topic its own section, named as the plan names it.
pub(super) fn identity(plan: &DocumentLearningPlan) -> Vec<PlannedGuideSection> {
    plan.topics
        .iter()
        .enumerate()
        .map(|(index, topic)| PlannedGuideSection {
            first_topic: index,
            title: topic.title.clone(),
            key_ideas: Vec::new(),
        })
        .collect()
}

pub(super) fn is_identity(plan: &DocumentLearningPlan, sections: &[PlannedGuideSection]) -> bool {
    sections == identity(plan).as_slice()
}

/// What the planner sees of each topic: its own words, plus the document's
/// core knowledge statements first, then the rest in catalog order.
pub(super) fn briefs<'a>(
    plan: &'a DocumentLearningPlan,
    evidence: &'a [StudyGuideTopicEvidence],
    catalogs: &'a [KnowledgeCatalog],
) -> Vec<GuideTopicBrief<'a>> {
    plan.topics
        .iter()
        .zip(evidence)
        .zip(catalogs)
        .map(|((topic, evidence), catalog)| {
            let heading = topic
                .parent_topic_id
                .as_deref()
                .and_then(|parent| plan.topics.iter().find(|other| other.id == parent))
                .map(|parent| parent.title.as_str())
                .or(topic.source_locator.section_label.as_deref());
            let (core, other): (Vec<_>, Vec<_>) = catalog
                .units
                .iter()
                .partition(|unit| catalog.document_core_units.contains(&unit.id));
            GuideTopicBrief {
                title: &topic.title,
                heading,
                summary: &topic.summary,
                key_concepts: &topic.key_concepts,
                first_unit: topic.source_locator.first(),
                last_unit: topic.source_locator.last(),
                source_words: evidence.source_text.split_whitespace().count(),
                core_statements: core
                    .into_iter()
                    .chain(other)
                    .map(|unit| unit.content.statement.as_str())
                    .collect(),
            }
        })
        .collect()
}

/// The member topics of each section, as plan index ranges.
pub(super) fn ranges(
    sections: &[PlannedGuideSection],
    topic_count: usize,
) -> Vec<std::ops::Range<usize>> {
    sections
        .iter()
        .enumerate()
        .map(|(index, section)| {
            let end = sections
                .get(index + 1)
                .map_or(topic_count, |next| next.first_topic);
            section.first_topic..end
        })
        .collect()
}

/// The plan the guide is written from: one topic per section. An identity
/// plan is the learning plan itself, so earlier checkpoints still apply.
pub(super) fn guide_plan(
    plan: &DocumentLearningPlan,
    sections: &[PlannedGuideSection],
) -> Result<DocumentLearningPlan, CommandError> {
    validate_section_plan(sections, plan.topics.len())?;
    if is_identity(plan, sections) {
        return Ok(plan.clone());
    }
    let topics = sections
        .iter()
        .zip(ranges(sections, plan.topics.len()))
        .map(|(section, range)| merge_topics(&plan.topics[range], section))
        .collect::<Result<Vec<_>, CommandError>>()?;
    let identity = topics
        .iter()
        .map(|topic| format!("{}:{}", topic.id, topic.evidence_hash))
        .collect::<Vec<_>>()
        .join(",");
    Ok(DocumentLearningPlan {
        plan_fingerprint: hash_text(&format!(
            "guide-sections-v1|{}|{identity}",
            plan.plan_fingerprint
        )),
        topics,
        ..plan.clone()
    })
}

fn merge_topics(
    members: &[LearningTopic],
    section: &PlannedGuideSection,
) -> Result<LearningTopic, CommandError> {
    let first = members.first().ok_or_else(|| {
        CommandError::new(
            "document_study_guide_invalid",
            "A planned guide section has no learning topics.",
        )
    })?;
    let summary = members
        .iter()
        .map(|topic| topic.summary.trim())
        .filter(|summary| !summary.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let summary = if summary.chars().count() > MAX_MERGED_SUMMARY_CHARS {
        summary.chars().take(MAX_MERGED_SUMMARY_CHARS).collect()
    } else {
        summary
    };
    let mut evidence_chunk_indices = members
        .iter()
        .flat_map(|topic| topic.evidence_chunk_indices.iter().copied())
        .collect::<Vec<_>>();
    evidence_chunk_indices.sort_unstable();
    evidence_chunk_indices.dedup();
    let evidence_hashes = members
        .iter()
        .flat_map(|topic| topic.evidence_hashes.iter().cloned())
        .collect::<Vec<_>>();
    let key_concepts = if section.key_ideas.is_empty() {
        distinct(members.iter().flat_map(|topic| &topic.key_concepts))
    } else {
        section.key_ideas.clone()
    };
    let evidence_hash = hash_text(&format!(
        "guide-section-v1|{}|{}|{}",
        members
            .iter()
            .map(|topic| topic.evidence_hash.as_str())
            .collect::<Vec<_>>()
            .join(","),
        section.title,
        section.key_ideas.join("\u{1f}"),
    ));
    Ok(LearningTopic {
        id: first.id.clone(),
        parent_topic_id: first.parent_topic_id.clone(),
        sort_index: first.sort_index,
        title: section.title.clone(),
        summary,
        learning_objectives: distinct(members.iter().flat_map(|topic| &topic.learning_objectives)),
        key_concepts,
        source_locator: merged_locator(members)?,
        evidence_chunk_indices,
        evidence_hashes,
        evidence_hash,
        anchor_hash: first.anchor_hash.clone(),
    })
}

fn distinct<'a>(values: impl Iterator<Item = &'a String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .filter(|value| seen.insert(value.as_str()))
        .take(MAX_MERGED_LIST)
        .cloned()
        .collect()
}

fn merged_locator(members: &[LearningTopic]) -> Result<SourceLocator, CommandError> {
    let first = &members[0].source_locator;
    let units = members
        .iter()
        .flat_map(|topic| topic.source_locator.unit_numbers.iter().copied())
        .collect();
    let assets = members
        .iter()
        .flat_map(|topic| topic.source_locator.asset_ids.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(MAX_LOCATOR_ASSETS)
        .collect();
    let ranges = members
        .iter()
        .map(|topic| topic.source_locator.time_range_ms)
        .collect::<Option<Vec<_>>>();
    let time_range = ranges.and_then(|ranges| {
        Some(TimeRangeMs {
            start: ranges.iter().map(|range| range.start).min()?,
            end: ranges.iter().map(|range| range.end).max()?,
        })
    });
    SourceLocator::with_evidence(
        first.unit_kind,
        units,
        first.section_label.clone(),
        assets,
        time_range,
    )
}

/// Each section's source text in reading order, and its members' page images
/// up to the most any one topic was given.
pub(super) fn guide_evidence(
    guide: &DocumentLearningPlan,
    sections: &[PlannedGuideSection],
    evidence: Vec<StudyGuideTopicEvidence>,
) -> Vec<StudyGuideTopicEvidence> {
    if guide.topics.len() == evidence.len() {
        return evidence;
    }
    let maximum_images = evidence
        .iter()
        .map(|topic| topic.images.len())
        .max()
        .unwrap_or(0);
    ranges(sections, evidence.len())
        .into_iter()
        .zip(&guide.topics)
        .map(|(range, topic)| {
            let members = &evidence[range];
            StudyGuideTopicEvidence {
                topic_id: topic.id.clone(),
                source_text: members
                    .iter()
                    .map(|member| member.source_text.as_str())
                    .filter(|text| !text.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n"),
                images: members
                    .iter()
                    .flat_map(|member| member.images.iter().cloned())
                    .take(maximum_images)
                    .collect(),
            }
        })
        .collect()
}

fn plan_evidence_hash(plan: &DocumentLearningPlan) -> String {
    hash_text(&format!(
        "guide-section-plan-evidence-v1|{}",
        plan.topics
            .iter()
            .map(|topic| format!("{}:{}", topic.id, topic.evidence_hash))
            .collect::<Vec<_>>()
            .join(",")
    ))
}

/// A section plan already paid for, so a resumed or remade guide keeps its
/// sections and their written checkpoints.
pub(super) fn load(
    database: &Database,
    document_id: &str,
    contract: &str,
    plan: &DocumentLearningPlan,
) -> Option<Vec<PlannedGuideSection>> {
    let first = plan.topics.first()?;
    let row = database
        .with_connection(|connection| {
            use rusqlite::OptionalExtension;
            connection
                .query_row(
                    "SELECT evidence_hash, section_json, content_hash
                     FROM document_study_guide_checkpoints
                     WHERE document_id = ?1 AND contract_key = ?2 AND topic_id = ?3",
                    rusqlite::params![document_id, contract, first.id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()
                .map_err(CommandError::database)
        })
        .ok()??;
    let (evidence_hash, json, content_hash) = row;
    if evidence_hash != plan_evidence_hash(plan) || hash_text(&json) != content_hash {
        return None;
    }
    let sections = serde_json::from_str::<Vec<PlannedGuideSection>>(&json).ok()?;
    validate_section_plan(&sections, plan.topics.len())
        .ok()
        .map(|()| sections)
}

pub(super) fn persist_best_effort(
    database: &Database,
    document_id: &str,
    contract: &str,
    plan: &DocumentLearningPlan,
    sections: &[PlannedGuideSection],
) {
    let (Some(first), Ok(json), Ok(now)) = (
        plan.topics.first(),
        serde_json::to_string(sections),
        unix_time_millis(),
    ) else {
        return;
    };
    let result = database.with_connection(|connection| {
        connection
            .execute(
                "INSERT INTO document_study_guide_checkpoints (
                     document_id, contract_key, topic_id, evidence_hash,
                     section_json, content_hash, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                 ON CONFLICT(document_id, contract_key, topic_id) DO UPDATE SET
                     evidence_hash = excluded.evidence_hash,
                     section_json = excluded.section_json,
                     content_hash = excluded.content_hash,
                     updated_at = excluded.updated_at",
                rusqlite::params![
                    document_id,
                    contract,
                    first.id,
                    plan_evidence_hash(plan),
                    json,
                    hash_text(&json),
                    now,
                ],
            )
            .map_err(CommandError::database)
    });
    if let Err(error) = result {
        crate::logging::debug_target(
            "ai.document.study_guide",
            "study_guide_section_plan_checkpoint_failed",
            "Could not save the guide's section plan; no content was logged.",
            serde_json::json!({"code": error.code}),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_domain::SourceUnitKind;

    fn topic(index: usize) -> LearningTopic {
        LearningTopic {
            id: format!("{index:032}"),
            parent_topic_id: None,
            sort_index: index,
            title: format!("Topic {index}"),
            summary: format!("Summary {index}."),
            learning_objectives: vec!["Explain gas exchange".to_string()],
            key_concepts: vec![format!("Concept {index}")],
            source_locator: SourceLocator::new(
                SourceUnitKind::Page,
                vec![index as i64 + 1, index as i64 + 2],
                None,
            )
            .unwrap(),
            evidence_chunk_indices: vec![index * 2, index * 2 + 1],
            evidence_hashes: vec![format!("{index:064}")],
            evidence_hash: format!("{index:064}"),
            anchor_hash: format!("{index:064}"),
        }
    }

    fn plan(count: usize) -> DocumentLearningPlan {
        DocumentLearningPlan {
            id: "a".repeat(32),
            document_id: "b".repeat(32),
            revision: 1,
            compiler_version: crate::document_learning::LEARNING_PLAN_COMPILER_VERSION,
            outline_fingerprint: "c".repeat(64),
            plan_fingerprint: "d".repeat(64),
            summary: "Breathing.".to_string(),
            learning_objectives: Vec::new(),
            key_concepts: Vec::new(),
            topics: (0..count).map(topic).collect(),
        }
    }

    fn section(first_topic: usize, title: &str) -> PlannedGuideSection {
        PlannedGuideSection {
            first_topic,
            title: title.to_string(),
            key_ideas: vec![format!("{title} matters.")],
        }
    }

    fn evidence(plan: &DocumentLearningPlan) -> Vec<StudyGuideTopicEvidence> {
        plan.topics
            .iter()
            .enumerate()
            .map(|(index, topic)| StudyGuideTopicEvidence {
                topic_id: topic.id.clone(),
                source_text: format!("Text {index}"),
                images: Vec::new(),
            })
            .collect()
    }

    #[test]
    fn an_identity_plan_is_the_learning_plan_itself() {
        let plan = plan(3);
        let guide = guide_plan(&plan, &identity(&plan)).unwrap();
        assert_eq!(guide.plan_fingerprint, plan.plan_fingerprint);
        assert_eq!(guide.topics.len(), 3);
        assert_eq!(guide.topics[1].evidence_hash, plan.topics[1].evidence_hash);
    }

    #[test]
    fn grouped_sections_cover_every_topic_once_under_their_first_topic() {
        let plan = plan(5);
        let sections = [section(0, "Airways"), section(2, "Gas exchange")];
        let guide = guide_plan(&plan, &sections).unwrap();
        assert_ne!(guide.plan_fingerprint, plan.plan_fingerprint);
        assert_eq!(guide.id, plan.id);
        assert_eq!(guide.topics.len(), 2);
        assert_eq!(guide.topics[0].id, plan.topics[0].id);
        assert_eq!(guide.topics[1].id, plan.topics[2].id);
        assert_eq!(guide.topics[1].title, "Gas exchange");
        assert_eq!(guide.topics[1].key_concepts, ["Gas exchange matters."]);
        assert_eq!(guide.topics[1].evidence_chunk_indices, [4, 5, 6, 7, 8, 9]);
        assert_eq!(guide.topics[1].source_locator.unit_numbers, [3, 4, 5, 6]);
        assert_eq!(guide.topics[0].summary, "Summary 0. Summary 1.");

        let merged = guide_evidence(&guide, &sections, evidence(&plan));
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[1].topic_id, plan.topics[2].id);
        assert_eq!(merged[1].source_text, "Text 2\n\nText 3\n\nText 4");
    }

    #[test]
    fn a_renamed_single_topic_section_gets_a_new_identity() {
        let plan = plan(3);
        let sections = [section(0, "A"), section(1, "B"), section(2, "C")];
        let guide = guide_plan(&plan, &sections).unwrap();
        assert_eq!(guide.topics.len(), 3);
        assert_ne!(guide.topics[0].evidence_hash, plan.topics[0].evidence_hash);
        assert_ne!(guide.plan_fingerprint, plan.plan_fingerprint);
    }

    #[test]
    fn an_invalid_plan_is_refused() {
        let plan = plan(3);
        assert!(guide_plan(&plan, &[section(1, "A")]).is_err());
        assert!(guide_plan(&plan, &[section(0, "A"), section(3, "B")]).is_err());
    }

    fn stored_plan() -> (Database, DocumentLearningPlan) {
        use crate::document_learning::{
            ensure_document_learning_plan, DocumentLearningPlanInput, LearningEvidenceChunk,
            LearningTopicInput,
        };
        let database = Database::open_in_memory().unwrap();
        let document_id = "1".repeat(32);
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO documents (
                            id, title, original_filename, mime_type, byte_size, sha256,
                            source_relative_path, lifecycle_state, created_at, updated_at
                        ) VALUES (?1, 'Lungs', 'lungs.pdf', 'application/pdf', 1,
                                  ?2, 'documents/lungs/source.pdf', 'ready', 1, 1)",
                        rusqlite::params![document_id, "2".repeat(64)],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();
        let locators = [1, 2, 3]
            .map(|page| SourceLocator::new(SourceUnitKind::Page, vec![page], None).unwrap());
        let plan = ensure_document_learning_plan(
            &database,
            &document_id,
            DocumentLearningPlanInput {
                summary: "Breathing.".into(),
                learning_objectives: vec![],
                key_concepts: vec![],
                topics: locators
                    .iter()
                    .enumerate()
                    .map(|(index, locator)| LearningTopicInput {
                        parent_index: None,
                        title: format!("Topic {index}"),
                        summary: format!("Summary {index}."),
                        learning_objectives: vec![],
                        key_concepts: vec![],
                        source_locator: locator.clone(),
                    })
                    .collect(),
            },
            &locators
                .iter()
                .enumerate()
                .map(|(index, locator)| LearningEvidenceChunk {
                    chunk_index: index,
                    source_locator: locator.clone(),
                    content_hash: format!("{index:064x}"),
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        (database, plan)
    }

    #[test]
    fn section_plans_are_reused_only_for_the_same_topics() {
        let (database, plan) = stored_plan();
        let sections = vec![section(0, "Airways"), section(2, "Gas exchange")];
        let key = contract_key(&"e".repeat(64));
        persist_best_effort(&database, &plan.document_id, &key, &plan, &sections);
        assert_eq!(
            load(&database, &plan.document_id, &key, &plan),
            Some(sections)
        );
        let mut changed = plan.clone();
        changed.topics[2].evidence_hash = "f".repeat(64);
        assert_eq!(load(&database, &plan.document_id, &key, &changed), None);
        assert_eq!(
            load(
                &database,
                &plan.document_id,
                &contract_key(&"0".repeat(64)),
                &plan
            ),
            None
        );
    }
}
