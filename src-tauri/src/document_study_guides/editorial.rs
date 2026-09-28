//! Guide-only editorial organization over immutable learning-topic identities.
//!
//! The shared learning plan still owns source coverage and flashcard links.
//! This versioned projection changes reading order/headings, never those owners.
//! It lives as structured metadata in the guide's existing overview JSON, not
//! as hidden markup in prose or a new interpretation of the original document.
use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::StudyGuideOverview;
use crate::error::CommandError;

pub(crate) const EDITORIAL_SCHEMA_VERSION: i64 = 3;
const MAX_CHAPTERS: usize = 64;
const MAX_TOPIC_COUNT: usize = 512;
const MAX_METADATA_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StudyGuideEditorial {
    pub title: String,
    pub chapters: Vec<StudyGuideChapter>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StudyGuideChapter {
    pub id: String,
    pub title: String,
    pub introduction: String,
    pub topics: Vec<StudyGuideTopicHeading>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StudyGuideTopicHeading {
    pub topic_id: String,
    pub title: String,
}

fn invalid(message: &str) -> CommandError {
    CommandError::new("study_guide_editorial_invalid", message)
}

/// A chapter identity depends on native membership, not an LLM-generated ID or
/// changing capitalization. Order remains an explicit editorial property.
pub(crate) fn chapter_id(topics: &[StudyGuideTopicHeading]) -> String {
    let mut ids = topics
        .iter()
        .map(|topic| topic.topic_id.as_str())
        .collect::<Vec<_>>();
    ids.sort_unstable();
    let mut hash = Sha256::new();
    hash.update(b"ostudy-guide-chapter-v1\0");
    for id in ids {
        hash.update(id.as_bytes());
        hash.update([0]);
    }
    format!("{:x}", hash.finalize())[..32].to_string()
}

/// Capitalize an all-lowercase opening word without damaging pH, eGFR, drug
/// identifiers, chemical symbols or already-correct acronyms. The author still
/// owns choosing a meaningful title; this is not a topic-naming heuristic.
pub(crate) fn capitalize_heading(value: &str) -> String {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let first_word = value.split_whitespace().next().unwrap_or_default();
    if first_word.chars().any(char::is_uppercase) {
        return value;
    }
    let Some((index, first)) = value.char_indices().find(|(_, ch)| ch.is_alphabetic()) else {
        return value;
    };
    format!(
        "{}{}{}",
        &value[..index],
        first.to_uppercase(),
        &value[index + first.len_utf8()..]
    )
}

pub(crate) fn validate<'a>(
    editorial: &StudyGuideEditorial,
    expected_topics: impl IntoIterator<Item = &'a str>,
) -> Result<(), CommandError> {
    bounded_text(
        &editorial.title,
        200,
        "The study-guide title is missing or too long.",
    )?;
    if editorial.chapters.is_empty() || editorial.chapters.len() > MAX_CHAPTERS {
        return Err(invalid(
            "The study guide must contain bounded, nonempty major chapters.",
        ));
    }
    let expected = expected_topics.into_iter().collect::<Vec<_>>();
    let expected_set = expected.iter().copied().collect::<HashSet<_>>();
    if expected.is_empty()
        || expected.len() > MAX_TOPIC_COUNT
        || expected.len() != expected_set.len()
    {
        return Err(invalid(
            "The source learning plan has invalid or duplicate topic identities.",
        ));
    }
    let mut covered = HashSet::new();
    let mut chapter_ids = HashSet::new();
    for chapter in &editorial.chapters {
        bounded_text(&chapter.title, 200, "A major chapter needs a bounded heading.")?;
        bounded_text(
            &chapter.introduction,
            4_000,
            "A major chapter needs a bounded narrative introduction.",
        )?;
        if chapter.topics.is_empty() || chapter.topics.len() > MAX_TOPIC_COUNT {
            return Err(invalid(
                "A major chapter has no topics or exceeds the topic limit.",
            ));
        }
        if chapter.id != chapter_id(&chapter.topics)
            || !chapter_ids.insert(chapter.id.as_str())
        {
            return Err(invalid(
                "A major chapter has an invalid or duplicate native identity.",
            ));
        }
        for topic in &chapter.topics {
            bounded_text(
                &topic.title,
                200,
                "A study-guide subsection needs a bounded heading.",
            )?;
            if !expected_set.contains(topic.topic_id.as_str())
                || !covered.insert(topic.topic_id.as_str())
            {
                return Err(invalid(
                    "Guide organization references an unknown or repeated learning topic.",
                ));
            }
        }
    }
    if covered != expected_set {
        return Err(invalid(
            "Guide organization must retain every learning topic exactly once.",
        ));
    }
    if serde_json::to_vec(editorial)
        .map_err(CommandError::serialization)?
        .len()
        > MAX_METADATA_BYTES
    {
        return Err(invalid(
            "The study-guide organization exceeds its bounded metadata size.",
        ));
    }
    Ok(())
}

fn bounded_text(value: &str, maximum: usize, message: &str) -> Result<(), CommandError> {
    if value.trim().is_empty() || value.chars().count() > maximum || value.contains('\0') {
        Err(invalid(message))
    } else {
        Ok(())
    }
}

/// Preserve the legacy overview fields byte-for-byte when no editorial metadata
/// is present. A new document guide owns the additional versioned field; the
/// shared legacy overview DTO remains usable by historical/manual publishers.
pub(super) fn encode_overview(
    overview: &StudyGuideOverview,
    editorial: Option<&StudyGuideEditorial>,
) -> Result<String, CommandError> {
    if editorial.is_none() {
        return serde_json::to_string(overview).map_err(CommandError::serialization);
    }
    let mut value = serde_json::to_value(overview).map_err(CommandError::serialization)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid("The guide overview is not an object."))?;
    object.insert(
        "editorial".to_string(),
        serde_json::to_value(editorial).map_err(CommandError::serialization)?,
    );
    serde_json::to_string(&value).map_err(CommandError::serialization)
}

