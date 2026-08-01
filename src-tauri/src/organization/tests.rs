use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::params;

use crate::{database::Database, paths::AppPaths};

use super::{
    browse::{browse_cards_in_connection, build_fts_query, BrowseSort, BrowseStatus},
    bulk::{
        bulk_create_cards_in_connection, prepare_rows, BulkCardRowInput, DuplicatePolicy,
    },
    duplicates::find_duplicate_matches,
    tags::normalize_tag_names,
    transfer::{transfer_cards_in_connection, TransferMode},
};

fn temporary_paths() -> (AppPaths, PathBuf) {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be valid")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ostudy-stage2-test-{}-{suffix}",
        std::process::id()
    ));
    let paths = AppPaths::new(root.clone()).expect("temporary paths should initialize");
    (paths, root)
}

#[test]
fn tag_normalization_is_case_insensitive_and_bounded() {
    let tags = normalize_tag_names(vec![
        "  High   Yield ".to_string(),
        "high yield".to_string(),
        "Cardiology".to_string(),
    ])
    .expect("tags should normalize");
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].name, "High Yield");
    assert_eq!(tags[0].normalized_name, "high yield");
    assert!(normalize_tag_names(vec!["bad,tag".to_string()]).is_err());
}

#[test]
fn fts_query_builder_never_passes_raw_search_syntax() {
    assert_eq!(
        build_fts_query("atrial fibrillation"),
        Some("\"atrial\"* AND \"fibrillation\"*".to_string())
    );
    assert_eq!(
        build_fts_query("title:foo OR *"),
        Some("\"titlefoo\"* AND \"OR\"*".to_string())
    );
    assert_eq!(build_fts_query("   "), None);
}

#[test]
fn bulk_tags_search_duplicates_and_transfer_work_together() {
    let database = Database::open_in_memory().expect("database should open");
    let (paths, root) = temporary_paths();
    let deck_a = "a".repeat(32);
    let deck_b = "b".repeat(32);

    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO decks (id, name, description, sort_order, created_at, updated_at, revision)\n\
                     VALUES (?1, 'Cardiology', '', 0, 1, 1, 1)",
                    [&deck_a],
                )
                .map_err(crate::error::CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO decks (id, name, description, sort_order, created_at, updated_at, revision)\n\
                     VALUES (?1, 'Mixed review', '', 1, 1, 1, 1)",
                    [&deck_b],
                )
                .map_err(crate::error::CommandError::database)?;

            let prepared = prepare_rows(vec![
                BulkCardRowInput {
                    front_text: "What rhythm lacks organized atrial activity?".to_string(),
                    back_text: "Atrial fibrillation".to_string(),
                    tags: vec!["Cardiology".to_string(), "High yield".to_string()],
                },
                BulkCardRowInput {
                    front_text: "What rhythm lacks organized atrial activity?".to_string(),
                    back_text: "Atrial fibrillation".to_string(),
                    tags: vec!["duplicate".to_string()],
                },
                BulkCardRowInput {
                    front_text: "Which chamber pumps to the systemic circulation?".to_string(),
                    back_text: "The left ventricle".to_string(),
                    tags: vec!["Cardiology".to_string()],
                },
            ])?;
            let bulk = bulk_create_cards_in_connection(
                connection,
                &deck_a,
                prepared,
                DuplicatePolicy::Skip,
            )?;
            assert_eq!(bulk.created_count, 2);
            assert_eq!(bulk.skipped.len(), 1);

            let search = browse_cards_in_connection(
                connection,
                &paths,
                &deck_a,
                build_fts_query("atrial").as_deref(),
                Vec::new(),
                BrowseStatus::All,
                BrowseSort::Relevance,
                100,
                0,
            )?;
            assert_eq!(search.total, 1);
            assert_eq!(search.items[0].tags.len(), 2);

            let tag_search = browse_cards_in_connection(
                connection,
                &paths,
                &deck_a,
                build_fts_query("Cardiology").as_deref(),
                Vec::new(),
                BrowseStatus::All,
                BrowseSort::Relevance,
                100,
                0,
            )?;
            assert_eq!(tag_search.total, 2);

            let atrial_note_id = search.items[0].card.note_id.clone();
            let duplicates = find_duplicate_matches(
                connection,
                "What rhythm lacks organized atrial activity?",
                "Atrial fibrillation",
                None,
            )?;
            assert_eq!(duplicates.len(), 1);

            let copied = transfer_cards_in_connection(
                connection,
                std::slice::from_ref(&atrial_note_id),
                &deck_b,
                TransferMode::Copy,
            )?;
            assert_eq!(copied.transferred_count, 1);
            assert!(copied.skipped.is_empty());

            let copied_rows = browse_cards_in_connection(
                connection,
                &paths,
                &deck_b,
                None,
                Vec::new(),
                BrowseStatus::All,
                BrowseSort::UpdatedDescending,
                100,
                0,
            )?;
            assert_eq!(copied_rows.total, 1);
            assert_eq!(copied_rows.items[0].tags.len(), 2);
            assert_ne!(copied_rows.items[0].card.note_id, atrial_note_id);
            assert_eq!(copied_rows.items[0].review_count, 0);

            let ventricle_note_id: String = connection
                .query_row(
                    "SELECT id FROM notes WHERE deck_id = ?1 AND front_text LIKE 'Which chamber%'",
                    [&deck_a],
                    |row| row.get(0),
                )
                .map_err(crate::error::CommandError::database)?;
            let moved = transfer_cards_in_connection(
                connection,
                std::slice::from_ref(&ventricle_note_id),
                &deck_b,
                TransferMode::Move,
            )?;
            assert_eq!(moved.transferred_count, 1);

            let source_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM notes WHERE deck_id = ?1 AND deleted_at IS NULL",
                    [&deck_a],
                    |row| row.get(0),
                )
                .map_err(crate::error::CommandError::database)?;
            let destination_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM notes WHERE deck_id = ?1 AND deleted_at IS NULL",
                    [&deck_b],
                    |row| row.get(0),
                )
                .map_err(crate::error::CommandError::database)?;
            assert_eq!(source_count, 1);
            assert_eq!(destination_count, 2);

            let fts_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM card_search WHERE card_search MATCH 'ventricle*' AND deck_id = ?1",
                    params![deck_b],
                    |row| row.get(0),
                )
                .map_err(crate::error::CommandError::database)?;
            assert_eq!(fts_count, 1);
            Ok(())
        })
        .expect("Stage 2 workflows should succeed");

    fs::remove_dir_all(root).expect("temporary paths should be removable");
}
