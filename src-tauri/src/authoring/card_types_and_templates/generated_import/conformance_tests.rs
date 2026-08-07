use super::*;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const VALID_FIXTURE: &str =
    include_str!("../../../../../tests/fixtures/generated-import/valid-conformance.ostudy.json");
const INVALID_CORPUS: &str =
    include_str!("../../../../../tests/fixtures/generated-import/conformance-corpus.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConformanceCorpus {
    invalid_documents: Vec<InvalidDocumentCase>,
    invalid_cards: Vec<InvalidCardCase>,
}

#[derive(Debug, Deserialize)]
struct InvalidDocumentCase {
    name: String,
    document: Value,
}

#[derive(Debug, Deserialize)]
struct InvalidCardCase {
    name: String,
    card: Value,
}

struct TemporaryImportRoot {
    path: PathBuf,
}

impl TemporaryImportRoot {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ostudy-generated-import-conformance-{}-{nanos}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temporary import root should be created");
        Self { path }
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.path.join(name);
        fs::write(&path, bytes).expect("temporary import file should be written");
        path
    }
}

impl Drop for TemporaryImportRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("temporary paths should be UTF-8")
}

fn import_path(database: &Database, path: &Path) -> Result<FlashcardImportResultDto, CommandError> {
    let prepared = prepare_flashcard_import(path_text(path))?;
    database.with_connection(|connection| import_prepared_flashcards(connection, prepared))
}

