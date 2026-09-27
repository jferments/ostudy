//! Pure normalization, fallback, and exact-template recognition for generated
//! document artifact metadata.

use crate::ai::document_intelligence::{
    DocumentArtifactMetadataCandidate, DocumentOutline, ARTIFACT_DESCRIPTION_MAX_CHARS,
    ARTIFACT_TITLE_MAX_CHARS,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DesiredMetadata {
    pub(super) metadata_origin: &'static str,
    pub(super) document_title: String,
    pub(super) deck_title: String,
    pub(super) deck_description: String,
}

pub(super) fn derive_desired_metadata(
    outline: &DocumentOutline,
    provider_candidate: Option<&DocumentArtifactMetadataCandidate>,
    source_title_hint: Option<&str>,
    original_filename: &str,
    source_format: &str,
) -> DesiredMetadata {
    let source_title = source_title_hint
        .and_then(|value| accepted_title(value, original_filename, source_format))
        .filter(|title| title_has_document_scope(title, outline));
    let fallback_title = deterministic_title(outline, original_filename, source_format);
    let provider_document_title = provider_candidate
        .and_then(|candidate| {
            accepted_title(&candidate.document_title, original_filename, source_format)
        })
        .filter(|title| title_has_document_scope(title, outline));
    let provider_deck_title = provider_candidate
        .and_then(|candidate| {
            accepted_title(&candidate.deck_title, original_filename, source_format)
        })
        .filter(|title| title_has_document_scope(title, outline));
    let provider_description = provider_candidate
        .and_then(|candidate| accepted_description(&candidate.deck_description, original_filename));
    let used_provider = provider_document_title.is_some()
        || provider_deck_title.is_some()
        || provider_description.is_some();
    let document_title = source_title
        .clone()
        .or(provider_document_title)
        .unwrap_or_else(|| fallback_title.clone());
    let deck_title = source_title
        .or(provider_deck_title)
        .unwrap_or_else(|| fallback_title.clone());
    let deck_description = provider_description
        .unwrap_or_else(|| deterministic_description(outline, &document_title, original_filename));
    DesiredMetadata {
        metadata_origin: if used_provider {
            "provider"
        } else {
            "deterministic_fallback"
        },
        document_title,
        deck_title,
        deck_description,
    }
}

pub(super) fn exact_source_document_title(
    value: &str,
    source_title: &str,
    original_filename: &str,
    source_format: &str,
) -> bool {
    let normalized = normalized_key(value);
    let stem = source_stem(original_filename, source_format);
    [
        source_title.to_string(),
        original_filename.to_string(),
        stem.clone(),
        stem.replace('_', " "),
        stem.replace('-', " "),
    ]
    .into_iter()
    .any(|candidate| normalized == normalized_key(&candidate))
}

pub(super) fn exact_source_deck_title(
    value: &str,
    source_title: &str,
    original_filename: &str,
    source_format: &str,
) -> bool {
    if exact_source_document_title(value, source_title, original_filename, source_format) {
        return true;
    }
    let stem = source_stem(original_filename, source_format);
    let normalized_stem =
        normalize_metadata(&stem.replace(['_', '-'], " "), ARTIFACT_TITLE_MAX_CHARS);
    let normalized = normalized_key(value);
    [
        format!("Flashcards from {stem}"),
        format!("{stem} flashcards"),
        format!("Flashcards from {normalized_stem}"),
        format!("{normalized_stem} flashcards"),
    ]
    .into_iter()
    .any(|candidate| normalized == normalized_key(&candidate))
}

pub(super) fn exact_generated_description(value: &str, original_filename: &str) -> bool {
    value.trim().is_empty()
        || value
            == format!(
                "AI-generated flashcards from {original_filename}. Edit or delete these cards through the normal deck tools."
            )
}

fn deterministic_title(
    outline: &DocumentOutline,
    original_filename: &str,
    source_format: &str,
) -> String {
    repeated_parent_subject(outline)
        .and_then(|value| accepted_title(&value, original_filename, source_format))
        .or_else(|| {
            // A local fallback used to bypass the provider's scope check and
            // promote the first section to the entire document's title. Retain
            // an honest source-level subject rather than invent a narrow scope.
            if outline.sections.len() > 1 {
                filename_subject(original_filename, source_format)
                    .filter(|title| title_has_document_scope(title, outline))
            } else {
                outline.sections.iter().map(|section| section.title.as_str())
                    .chain(outline.key_concepts.iter().map(String::as_str))
                    .find_map(|value| accepted_title(value, original_filename, source_format))
            }
        })
        .or_else(|| {
            accepted_title(first_sentence(&outline.summary), original_filename, source_format)
                .filter(|title| title_has_document_scope(title, outline))
        })
        .unwrap_or_else(|| "Study material".to_string())
}

/// Source filenames are provenance, not provider-generated semantics. A short
/// descriptive suffix in a bibliographic filename is a safer fallback than one
/// unrelated subsection. Generic counters/exports are not a document subject.
fn filename_subject(original_filename: &str, source_format: &str) -> Option<String> {
    let stem = source_stem(original_filename, source_format);
    let subject = stem.rsplit_once(" - ").map_or(stem.as_str(), |(_, suffix)| suffix);
    let normalized = normalize_metadata(&subject.replace('_', " "), ARTIFACT_TITLE_MAX_CHARS);
    let words = normalized.split_whitespace().collect::<Vec<_>>();
    if words.len() < 2 || looks_like_inventory_title(&normalized)
        || contains_generator_boilerplate(&normalized)
        || words.iter().filter(|word| word.chars().any(char::is_alphabetic)).count() < 2
        || matches!(words[0].to_ascii_lowercase().as_str(), "chapter" | "scan" | "document" | "untitled" | "export")
    {
        return None;
    }
    Some(normalized)
}

fn repeated_parent_subject(outline: &DocumentOutline) -> Option<String> {
    let mut counts = std::collections::BTreeMap::<String, (String, usize)>::new();
    for title in outline.sections.iter().map(|section| section.title.as_str()) {
        let Some((parent, _)) = title.split_once(" — ") else {
            continue;
        };
        let parent = normalize_metadata(parent, ARTIFACT_TITLE_MAX_CHARS);
        if parent.split_whitespace().count() < 2 {
            continue;
        }
        let key = parent.to_ascii_lowercase();
        let entry = counts.entry(key).or_insert((parent, 0));
        entry.1 = entry.1.saturating_add(1);
    }
    let required = outline.sections.len().saturating_mul(2).div_ceil(3).max(2);
    counts.into_values()
        .filter(|(_, count)| *count >= required)
        .max_by_key(|(_, count)| *count)
        .map(|(title, _)| title)
}

fn title_has_document_scope(title: &str, outline: &DocumentOutline) -> bool {
    if outline.sections.len() < 2 {
        return true;
    }
    let candidate = normalized_key(title);
    let section_coverage = outline.sections.iter().filter(|section| {
        let section_text = format!("{} {} {}", section.title, section.summary, section.key_concepts.join(" "));
        normalized_key(&section_text).contains(&candidate)
    }).count();
    if section_coverage >= outline.sections.len().saturating_mul(2).div_ceil(3) {
        return true;
    }
    let is_leaf_label = outline.sections.iter().any(|section| {
        section.title.split(" / ").map(normalized_key).any(|label| label == candidate)
            || section.key_concepts.iter().map(|value| normalized_key(value)).any(|label| label == candidate)
    });
    !is_leaf_label
}

fn deterministic_description(
    outline: &DocumentOutline,
    title: &str,
    original_filename: &str,
) -> String {
    let concepts = outline
        .key_concepts
        .iter()
        .filter_map(|value| accepted_description(value, original_filename))
        .take(3)
        .collect::<Vec<_>>();
    let candidate = if concepts.is_empty() {
        format!("Review {title} with source-grounded explanations and linked flashcards.")
    } else {
        format!(
            "Review {title}, including {}, with source-grounded explanations and linked flashcards.",
            concepts.join(", ")
        )
    };
    normalize_metadata(&candidate, ARTIFACT_DESCRIPTION_MAX_CHARS)
}

fn accepted_title(value: &str, original_filename: &str, source_format: &str) -> Option<String> {
    let normalized = normalize_metadata(value, ARTIFACT_TITLE_MAX_CHARS);
    if normalized.is_empty()
        || looks_like_inventory_title(&normalized)
        || contains_generator_boilerplate(&normalized)
        || exact_source_document_title(
            &normalized,
            original_filename,
            original_filename,
            source_format,
        )
        || normalized
            .to_ascii_lowercase()
            .contains(&original_filename.to_ascii_lowercase())
    {
        return None;
    }
    Some(normalized)
}

fn looks_like_inventory_title(value: &str) -> bool {
    let trimmed = value.trim();
    let comma_count = trimmed.matches(',').count();
    let word_count = trimmed.split_whitespace().count();
    trimmed.contains(" / ")
        || trimmed.contains(" | ")
        || trimmed.contains("; ")
        || comma_count >= 2
        || trimmed.lines().count() > 1
        || word_count > 16
}

fn accepted_description(value: &str, original_filename: &str) -> Option<String> {
    let normalized = normalize_metadata(value, ARTIFACT_DESCRIPTION_MAX_CHARS);
    if normalized.is_empty()
        || contains_generator_boilerplate(&normalized)
        || normalized
            .to_ascii_lowercase()
            .contains(&original_filename.to_ascii_lowercase())
    {
        return None;
    }
    Some(normalized)
}

fn contains_generator_boilerplate(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    [
        "ai-generated",
        "ai generated",
        "generated from",
        "source file",
        "uploaded file",
        "this document contains",
    ]
    .iter()
    .any(|forbidden| normalized.contains(forbidden))
}

fn source_stem(original_filename: &str, source_format: &str) -> String {
    let suffix = format!(".{source_format}");
    if let Some(stem) = original_filename.strip_suffix(&suffix) {
        return stem.to_string();
    }
    let lower = original_filename.to_ascii_lowercase();
    let lower_suffix = suffix.to_ascii_lowercase();
    if let Some(stem) = lower.strip_suffix(&lower_suffix) {
        return original_filename[..stem.len()].to_string();
    }
    original_filename.to_string()
}

fn first_sentence(value: &str) -> &str {
    value
        .split(['.', '!', '?'])
        .find(|part| !part.trim().is_empty())
        .unwrap_or(value)
}

fn normalized_key(value: &str) -> String {
    normalize_metadata(value, ARTIFACT_DESCRIPTION_MAX_CHARS).to_ascii_lowercase()
}

pub(super) fn normalize_metadata(value: &str, maximum_chars: usize) -> String {
    let mut normalized = String::new();
    let mut pending_space = false;
    for character in value.chars() {
        if character.is_whitespace() || character.is_control() {
            pending_space = !normalized.is_empty();
            continue;
        }
        if pending_space {
            normalized.push(' ');
            pending_space = false;
        }
        normalized.push(character);
    }
    let trimmed = normalized.trim_matches(|character: char| {
        character.is_whitespace()
            || matches!(
                character,
                '.' | ',' | ':' | ';' | '-' | '–' | '—' | '|' | '\u{2022}'
            )
    });
    trimmed.chars().take(maximum_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outline() -> DocumentOutline {
        DocumentOutline {
            summary: "A whole-document summary.".to_string(),
            learning_objectives: Vec::new(),
            key_concepts: vec![
                "Distributive shock physiology".to_string(),
                "Perfusion".to_string(),
            ],
            sections: Vec::new(),
        }
    }

    #[test]
    fn normalization_removes_controls_collapses_whitespace_and_bounds_chars() {
        assert_eq!(
            normalize_metadata(" \n  Shock\t physiology \u{0000} -- ", 17),
            "Shock physiology"
        );
        assert_eq!(normalize_metadata("αβγδε", 3), "αβγ");
    }

    #[test]
    fn slash_and_keyword_inventory_titles_are_rejected_field_by_field() {
        let candidate = DocumentArtifactMetadataCandidate {
            document_title: "Enteric neurotransmitters / GI extrinsic nervous system / GI hormones / intrinsic nervous system".to_string(),
            deck_title: "Gastrointestinal hormones and nervous system".to_string(),
            deck_description: "Review gastrointestinal signaling and neural control.".to_string(),
        };
        let desired =
            derive_desired_metadata(&outline(), Some(&candidate), None, "shock_notes.pdf", "pdf");
        assert_ne!(desired.document_title, candidate.document_title);
        assert_eq!(
            desired.deck_title,
            "Gastrointestinal hormones and nervous system"
        );
        assert!(looks_like_inventory_title("A / B / C"));
        assert!(looks_like_inventory_title("A, B, C, and D"));
        assert!(!looks_like_inventory_title(
            "Gastrointestinal hormones and nervous system"
        ));
    }

    #[test]
    fn provider_boilerplate_falls_back_field_by_field() {
        let candidate = DocumentArtifactMetadataCandidate {
            document_title: "AI-generated shock_notes.pdf".to_string(),
            deck_title: "Shock response".to_string(),
            deck_description: "Generated from shock_notes.pdf".to_string(),
        };
        let desired =
            derive_desired_metadata(&outline(), Some(&candidate), None, "shock_notes.pdf", "pdf");
        assert_eq!(desired.document_title, "Distributive shock physiology");
        assert_eq!(desired.deck_title, "Shock response");
        assert!(desired.deck_description.starts_with("Review "));
        assert_eq!(desired.metadata_origin, "provider");
    }

    #[test]
    fn strong_source_title_beats_a_narrow_provider_subtopic() {
        let mut outline = outline();
        outline.sections = vec![
            crate::ai::document_intelligence::OutlineSection {
                title: "GI regulatory substances".into(),
                page_start: 1,
                page_end: 9,
                summary: "Hormones and neural regulators coordinate gastrointestinal function."
                    .into(),
                learning_objectives: vec![],
                key_concepts: vec!["Gastrin".into()],
            },
            crate::ai::document_intelligence::OutlineSection {
                title: "Motility".into(),
                page_start: 10,
                page_end: 17,
                summary: "Smooth muscle and reflexes move luminal contents.".into(),
                learning_objectives: vec![],
                key_concepts: vec!["Slow waves".into()],
            },
            crate::ai::document_intelligence::OutlineSection {
                title: "Amino acid digestion".into(),
                page_start: 37,
                page_end: 42,
                summary: "Proteins are digested and absorbed.".into(),
                learning_objectives: vec![],
                key_concepts: vec!["Amino acid digestion".into()],
            },
        ];
        let candidate = DocumentArtifactMetadataCandidate {
            document_title: "Amino acid digestion".into(),
            deck_title: "Amino acid digestion".into(),
            deck_description: "Review digestion and absorption.".into(),
        };
        let desired = derive_desired_metadata(
            &outline,
            Some(&candidate),
            Some("Gastrointestinal Physiology"),
            "chapter8.pdf",
            "pdf",
        );
        assert_eq!(desired.document_title, "Gastrointestinal Physiology");
        assert_eq!(desired.deck_title, "Gastrointestinal Physiology");
    }

    #[test]
    fn narrow_leaf_title_is_rejected_when_the_document_has_several_major_sections() {
        let mut outline = outline();
        outline.sections = vec![
            crate::ai::document_intelligence::OutlineSection {
                title: "Motility".into(),
                page_start: 1,
                page_end: 10,
                summary: "GI motility and smooth muscle.".into(),
                learning_objectives: vec![],
                key_concepts: vec!["Motility".into()],
            },
            crate::ai::document_intelligence::OutlineSection {
                title: "Secretion".into(),
                page_start: 11,
                page_end: 20,
                summary: "Salivary, gastric, and pancreatic secretion.".into(),
                learning_objectives: vec![],
                key_concepts: vec!["Secretion".into()],
            },
            crate::ai::document_intelligence::OutlineSection {
                title: "Amino acid digestion".into(),
                page_start: 21,
                page_end: 30,
                summary: "Protein digestion and absorption.".into(),
                learning_objectives: vec![],
                key_concepts: vec!["Amino acid digestion".into()],
            },
        ];
        assert!(!title_has_document_scope("Amino acid digestion", &outline));
    }

    #[test]
    fn document_fallback_never_promotes_one_subsection_over_a_broad_source_subject() {
        let mut outline = outline();
        outline.sections = [
            "Patient education for hyperlipidemia therapy", "Antihypertensive mechanisms",
            "Heart failure treatment", "Antiarrhythmic classes", "Anticoagulant monitoring",
        ].into_iter().enumerate().map(|(index, title)| crate::ai::document_intelligence::OutlineSection {
            title: title.into(), page_start: index as i64 + 1, page_end: index as i64 + 1,
            summary: title.into(), learning_objectives: vec![], key_concepts: vec![title.into()],
        }).collect();
        let leaf = outline.sections[0].title.clone();
        let provider = DocumentArtifactMetadataCandidate {
            document_title: leaf.clone(), deck_title: leaf.clone(), deck_description: "Drug therapy concepts.".into(),
        };
        for candidate in [None, Some(&provider)] {
            let result = derive_desired_metadata(&outline, candidate, Some(&leaf),
                "Example textbook (2021, publisher) - Cardiovascular drugs.pdf", "pdf");
            assert_eq!(result.document_title, "Cardiovascular drugs");
            assert_eq!(result.deck_title, "Cardiovascular drugs");
        }
        outline.sections.reverse();
        assert_eq!(deterministic_title(&outline, "Cardiovascular_drugs.pdf", "pdf"), "Cardiovascular drugs");
        for filename in ["chapter8.pdf", "scan 123.pdf", "document 2026.pdf"] {
            assert!(filename_subject(filename, "pdf").is_none());
        }
    }

    #[test]
    fn a_repeated_parent_for_only_two_sections_is_not_the_entire_document() {
        let mut outline = outline();
        outline.sections = (0..6).map(|index| crate::ai::document_intelligence::OutlineSection {
            title: if index < 2 { format!("Minor subject — example {index}") } else { format!("Other subject {index}") },
            page_start: index + 1, page_end: index + 1, summary: String::new(), learning_objectives: vec![], key_concepts: vec![],
        }).collect();
        assert!(repeated_parent_subject(&outline).is_none());
    }

    #[test]
    fn exact_generated_templates_do_not_expand_to_ambiguous_user_titles() {
        assert!(exact_source_deck_title(
            "Flashcards from shock_notes",
            "shock_notes",
            "shock_notes.pdf",
            "pdf"
        ));
        assert!(!exact_source_deck_title(
            "My shock flashcards",
            "shock_notes",
            "shock_notes.pdf",
            "pdf"
        ));
        assert!(!exact_generated_description(
            "My own notes about shock_notes.pdf",
            "shock_notes.pdf"
        ));
    }
}
