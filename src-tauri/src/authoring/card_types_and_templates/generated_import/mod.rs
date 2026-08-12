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

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[cfg(test)]
use apply::import_prepared_flashcards;
use apply::import_prepared_flashcards_with_commit;
#[cfg(test)]
use prepare::prepare_flashcard_import;
use prepare::prepare_flashcard_import_bytes;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportGeneratedFlashcardsInput {
    pub staged_file_id: String,
}

#[tauri::command]
pub fn import_generated_flashcards(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ImportGeneratedFlashcardsInput,
) -> Result<FlashcardImportResultDto, CommandError> {
    validate_entity_id(&input.staged_file_id, "staged generated flashcards")?;
    if let Some(result) = database.with_connection(|connection| {
        crate::external_files::consumed_import_result::<FlashcardImportResultDto>(
            connection,
            &input.staged_file_id,
        )
    })? {
        return Ok(result);
    }
    let staged = database.with_connection(|connection| {
        crate::external_files::resolve_staged_import(
            connection,
            paths.inner(),
            &input.staged_file_id,
            crate::external_files::ExternalImportPurpose::StudyContent,
            &[crate::external_files::ExternalFileKind::GeneratedJson],
        )
    })?;
    let bytes = std::fs::read(&staged.path).map_err(CommandError::io)?;
    let prepared = prepare_flashcard_import_bytes(&bytes)?;
    database.with_connection(|connection| {
        import_prepared_flashcards_with_commit(connection, prepared, |connection, result, now| {
            crate::external_files::mark_import_consumed(
                connection,
                &input.staged_file_id,
                result,
                now,
            )
        })
    })
}

pub(crate) fn import_generated_flashcards_bytes_with_commit<F>(
    database: &Database,
    bytes: &[u8],
    before_commit: F,
) -> Result<FlashcardImportResultDto, CommandError>
where
    F: FnOnce(&rusqlite::Connection, &FlashcardImportResultDto, i64) -> Result<(), CommandError>,
{
    let prepared = prepare_flashcard_import_bytes(bytes)?;
    database.with_connection(|connection| {
        import_prepared_flashcards_with_commit(connection, prepared, before_commit)
    })
}

mod markdown;
#[cfg(test)]
use markdown::markdown_document;

#[cfg(test)]
mod conformance_tests;

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
    fn markdown_allows_unsupported_looking_text_inside_code_contexts() {
        let inline = markdown_document("`<script>x</script> ![not-image](x) | A | B |`", "front")
            .expect("inline code should preserve literal syntax-looking text");
        crate::cards::validate_document(&inline).expect("inline code should validate");

        let fenced = markdown_document(
            "```html\n<script>x</script>\n![not-image](x)\n| A | B |\n```",
            "front",
        )
        .expect("fenced code should preserve literal syntax-looking text");
        crate::cards::validate_document(&fenced).expect("fenced code should validate");
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
    fn commit_hook_failure_rolls_back_generated_cards_and_deck() {
        let database = Database::open_in_memory().expect("database should open");
        let error = import_generated_flashcards_bytes_with_commit(
            &database,
            &generated_file("create", "Atomic generation", "Question", "Answer"),
            |_, _, _| {
                Err(CommandError::new(
                    "test_commit_hook_failed",
                    "The test provenance write failed.",
                ))
            },
        )
        .expect_err("hook failure should abort the whole authoring transaction");
        assert_eq!(error.code, "test_commit_hook_failed");
        database
            .with_connection(|connection| {
                let deck_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM decks WHERE name = 'Atomic generation'",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                let note_count: i64 = connection
                    .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                assert_eq!(deck_count, 0);
                assert_eq!(note_count, 0);
                Ok(())
            })
            .expect("rolled-back authoring state should be readable");
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
