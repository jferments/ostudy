use rusqlite::{params, Connection};

const MIGRATION: &str = include_str!("../migrations/0007_document_artifact_metadata.sql");

fn fixture_connection() -> Connection {
    let connection = Connection::open_in_memory().expect("fixture database should open");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE decks (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 description TEXT NOT NULL DEFAULT '',
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL,
                 deleted_at INTEGER,
                 revision INTEGER NOT NULL DEFAULT 1
             ) STRICT;
             CREATE TABLE documents (
                 id TEXT PRIMARY KEY,
                 title TEXT NOT NULL,
                 original_filename TEXT NOT NULL,
                 sha256 TEXT NOT NULL,
                 generated_deck_id TEXT REFERENCES decks(id) ON DELETE SET NULL,
                 source_format TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL,
                 revision INTEGER NOT NULL DEFAULT 1
             ) STRICT;
             CREATE TABLE source_citation_snapshots (
                 id TEXT PRIMARY KEY,
                 source_document_id TEXT NOT NULL UNIQUE,
                 source_title TEXT NOT NULL,
                 original_filename TEXT NOT NULL,
                 source_format TEXT NOT NULL,
                 updated_at INTEGER NOT NULL
             ) STRICT;
             CREATE TABLE document_learning_plans (
                 id TEXT PRIMARY KEY,
                 document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                 summary_text TEXT NOT NULL,
                 key_concepts_json TEXT NOT NULL,
                 state TEXT NOT NULL,
                 updated_at INTEGER NOT NULL
             ) STRICT;
             CREATE TABLE document_learning_plan_topics (
                 plan_id TEXT NOT NULL REFERENCES document_learning_plans(id) ON DELETE CASCADE,
                 sort_index INTEGER NOT NULL,
                 title TEXT NOT NULL
             ) STRICT;",
        )
        .expect("fixture schema should load");
    connection
}

fn insert_document_plan(
    connection: &Connection,
    document_id: &str,
    plan_id: &str,
    title: &str,
    filename: &str,
    deck_id: Option<&str>,
    topic_title: &str,
) {
    connection
        .execute(
            "INSERT INTO documents (
                 id, title, original_filename, sha256, generated_deck_id,
                 source_format, created_at, updated_at, revision
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'pdf', 1, 2, 1)",
            params![document_id, title, filename, "a".repeat(64), deck_id],
        )
        .expect("document should insert");
    connection
        .execute(
            "INSERT INTO source_citation_snapshots (
                 id, source_document_id, source_title, original_filename,
                 source_format, updated_at
             ) VALUES (?1, ?1, ?2, ?3, 'pdf', 2)",
            params![document_id, title, filename],
        )
        .expect("citation should insert");
    connection
        .execute(
            "INSERT INTO document_learning_plans (
                 id, document_id, summary_text, key_concepts_json, state, updated_at
             ) VALUES (?1, ?2, 'Whole-document summary', '[\"Perfusion\"]', 'active', 3)",
            params![plan_id, document_id],
        )
        .expect("plan should insert");
    connection
        .execute(
            "INSERT INTO document_learning_plan_topics (plan_id, sort_index, title)
             VALUES (?1, 0, ?2)",
            params![plan_id, topic_title],
        )
        .expect("topic should insert");
}

