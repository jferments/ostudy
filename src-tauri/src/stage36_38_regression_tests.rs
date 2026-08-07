use rusqlite::{params, Connection};

use crate::{
    analytics::{load_analytics, AnalyticsQueryDto},
    database::Database,
    deletion_purge::purge_finalized_in_connection,
    study_calendar::StudyTimeContext,
    studyability::CardEligibility,
};

#[derive(Clone)]
struct SeededCard {
    note_id: String,
    card_id: String,
    review_id: String,
}

struct ReviewSeed {
    deck_id: String,
    note_id: String,
    card_id: String,
    review_id: String,
    submission_id: String,
    captured_deck_name: String,
}

fn insert_deck(connection: &Connection, id: &str, name: &str, sort_order: i64, now: i64) {
    connection
        .execute(
            "INSERT INTO decks (
                 id, name, description, sort_order, created_at, updated_at, revision
             ) VALUES (?1, ?2, '', ?3, ?4, ?4, 1)",
            params![id, name, sort_order, now],
        )
        .expect("deck should insert");
}

fn insert_reviewed_card(
    connection: &Connection,
    seed: &ReviewSeed,
    time: &StudyTimeContext,
) -> SeededCard {
    let front_doc = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Original question"}]}]}"#;
    let back_doc = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Original answer"}]}]}"#;
    let now = time.now_utc_millis;
    connection
        .execute(
            "INSERT INTO notes (
                 id, deck_id, note_type, content_format, content_schema_version,
                 front_doc, back_doc, front_text, back_text,
                 created_at, updated_at, revision, duplicate_hash,
                 reverse_enabled, cloze_source, cloze_mode
             ) VALUES (
                 ?1, ?2, 'basic', 'tiptap-json', 1,
                 ?3, ?4, 'Original question', 'Original answer',
                 ?5, ?5, 1, ?6, 0, '', 'one_at_a_time'
             )",
            params![
                seed.note_id,
                seed.deck_id,
                front_doc,
                back_doc,
                now,
                "d".repeat(64)
            ],
        )
        .expect("note should insert");
    connection
        .execute(
            "INSERT INTO cards (
                 id, note_id, deck_id, direction, template_ordinal,
                 created_at, updated_at
             ) VALUES (?1, ?2, ?3, 'forward', 0, ?4, ?4)",
            params![seed.card_id, seed.note_id, seed.deck_id, now],
        )
        .expect("card should insert");
    connection
        .execute(
            "INSERT INTO card_schedules (
                 card_id, state, due_at, stability, difficulty,
                 elapsed_days, scheduled_days, reps, lapses, last_review_at,
                 scheduler_name, scheduler_version, updated_at
             ) VALUES (
                 ?1, 2, ?2, 2.0, 5.0,
                 1, 1, 1, 0, ?3,
                 'fsrs', 'rs-fsrs-1.2.1', ?3
             )",
            params![seed.card_id, now.saturating_add(86_400_000), now],
        )
        .expect("schedule should insert");
    connection
        .execute(
            "INSERT INTO review_events (
                 id, submission_id, session_id, card_id, reviewed_at,
                 rating, answer_correct, confidence, response_time_ms,
                 state_before, state_after, due_before, due_after,
                 stability_before, stability_after,
                 difficulty_before, difficulty_after,
                 scheduled_days_before, scheduled_days_after,
                 scheduler_name, scheduler_version, created_at
             ) VALUES (
                 ?1, ?2, NULL, ?3, ?4,
                 3, 1, 'somewhat_confident', 1200,
                 1, 2, ?5, ?6,
                 1.0, 2.0,
                 5.0, 5.0,
                 1, 2,
                 'fsrs', 'rs-fsrs-1.2.1', ?4
             )",
            params![
                seed.review_id,
                seed.submission_id,
                seed.card_id,
                now,
                now.saturating_sub(1_000),
                now.saturating_add(86_400_000),
            ],
        )
        .expect("review should insert");
    connection
        .execute(
            "INSERT INTO review_event_attributions (
                 review_event_id, reviewed_at, review_local_date,
                 time_zone_id, utc_offset_minutes,
                 card_id, note_id, note_revision,
                 owner_deck_id_at_review, captured_deck_name,
                 direction, card_type, card_label, presentation_variant,
                 question_excerpt, question_text_hash, question_was_truncated,
                 attribution_source
             ) VALUES (
                 ?1, ?2, ?3,
                 ?4, ?5,
                 ?6, ?7, 1,
                 ?8, ?9,
                 'forward', 'basic', 'Basic', 'forward',
                 'Original question', ?10, 0,
                 'captured'
             )",
            params![
                seed.review_id,
                now,
                time.local_date.to_string(),
                time.time_zone_id,
                time.utc_offset_minutes,
                seed.card_id,
                seed.note_id,
                seed.deck_id,
                seed.captured_deck_name,
                "f".repeat(64),
            ],
        )
        .expect("review attribution should insert");
    SeededCard {
        note_id: seed.note_id.clone(),
        card_id: seed.card_id.clone(),
        review_id: seed.review_id.clone(),
    }
}

