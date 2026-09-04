use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

use rusqlite::{
    params,
    trace::{TraceEvent, TraceEventCodes},
    Connection,
};

use crate::{
    analytics::{load_analytics, AnalyticsQueryDto},
    database::{Database, LATEST_SCHEMA_VERSION},
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

static ANALYTICS_TRACE_LOCK: Mutex<()> = Mutex::new(());
static ANALYTICS_READ_STATEMENT_COUNT: AtomicUsize = AtomicUsize::new(0);

fn count_analytics_read_statements(event: TraceEvent<'_>) {
    if let TraceEvent::Stmt(_, sql) = event {
        let normalized = sql.trim_start().to_ascii_uppercase();
        if normalized.starts_with("SELECT") || normalized.starts_with("WITH") {
            ANALYTICS_READ_STATEMENT_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }
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

fn insert_note_without_card(connection: &Connection, id: &str, deck_id: &str, now: i64) {
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
                 ?3, ?3, 1, ?4, 0, '', 'one_at_a_time'
             )",
            params![id, deck_id, now, format!("{id:0<64}")],
        )
        .expect("note without a card should insert");
}

fn insert_tag(connection: &Connection, id: &str, name: &str, normalized_name: &str, now: i64) {
    connection
        .execute(
            "INSERT INTO tags (id, name, normalized_name, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, name, normalized_name, now],
        )
        .expect("tag should insert");
}

fn attribute_review_tag(
    connection: &Connection,
    review_id: &str,
    tag_id: &str,
    captured_name: &str,
) {
    connection
        .execute(
            "INSERT INTO review_event_tag_attributions (review_event_id, tag_id, tag_name)
             VALUES (?1, ?2, ?3)",
            params![review_id, tag_id, captured_name],
        )
        .expect("review tag attribution should insert");
}

fn assign_note_tag(connection: &Connection, note_id: &str, tag_id: &str) {
    connection
        .execute(
            "INSERT INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
            params![note_id, tag_id],
        )
        .expect("note tag should insert");
}

fn hex_id(value: usize) -> String {
    format!("{value:032x}")
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

struct AdditionalReviewSeed<'a> {
    review_id: &'a str,
    submission_id: &'a str,
    reviewed_at: i64,
    review_local_date: &'a str,
    captured_deck_id: &'a str,
    captured_deck_name: &'a str,
}

fn insert_additional_review(
    connection: &Connection,
    card: &SeededCard,
    seed: &AdditionalReviewSeed<'_>,
) {
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
                card.card_id,
                seed.reviewed_at,
                seed.reviewed_at.saturating_sub(1_000),
                seed.reviewed_at.saturating_add(86_400_000),
            ],
        )
        .expect("additional review should insert");
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
                 'Etc/UTC', 0,
                 ?4, ?5, 1,
                 ?6, ?7,
                 'forward', 'basic', 'Basic', 'forward',
                 'Original question', ?8, 0,
                 'captured'
             )",
            params![
                seed.review_id,
                seed.reviewed_at,
                seed.review_local_date,
                card.card_id,
                card.note_id,
                seed.captured_deck_id,
                seed.captured_deck_name,
                "f".repeat(64),
            ],
        )
        .expect("additional review attribution should insert");
}

