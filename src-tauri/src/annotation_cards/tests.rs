use rusqlite::params;

use super::*;
use crate::collections::test_support::seed_document;

const DOCUMENT: &str = "dddddddddddddddddddddddddddddddd";
const OTHER_DOCUMENT: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const DECK: &str = "11111111111111111111111111111111";
const GENERATED_DECK: &str = "22222222222222222222222222222222";
const NOTE: &str = "33333333333333333333333333333333";
const OTHER_NOTE: &str = "44444444444444444444444444444444";
const HIGHLIGHT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const STICKY: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const INK: &str = "cccccccccccccccccccccccccccccccc";
const NOW: i64 = 1_700_000_000_000;

fn library() -> Database {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            seed_document(connection, DOCUMENT, 3, 'a')?;
            seed_document(connection, OTHER_DOCUMENT, 1, 'b')?;
            for (deck, name) in [(DECK, "Cardiology"), (GENERATED_DECK, "Heart chapter")] {
                connection
                    .execute(
                        "INSERT INTO decks(id, name, description, sort_order, created_at,
                             updated_at, revision)
                         VALUES (?1, ?2, '', 0, 1, 1, 1)",
                        params![deck, name],
                    )
                    .map_err(CommandError::database)?;
            }
            for note in [NOTE, OTHER_NOTE] {
                insert_note(connection, note, DECK);
            }
            for (id, page, payload) in [
                (
                    HIGHLIGHT,
                    2,
                    r##"{"points":[],"color":"#ffd400","strokeWidth":12,"rects":[10,10,80,12],"text":"  The SA node sets the heart rate.  "}"##,
                ),
                (
                    STICKY,
                    3,
                    r##"{"x":1,"y":1,"width":120,"text":"Why does vagal tone slow it?","color":"#fff3a8","fontSize":12,"sticky":true}"##,
                ),
                (
                    INK,
                    1,
                    r##"{"points":[1,1,0.5,2,2,0.5],"color":"#111111","strokeWidth":2}"##,
                ),
            ] {
                let kind = match id {
                    HIGHLIGHT => "highlight",
                    STICKY => "text",
                    _ => "ink",
                };
                connection
                    .execute(
                        "INSERT INTO document_annotations(
                             id, document_id, page_number, kind, payload_json,
                             revision, created_at, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 1, 1)",
                        params![id, DOCUMENT, page, kind, payload],
                    )
                    .map_err(CommandError::database)?;
            }
            Ok(())
        })
        .unwrap();
    database
}

fn insert_note(connection: &Connection, id: &str, deck: &str) {
    connection
        .execute(
            "INSERT INTO notes (
                 id, deck_id, note_type, content_format, content_schema_version,
                 front_doc, back_doc, front_text, back_text,
                 created_at, updated_at, revision, duplicate_hash,
                 reverse_enabled, cloze_source, cloze_mode
             ) VALUES (
                 ?1, ?2, 'basic', 'tiptap-json', 1,
                 '{}', '{}', 'Question', 'Answer',
                 1, 1, 1, ?3, 0, '', 'one_at_a_time'
             )",
            params![id, deck, format!("{id:0<64}")],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO cards(id, note_id, deck_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, 1, 1)",
            params![card_id(id), id, deck],
        )
        .unwrap();
}

/// Each test note has one card, whose id is the note's with its first digit changed.
fn card_id(note: &str) -> String {
    format!("f{}", &note[1..])
}

fn link(
    database: &Database,
    note: &str,
    annotation: &str,
) -> Result<CardMarkSourceDto, CommandError> {
    database.with_connection(|connection| {
        link_note(
            connection,
            LinkCardNoteToAnnotationInput {
                note_id: note.to_string(),
                annotation_id: annotation.to_string(),
            },
            NOW,
        )
    })
}

fn marked(database: &Database, document: &str) -> Result<DocumentMarkedCardsDto, CommandError> {
    database.with_connection(|connection| marked_cards(connection, document.to_string()))
}

fn citation(database: &Database, note: &str) -> Option<(String, String)> {
    database
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT citation_id, source_locator_json
                     FROM note_source_citations WHERE note_id = ?1",
                    [note],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(CommandError::database)
        })
        .unwrap()
}

fn stored_link(
    database: &Database,
    note: &str,
) -> Option<(Option<String>, Option<String>, i64, String)> {
    database
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT document_id, annotation_id, page_number, quote
                     FROM note_annotation_sources WHERE note_id = ?1",
                    [note],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()
                .map_err(CommandError::database)
        })
        .unwrap()
}