fn active_counts(database: &Database, deck_id: &str) -> (i64, i64, i64, i64) {
    database
        .with_connection(|connection| {
            let notes: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM notes WHERE deck_id = ?1 AND deleted_at IS NULL",
                    [deck_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            let cards: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM cards WHERE deck_id = ?1 AND deleted_at IS NULL",
                    [deck_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            let schedules: i64 = connection
                .query_row(
                    "SELECT COUNT(*)
                     FROM card_schedules s
                     JOIN cards c ON c.id = s.card_id
                     WHERE c.deck_id = ?1 AND c.deleted_at IS NULL",
                    [deck_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            let reviews: i64 = connection
                .query_row(
                    "SELECT COUNT(*)
                     FROM review_events r
                     JOIN cards c ON c.id = r.card_id
                     WHERE c.deck_id = ?1",
                    [deck_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            Ok((notes, cards, schedules, reviews))
        })
        .expect("test graph counts should be readable")
}

fn delete_test_deck_graph(database: &Database, deck_id: &str) {
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "DELETE FROM card_schedules
                     WHERE card_id IN (SELECT id FROM cards WHERE deck_id = ?1)",
                    [deck_id],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "DELETE FROM note_tags
                     WHERE note_id IN (SELECT id FROM notes WHERE deck_id = ?1)",
                    [deck_id],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "DELETE FROM note_assets
                     WHERE note_id IN (SELECT id FROM notes WHERE deck_id = ?1)",
                    [deck_id],
                )
                .map_err(CommandError::database)?;
            connection
                .execute("DELETE FROM cards WHERE deck_id = ?1", [deck_id])
                .map_err(CommandError::database)?;
            connection
                .execute("DELETE FROM notes WHERE deck_id = ?1", [deck_id])
                .map_err(CommandError::database)?;
            connection
                .execute("DELETE FROM decks WHERE id = ?1", [deck_id])
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "DELETE FROM tags
                     WHERE NOT EXISTS (
                         SELECT 1 FROM note_tags nt WHERE nt.tag_id = tags.id
                     )",
                    [],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .expect("temporary conformance graph should be deleted");

    let remaining = active_counts(database, deck_id);
    assert_eq!(remaining, (0, 0, 0, 0), "test deck graph must be removed");
}

fn wrapped_card(name: &str, card: Value) -> Value {
    json!({
        "format": GENERATED_IMPORT_FORMAT,
        "formatVersion": GENERATED_IMPORT_VERSION,
        "deck": {"mode": "create", "name": format!("Invalid case: {name}")},
        "cards": [card]
    })
}

#[test]
fn valid_conformance_file_imports_every_supported_card_shape_and_cleans_up() {
    let root = TemporaryImportRoot::new();
    let valid_path = root.write("valid-conformance.ostudy.json", VALID_FIXTURE.as_bytes());
    let database = Database::open_in_memory().expect("database should open");

    let created =
        import_path(&database, &valid_path).expect("valid conformance file should import");
    assert_eq!(created.created_note_count, 41);
    assert_eq!(created.created_card_count, 42);
    assert_eq!(created.skipped_duplicate_count, 0);
    assert_eq!(active_counts(&database, &created.deck_id), (41, 42, 42, 0));

    database
        .with_connection(|connection| {
            let reverse_cards: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM cards WHERE deck_id = ?1 AND direction = 'reverse' AND deleted_at IS NULL",
                    [created.deck_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(reverse_cards, 1, "one reverse Basic note should create one reverse presentation");

            let cloze_notes: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM notes WHERE deck_id = ?1 AND note_type = 'cloze' AND deleted_at IS NULL",
                    [created.deck_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            let cloze_cards: i64 = connection
                .query_row(
                    "SELECT COUNT(*)
                     FROM cards c
                     JOIN notes n ON n.id = c.note_id
                     WHERE c.deck_id = ?1 AND n.note_type = 'cloze' AND c.deleted_at IS NULL",
                    [created.deck_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(cloze_notes, 9);
            assert_eq!(cloze_cards, 9, "every Cloze note keeps one stable scheduled card");

            let medication_tags: i64 = connection
                .query_row(
                    "SELECT COUNT(*)
                     FROM tags
                     WHERE normalized_name = 'medication'",
                    [],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(medication_tags, 1, "tag normalization should deduplicate case/whitespace variants");
            Ok(())
        })
        .expect("valid conformance graph should be inspectable");

    let append = json!({
        "format": GENERATED_IMPORT_FORMAT,
        "formatVersion": GENERATED_IMPORT_VERSION,
        "deck": {"mode": "append", "name": "Stage 3.12 generated-import conformance"},
        "cards": [
            {"type":"basic","front":"Plain ASCII question","back":"Plain ASCII answer"},
            {"type":"basic","front":"Unique append question","back":"Unique append answer"}
        ]
    });
    let append_path = root.write(
        "append.ostudy.json",
        serde_json::to_vec_pretty(&append)
            .expect("append fixture should serialize")
            .as_slice(),
    );
    let appended = import_path(&database, &append_path).expect("valid append should import");
    assert_eq!(appended.deck_id, created.deck_id);
    assert_eq!(appended.created_note_count, 1);
    assert_eq!(appended.created_card_count, 1);
    assert_eq!(appended.skipped_duplicate_count, 1);
    assert_eq!(active_counts(&database, &created.deck_id), (42, 43, 43, 0));

    let invalid_append = json!({
        "format": GENERATED_IMPORT_FORMAT,
        "formatVersion": GENERATED_IMPORT_VERSION,
        "deck": {"mode": "append", "name": "Stage 3.12 generated-import conformance"},
        "cards": [
            {"type":"basic","front":"Would be valid","back":"Must not be written"},
            {"type":"basic","front":"![forbidden](https://example.com/x.png)","back":"A"}
        ]
    });
    let invalid_path = root.write(
        "invalid-atomic.ostudy.json",
        serde_json::to_vec_pretty(&invalid_append)
            .expect("invalid fixture should serialize")
            .as_slice(),
    );
    let before = active_counts(&database, &created.deck_id);
    let error =
        import_path(&database, &invalid_path).expect_err("mixed invalid file must fail atomically");
    assert_eq!(error.code, "invalid_flashcard_import_cards");
    assert_eq!(active_counts(&database, &created.deck_id), before);

    delete_test_deck_graph(&database, &created.deck_id);
}

#[test]
fn rejection_corpus_rejects_every_named_invalid_document_and_card() {
    let corpus: ConformanceCorpus =
        serde_json::from_str(INVALID_CORPUS).expect("conformance corpus should be valid JSON");
    let root = TemporaryImportRoot::new();

    for (index, case) in corpus.invalid_documents.into_iter().enumerate() {
        let bytes =
            serde_json::to_vec_pretty(&case.document).expect("invalid document should serialize");
        let path = root.write(&format!("invalid-document-{index}.ostudy.json"), &bytes);
        let result = prepare_flashcard_import(path_text(&path));
        assert!(
            result.is_err(),
            "invalid document unexpectedly accepted: {}",
            case.name
        );
    }

    for (index, case) in corpus.invalid_cards.into_iter().enumerate() {
        let document = wrapped_card(&case.name, case.card);
        let bytes =
            serde_json::to_vec_pretty(&document).expect("invalid card fixture should serialize");
        let path = root.write(&format!("invalid-card-{index}.ostudy.json"), &bytes);
        let result = prepare_flashcard_import(path_text(&path));
        assert!(
            result.is_err(),
            "invalid card unexpectedly accepted: {}",
            case.name
        );
    }

    let malformed = root.write(
        "malformed.ostudy.json",
        br#"{"format":"ostudy-flashcards","#,
    );
    assert!(prepare_flashcard_import(path_text(&malformed)).is_err());

    let invalid_utf8 = root.write("invalid-utf8.ostudy.json", &[0xff, 0xfe, 0xfd]);
    let utf8_error = match prepare_flashcard_import(path_text(&invalid_utf8)) {
        Ok(_) => panic!("invalid UTF-8 should be rejected"),
        Err(error) => error,
    };
    assert_eq!(utf8_error.code, "invalid_flashcard_import_utf8");
}

#[test]
fn generated_import_enforces_file_card_tag_and_cloze_boundaries() {
    let root = TemporaryImportRoot::new();

    let maximum_cards = (0..MAX_GENERATED_IMPORT_CARDS)
        .map(|index| json!({"type":"basic","front":format!("Q{index}"),"back":"A"}))
        .collect::<Vec<_>>();
    let maximum_document = json!({
        "format": GENERATED_IMPORT_FORMAT,
        "formatVersion": GENERATED_IMPORT_VERSION,
        "deck": {"mode":"create","name":"Maximum card count"},
        "cards": maximum_cards
    });
    let maximum_path = root.write(
        "maximum-card-count.ostudy.json",
        serde_json::to_vec(&maximum_document)
            .expect("maximum card-count fixture should serialize")
            .as_slice(),
    );
    prepare_flashcard_import(path_text(&maximum_path))
        .expect("exact maximum source-note count should validate");

    let too_many_cards = (0..=MAX_GENERATED_IMPORT_CARDS)
        .map(|index| json!({"type":"basic","front":format!("Q{index}"),"back":"A"}))
        .collect::<Vec<_>>();
    let too_many_document = json!({
        "format": GENERATED_IMPORT_FORMAT,
        "formatVersion": GENERATED_IMPORT_VERSION,
        "deck": {"mode":"create","name":"Too many cards"},
        "cards": too_many_cards
    });
    let too_many_path = root.write(
        "too-many-cards.ostudy.json",
        serde_json::to_vec(&too_many_document)
            .expect("too-many fixture should serialize")
            .as_slice(),
    );
    let too_many = match prepare_flashcard_import(path_text(&too_many_path)) {
        Ok(_) => panic!("maximum plus one source note should fail"),
        Err(error) => error,
    };
    assert_eq!(too_many.code, "too_many_flashcard_import_cards");

    let over_size = root.write(
        "too-large.ostudy.json",
        &vec![b' '; MAX_GENERATED_IMPORT_BYTES + 1],
    );
    let too_large = match prepare_flashcard_import(path_text(&over_size)) {
        Ok(_) => panic!("maximum file bytes plus one should fail"),
        Err(error) => error,
    };
    assert_eq!(too_large.code, "invalid_flashcard_import_size");

    let max_tag = "x".repeat(MAX_TAG_CHARS);
    let valid_tag = wrapped_card(
        "maximum tag length",
        json!({"type":"basic","front":"Q","back":"A","tags":[max_tag]}),
    );
    prepare_flashcard_import_bytes(
        &serde_json::to_vec(&valid_tag).expect("valid tag boundary should serialize"),
    )
    .expect("exact maximum tag length should validate");

    let max_source_padding = MAX_CLOZE_SOURCE_CHARS - "{{c1::x}}".chars().count();
    let max_source = format!("{}{{{{c1::x}}}}", "a".repeat(max_source_padding));
    assert_eq!(max_source.chars().count(), MAX_CLOZE_SOURCE_CHARS);
    let valid_cloze = wrapped_card(
        "maximum cloze source",
        json!({"type":"cloze","text":max_source}),
    );
    prepare_flashcard_import_bytes(
        &serde_json::to_vec(&valid_cloze).expect("valid cloze boundary should serialize"),
    )
    .expect("exact maximum Cloze source length should validate");

    let over_source = format!("{}{{{{c1::x}}}}", "a".repeat(max_source_padding + 1));
    let invalid_cloze = wrapped_card(
        "overlong cloze source",
        json!({"type":"cloze","text":over_source}),
    );
    assert!(prepare_flashcard_import_bytes(
        &serde_json::to_vec(&invalid_cloze).expect("invalid cloze boundary should serialize")
    )
    .is_err());
}

#[test]
fn append_requires_one_exact_active_destination_and_create_allows_duplicate_names() {
    let database = Database::open_in_memory().expect("database should open");

    let create = |front: &str| {
        prepare_flashcard_import_bytes(
            serde_json::to_string(&json!({
                "format": GENERATED_IMPORT_FORMAT,
                "formatVersion": GENERATED_IMPORT_VERSION,
                "deck": {"mode":"create","name":"Same display name"},
                "cards": [{"type":"basic","front":front,"back":"A"}]
            }))
            .expect("fixture should serialize")
            .as_bytes(),
        )
    };
    database
        .with_connection(|connection| import_prepared_flashcards(connection, create("Q1")?))
        .expect("first duplicate-name create should succeed");
    database
        .with_connection(|connection| import_prepared_flashcards(connection, create("Q2")?))
        .expect("second duplicate-name create should succeed");

    let append = prepare_flashcard_import_bytes(
        serde_json::to_string(&json!({
            "format": GENERATED_IMPORT_FORMAT,
            "formatVersion": GENERATED_IMPORT_VERSION,
            "deck": {"mode":"append","name":"Same display name"},
            "cards": [{"type":"basic","front":"Q3","back":"A"}]
        }))
        .expect("append fixture should serialize")
        .as_bytes(),
    )
    .expect("append content should prepare");
    let ambiguous = database
        .with_connection(|connection| import_prepared_flashcards(connection, append))
        .expect_err("ambiguous exact-name append must fail");
    assert_eq!(ambiguous.code, "generated_import_deck_ambiguous");

    let missing = prepare_flashcard_import_bytes(
        serde_json::to_string(&json!({
            "format": GENERATED_IMPORT_FORMAT,
            "formatVersion": GENERATED_IMPORT_VERSION,
            "deck": {"mode":"append","name":"same display name"},
            "cards": [{"type":"basic","front":"Q4","back":"A"}]
        }))
        .expect("missing fixture should serialize")
        .as_bytes(),
    )
    .expect("missing destination content should prepare");
    let missing_error = database
        .with_connection(|connection| import_prepared_flashcards(connection, missing))
        .expect_err("case-different destination must not be treated as an exact match");
    assert_eq!(missing_error.code, "generated_import_deck_not_found");
}
