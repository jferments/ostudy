use serde_json::{json, Value};

use super::contracts::{
    MAX_CARD_TEXT_CHARS, MAX_CLOZE_BLANKS, MAX_CLOZE_BLANK_CHARS, MAX_FRAGMENT_CONCEPTS,
    MAX_FRAGMENT_OBJECTIVES, MAX_FRAGMENT_SECTIONS, MAX_FRAGMENT_SECTION_CONCEPTS,
    MAX_FRAGMENT_SECTION_OBJECTIVES, MAX_TAGS_PER_CARD, MAX_TAG_CHARS, MAX_WINDOW_CARDS,
};

pub(super) fn outline_fragment_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["summary", "learningObjectives", "keyConcepts", "sections"],
        "properties": {
            "summary": string_schema(1_800),
            "learningObjectives": string_array_schema(MAX_FRAGMENT_OBJECTIVES, 500),
            "keyConcepts": string_array_schema(MAX_FRAGMENT_CONCEPTS, 240),
            "sections": {
                "type": "array",
                "maxItems": MAX_FRAGMENT_SECTIONS,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "title",
                        "pageStart",
                        "pageEnd",
                        "summary",
                        "learningObjectives",
                        "keyConcepts"
                    ],
                    "properties": {
                        "title": string_schema(240),
                        "pageStart": { "type": "integer", "minimum": 1 },
                        "pageEnd": { "type": "integer", "minimum": 1 },
                        "summary": string_schema(900),
                        "learningObjectives": string_array_schema(MAX_FRAGMENT_SECTION_OBJECTIVES, 420),
                        "keyConcepts": string_array_schema(MAX_FRAGMENT_SECTION_CONCEPTS, 220)
                    }
                }
            }
        }
    })
}

pub(super) fn global_outline_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["summary", "learningObjectives", "keyConcepts"],
        "properties": {
            "summary": string_schema(8_000),
            "learningObjectives": string_array_schema(16, 1_200),
            "keyConcepts": string_array_schema(24, 700)
        }
    })
}

pub(super) fn flashcard_schema(maximum_cards: usize) -> Value {
    let maximum_cards = maximum_cards.clamp(1, MAX_WINDOW_CARDS);
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["cards"],
        "properties": {
            "cards": {
                "type": "array",
                "maxItems": maximum_cards,
                "items": {
                    "oneOf": [
                        basic_card_schema(),
                        definition_card_schema(),
                        multiple_choice_card_schema(),
                        critical_thinking_card_schema(),
                        cloze_card_schema()
                    ]
                }
            }
        }
    })
}

pub(super) fn curation_schema(candidate_count: usize) -> Value {
    let maximum_index = candidate_count.saturating_sub(1);
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["keepIndices"],
        "properties": {
            "keepIndices": {
                "type": "array",
                "maxItems": candidate_count,
                "items": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": maximum_index
                }
            }
        }
    })
}

fn basic_card_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "front", "back"],
        "properties": {
            "type": { "const": "basic" },
            "front": string_schema(MAX_CARD_TEXT_CHARS),
            "back": string_schema(MAX_CARD_TEXT_CHARS),
            "tags": tag_schema()
        }
    })
}

fn definition_card_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "term", "definition"],
        "properties": {
            "type": { "const": "definition" },
            "term": string_schema(MAX_CARD_TEXT_CHARS),
            "definition": string_schema(MAX_CARD_TEXT_CHARS),
            "tags": tag_schema()
        }
    })
}

fn multiple_choice_card_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "question", "options", "correctIndex", "explanation"],
        "properties": {
            "type": { "const": "multiple_choice" },
            "question": string_schema(MAX_CARD_TEXT_CHARS),
            "options": {
                "type": "array",
                "minItems": 4,
                "maxItems": 4,
                "items": string_schema(MAX_CARD_TEXT_CHARS)
            },
            "correctIndex": { "type": "integer", "minimum": 0, "maximum": 3 },
            "explanation": string_schema(MAX_CARD_TEXT_CHARS),
            "tags": tag_schema()
        }
    })
}

fn critical_thinking_card_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "prompt", "answer"],
        "properties": {
            "type": { "const": "critical_thinking" },
            "prompt": string_schema(MAX_CARD_TEXT_CHARS),
            "answer": string_schema(MAX_CARD_TEXT_CHARS),
            "tags": tag_schema()
        }
    })
}

fn cloze_card_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "text", "blanks"],
        "properties": {
            "type": { "const": "cloze" },
            "text": string_schema(MAX_CARD_TEXT_CHARS),
            "blanks": {
                "type": "array",
                "minItems": 1,
                "maxItems": MAX_CLOZE_BLANKS,
                "items": string_schema(MAX_CLOZE_BLANK_CHARS)
            },
            "mode": {
                "type": "string",
                "enum": ["one_at_a_time", "all_together"]
            },
            "tags": tag_schema()
        }
    })
}

fn string_schema(maximum: usize) -> Value {
    json!({ "type": "string", "minLength": 1, "maxLength": maximum })
}

fn string_array_schema(maximum_items: usize, maximum_string: usize) -> Value {
    json!({
        "type": "array",
        "maxItems": maximum_items,
        "items": string_schema(maximum_string)
    })
}

fn tag_schema() -> Value {
    string_array_schema(MAX_TAGS_PER_CARD, MAX_TAG_CHARS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemas_cover_all_generation_forms() {
        let flashcards = serde_json::to_string(&flashcard_schema(8)).unwrap();
        let outline = serde_json::to_string(&outline_fragment_schema()).unwrap();
        assert!(flashcards.contains("multiple_choice"));
        assert!(flashcards.contains("critical_thinking"));
        assert!(flashcards.contains("cloze"));
        assert!(!flashcards.contains("reverseEnabled"));
        assert!(outline.contains("learningObjectives"));
    }

    #[test]
    fn outline_fragment_schema_is_deliberately_small() {
        let schema = outline_fragment_schema();
        assert_eq!(
            schema
                .pointer("/properties/sections/maxItems")
                .and_then(Value::as_u64),
            Some(4)
        );
        assert_eq!(
            schema
                .pointer("/properties/summary/maxLength")
                .and_then(Value::as_u64),
            Some(1_800)
        );
    }

    #[test]
    fn flashcard_schema_uses_the_requested_window_budget() {
        let schema = flashcard_schema(4);
        assert_eq!(
            schema
                .pointer("/properties/cards/maxItems")
                .and_then(Value::as_u64),
            Some(4)
        );
    }

    #[test]
    fn curation_schema_never_allows_an_out_of_range_index() {
        let schema = curation_schema(3);
        assert_eq!(
            schema
                .pointer("/properties/keepIndices/items/maximum")
                .and_then(Value::as_u64),
            Some(2)
        );
    }
}