#[test]
fn a_card_made_from_a_highlight_keeps_its_words_page_and_a_page_citation() {
    let database = library();
    let linked = link(&database, NOTE, HIGHLIGHT).unwrap();
    assert_eq!(
        linked,
        CardMarkSourceDto {
            note_id: NOTE.to_string(),
            document_id: Some(DOCUMENT.to_string()),
            annotation_id: Some(HIGHLIGHT.to_string()),
            page_number: 2,
            quote: "The SA node sets the heart rate.".to_string(),
        }
    );
    let (citation_id, locator) = citation(&database, NOTE).expect("the note cites its page");
    assert_eq!(citation_id, DOCUMENT);
    let locator: serde_json::Value = serde_json::from_str(&locator).unwrap();
    assert_eq!(locator["unitKind"], "page");
    assert_eq!(locator["unitNumbers"], serde_json::json!([2]));

    // Repeating the request changes nothing further.
    assert_eq!(link(&database, NOTE, HIGHLIGHT).unwrap(), linked);
    let rows: i64 = database
        .with_connection(|connection| {
            connection
                .query_row("SELECT COUNT(*) FROM note_annotation_sources", [], |row| {
                    row.get(0)
                })
                .map_err(CommandError::database)
        })
        .unwrap();
    assert_eq!(rows, 1);
}

#[test]
fn notes_give_their_text_and_strokes_give_no_words() {
    let database = library();
    assert_eq!(
        link(&database, NOTE, STICKY).unwrap().quote,
        "Why does vagal tone slow it?"
    );
    let ink = link(&database, OTHER_NOTE, INK).unwrap();
    assert_eq!((ink.page_number, ink.quote.as_str()), (1, ""));
}

#[test]
fn relinking_moves_the_link_but_keeps_the_first_citation() {
    let database = library();
    link(&database, NOTE, HIGHLIGHT).unwrap();
    let first = citation(&database, NOTE);
    let moved = link(&database, NOTE, STICKY).unwrap();
    assert_eq!(moved.page_number, 3);
    assert_eq!(
        stored_link(&database, NOTE).unwrap(),
        (
            Some(DOCUMENT.to_string()),
            Some(STICKY.to_string()),
            3,
            "Why does vagal tone slow it?".to_string()
        )
    );
    assert_eq!(citation(&database, NOTE), first);
}

#[test]
fn a_card_generated_from_the_document_keeps_its_own_citation() {
    let database = library();
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO source_citation_snapshots (
                         id, schema_version, source_document_id, source_identity,
                         source_fingerprint_sha256, source_title, original_filename,
                         source_format, source_media_type, source_unit_kind,
                         source_unit_count, source_removed, created_at, updated_at
                     )
                     SELECT id, 1, id, 'sha256:' || sha256, sha256, title, original_filename,
                            source_format, source_media_type, source_unit_kind,
                            source_unit_count, 0, created_at, updated_at
                     FROM documents WHERE id = ?1",
                    [DOCUMENT],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO note_source_citations(
                         note_id, citation_id, source_locator_json, created_at, updated_at
                     ) VALUES (?1, ?2, '{\"unitKind\":\"page\",\"unitNumbers\":[1]}', 1, 1)",
                    params![NOTE, DOCUMENT],
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    let before = citation(&database, NOTE);
    link(&database, NOTE, HIGHLIGHT).unwrap();
    assert_eq!(citation(&database, NOTE), before);
}

#[test]
fn missing_marks_deleted_cards_and_bad_ids_are_refused_without_writing() {
    let database = library();
    let missing = link(&database, NOTE, &"9".repeat(32)).unwrap_err();
    assert_eq!(missing.code, "document_annotation_not_found");
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE notes SET deleted_at = 5 WHERE id = ?1",
                    [OTHER_NOTE],
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    assert_eq!(
        link(&database, OTHER_NOTE, HIGHLIGHT).unwrap_err().code,
        "card_not_found"
    );
    assert_eq!(
        link(&database, "not-an-id", HIGHLIGHT).unwrap_err().code,
        "invalid_identifier"
    );
    assert!(stored_link(&database, NOTE).is_none());
    assert!(stored_link(&database, OTHER_NOTE).is_none());
    assert!(citation(&database, NOTE).is_none());
}

