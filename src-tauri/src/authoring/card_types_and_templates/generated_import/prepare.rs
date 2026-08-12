#[cfg(test)]
pub(super) fn prepare_flashcard_import(source_path: &str) -> Result<PreparedImport, CommandError> {
    let source = std::fs::canonicalize(source_path).map_err(CommandError::io)?;
    if !source.is_file() {
        return Err(CommandError::new(
            "invalid_flashcard_import",
            "Select an ostudy generated-flashcard file.",
        ));
    }
    let bytes = std::fs::read(source).map_err(CommandError::io)?;
    prepare_flashcard_import_bytes(&bytes)
}

use super::markdown::markdown_document;
use super::*;

pub(super) fn prepare_flashcard_import_bytes(bytes: &[u8]) -> Result<PreparedImport, CommandError> {
    if bytes.is_empty() || bytes.len() > MAX_GENERATED_IMPORT_BYTES {
        return Err(CommandError::new(
            "invalid_flashcard_import_size",
            "The generated flashcard file is empty or exceeds the 16 MB limit.",
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| {
        CommandError::new(
            "invalid_flashcard_import_utf8",
            "The generated flashcard file must be valid UTF-8 JSON.",
        )
    })?;
    let parsed: GeneratedImportFile = serde_json::from_str(text).map_err(|error| {
        CommandError::new(
            "invalid_flashcard_import_json",
            format!(
                "Could not parse generated flashcards at line {}, column {}: {error}",
                error.line(),
                error.column()
            ),
        )
    })?;

    if parsed.format != GENERATED_IMPORT_FORMAT || parsed.format_version != GENERATED_IMPORT_VERSION
    {
        return Err(CommandError::new(
            "unsupported_flashcard_import",
            "This is not a supported ostudy generated-flashcard file (expected format `ostudy-flashcards`, version 1).",
        ));
    }

    let name = crate::decks::validate_name(parsed.deck.name)?;
    let description = crate::decks::validate_description(parsed.deck.description)?;
    if parsed.deck.mode == GeneratedImportDeckMode::Append && !description.is_empty() {
        return Err(CommandError::new(
            "invalid_flashcard_import_deck",
            "deck.description must be omitted or empty when deck.mode is `append`.",
        ));
    }
    if parsed.cards.is_empty() {
        return Err(CommandError::new(
            "empty_flashcard_import",
            "The generated flashcard file does not contain any cards.",
        ));
    }
    if parsed.cards.len() > MAX_GENERATED_IMPORT_CARDS {
        return Err(CommandError::new(
            "too_many_flashcard_import_cards",
            format!(
                "A generated flashcard file may contain at most {MAX_GENERATED_IMPORT_CARDS} source notes."
            ),
        ));
    }

    validate_import_tag_syntax(&parsed.default_tags, "defaultTags")?;
    let mut notes = Vec::with_capacity(parsed.cards.len());
    let mut error_count = 0_usize;
    let mut error_examples = Vec::new();
    for (index, card) in parsed.cards.into_iter().enumerate() {
        match prepare_import_card(index, &parsed.default_tags, card) {
            Ok(note) => notes.push(note),
            Err(error) => {
                error_count += 1;
                if error_examples.len() < MAX_GENERATED_IMPORT_ERROR_EXAMPLES {
                    error_examples.push(error.message);
                }
            }
        }
    }
    if error_count > 0 {
        return Err(CommandError::new(
            "invalid_flashcard_import_cards",
            format_import_card_errors(error_count, &error_examples),
        ));
    }

    Ok(PreparedImport {
        mode: parsed.deck.mode,
        name,
        description,
        notes,
    })
}

fn format_import_card_errors(total: usize, examples: &[String]) -> String {
    let mut message = format!(
        "{total} {} could not be imported:\n{}",
        if total == 1 { "card" } else { "cards" },
        examples.join("\n")
    );
    let omitted = total.saturating_sub(examples.len());
    if omitted > 0 {
        message.push_str(&format!(
            "\n… {omitted} additional {} omitted from this diagnostic.",
            if omitted == 1 {
                "error was"
            } else {
                "errors were"
            }
        ));
    }
    message
}

fn prepare_import_card(
    index: usize,
    default_tags: &[String],
    card: GeneratedImportCard,
) -> Result<PreparedImportNote, CommandError> {
    let path = format!("cards[{index}]");
    let (draft, card_tags) = match card {
        GeneratedImportCard::Basic {
            front,
            back,
            reverse_enabled,
            tags,
        } => {
            let front_doc = markdown_document(&front, &format!("{path}.front"))?;
            let back_doc = markdown_document(&back, &format!("{path}.back"))?;
            (
                CardNoteDraft::Basic {
                    reverse_enabled,
                    front_doc,
                    back_doc,
                },
                tags,
            )
        }
        GeneratedImportCard::Cloze { text, mode, tags } => {
            (CardNoteDraft::Cloze { source: text, mode }, tags)
        }
    };

    validate_import_tag_syntax(&card_tags, &format!("{path}.tags"))?;
    let mut combined = default_tags.to_vec();
    combined.extend(card_tags.iter().cloned());
    let tags = validate_tags(combined).map_err(|error| {
        CommandError::new(error.code, format!("{path}.tags: {}", error.message))
    })?;
    let prepared = prepare_card_note(draft)
        .map_err(|error| CommandError::new(error.code, format!("{path}: {}", error.message)))?;
    let normalized = normalized_pair(&prepared.content.front_text, &prepared.content.back_text);
    let duplicate_hash = duplicate_hash(&normalized.0, &normalized.1);

    Ok(PreparedImportNote {
        prepared,
        tags,
        normalized,
        duplicate_hash,
    })
}

fn validate_import_tag_syntax(tags: &[String], path: &str) -> Result<(), CommandError> {
    for (index, tag) in tags.iter().enumerate() {
        if tag.starts_with('#') {
            return Err(CommandError::new(
                "invalid_import_tag",
                format!("{path}[{index}]: omit the leading `#` from tag names."),
            ));
        }
    }
    Ok(())
}
