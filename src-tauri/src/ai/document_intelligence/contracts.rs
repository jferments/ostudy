use std::{cmp::Reverse, collections::HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::CommandError;

use super::support::{bounded_diagnostic, output_error};

pub(super) const MAX_WINDOW_CARDS: usize = 16;
pub(super) const MAX_CARD_TEXT_CHARS: usize = 50_000;
pub(super) const MAX_TAGS_PER_CARD: usize = 32;
pub(super) const MAX_TAG_CHARS: usize = 64;
pub(super) const MAX_CLOZE_BLANKS: usize = 4;
pub(super) const MAX_CLOZE_BLANK_CHARS: usize = 160;
pub(super) const MAX_OUTLINE_OBJECTIVES: usize = 96;
pub(super) const MAX_OUTLINE_CONCEPTS: usize = 160;
pub(super) const MAX_OUTLINE_SECTIONS: usize = 160;
pub(super) const MAX_REQUEST_JSON_BYTES: usize = 64 * 1024 * 1024;
pub(super) const MAX_COMPLETION_BYTES: usize = 2 * 1024 * 1024;
pub(super) const MAX_CURATE_CARDS_PER_BATCH: usize = 32;
pub(super) const MAX_INLINE_IMAGES: usize = 6;
pub(super) const MAX_FRAGMENT_OBJECTIVES: usize = 4;
pub(super) const MAX_FRAGMENT_CONCEPTS: usize = 8;
pub(super) const MAX_FRAGMENT_SECTIONS: usize = 4;
pub(super) const MAX_FRAGMENT_SECTION_OBJECTIVES: usize = 3;
pub(super) const MAX_FRAGMENT_SECTION_CONCEPTS: usize = 4;
pub(super) const MAX_GLOBAL_SYNTHESIS_INPUT_CHARS: usize = 12_000;

/// Provider-neutral, reusable learning outline assembled from canonical source
/// windows. The `page_*` field names are persisted compatibility projections;
/// reasoning code treats them as one-based canonical source-unit bounds.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DocumentOutline {
    pub summary: String,
    pub learning_objectives: Vec<String>,
    pub key_concepts: Vec<String>,
    pub sections: Vec<OutlineSection>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OutlineSection {
    pub title: String,
    pub page_start: i64,
    pub page_end: i64,
    pub summary: String,
    pub learning_objectives: Vec<String>,
    pub key_concepts: Vec<String>,
}

/// Bounded visual evidence selected by the canonical document workflow.
///
/// `SourcePageImage` remains the public compatibility name while PDF is the only
/// shipping visual adapter. Provider transport treats `page_number` as a
/// canonical source-unit number and never performs document-format parsing.
#[derive(Clone, Debug)]
pub(crate) struct SourcePageImage {
    pub page_number: i64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FlashcardOutput {
    cards: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum GeneratedStudyCard {
    Basic {
        front: String,
        back: String,
        #[serde(default)]
        tags: Vec<String>,
    },
    Definition {
        term: String,
        definition: String,
        #[serde(default)]
        tags: Vec<String>,
    },
    MultipleChoice {
        question: String,
        options: Vec<String>,
        #[serde(rename = "correctIndex")]
        correct_index: usize,
        explanation: String,
        #[serde(default)]
        tags: Vec<String>,
    },
    CriticalThinking {
        prompt: String,
        answer: String,
        #[serde(default)]
        tags: Vec<String>,
    },
    Cloze {
        text: String,
        blanks: Vec<String>,
        #[serde(default = "default_cloze_mode")]
        mode: String,
        #[serde(default)]
        tags: Vec<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CurationOutput {
    keep_indices: Vec<usize>,
}

pub(super) fn parse_flashcards(
    value: Value,
    maximum_cards: usize,
) -> Result<Vec<Value>, CommandError> {
    let output: FlashcardOutput = serde_json::from_value(value).map_err(|error| {
        output_error(format!(
            "The model returned an invalid flashcard result object: {}",
            bounded_diagnostic(&error.to_string(), 320)
        ))
    })?;
    let maximum_cards = maximum_cards.clamp(1, MAX_WINDOW_CARDS);
    if output.cards.len() > maximum_cards {
        return Err(output_error(format!(
            "The model returned more than {maximum_cards} cards for this semantic source window."
        )));
    }
    if output.cards.is_empty() {
        return Ok(Vec::new());
    }

    let returned = output.cards.len();
    let mut accepted = Vec::with_capacity(returned);
    let mut rejected = 0_usize;
    let mut first_error = None::<String>;
    for (index, raw_card) in output.cards.into_iter().enumerate() {
        let candidate = serde_json::from_value::<GeneratedStudyCard>(raw_card).map_err(|error| {
            output_error(format!(
                "Generated card {} did not match a supported card type: {}",
                index + 1,
                bounded_diagnostic(&error.to_string(), 240)
            ))
        });
        match candidate.and_then(|card| normalize_generated_card(index, card)) {
            Ok(mut card) => {
                strip_document_provenance_tags(&mut card);
                accepted.push(card);
            }
            Err(error) => {
                rejected = rejected.saturating_add(1);
                if first_error.is_none() {
                    first_error = Some(bounded_diagnostic(&error.message, 320));
                }
            }
        }
    }
    if rejected > 0 {
        crate::logging::info(
            "document_flashcards_filtered",
            format!("accepted={} rejected={rejected}", accepted.len()),
        );
    }
    if accepted.is_empty() {
        return Err(output_error(format!(
            "All {returned} returned flashcard candidate(s) failed local validation. First validation detail: {}",
            first_error.unwrap_or_else(|| "unknown card contract mismatch".to_string())
        )));
    }
    Ok(accepted)
}

pub(super) fn parse_curation(
    value: Value,
    candidate_count: usize,
) -> Result<Vec<usize>, CommandError> {
    let output: CurationOutput = serde_json::from_value(value)
        .map_err(|_| output_error("The model returned an invalid card-curation response."))?;
    let mut seen = HashSet::new();
    for index in &output.keep_indices {
        if *index >= candidate_count || !seen.insert(*index) {
            return Err(output_error(
                "The card-curation response contained an invalid or duplicate index.",
            ));
        }
    }
    Ok(output.keep_indices)
}

fn strip_document_provenance_tags(card: &mut Value) {
    let Some(tags) = card.get_mut("tags").and_then(Value::as_array_mut) else {
        return;
    };
    tags.retain(|tag| {
        tag.as_str()
            .is_some_and(|value| !is_document_provenance_tag(value))
    });
}

fn is_document_provenance_tag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "ai generated" | "ai-generated" | "ai_generated" | "generated by ai" | "pdf"
    )
}

fn normalize_generated_card(
    index: usize,
    card: GeneratedStudyCard,
) -> Result<Value, CommandError> {
    match card {
        GeneratedStudyCard::Basic { front, back, tags } => {
            validate_card_text(index, "front", &front)?;
            validate_card_text(index, "back", &back)?;
            validate_tags(index, &tags)?;
            Ok(json!({
                "type": "basic",
                "front": front,
                "back": back,
                "reverseEnabled": false,
                "tags": tags
            }))
        }
        GeneratedStudyCard::Definition {
            term,
            definition,
            tags,
        } => {
            validate_card_text(index, "term", &term)?;
            validate_card_text(index, "definition", &definition)?;
            validate_tags(index, &tags)?;
            Ok(json!({
                "type": "basic",
                "front": term,
                "back": definition,
                "reverseEnabled": true,
                "tags": tags
            }))
        }
        GeneratedStudyCard::MultipleChoice {
            question,
            options,
            correct_index,
            explanation,
            tags,
        } => {
            validate_card_text(index, "question", &question)?;
            validate_card_text(index, "explanation", &explanation)?;
            validate_tags(index, &tags)?;
            if options.len() != 4 || correct_index >= 4 {
                return Err(output_error(format!(
                    "Generated card {} must have four options and a correctIndex from 0 through 3.",
                    index + 1
                )));
            }
            let mut normalized = HashSet::new();
            for (option_index, option) in options.iter().enumerate() {
                validate_card_text(index, &format!("options[{option_index}]"), option)?;
                if !normalized.insert(option.trim().to_lowercase()) {
                    return Err(output_error(format!(
                        "Generated card {} contains duplicate multiple-choice options.",
                        index + 1
                    )));
                }
            }
            let labels = ["A", "B", "C", "D"];
            let front = format!(
                "{}\n\nA. {}\nB. {}\nC. {}\nD. {}",
                question.trim(),
                options[0].trim(),
                options[1].trim(),
                options[2].trim(),
                options[3].trim()
            );
            let back = format!(
                "**Answer: {}. {}**\n\n{}",
                labels[correct_index],
                options[correct_index].trim(),
                explanation.trim()
            );
            Ok(json!({
                "type": "basic",
                "front": front,
                "back": back,
                "reverseEnabled": false,
                "tags": tags
            }))
        }
        GeneratedStudyCard::CriticalThinking {
            prompt,
            answer,
            tags,
        } => {
            validate_card_text(index, "prompt", &prompt)?;
            validate_card_text(index, "answer", &answer)?;
            validate_tags(index, &tags)?;
            Ok(json!({
                "type": "basic",
                "front": prompt,
                "back": answer,
                "reverseEnabled": false,
                "tags": tags
            }))
        }
        GeneratedStudyCard::Cloze {
            text,
            blanks,
            mode,
            tags,
        } => {
            validate_card_text(index, "text", &text)?;
            validate_tags(index, &tags)?;
            if !matches!(mode.as_str(), "one_at_a_time" | "all_together") {
                return Err(output_error(format!(
                    "Generated card {} has an unsupported cloze mode.",
                    index + 1
                )));
            }
            Ok(json!({
                "type": "cloze",
                "text": build_cloze_text(index, &text, &blanks)?,
                "mode": mode,
                "tags": tags
            }))
        }
    }
}

fn build_cloze_text(
    index: usize,
    text: &str,
    blanks: &[String],
) -> Result<String, CommandError> {
    if blanks.is_empty() || blanks.len() > MAX_CLOZE_BLANKS {
        return Err(output_error(format!(
            "Generated card {} must select between one and {MAX_CLOZE_BLANKS} cloze blanks.",
            index + 1
        )));
    }

    let mut unique = HashSet::new();
    let mut ranges: Vec<(usize, usize, usize, String)> = Vec::with_capacity(blanks.len());
    for (blank_index, blank) in blanks.iter().enumerate() {
        let trimmed = blank.trim();
        if trimmed.is_empty()
            || trimmed.chars().count() > MAX_CLOZE_BLANK_CHARS
            || trimmed.contains("{{")
            || trimmed.contains("}}")
            || !unique.insert(trimmed.to_string())
        {
            return Err(output_error(format!(
                "Generated card {} contains an invalid or duplicate cloze blank.",
                index + 1
            )));
        }

        let mut search_start = 0_usize;
        let selected = loop {
            let Some(relative) = text[search_start..].find(trimmed) else {
                break None;
            };
            let start = search_start + relative;
            let end = start + trimmed.len();
            if ranges
                .iter()
                .all(|(other_start, other_end, _, _)| end <= *other_start || start >= *other_end)
            {
                break Some((start, end));
            }
            search_start = end;
            if search_start >= text.len() {
                break None;
            }
        };
        let Some((start, end)) = selected else {
            return Err(output_error(format!(
                "Generated card {} selected a cloze blank that is not an exact non-overlapping substring of its text.",
                index + 1
            )));
        };
        ranges.push((start, end, blank_index + 1, trimmed.to_string()));
    }

    ranges.sort_by_key(|range| Reverse(range.0));
    let mut marked = text.to_string();
    for (start, end, group, blank) in ranges {
        marked.replace_range(start..end, &format!("{{{{c{group}::{blank}}}}}"));
    }
    Ok(marked)
}

fn validate_card_text(index: usize, field: &str, value: &str) -> Result<(), CommandError> {
    let count = value.chars().count();
    if value.trim().is_empty()
        || count > MAX_CARD_TEXT_CHARS
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(output_error(format!(
            "Generated card {} has an empty, oversized, or invalid {field} field.",
            index + 1
        )));
    }
    Ok(())
}

fn validate_tags(index: usize, tags: &[String]) -> Result<(), CommandError> {
    if tags.len() > MAX_TAGS_PER_CARD {
        return Err(output_error(format!(
            "Generated card {} contains too many tags.",
            index + 1
        )));
    }
    for tag in tags {
        if tag.trim().is_empty()
            || tag.chars().count() > MAX_TAG_CHARS
            || tag.starts_with('#')
            || tag.chars().any(char::is_control)
        {
            return Err(output_error(format!(
                "Generated card {} contains an invalid tag.",
                index + 1
            )));
        }
    }
    Ok(())
}

fn default_cloze_mode() -> String {
    "one_at_a_time".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_document_tags_drop_provenance_but_keep_subjects() {
        let parsed = parse_flashcards(
            json!({
                "cards": [{
                    "type": "basic",
                    "front": "What is preload?",
                    "back": "Ventricular filling before contraction.",
                    "tags": ["Cardiology", "AI generated", "PDF", "Hemodynamics"]
                }]
            }),
            4,
        )
        .unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["tags"], json!(["Cardiology", "Hemodynamics"]));
    }

    #[test]
    fn malformed_candidate_does_not_discard_valid_cards_from_the_window() {
        let parsed = parse_flashcards(
            json!({
                "cards": [
                    {
                        "type": "basic",
                        "front": "What is preload?",
                        "back": "Ventricular filling before contraction.",
                        "tags": ["Hemodynamics"]
                    },
                    {
                        "type": "multiple_choice",
                        "question": "Malformed candidate",
                        "options": ["A", "B", "C", "D"],
                        "correctIndex": "not-an-integer",
                        "explanation": "This card is intentionally malformed.",
                        "tags": []
                    }
                ]
            }),
            4,
        )
        .unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["front"], "What is preload?");
    }

    #[test]
    fn all_malformed_candidates_return_a_specific_validation_detail() {
        let error = parse_flashcards(
            json!({
                "cards": [{
                    "type": "multiple_choice",
                    "question": "Malformed candidate",
                    "options": ["A", "B", "C", "D"],
                    "correctIndex": "not-an-integer",
                    "explanation": "Malformed",
                    "tags": []
                }]
            }),
            4,
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_document_output_invalid");
        assert!(error.message.contains("All 1 returned flashcard"));
        assert!(error.message.contains("correctIndex") || error.message.contains("usize"));
    }

    #[test]
    fn only_definition_cards_normalize_as_reversible() {
        let basic = normalize_generated_card(
            0,
            GeneratedStudyCard::Basic {
                front: "What causes hypoxic pulmonary vasoconstriction?".to_string(),
                back: "Low alveolar oxygen triggers local pulmonary arteriolar constriction."
                    .to_string(),
                tags: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(basic["reverseEnabled"], false);

        let definition = normalize_generated_card(
            1,
            GeneratedStudyCard::Definition {
                term: "Shunt".to_string(),
                definition: "Perfusion without ventilation.".to_string(),
                tags: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(definition["front"], "Shunt");
        assert_eq!(definition["back"], "Perfusion without ventilation.");
        assert_eq!(definition["reverseEnabled"], true);
    }

    #[test]
    fn cloze_blanks_are_constructed_by_rust() {
        let marked = build_cloze_text(
            0,
            "Cardiac output equals heart rate multiplied by stroke volume.",
            &["heart rate".to_string(), "stroke volume".to_string()],
        )
        .unwrap();
        assert_eq!(
            marked,
            "Cardiac output equals {{c1::heart rate}} multiplied by {{c2::stroke volume}}."
        );
    }

    #[test]
    fn curation_rejects_duplicate_or_out_of_range_indices() {
        assert!(parse_curation(json!({ "keepIndices": [0, 0] }), 2).is_err());
        assert!(parse_curation(json!({ "keepIndices": [2] }), 2).is_err());
        assert_eq!(
            parse_curation(json!({ "keepIndices": [1] }), 2).unwrap(),
            vec![1]
        );
    }

    #[test]
    fn dynamic_generation_card_limit_is_enforced() {
        let cards = (0..3)
            .map(|index| {
                json!({
                    "type": "basic",
                    "front": format!("Question {index}"),
                    "back": format!("Answer {index}"),
                    "tags": []
                })
            })
            .collect::<Vec<_>>();
        let error = parse_flashcards(json!({ "cards": cards }), 2).unwrap_err();
        assert_eq!(error.code, "ai_document_output_invalid");
        assert!(error.message.contains("more than 2 cards"));
    }
}