pub(super) fn decode_overview(
    text: &str,
    schema_version: i64,
) -> Result<(StudyGuideOverview, Option<StudyGuideEditorial>), CommandError> {
    if !(1..=EDITORIAL_SCHEMA_VERSION).contains(&schema_version) {
        return Err(invalid("This study guide uses an unsupported content schema."));
    }
    let mut value: Value = serde_json::from_str(text).map_err(CommandError::serialization)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid("The guide overview is not an object."))?;
    let editorial = object.remove("editorial");
    if (schema_version == EDITORIAL_SCHEMA_VERSION) != editorial.is_some() {
        return Err(invalid(
            "The guide organization does not match its content schema version.",
        ));
    }
    let editorial = editorial
        .map(serde_json::from_value)
        .transpose()
        .map_err(CommandError::serialization)?;
    // The old DTO still rejects every unknown residual field. Removing only the
    // single versioned extension does not make arbitrary JSON acceptable.
    let overview = serde_json::from_value(value).map_err(CommandError::serialization)?;
    Ok((overview, editorial))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> StudyGuideEditorial {
        let topics = vec![
            StudyGuideTopicHeading {
                topic_id: "a".repeat(32),
                title: "Foundations".into(),
            },
            StudyGuideTopicHeading {
                topic_id: "b".repeat(32),
                title: "Applications".into(),
            },
        ];
        StudyGuideEditorial {
            title: "A coherent subject overview".into(),
            chapters: vec![StudyGuideChapter {
                id: chapter_id(&topics),
                title: "From principle to application".into(),
                introduction: "The source first introduces a principle, then shows its qualified application.".into(),
                topics,
            }],
        }
    }

    #[test]
    fn editorial_grouping_preserves_exact_coverage_and_native_topic_identity() {
        let value = fixture();
        let expected = ["a".repeat(32), "b".repeat(32)];
        assert!(validate(&value, expected.iter().map(String::as_str)).is_ok());
        for case in ["missing", "duplicate", "invented", "wrong_identity"] {
            let mut changed = value.clone();
            match case {
                "missing" => {
                    changed.chapters[0].topics.pop();
                }
                "duplicate" => {
                    let duplicate = changed.chapters[0].topics[0].clone();
                    changed.chapters[0].topics.push(duplicate);
                }
                "invented" => {
                    changed.chapters[0].topics[0].topic_id = "c".repeat(32);
                }
                _ => {}
            }
            changed.chapters[0].id = if case == "wrong_identity" {
                "d".repeat(32)
            } else {
                chapter_id(&changed.chapters[0].topics)
            };
            assert!(
                validate(&changed, expected.iter().map(String::as_str)).is_err(),
                "{case}"
            );
        }
    }

    #[test]
    fn legacy_overview_bytes_and_strict_admission_remain_compatible() {
        let legacy = StudyGuideOverview {
            summary: "Original retained prose.".into(),
            learning_objectives: vec![],
            key_concepts: vec![],
        };
        let bytes = serde_json::to_string(&legacy).unwrap();
        assert_eq!(encode_overview(&legacy, None).unwrap(), bytes);
        let (decoded, editorial) = decode_overview(&bytes, 2).unwrap();
        assert_eq!(decoded.summary, legacy.summary);
        assert!(editorial.is_none());
        assert!(decode_overview(&bytes, 3).is_err());
        let mut unknown: Value = serde_json::from_str(&bytes).unwrap();
        unknown["unrecognized"] = Value::Bool(true);
        assert!(decode_overview(&unknown.to_string(), 2).is_err());
    }

    #[test]
    fn new_overview_round_trips_prose_without_the_legacy_inventory_rewrite() {
        let summary = "This introduction connects the central principles. It preserves the source's restrictions.\n\nThe second paragraph explains how the major chapters fit together.";
        let overview = StudyGuideOverview {
            summary: summary.into(),
            learning_objectives: vec![],
            key_concepts: vec![],
        };
        let bytes = encode_overview(&overview, Some(&fixture())).unwrap();
        let (decoded, layout) = decode_overview(&bytes, 3).unwrap();
        assert_eq!(decoded.summary, summary);
        assert_eq!(layout.unwrap().chapters[0].topics.len(), 2);
        assert!(decode_overview(&bytes, 2).is_err());
        assert!(decode_overview(&bytes, 999).is_err());
    }

    #[test]
    fn capitalization_does_not_rewrite_scientific_case() {
        assert_eq!(capitalize_heading("cardiovascular drugs"), "Cardiovascular drugs");
        assert_eq!(capitalize_heading("pH regulation"), "pH regulation");
        assert_eq!(capitalize_heading("ACE inhibitors"), "ACE inhibitors");
        assert_eq!(capitalize_heading("énergie et travail"), "Énergie et travail");
    }
}
