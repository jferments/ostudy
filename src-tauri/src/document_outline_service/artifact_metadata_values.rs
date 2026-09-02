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
    original_filename: &str,
    source_format: &str,
) -> DesiredMetadata {
    let fallback_title = deterministic_title(outline, original_filename, source_format);
    let provider_document_title = provider_candidate.and_then(|candidate| {
        accepted_title(
            &candidate.document_title,
            original_filename,
            source_format,
        )
    });
    let provider_deck_title = provider_candidate.and_then(|candidate| {
        accepted_title(&candidate.deck_title, original_filename, source_format)
    });
    let provider_description = provider_candidate.and_then(|candidate| {
        accepted_description(&candidate.deck_description, original_filename)
    });
    let used_provider = provider_document_title.is_some()
        || provider_deck_title.is_some()
        || provider_description.is_some();
    let document_title = provider_document_title.unwrap_or_else(|| fallback_title.clone());
    let deck_title = provider_deck_title.unwrap_or_else(|| fallback_title.clone());
    let deck_description = provider_description.unwrap_or_else(|| {
        deterministic_description(outline, &document_title, original_filename)
    });
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
    let normalized_stem = normalize_metadata(
        &stem.replace('_', " ").replace('-', " "),
        ARTIFACT_TITLE_MAX_CHARS,
    );
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
    outline
        .sections
        .iter()
        .map(|section| section.title.as_str())
        .chain(outline.key_concepts.iter().map(String::as_str))
        .chain(std::iter::once(first_sentence(&outline.summary)))
        .find_map(|value| accepted_title(value, original_filename, source_format))
        .unwrap_or_else(|| "Study material".to_string())
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

fn accepted_title(
    value: &str,
    original_filename: &str,
    source_format: &str,
) -> Option<String> {
    let normalized = normalize_metadata(value, ARTIFACT_TITLE_MAX_CHARS);
    if normalized.is_empty()
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
    fn provider_boilerplate_falls_back_field_by_field() {
        let candidate = DocumentArtifactMetadataCandidate {
            document_title: "AI-generated shock_notes.pdf".to_string(),
            deck_title: "Shock response".to_string(),
            deck_description: "Generated from shock_notes.pdf".to_string(),
        };
        let desired =
            derive_desired_metadata(&outline(), Some(&candidate), "shock_notes.pdf", "pdf");
        assert_eq!(desired.document_title, "Distributive shock physiology");
        assert_eq!(desired.deck_title, "Shock response");
        assert!(desired.deck_description.starts_with("Review "));
        assert_eq!(desired.metadata_origin, "provider");
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
