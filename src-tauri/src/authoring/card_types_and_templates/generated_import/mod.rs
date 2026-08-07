use super::*;

// Strict, direct import for LLM-generated `.ostudy.json` flashcard files. The
// importer reuses canonical authoring validation and persistence rather than
// maintaining a parallel card-storage path.

const GENERATED_IMPORT_FORMAT: &str = "ostudy-flashcards";
const GENERATED_IMPORT_VERSION: i64 = 1;
const MAX_GENERATED_IMPORT_BYTES: usize = 16 * 1024 * 1024;
const MAX_GENERATED_IMPORT_CARDS: usize = 10_000;
const MAX_GENERATED_IMPORT_ERROR_EXAMPLES: usize = 20;

mod schema;
use schema::{GeneratedImportCard, GeneratedImportDeckMode, GeneratedImportFile};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardImportResultDto {
    pub deck_id: String,
    pub created_note_count: i64,
    pub created_card_count: i64,
    pub skipped_duplicate_count: i64,
}

struct PreparedImportNote {
    prepared: PreparedGeneratedNote,
    tags: Vec<ValidatedTag>,
    normalized: (String, String),
    duplicate_hash: String,
}

struct PreparedImport {
    mode: GeneratedImportDeckMode,
    name: String,
    description: String,
    notes: Vec<PreparedImportNote>,
}

mod apply;
mod prepare;

use apply::import_prepared_flashcards;
use prepare::prepare_flashcard_import;
#[cfg(test)]
use prepare::prepare_flashcard_import_bytes;

#[tauri::command]
pub fn import_generated_flashcards(
    database: State<'_, Database>,
    source_path: String,
) -> Result<FlashcardImportResultDto, CommandError> {
    let prepared = prepare_flashcard_import(&source_path)?;
    database.with_connection(|connection| import_prepared_flashcards(connection, prepared))
}

mod markdown;
#[cfg(test)]
use markdown::markdown_document;

#[cfg(test)]
mod generated_import_tests {
    use super::*;