fn all_time_query() -> AnalyticsQueryDto {
    AnalyticsQueryDto {
        range: "all".to_string(),
        deck_ids: Vec::new(),
        tag_ids: Vec::new(),
        custom_start_local_date: None,
        custom_end_local_date: None,
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
fn recent_review_tags_preserve_identity_and_match_active_note_currentness() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let time = StudyTimeContext::capture()?;
            let review_deck_id = hex_id(1);
            let other_active_deck_id = hex_id(2);
            let archived_deck_id = hex_id(3);
            let deleted_deck_id = hex_id(4);
            insert_deck(
                connection,
                &review_deck_id,
                "Review deck",
                0,
                time.now_utc_millis,
            );
            insert_deck(
                connection,
                &other_active_deck_id,
                "Other active deck",
                1,
                time.now_utc_millis,
            );
            insert_deck(
                connection,
                &archived_deck_id,
                "Archived deck",
                2,
                time.now_utc_millis,
            );
            insert_deck(
                connection,
                &deleted_deck_id,
                "Deleted deck",
                3,
                time.now_utc_millis,
            );
            connection
                .execute(
                    "UPDATE decks SET archived_at = ?1 WHERE id = ?2",
                    params![time.now_utc_millis, archived_deck_id],
                )
                .expect("deck should archive");
            connection
                .execute(
                    "UPDATE decks SET deleted_at = ?1 WHERE id = ?2",
                    params![time.now_utc_millis, deleted_deck_id],
                )
                .expect("deck should tombstone");

            let tagged_card = insert_reviewed_card(
                connection,
                &ReviewSeed {
                    deck_id: review_deck_id.clone(),
                    note_id: hex_id(5),
                    card_id: hex_id(6),
                    review_id: hex_id(7),
                    submission_id: hex_id(8),
                    captured_deck_name: "Review deck".to_string(),
                },
                &time,
            );
            let tagless_card = insert_reviewed_card(
                connection,
                &ReviewSeed {
                    deck_id: review_deck_id.clone(),
                    note_id: hex_id(9),
                    card_id: hex_id(10),
                    review_id: hex_id(11),
                    submission_id: hex_id(12),
                    captured_deck_name: "Review deck".to_string(),
                },
                &time,
            );

            let no_card_note_id = hex_id(13);
            let archived_note_id = hex_id(14);
            let deleted_note_id = hex_id(15);
            let deleted_deck_note_id = hex_id(16);
            insert_note_without_card(
                connection,
                &no_card_note_id,
                &other_active_deck_id,
                time.now_utc_millis,
            );
            insert_note_without_card(
                connection,
                &archived_note_id,
                &archived_deck_id,
                time.now_utc_millis,
            );
            insert_note_without_card(
                connection,
                &deleted_note_id,
                &review_deck_id,
                time.now_utc_millis,
            );
            insert_note_without_card(
                connection,
                &deleted_deck_note_id,
                &deleted_deck_id,
                time.now_utc_millis,
            );
            connection
                .execute(
                    "UPDATE notes SET deleted_at = ?1 WHERE id = ?2",
                    params![time.now_utc_millis, deleted_note_id],
                )
                .expect("note should tombstone");

            let renamed_active_tag_id = hex_id(20);
            let historical_unassigned_tag_id = hex_id(21);
            let archived_only_tag_id = hex_id(22);
            let deleted_note_only_tag_id = hex_id(23);
            let deleted_deck_only_tag_id = hex_id(24);
            let no_card_active_tag_id = hex_id(25);
            let current_duplicate_tag_id = hex_id(26);
            let historical_duplicate_tag_id = hex_id(27);
            let unit_separator_tag_id = hex_id(28);
            for (tag_id, name, normalized_name) in [
                (&renamed_active_tag_id, "Original active", "original active"),
                (
                    &historical_unassigned_tag_id,
                    "Stored unassigned",
                    "stored unassigned",
                ),
                (&archived_only_tag_id, "Stored archived", "stored archived"),
                (
                    &deleted_note_only_tag_id,
                    "Stored deleted note",
                    "stored deleted note",
                ),
                (
                    &deleted_deck_only_tag_id,
                    "Stored deleted deck",
                    "stored deleted deck",
                ),
                (&no_card_active_tag_id, "No-card current", "no-card current"),
                (&current_duplicate_tag_id, "Shared name", "shared name"),
            ] {
                insert_tag(
                    connection,
                    tag_id,
                    name,
                    normalized_name,
                    time.now_utc_millis,
                );
            }

            assign_note_tag(connection, &tagged_card.note_id, &renamed_active_tag_id);
            assign_note_tag(connection, &archived_note_id, &archived_only_tag_id);
            assign_note_tag(connection, &deleted_note_id, &deleted_note_only_tag_id);
            assign_note_tag(connection, &deleted_deck_note_id, &deleted_deck_only_tag_id);
            assign_note_tag(connection, &no_card_note_id, &no_card_active_tag_id);
            assign_note_tag(connection, &no_card_note_id, &current_duplicate_tag_id);
            connection
                .execute(
                    "UPDATE tags SET name = 'Renamed active', normalized_name = 'renamed active'
                     WHERE id = ?1",
                    [renamed_active_tag_id.as_str()],
                )
                .expect("active tag should rename");

            for (tag_id, captured_name) in [
                (&renamed_active_tag_id, "Original active"),
                (&historical_unassigned_tag_id, "Captured unassigned"),
                (&archived_only_tag_id, "Captured archived"),
                (&deleted_note_only_tag_id, "Captured deleted note"),
                (&deleted_deck_only_tag_id, "Captured deleted deck"),
                (&no_card_active_tag_id, "Captured no-card"),
                (&current_duplicate_tag_id, "Captured shared"),
                (&historical_duplicate_tag_id, "Shared name"),
                (&unit_separator_tag_id, "Medication\u{001f}safety"),
            ] {
                attribute_review_tag(connection, &tagged_card.review_id, tag_id, captured_name);
            }

            let analytics = load_analytics(
                connection,
                AnalyticsQueryDto {
                    deck_ids: vec![review_deck_id],
                    ..all_time_query()
                },
            )?;
            assert_eq!(analytics.recent_reviews.len(), 2);
            assert_eq!(analytics.recent_reviews[0].id, tagless_card.review_id);
            assert!(analytics.recent_reviews[0].tags.is_empty());

            let review = &analytics.recent_reviews[1];
            let tags_by_id = review
                .tags
                .iter()
                .map(|tag| (tag.tag_id.as_str(), (tag.tag_name.as_str(), tag.is_current)))
                .collect::<std::collections::BTreeMap<_, _>>();
            assert_eq!(
                tags_by_id.get(renamed_active_tag_id.as_str()),
                Some(&("Renamed active", true))
            );
            assert_eq!(
                tags_by_id.get(historical_unassigned_tag_id.as_str()),
                Some(&("Captured unassigned", false))
            );
            assert_eq!(
                tags_by_id.get(archived_only_tag_id.as_str()),
                Some(&("Captured archived", false))
            );
            assert_eq!(
                tags_by_id.get(deleted_note_only_tag_id.as_str()),
                Some(&("Captured deleted note", false))
            );
            assert_eq!(
                tags_by_id.get(deleted_deck_only_tag_id.as_str()),
                Some(&("Captured deleted deck", false))
            );
            assert_eq!(
                tags_by_id.get(no_card_active_tag_id.as_str()),
                Some(&("No-card current", true))
            );
            assert_eq!(
                tags_by_id.get(current_duplicate_tag_id.as_str()),
                Some(&("Shared name", true))
            );
            assert_eq!(
                tags_by_id.get(historical_duplicate_tag_id.as_str()),
                Some(&("Shared name", false))
            );
            assert_eq!(
                tags_by_id.get(unit_separator_tag_id.as_str()),
                Some(&("Medication\u{001f}safety", false))
            );
            let serialized_tag = serde_json::to_value(
                review
                    .tags
                    .iter()
                    .find(|tag| tag.tag_id == renamed_active_tag_id)
                    .expect("renamed active tag should be present"),
            )
            .expect("recent review tag should serialize");
            assert_eq!(
                serialized_tag,
                serde_json::json!({
                    "tagId": renamed_active_tag_id,
                    "tagName": "Renamed active",
                    "isCurrent": true,
                })
            );
            assert_eq!(
                review
                    .tags
                    .iter()
                    .filter(|tag| tag.tag_name == "Shared name")
                    .map(|tag| tag.tag_id.as_str())
                    .collect::<Vec<_>>(),
                vec![
                    current_duplicate_tag_id.as_str(),
                    historical_duplicate_tag_id.as_str(),
                ]
            );
            Ok(())
        })
        .expect("recent review tags should retain structured identity");
}