#[test]
fn study_what_i_marked_lists_this_documents_cards_and_survives_erasing_the_mark() {
    let database = library();
    assert_eq!(
        marked(&database, DOCUMENT).unwrap(),
        DocumentMarkedCardsDto {
            document_id: DOCUMENT.to_string(),
            card_ids: Vec::new(),
            suggested_deck_id: None,
        }
    );
    link(&database, NOTE, HIGHLIGHT).unwrap();
    link(&database, OTHER_NOTE, STICKY).unwrap();
    let both = marked(&database, DOCUMENT).unwrap();
    assert_eq!(both.card_ids, vec![card_id(NOTE), card_id(OTHER_NOTE)]);
    assert_eq!(both.suggested_deck_id.as_deref(), Some(DECK));
    assert!(marked(&database, OTHER_DOCUMENT)
        .unwrap()
        .card_ids
        .is_empty());

    // Erasing a mark keeps its card and its link, which still counts as marked
    // here; undoing the erase restores the mark under the same id.
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "DELETE FROM document_annotations WHERE id = ?1",
                    [HIGHLIGHT],
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    assert_eq!(
        stored_link(&database, NOTE).unwrap(),
        (
            Some(DOCUMENT.to_string()),
            Some(HIGHLIGHT.to_string()),
            2,
            "The SA node sets the heart rate.".to_string()
        )
    );
    assert_eq!(marked(&database, DOCUMENT).unwrap().card_ids.len(), 2);

    // A deleted card leaves the session; deleting a note removes its link.
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE cards SET deleted_at = 5 WHERE note_id = ?1",
                    [OTHER_NOTE],
                )
                .map_err(CommandError::database)?;
            connection
                .execute("DELETE FROM notes WHERE id = ?1", [NOTE])
                .map_err(CommandError::database)
        })
        .unwrap();
    assert!(marked(&database, DOCUMENT).unwrap().card_ids.is_empty());
    assert!(stored_link(&database, NOTE).is_none());
}

#[test]
fn new_cards_go_to_the_documents_generated_deck_first() {
    let database = library();
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE documents SET generated_deck_id = ?1 WHERE id = ?2",
                    params![GENERATED_DECK, DOCUMENT],
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    link(&database, NOTE, HIGHLIGHT).unwrap();
    assert_eq!(
        marked(&database, DOCUMENT)
            .unwrap()
            .suggested_deck_id
            .as_deref(),
        Some(GENERATED_DECK)
    );
    // An archived generated deck is not offered.
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE decks SET archived_at = 5 WHERE id = ?1",
                    [GENERATED_DECK],
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    assert_eq!(
        marked(&database, DOCUMENT)
            .unwrap()
            .suggested_deck_id
            .as_deref(),
        Some(DECK)
    );
}

#[test]
fn deleting_the_document_keeps_the_card_and_its_words() {
    let database = library();
    link(&database, NOTE, HIGHLIGHT).unwrap();
    database
        .with_connection(|connection| {
            connection
                .execute("DELETE FROM documents WHERE id = ?1", [DOCUMENT])
                .map_err(CommandError::database)
        })
        .unwrap();
    // The mark's id stays as a plain id; only the document link is cleared.
    assert_eq!(
        stored_link(&database, NOTE).unwrap(),
        (
            None,
            Some(HIGHLIGHT.to_string()),
            2,
            "The SA node sets the heart rate.".to_string()
        )
    );
    assert_eq!(
        marked(&database, DOCUMENT).unwrap_err().code,
        "document_not_found"
    );
}

fn source(database: &Database, note: &str) -> Result<Option<CardMarkSourceDto>, CommandError> {
    database.with_connection(|connection| mark_source(connection, note))
}

#[test]
fn a_cards_source_names_its_mark_until_the_mark_is_erased() {
    let database = library();
    assert_eq!(source(&database, NOTE).unwrap(), None);
    assert_eq!(
        source(&database, "not an id").unwrap_err().code,
        "invalid_identifier"
    );

    let linked = link(&database, NOTE, HIGHLIGHT).unwrap();
    assert_eq!(source(&database, NOTE).unwrap(), Some(linked.clone()));

    // An erased mark leaves the page and words but no mark to open at.
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "DELETE FROM document_annotations WHERE id = ?1",
                    [HIGHLIGHT],
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    assert_eq!(
        source(&database, NOTE).unwrap(),
        Some(CardMarkSourceDto {
            annotation_id: None,
            ..linked
        })
    );
}

#[test]
fn a_question_is_asked_about_a_marks_words_only() {
    let database = library();
    let words = database
        .with_connection(|connection| mark_words(connection, HIGHLIGHT))
        .unwrap();
    assert_eq!(words.page_number, 2);
    assert_eq!(words.words, "The SA node sets the heart rate.");
    assert!(!words.document_title.is_empty());

    let refused = |annotation: &str| {
        database
            .with_connection(|connection| mark_words(connection, annotation))
            .unwrap_err()
            .code
    };
    assert_eq!(refused(INK), "mark_has_no_words");
    assert_eq!(refused(&"9".repeat(32)), "document_annotation_not_found");
    assert_eq!(refused("not an id"), "invalid_identifier");
}