fn all_time_query() -> AnalyticsQueryDto {
    AnalyticsQueryDto {
        range: "all".to_string(),
        deck_ids: Vec::new(),
        tag_ids: Vec::new(),
        highlighted_session_id: None,
    }
}

#[test]
fn moved_card_history_follows_current_owner_and_keeps_review_presentation() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let time = StudyTimeContext::capture()?;
            let deck_a = "a".repeat(32);
            let deck_b = "b".repeat(32);
            insert_deck(connection, &deck_a, "Deck A", 0, time.now_utc_millis);
            insert_deck(connection, &deck_b, "Deck B", 1, time.now_utc_millis);
            let card = insert_reviewed_card(
                connection,
                &ReviewSeed {
                    deck_id: deck_a.clone(),
                    note_id: "c".repeat(32),
                    card_id: "d".repeat(32),
                    review_id: "e".repeat(32),
                    submission_id: "f".repeat(32),
                    captured_deck_name: "Deck A".to_string(),
                },
                &time,
            );

            connection
                .execute(
                    "UPDATE notes
                     SET deck_id = ?1, front_text = 'Edited question', revision = revision + 1
                     WHERE id = ?2",
                    params![deck_b, card.note_id],
                )
                .expect("note should move");
            connection
                .execute(
                    "UPDATE cards SET deck_id = ?1 WHERE id = ?2",
                    params![deck_b, card.card_id],
                )
                .expect("card should move");
            connection
                .execute(
                    "UPDATE decks SET name = 'Renamed Deck B', revision = revision + 1
                     WHERE id = ?1",
                    [deck_b.as_str()],
                )
                .expect("destination deck should rename");

            let analytics = load_analytics(connection, all_time_query())?;
            assert_eq!(analytics.history.review_count, 1);
            assert_eq!(analytics.deck_stats.len(), 1);
            assert_eq!(analytics.deck_stats[0].deck_id, deck_b);
            assert_eq!(analytics.deck_stats[0].deck_name, "Renamed Deck B");
            assert_eq!(analytics.deck_stats[0].review_count, 1);
            assert_eq!(analytics.recent_reviews.len(), 1);
            assert_eq!(analytics.recent_reviews[0].deck_name, "Renamed Deck B");
            assert_eq!(analytics.recent_reviews[0].captured_deck_name, "Deck A");
            assert_eq!(
                analytics.recent_reviews[0].question_excerpt,
                "Original question"
            );
            Ok(())
        })
        .expect("ownership analytics should remain coherent");
}