#[test]
fn recent_review_limit_precedes_tag_fan_out_and_query_count_is_fixed() {
    let _guard = ANALYTICS_TRACE_LOCK
        .lock()
        .expect("analytics trace lock should be available");
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let time = StudyTimeContext::capture()?;
            let deck_id = hex_id(100);
            insert_deck(connection, &deck_id, "Fan-out deck", 0, time.now_utc_millis);
            let oldest_review_id = hex_id(101);
            let card = insert_reviewed_card(
                connection,
                &ReviewSeed {
                    deck_id: deck_id.clone(),
                    note_id: hex_id(102),
                    card_id: hex_id(103),
                    review_id: oldest_review_id.clone(),
                    submission_id: hex_id(104),
                    captured_deck_name: "Fan-out deck".to_string(),
                },
                &time,
            );

            ANALYTICS_READ_STATEMENT_COUNT.store(0, Ordering::SeqCst);
            connection.trace_v2(
                TraceEventCodes::SQLITE_TRACE_STMT,
                Some(count_analytics_read_statements),
            );
            let one_review = load_analytics(connection, all_time_query())?;
            connection.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, None);
            let one_review_statement_count = ANALYTICS_READ_STATEMENT_COUNT.load(Ordering::SeqCst);
            assert_eq!(one_review.recent_reviews.len(), 1);

            let mut newest_review_id = String::new();
            for index in 1..=20_usize {
                let review_id = hex_id(200 + index);
                insert_additional_review(
                    connection,
                    &card,
                    &AdditionalReviewSeed {
                        review_id: &review_id,
                        submission_id: &hex_id(300 + index),
                        reviewed_at: time
                            .now_utc_millis
                            .saturating_add(i64::try_from(index).unwrap_or(i64::MAX)),
                        review_local_date: &time.local_date.to_string(),
                        captured_deck_id: &deck_id,
                        captured_deck_name: "Fan-out deck",
                    },
                );
                newest_review_id = review_id;
            }
            for index in 0..64_usize {
                attribute_review_tag(
                    connection,
                    &newest_review_id,
                    &hex_id(1_000 + index),
                    &format!("Fan-out tag {index:02}"),
                );
            }

            ANALYTICS_READ_STATEMENT_COUNT.store(0, Ordering::SeqCst);
            connection.trace_v2(
                TraceEventCodes::SQLITE_TRACE_STMT,
                Some(count_analytics_read_statements),
            );
            let many_reviews = load_analytics(connection, all_time_query())?;
            connection.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, None);
            let many_review_statement_count = ANALYTICS_READ_STATEMENT_COUNT.load(Ordering::SeqCst);

            assert_eq!(many_review_statement_count, one_review_statement_count);
            assert!(many_review_statement_count > 0);
            assert_eq!(many_reviews.recent_reviews.len(), 20);
            assert_eq!(many_reviews.recent_reviews[0].id, newest_review_id);
            assert_eq!(many_reviews.recent_reviews[0].tags.len(), 64);
            assert!(many_reviews
                .recent_reviews
                .iter()
                .all(|review| review.id != oldest_review_id));
            assert!(many_reviews.recent_reviews[1..]
                .iter()
                .all(|review| review.tags.is_empty()));
            Ok(())
        })
        .expect("recent review hydration should remain bounded under fan-out");
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
fn public_baseline_preserves_stage36_38_attribution_schema() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let schema_version: i64 = connection
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .expect("schema version should load");
            let schema_lineage: String = connection
                .query_row(
                    "SELECT value FROM schema_metadata WHERE key = 'schema_lineage'",
                    [],
                    |row| row.get(0),
                )
                .expect("public schema lineage should load");
            let public_baseline_present: i64 = connection
                .query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM schema_migrations
                         WHERE version = 1 AND name = 'public_baseline'
                     )",
                    [],
                    |row| row.get(0),
                )
                .expect("public baseline ledger entry should load");
            let canonical_migration_present: i64 = connection
                .query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM schema_migrations
                         WHERE version = 2 AND name = 'canonical_documents'
                     )",
                    [],
                    |row| row.get(0),
                )
                .expect("canonical migration ledger entry should load");
            let migration_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                    row.get(0)
                })
                .expect("migration count should load");
            let repair_state: (i64, i64) = connection
                .query_row(
                    "SELECT COUNT(*), COALESCE(SUM(changed_rows), 0) FROM data_repairs",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("repair state should load");
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
            assert_eq!(schema_version, LATEST_SCHEMA_VERSION);
            assert_eq!(schema_lineage, "ostudy-public-v1");
            assert_eq!(public_baseline_present, 1);
            assert_eq!(canonical_migration_present, 1);
            assert_eq!(migration_count, LATEST_SCHEMA_VERSION);
            assert_eq!(repair_state, (3, 0));
            assert_eq!(attribution_tables, 4);
            Ok(())
        })
        .expect("public schema should retain Stage 3.6-3.8 attribution state");
}