#[test]
fn migration_backfills_metadata_and_preserves_source_identity_for_followup_repairs() {
    let connection = fixture_connection();
    let generated_document = "1".repeat(32);
    let generated_plan = "2".repeat(32);
    let generated_deck = "3".repeat(32);
    let user_document = "4".repeat(32);
    let user_plan = "5".repeat(32);
    let user_deck = "6".repeat(32);

    connection
        .execute(
            "INSERT INTO decks (id, name, description, created_at, updated_at, revision)
             VALUES (?1, 'shock_notes',
                     'AI-generated flashcards from shock_notes.pdf. Edit or delete these cards through the normal deck tools.',
                     1, 1, 1)",
            [generated_deck.as_str()],
        )
        .expect("generated deck should insert");
    insert_document_plan(
        &connection,
        &generated_document,
        &generated_plan,
        "shock_notes",
        "shock_notes.pdf",
        Some(&generated_deck),
        "Distributive shock physiology",
    );

    connection
        .execute(
            "INSERT INTO decks (id, name, description, created_at, updated_at, revision)
             VALUES (?1, 'My shock review', 'My bedside notes', 1, 1, 1)",
            [user_deck.as_str()],
        )
        .expect("user deck should insert");
    insert_document_plan(
        &connection,
        &user_document,
        &user_plan,
        "My vasopressor notes",
        "vasopressors.pdf",
        Some(&user_deck),
        "Vasopressor pharmacology",
    );

    connection
        .execute_batch(MIGRATION)
        .expect("migration should apply");

    let repaired_title: String = connection
        .query_row(
            "SELECT title FROM documents WHERE id = ?1",
            [generated_document.as_str()],
            |row| row.get(0),
        )
        .expect("repaired title should load");
    let repaired_deck: (String, String) = connection
        .query_row(
            "SELECT name, description FROM decks WHERE id = ?1",
            [generated_deck.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("repaired deck should load");
    assert_eq!(repaired_title, "Distributive shock physiology");
    assert_eq!(repaired_deck.0, "Distributive shock physiology");
    assert!(repaired_deck
        .1
        .starts_with("Review Distributive shock physiology"));

    // Migration 7 is already checksum-published in development databases. Its
    // historical source-title branch therefore remains immutable. The versioned
    // data repair that follows migration 7 uses the preserved citation title to
    // restore this user-owned display title and clear generated-value ownership.
    let migrated_user_title: String = connection
        .query_row(
            "SELECT title FROM documents WHERE id = ?1",
            [user_document.as_str()],
            |row| row.get(0),
        )
        .expect("migrated user title should load");
    let preserved_user_source_title: String = connection
        .query_row(
            "SELECT source_title FROM source_citation_snapshots WHERE source_document_id = ?1",
            [user_document.as_str()],
            |row| row.get(0),
        )
        .expect("preserved source title should load");
    let user_deck_values: (String, String) = connection
        .query_row(
            "SELECT name, description FROM decks WHERE id = ?1",
            [user_deck.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("user deck should load");
    assert_eq!(migrated_user_title, "Vasopressor pharmacology");
    assert_eq!(preserved_user_source_title, "My vasopressor notes");
    assert_eq!(user_deck_values.0, "My shock review");
    assert_eq!(user_deck_values.1, "My bedside notes");

    connection
        .execute(
            "UPDATE source_citation_snapshots SET source_title = 'Rewritten display title'
             WHERE source_document_id = ?1",
            [generated_document.as_str()],
        )
        .expect("ordinary citation refresh should complete");
    let source_title: String = connection
        .query_row(
            "SELECT source_title FROM source_citation_snapshots WHERE source_document_id = ?1",
            [generated_document.as_str()],
            |row| row.get(0),
        )
        .expect("source title should load");
    assert_eq!(source_title, "shock_notes");
}

#[test]
fn generated_deck_link_applies_saved_metadata_after_initial_generation() {
    let connection = fixture_connection();
    let document_id = "7".repeat(32);
    let plan_id = "8".repeat(32);
    let deck_id = "9".repeat(32);
    insert_document_plan(
        &connection,
        &document_id,
        &plan_id,
        "hemodynamics",
        "hemodynamics.pdf",
        None,
        "Hemodynamic monitoring",
    );
    connection
        .execute_batch(MIGRATION)
        .expect("migration should apply");
    connection
        .execute(
            "INSERT INTO decks (id, name, description, created_at, updated_at, revision)
             VALUES (?1, 'hemodynamics',
                     'AI-generated flashcards from hemodynamics.pdf. Edit or delete these cards through the normal deck tools.',
                     4, 4, 1)",
            [deck_id.as_str()],
        )
        .expect("initial generated deck should insert");
    connection
        .execute(
            "UPDATE documents SET generated_deck_id = ?1, updated_at = 5 WHERE id = ?2",
            params![deck_id, document_id],
        )
        .expect("generated deck should link");

    let deck: (String, String) = connection
        .query_row(
            "SELECT name, description FROM decks WHERE id = ?1",
            [deck_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("linked deck should load");
    assert_eq!(deck.0, "Hemodynamic monitoring");
    assert!(deck.1.starts_with("Review Hemodynamic monitoring"));

    let ledger: (Option<String>, Option<String>, Option<String>) = connection
        .query_row(
            "SELECT applied_document_title, applied_deck_title, applied_deck_description
             FROM document_artifact_metadata WHERE document_id = ?1",
            [document_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("applied-value ledger should load");
    assert_eq!(ledger.0.as_deref(), Some("Hemodynamic monitoring"));
    assert_eq!(ledger.1.as_deref(), Some("Hemodynamic monitoring"));
    assert_eq!(ledger.2.as_deref(), Some(deck.1.as_str()));
}