#[test]
fn deleting_old_owner_preserves_moved_history_and_deleting_new_owner_purges_it() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let time = StudyTimeContext::capture()?;
            let deck_a = "1".repeat(32);
            let deck_b = "2".repeat(32);
            insert_deck(connection, &deck_a, "Old owner", 0, time.now_utc_millis);
            insert_deck(connection, &deck_b, "New owner", 1, time.now_utc_millis);
            let card = insert_reviewed_card(
                connection,
                &ReviewSeed {
                    deck_id: deck_a.clone(),
                    note_id: "3".repeat(32),
                    card_id: "4".repeat(32),
                    review_id: "5".repeat(32),
                    submission_id: "6".repeat(32),
                    captured_deck_name: "Old owner".to_string(),
                },
                &time,
            );
            connection
                .execute(
                    "UPDATE notes SET deck_id = ?1 WHERE id = ?2",
                    params![deck_b, card.note_id],
                )
                .expect("note should move");
            connection
                .execute(
                    "UPDATE cards SET deck_id = ?1 WHERE id = ?2",
                    params![deck_b, card.card_id],
                )
                .expect("card should move");

            let deleted_at = time.now_utc_millis;
            connection
                .execute(
                    "UPDATE decks SET deleted_at = ?1 WHERE id = ?2",
                    params![deleted_at, deck_a],
                )
                .expect("old deck should tombstone");
            insert_finalized_deck_deletion(connection, &"7".repeat(32), &deck_a, &card, deleted_at);
            purge_finalized_in_connection(connection)?;

            let review_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM review_events WHERE id = ?1",
                    [card.review_id.as_str()],
                    |row| row.get(0),
                )
                .expect("review count should load");
            assert_eq!(review_count, 1);
            let current_owner: String = connection
                .query_row(
                    "SELECT deck_id FROM cards WHERE id = ?1",
                    [card.card_id.as_str()],
                    |row| row.get(0),
                )
                .expect("moved card should remain");
            assert_eq!(current_owner, deck_b);

            connection
                .execute(
                    "UPDATE decks SET deleted_at = ?1 WHERE id = ?2",
                    params![deleted_at, deck_b],
                )
                .expect("new deck should tombstone");
            insert_finalized_deck_deletion(connection, &"8".repeat(32), &deck_b, &card, deleted_at);
            purge_finalized_in_connection(connection)?;

            let remaining_reviews: i64 = connection
                .query_row("SELECT COUNT(*) FROM review_events", [], |row| row.get(0))
                .expect("review count should load");
            let remaining_cards: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM cards WHERE id = ?1",
                    [card.card_id.as_str()],
                    |row| row.get(0),
                )
                .expect("card count should load");
            assert_eq!(remaining_reviews, 0);
            assert_eq!(remaining_cards, 0);
            Ok(())
        })
        .expect("deletion ownership should remain coherent");
}

fn insert_finalized_deck_deletion(
    connection: &Connection,
    operation_id: &str,
    deck_id: &str,
    card: &SeededCard,
    deleted_at: i64,
) {
    connection
        .execute(
            "INSERT INTO deletion_operations (
                 id, entity_kind, subject_label, deck_id,
                 note_ids_json, card_ids_json,
                 deleted_at, expires_at, finalized_at, created_at
             ) VALUES (
                 ?1, 'deck', 'Deleted deck', ?2,
                 ?3, ?4,
                 ?5, ?5, ?5, ?5
             )",
            params![
                operation_id,
                deck_id,
                serde_json::to_string(&vec![card.note_id.clone()]).expect("note ids"),
                serde_json::to_string(&vec![card.card_id.clone()]).expect("card ids"),
                deleted_at,
            ],
        )
        .expect("finalized deletion should insert");
}

#[test]
fn due_boundary_never_overrides_archive_or_manual_suspension() {
    let now = 1_000;
    let active = CardEligibility {
        card_deleted: false,
        note_deleted: false,
        deck_deleted: false,
        deck_archived: false,
        manually_suspended: false,
        archive_suspended: false,
        buried_until: Some(now),
        due_at: now,
    };
    assert!(active.is_studyable_due(now));
    assert!(!CardEligibility {
        deck_archived: true,
        archive_suspended: true,
        ..active
    }
    .is_studyable_due(now));
    assert!(!CardEligibility {
        manually_suspended: true,
        ..active
    }
    .is_studyable_due(now));
}

#[test]
fn schema_fourteen_and_all_three_repairs_are_present_on_canonical_open() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let schema_version: i64 = connection
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .expect("schema version should load");
            let repair_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM data_repairs", [], |row| row.get(0))
                .expect("repair count should load");
            let attribution_tables: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master
                     WHERE type = 'table'
                       AND name IN (
                           'practice_session_card_snapshots',
                           'practice_session_card_tag_snapshots',
                           'review_event_attributions',
                           'review_event_tag_attributions'
                       )",
                    [],
                    |row| row.get(0),
                )
                .expect("attribution tables should load");
            assert_eq!(schema_version, 14);
            assert_eq!(repair_count, 3);
            assert_eq!(attribution_tables, 4);
            Ok(())
        })
        .expect("canonical schema should contain Stage 3.6-3.8 state");
}