    fn generated_file(mode: &str, deck_name: &str, front: &str, back: &str) -> Vec<u8> {
        format!(
            r#"{{
              "format":"ostudy-flashcards",
              "formatVersion":1,
              "deck":{{"mode":"{mode}","name":"{deck_name}"}},
              "cards":[{{"type":"basic","front":"{front}","back":"{back}"}}]
            }}"#
        )
        .into_bytes()
    }

    #[test]
    fn markdown_builds_supported_rich_document() {
        let document = markdown_document(
            "# Heading\n\nA **bold** answer.\n\n- one\n- two",
            "cards[0].front",
        )
        .expect("Markdown should parse");
        crate::cards::validate_document(&document)
            .expect("document should pass canonical validation");
    }

    #[test]
    fn markdown_allows_literal_comparison_angle_brackets() {
        let document = markdown_document("2 < 3 and 5 > 4", "front")
            .expect("comparison characters are ordinary text");
        crate::cards::validate_document(&document).expect("document should validate");
    }

    #[test]
    fn markdown_rejects_html_images_and_tables() {
        assert!(markdown_document("<script>x</script>", "front").is_err());
        assert!(markdown_document("![x](https://example.com/x.png)", "front").is_err());
        assert!(markdown_document("| A | B |", "front").is_err());
    }

    #[test]
    fn strict_json_requires_a_deck_mode_and_rejects_unknown_card_fields() {
        let missing_mode = br#"{
          "format":"ostudy-flashcards",
          "formatVersion":1,
          "deck":{"name":"Test"},
          "cards":[{"type":"basic","front":"Q","back":"A"}]
        }"#;
        assert!(prepare_flashcard_import_bytes(missing_mode).is_err());

        let unknown_field = br#"{
          "format":"ostudy-flashcards",
          "formatVersion":1,
          "deck":{"mode":"create","name":"Test"},
          "cards":[{"type":"basic","front":"Q","back":"A","overwrite":true}]
        }"#;
        assert!(prepare_flashcard_import_bytes(unknown_field).is_err());
    }

    #[test]
    fn create_then_append_uses_the_named_deck_and_skips_exact_duplicates() {
        let database = Database::open_in_memory().expect("database should open");
        let created = database
            .with_connection(|connection| {
                import_prepared_flashcards(
                    connection,
                    prepare_flashcard_import_bytes(&generated_file(
                        "create",
                        "Pharmacology",
                        "What reverses warfarin?",
                        "Vitamin K",
                    ))?,
                )
            })
            .expect("new deck import should succeed");
        assert_eq!(created.created_note_count, 1);
        assert_eq!(created.created_card_count, 1);

        let duplicate = database
            .with_connection(|connection| {
                import_prepared_flashcards(
                    connection,
                    prepare_flashcard_import_bytes(&generated_file(
                        "append",
                        "Pharmacology",
                        "What reverses warfarin?",
                        "Vitamin K",
                    ))?,
                )
            })
            .expect("duplicate append should succeed");
        assert_eq!(duplicate.deck_id, created.deck_id);
        assert_eq!(duplicate.created_note_count, 0);
        assert_eq!(duplicate.skipped_duplicate_count, 1);

        let appended = database
            .with_connection(|connection| {
                import_prepared_flashcards(
                    connection,
                    prepare_flashcard_import_bytes(&generated_file(
                        "append",
                        "Pharmacology",
                        "What monitors warfarin?",
                        "PT/INR",
                    ))?,
                )
            })
            .expect("unique append should succeed");
        assert_eq!(appended.deck_id, created.deck_id);
        assert_eq!(appended.created_note_count, 1);

        database
            .with_connection(|connection| {
                let count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM notes WHERE deck_id = ?1 AND deleted_at IS NULL",
                        [&created.deck_id],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(count, 2);
                Ok(())
            })
            .expect("note count should be readable");
    }

    #[test]
    fn append_requires_one_exact_active_deck_name() {
        let database = Database::open_in_memory().expect("database should open");
        let missing = database
            .with_connection(|connection| {
                import_prepared_flashcards(
                    connection,
                    prepare_flashcard_import_bytes(&generated_file(
                        "append", "Missing", "Question", "Answer",
                    ))?,
                )
            })
            .expect_err("missing append destination should fail");
        assert_eq!(missing.code, "generated_import_deck_not_found");

        database
            .with_connection(|connection| {
                let first = prepare_flashcard_import_bytes(&generated_file(
                    "create",
                    "Duplicate name",
                    "Question 1",
                    "Answer 1",
                ))?;
                import_prepared_flashcards(connection, first)?;
                let second = prepare_flashcard_import_bytes(&generated_file(
                    "create",
                    "Duplicate name",
                    "Question 2",
                    "Answer 2",
                ))?;
                import_prepared_flashcards(connection, second)?;
                Ok(())
            })
            .expect("duplicate deck names may be created explicitly");

        let ambiguous = database
            .with_connection(|connection| {
                import_prepared_flashcards(
                    connection,
                    prepare_flashcard_import_bytes(&generated_file(
                        "append",
                        "Duplicate name",
                        "Question 3",
                        "Answer 3",
                    ))?,
                )
            })
            .expect_err("ambiguous append destination should fail");
        assert_eq!(ambiguous.code, "generated_import_deck_ambiguous");
    }

    #[test]
    fn card_validation_reports_every_small_batch_error_before_writing() {
        let invalid = br#"{
          "format":"ostudy-flashcards",
          "formatVersion":1,
          "deck":{"mode":"create","name":"Invalid cards"},
          "cards":[
            {"type":"basic","front":"","back":"Answer"},
            {"type":"basic","front":"Question","back":"![image](https://example.com/image.png)"}
          ]
        }"#;
        let error = match prepare_flashcard_import_bytes(invalid) {
            Ok(_) => panic!("invalid cards should be reported together"),
            Err(error) => error,
        };
        assert_eq!(error.code, "invalid_flashcard_import_cards");
        assert!(error.message.contains("cards[0].front"));
        assert!(error.message.contains("cards[1].back"));
    }

    #[test]
    fn large_invalid_batches_report_a_bounded_example_set() {
        let cards = (0..50)
            .map(|_| r#"{"type":"basic","front":"","back":"Answer"}"#)
            .collect::<Vec<_>>()
            .join(",");
        let invalid = format!(
            r#"{{"format":"ostudy-flashcards","formatVersion":1,"deck":{{"mode":"create","name":"Invalid cards"}},"cards":[{cards}]}}"#
        );
        let error = match prepare_flashcard_import_bytes(invalid.as_bytes()) {
            Ok(_) => panic!("invalid cards should be summarized"),
            Err(error) => error,
        };
        assert!(error.message.starts_with("50 cards could not be imported:"));
        assert!(error.message.contains("30 additional errors were omitted"));
        assert!(!error.message.contains("cards[20].front"));
    }
}
