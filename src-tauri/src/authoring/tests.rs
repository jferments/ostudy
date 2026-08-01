#[cfg(test)]
mod tests {
    use super::*;
    use crate::decks::{create_deck_in_connection, CreateDeckInput};

    fn create_test_deck(database: &Database, name: &str) -> String {
        database
            .with_connection(|connection| {
                create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: name.to_string(),
                        description: None,
                    },
                )
            })
            .expect("deck should be created")
            .id
    }

    #[test]
    fn normalizes_tags_and_duplicate_text() {
        let tags = validate_tags(vec![
            "  Cardiology  ".to_string(),
            "cardiology".to_string(),
            " beta blocker ".to_string(),
        ])
        .expect("tags should validate");
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "Cardiology");
        assert_eq!(
            normalized_pair("  What\n is ATP? ", " Energy\tcurrency "),
            ("what is atp?".to_string(), "energy currency".to_string())
        );
    }

    #[test]
    fn creates_searches_and_detects_tagged_cards() {
        let database = Database::open_in_memory().expect("database should open");
        let temporary = std::env::temp_dir().join(format!(
            "ostudy-authoring-test-{}",
            unix_time_millis().expect("clock")
        ));
        let paths = AppPaths::new(temporary.clone()).expect("paths should initialize");
        let deck_id = create_test_deck(&database, "Pharmacology");

        let note_id = database
            .with_connection(|connection| {
                let content = validate_content(
                    plain_text_document("What reverses warfarin?"),
                    plain_text_document("Vitamin K"),
                    "What reverses warfarin?".to_string(),
                    "Vitamin K".to_string(),
                )?;
                let normalized = normalized_pair(&content.front_text, &content.back_text);
                let transaction = connection.transaction().map_err(CommandError::database)?;
                let note_id = insert_organized_card(
                    &transaction,
                    &deck_id,
                    content,
                    &validate_tags(vec!["Anticoagulants".to_string()])?,
                    &duplicate_hash(&normalized.0, &normalized.1),
                    unix_time_millis()?,
                )?;
                transaction.commit().map_err(CommandError::database)?;
                Ok(note_id)
            })
            .expect("card should be created");

        let page = database
            .with_connection(|connection| {
                search_cards_in_connection(
                    connection,
                    SearchCardsInput {
                        deck_id: Some(deck_id.clone()),
                        query: "warfarin".to_string(),
                        tag_ids: vec![],
                        due_filter: "all".to_string(),
                        sort: "relevance".to_string(),
                        limit: 100,
                        offset: 0,
                    },
                )
            })
            .expect("search should work");
        assert_eq!(page.total_count, 1);
        assert_eq!(page.cards[0].tags, vec!["Anticoagulants"]);

        let tag_page = database
            .with_connection(|connection| {
                search_cards_in_connection(
                    connection,
                    SearchCardsInput {
                        deck_id: Some(deck_id.clone()),
                        query: "anticoagulants".to_string(),
                        tag_ids: vec![],
                        due_filter: "all".to_string(),
                        sort: "relevance".to_string(),
                        limit: 100,
                        offset: 0,
                    },
                )
            })
            .expect("tag text should be searchable");
        assert_eq!(tag_page.total_count, 1);

        let organized = database
            .with_connection(|connection| hydrate_organized_card(connection, &paths, &note_id))
            .expect("card should hydrate");
        assert_eq!(organized.tags, vec!["Anticoagulants"]);

        let duplicates = database
            .with_connection(|connection| {
                let normalized = normalized_pair(
                    " what reverses   WARFARIN? ",
                    "vitamin k",
                );
                exact_duplicates(
                    connection,
                    &deck_id,
                    &duplicate_hash(&normalized.0, &normalized.1),
                    &normalized.0,
                    &normalized.1,
                    None,
                    20,
                )
            })
            .expect("duplicate lookup should work");
        assert_eq!(duplicates.len(), 1);

        let _ = std::fs::remove_dir_all(temporary);
    }

    #[test]
    fn bulk_entry_skips_exact_duplicates() {
        let database = Database::open_in_memory().expect("database should open");
        let deck_id = create_test_deck(&database, "Biology");
        let input = BulkCreateCardsInput {
            deck_id: deck_id.clone(),
            rows: vec![
                BulkCardRowInput {
                    question: "What is ATP?".to_string(),
                    answer: "Energy currency".to_string(),
                    tags: vec!["Cell".to_string()],
                },
                BulkCardRowInput {
                    question: " what  is atp? ".to_string(),
                    answer: "energy currency".to_string(),
                    tags: vec!["Cell".to_string()],
                },
            ],
            skip_duplicates: true,
        };
        let prepared = input
            .rows
            .into_iter()
            .enumerate()
            .map(|(index, row)| prepare_bulk_row(index, row))
            .collect::<Result<Vec<_>, _>>()
            .expect("rows should validate");
        let result = database
            .with_connection(|connection| {
                let transaction = connection.transaction().map_err(CommandError::database)?;
                let mut seen = HashSet::new();
                let mut created = 0;
                let mut skipped = 0;
                for (content, tags, normalized, hash) in prepared {
                    if !seen.insert(normalized.clone())
                        || has_exact_duplicate(
                            &transaction,
                            &deck_id,
                            &hash,
                            &normalized.0,
                            &normalized.1,
                            None,
                        )?
                    {
                        skipped += 1;
                        continue;
                    }
                    insert_organized_card(
                        &transaction,
                        &deck_id,
                        content,
                        &tags,
                        &hash,
                        unix_time_millis()?,
                    )?;
                    created += 1;
                }
                transaction.commit().map_err(CommandError::database)?;
                Ok((created, skipped))
            })
            .expect("bulk insert should work");
        assert_eq!(result, (1, 1));
    }

    #[test]
    fn moving_preserves_card_identity_and_copying_creates_a_fresh_schedule() {
        let database = Database::open_in_memory().expect("database should open");
        let source_deck = create_test_deck(&database, "Source");
        let target_deck = create_test_deck(&database, "Target");
        let (note_id, card_id) = database
            .with_connection(|connection| {
                let content = validate_content(
                    plain_text_document("Question"),
                    plain_text_document("Answer"),
                    "Question".to_string(),
                    "Answer".to_string(),
                )?;
                let normalized = normalized_pair(&content.front_text, &content.back_text);
                let transaction = connection.transaction().map_err(CommandError::database)?;
                let note_id = insert_organized_card(
                    &transaction,
                    &source_deck,
                    content,
                    &[],
                    &duplicate_hash(&normalized.0, &normalized.1),
                    unix_time_millis()?,
                )?;
                let card_id: String = transaction
                    .query_row(
                        "SELECT id FROM cards WHERE note_id = ?1",
                        [note_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                transaction.commit().map_err(CommandError::database)?;
                Ok((note_id, card_id))
            })
            .expect("source card should be created");

        database
            .with_connection(|connection| {
                let transaction = connection.transaction().map_err(CommandError::database)?;
                let source = load_source_note(&transaction, &note_id)?;
                transaction
                    .execute(
                        "UPDATE notes SET deck_id = ?1 WHERE id = ?2",
                        params![target_deck, note_id],
                    )
                    .map_err(CommandError::database)?;
                transaction
                    .execute(
                        "UPDATE cards SET deck_id = ?1 WHERE note_id = ?2",
                        params![target_deck, note_id],
                    )
                    .map_err(CommandError::database)?;
                let normalized = normalized_pair(&source.front_text, &source.back_text);
                let hash = duplicate_hash(&normalized.0, &normalized.1);
                copy_note_to_deck(
                    &transaction,
                    &source,
                    &source_deck,
                    &hash,
                    unix_time_millis()?,
                )?;
                transaction.commit().map_err(CommandError::database)
            })
            .expect("move and copy should succeed");

        database
            .with_connection(|connection| {
                let moved_card_id: String = connection
                    .query_row(
                        "SELECT id FROM cards WHERE note_id = ?1",
                        [note_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(moved_card_id, card_id);
                let source_copy_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM cards WHERE deck_id = ?1 AND note_id != ?2",
                        params![source_deck, note_id],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(source_copy_count, 1);
                Ok(())
            })
            .expect("results should be queryable");
    }
}
