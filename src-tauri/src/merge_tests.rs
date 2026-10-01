use super::*;
use crate::{
    authoring::{insert_organized_card, validate_tags},
    cards::validate_content,
    decks::{create_deck_in_connection, CreateDeckInput},
};
use serde_json::{json, Value};

fn id(value: u32) -> String {
    format!("{value:032x}")
}

fn rich(text: &str) -> Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
}

fn create_note(
    connection: &mut Connection,
    deck_id: &str,
    label: &str,
    at: i64,
) -> Result<(String, String), CommandError> {
    let content = validate_content(
        rich(label),
        rich(&format!("Answer {label}")),
        label.to_string(),
        format!("Answer {label}"),
    )?;
    let transaction = connection.transaction().map_err(CommandError::database)?;
    let note_id = insert_organized_card(
        &transaction,
        deck_id,
        content,
        &validate_tags(vec!["merge".to_string()])?,
        &format!("merge-{label}"),
        at,
    )?;
    let card_id: String = transaction
        .query_row(
            "SELECT id FROM cards WHERE note_id=?1 AND deleted_at IS NULL",
            [&note_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok((note_id, card_id))
}

fn seed(connection: &mut Connection) -> Result<(crate::decks::DeckSummaryDto, crate::decks::DeckSummaryDto, String, String), CommandError> {
    let source = create_deck_in_connection(
        connection,
        CreateDeckInput {
            name: "Source".to_string(),
            description: None,
        },
    )?;
    let destination = create_deck_in_connection(
        connection,
        CreateDeckInput {
            name: "Destination".to_string(),
            description: None,
        },
    )?;
    let (note_id, card_id) = create_note(connection, &source.id, "Question", 100)?;
    connection
        .execute(
            "UPDATE card_schedules
             SET state=2,reps=7,lapses=2,stability=3.5,difficulty=4.5,
                 revision=4
             WHERE card_id=?1",
            [&card_id],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO review_events (
                 id,submission_id,session_id,card_id,reviewed_at,rating,answer_correct,
                 confidence,response_time_ms,state_before,state_after,due_before,due_after,
                 stability_before,stability_after,difficulty_before,difficulty_after,
                 scheduled_days_before,scheduled_days_after,scheduler_name,scheduler_version,
                 created_at,request_fingerprint,result_completed,result_rated_count,
                 result_correct_count
             ) VALUES(?1,?2,NULL,?3,123,3,1,'somewhat_confident',100,0,2,100,5000,
                 0.0,3.5,0.0,4.5,0,5,'fsrs','test',123,?4,1,1,1)",
            params![id(801), id(802), card_id, "f".repeat(64)],
        )
        .map_err(CommandError::database)?;
    Ok((source, destination, note_id, card_id))
}

fn request(
    operation: u32,
    source: &crate::decks::DeckSummaryDto,
    destination: &crate::decks::DeckSummaryDto,
    notes: i64,
    cards: i64,
) -> MergeDecksInput {
    MergeDecksInput {
        operation_id: id(operation),
        source_deck_id: source.id.clone(),
        destination_deck_id: destination.id.clone(),
        source_revision: source.revision,
        destination_revision: destination.revision,
        expected_source_note_count: notes,
        expected_source_card_count: cards,
    }
}

fn dump(connection: &Connection, query: &str, owner: &str) -> Vec<Vec<String>> {
    let mut statement = connection.prepare(query).unwrap();
    let columns = statement.column_count();
    let mut rows = statement.query([owner]).unwrap();
    let mut result = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        result.push(
            (0..columns)
                .map(|index| format!("{:?}", row.get_ref(index).unwrap()))
                .collect(),
        );
    }
    result
}

#[test]
fn merge_moves_identity_not_history_and_retargets_future_generated_material() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let (source, destination, note_id, card_id) = seed(connection)?;
            let document_id = id(900);
            connection
                .execute(
                    "INSERT INTO documents(
                         id,title,original_filename,mime_type,byte_size,sha256,
                         source_relative_path,lifecycle_state,generated_deck_id,
                         created_at,updated_at
                     ) VALUES(?1,'Merge source','merge.pdf','application/pdf',1,?2,
                         'documents/merge/source.pdf','ready',?3,1,1)",
                    params![document_id, "d".repeat(64), source.id],
                )
                .map_err(CommandError::database)?;
            let target_id = id(901);
            connection
                .execute(
                    "INSERT INTO study_targets(
                         id,target_kind,document_id,deck_id,created_at,updated_at
                     ) VALUES(?1,'document',?2,?3,1,1)",
                    params![target_id, document_id, source.id],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO source_citation_snapshots(
                         id,source_document_id,source_identity,source_fingerprint_sha256,
                         source_title,original_filename,source_format,source_media_type,
                         source_unit_kind,source_unit_count,created_at,updated_at
                     ) VALUES(?1,?1,'sha256:merge',?2,'Merge source','merge.pdf',
                         'pdf','application/pdf','page',1,1,1)",
                    params![document_id, "d".repeat(64)],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO deck_source_citations(deck_id,citation_id,created_at)
                     VALUES(?1,?2,1)",
                    params![source.id, document_id],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO note_source_citations(
                         note_id,citation_id,source_locator_json,created_at,updated_at
                     ) VALUES(?1,?2,'{\"unitKind\":\"page\",\"unitNumbers\":[1],\"sectionLabel\":null,\"assetIds\":[]}',1,1)",
                    params![note_id, document_id],
                )
                .map_err(CommandError::database)?;

            let note_before = dump(connection, "SELECT id,revision,front_doc,back_doc,question_definition FROM notes WHERE id=?1", &note_id);
            let schedule_before = dump(connection, "SELECT * FROM card_schedules WHERE card_id=?1", &card_id);
            let reviews_before = dump(connection, "SELECT * FROM review_events WHERE card_id=?1", &card_id);
            let input = request(1, &source, &destination, 1, 1);
            let result = merge_in_connection(connection, input.clone())?;

            assert_eq!(result.moved_note_count, 1);
            assert_eq!(result.moved_card_count, 1);
            assert_eq!(result.retargeted_document_count, 1);
            assert_eq!(result.retargeted_study_target_count, 1);
            assert_eq!(note_before, dump(connection, "SELECT id,revision,front_doc,back_doc,question_definition FROM notes WHERE id=?1", &note_id));
            assert_eq!(schedule_before, dump(connection, "SELECT * FROM card_schedules WHERE card_id=?1", &card_id));
            assert_eq!(reviews_before, dump(connection, "SELECT * FROM review_events WHERE card_id=?1", &card_id));

            let owners: (String, String, String, String) = connection
                .query_row(
                    "SELECT n.deck_id,c.deck_id,d.generated_deck_id,t.deck_id
                     FROM notes n JOIN cards c ON c.note_id=n.id
                     JOIN documents d ON d.id=?2
                     JOIN study_targets t ON t.id=?3
                     WHERE n.id=?1 AND c.id=?4",
                    params![note_id, document_id, target_id, card_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(
                owners,
                (
                    destination.id.clone(),
                    destination.id.clone(),
                    destination.id.clone(),
                    destination.id.clone(),
                )
            );
            let lifecycle: (Option<i64>, Option<i64>) = connection
                .query_row(
                    "SELECT
                         (SELECT archived_at FROM decks WHERE id=?1),
                         (SELECT archived_at FROM decks WHERE id=?2)",
                    params![source.id, destination.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(CommandError::database)?;
            assert!(lifecycle.0.is_some());
            assert!(lifecycle.1.is_none());

            let citation_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM deck_source_citations
                     WHERE deck_id=?1 AND citation_id=?2",
                    params![destination.id, document_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(citation_count, 1);
            let fts_owner: String = connection
                .query_row(
                    "SELECT deck_id FROM card_search_fts WHERE note_id=?1",
                    [&note_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(fts_owner, destination.id);

            connection
                .execute(
                    "UPDATE decks SET name='Renamed destination',revision=revision+1
                     WHERE id=?1",
                    [&destination.id],
                )
                .map_err(CommandError::database)?;
            assert_eq!(merge_in_connection(connection, input.clone())?, result);

            let mut conflicting = input;
            conflicting.expected_source_card_count = 2;
            assert_eq!(
                merge_in_connection(connection, conflicting).unwrap_err().code,
                "deck_merge_operation_conflict"
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn merge_preserves_duplicate_looking_notes_instead_of_deduplicating() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let (source, destination, _, _) = seed(connection)?;
            create_note(connection, &source.id, "Question", 101)?;
            let result = merge_in_connection(
                connection,
                request(2, &source, &destination, 2, 2),
            )?;
            assert_eq!(result.moved_note_count, 2);
            assert_eq!(result.moved_card_count, 2);
            let destination_notes: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM notes
                     WHERE deck_id=?1 AND deleted_at IS NULL",
                    [&destination.id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(destination_notes, 2);
            Ok(())
        })
        .unwrap();
}

#[test]
fn failed_receipt_insert_rolls_back_every_merge_write() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let (source, destination, note_id, card_id) = seed(connection)?;
            connection
                .execute_batch(
                    "CREATE TEMP TRIGGER fail_deck_merge_receipt
                     BEFORE INSERT ON deck_merge_operations
                     BEGIN SELECT RAISE(ABORT,'injected receipt failure'); END;",
                )
                .map_err(CommandError::database)?;
            let note_before = dump(connection, "SELECT * FROM notes WHERE id=?1", &note_id);
            let card_before = dump(connection, "SELECT * FROM cards WHERE id=?1", &card_id);
            let input = request(3, &source, &destination, 1, 1);
            assert!(merge_in_connection(connection, input.clone()).is_err());
            assert_eq!(note_before, dump(connection, "SELECT * FROM notes WHERE id=?1", &note_id));
            assert_eq!(card_before, dump(connection, "SELECT * FROM cards WHERE id=?1", &card_id));
            let lifecycle: (Option<i64>, Option<i64>) = connection
                .query_row(
                    "SELECT
                         (SELECT archived_at FROM decks WHERE id=?1),
                         (SELECT archived_at FROM decks WHERE id=?2)",
                    params![source.id, destination.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(lifecycle, (None, None));
            connection
                .execute_batch("DROP TRIGGER fail_deck_merge_receipt;")
                .map_err(CommandError::database)?;
            assert_eq!(
                merge_in_connection(connection, input)?.moved_card_count,
                1
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn active_generation_and_pending_deletion_block_without_moving_material() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let (source, destination, note_id, _) = seed(connection)?;
            let document_id = id(910);
            connection
                .execute(
                    "INSERT INTO documents(
                         id,title,original_filename,mime_type,byte_size,sha256,
                         source_relative_path,lifecycle_state,generated_deck_id,
                         created_at,updated_at
                     ) VALUES(?1,'Busy','busy.pdf','application/pdf',1,?2,
                         'documents/busy/source.pdf','processing',?3,1,1)",
                    params![document_id, "e".repeat(64), source.id],
                )
                .map_err(CommandError::database)?;
            let input = request(4, &source, &destination, 1, 1);
            assert_eq!(
                merge_in_connection(connection, input.clone()).unwrap_err().code,
                "deck_merge_generation_active"
            );
            let owner: String = connection
                .query_row("SELECT deck_id FROM notes WHERE id=?1", [&note_id], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(owner, source.id);

            connection
                .execute(
                    "UPDATE documents SET lifecycle_state='ready' WHERE id=?1",
                    [&document_id],
                )
                .map_err(CommandError::database)?;
            let (_, destination_card) = create_note(connection, &destination.id, "Destination", 102)?;
            let destination_note: String = connection
                .query_row(
                    "SELECT note_id FROM cards WHERE id=?1",
                    [&destination_card],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            crate::deletion::delete_cards(
                connection,
                vec![destination_note],
                "Destination flashcard".to_string(),
            )?;
            assert_eq!(
                merge_in_connection(connection, input).unwrap_err().code,
                "deck_merge_deletion_pending"
            );
            let owner: String = connection
                .query_row("SELECT deck_id FROM notes WHERE id=?1", [&note_id], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(owner, source.id);
            Ok(())
        })
        .unwrap();
}

#[test]
fn stale_counts_and_same_destination_fail_before_any_move() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let (source, destination, note_id, _) = seed(connection)?;
            let mut stale = request(5, &source, &destination, 2, 1);
            assert_eq!(
                merge_in_connection(connection, stale.clone()).unwrap_err().code,
                "deck_merge_source_changed"
            );
            stale.destination_deck_id = stale.source_deck_id.clone();
            assert_eq!(
                merge_in_connection(connection, stale).unwrap_err().code,
                "same_destination_deck"
            );
            let owner: String = connection
                .query_row("SELECT deck_id FROM notes WHERE id=?1", [&note_id], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(owner, source.id);
            Ok(())
        })
        .unwrap();
}
