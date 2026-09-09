use rusqlite::params;
use serde_json::json;

use super::{
    adapters::{materialize_source, SourceSeed},
    contracts::{hash, KnowledgeKind, KnowledgeUnitInput, SourceKind},
    repository,
};
use crate::{database::Database, error::CommandError, scheduler::insert_new_schedule};

fn fixture() -> Database {
    let database = Database::open_in_memory().unwrap();
    database.with_connection(|connection| {
        let doc = |text: &str| json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]}).to_string();
        connection.execute("INSERT INTO decks (id,name,created_at,updated_at) VALUES (?1,'Manual cards',1,1)", ["a".repeat(32)]).map_err(CommandError::database)?;
        connection.execute(
            "INSERT INTO notes (id,deck_id,front_doc,back_doc,front_text,back_text,created_at,updated_at)
             VALUES (?1,?2,?3,?4,'How many sides?','Three',1,1)",
            params!["b".repeat(32),"a".repeat(32),doc("How many sides?"),doc("Three")],
        ).map_err(CommandError::database)?;
        connection.execute("INSERT INTO cards (id,note_id,deck_id,created_at,updated_at) VALUES (?1,?2,?3,1,1)",
            params!["c".repeat(32),"b".repeat(32),"a".repeat(32)]).map_err(CommandError::database)?;
        insert_new_schedule(connection,&"c".repeat(32),1)?;
        Ok(())
    }).unwrap();
    database
}

fn seed() -> SourceSeed {
    SourceSeed {
        kind: SourceKind::Card,
        owner_id: "c".repeat(32),
        parent_key: "presentation".to_string(),
    }
}

#[test]
fn manual_card_changes_discard_old_units_even_when_rendered_text_is_unchanged() {
    let database = fixture();
    let source = materialize_source(&database, &seed()).unwrap().remove(0);
    repository::publish_units(
        &database,
        &source,
        &hash("fixture"),
        vec![KnowledgeUnitInput {
            title: "Three sides".to_string(),
            kind: KnowledgeKind::Fact,
            statement: "Three".to_string(),
            qualifications: vec![],
            topic: "Geometry".to_string(),
            supporting_excerpt: "Three".to_string(),
        }],
    )
    .unwrap();
    database.with_connection(|connection| {
        connection.execute("UPDATE notes SET back_text = 'Changed native text', revision = revision + 1 WHERE id = ?1",["b".repeat(32)]).map_err(CommandError::database)?;
        assert!(!repository::source_is_current(connection,&source)?);
        assert!(repository::load_units(connection,&source.id)?.is_empty());
        Ok(())
    }).unwrap();
    let current = materialize_source(&database, &seed()).unwrap().remove(0);
    database
        .with_connection(|connection| {
            assert!(!repository::extraction_is_current(
                connection,
                &current,
                &hash("fixture")
            )?);
            assert!(repository::load_units(connection, &current.id)?.is_empty());
            let schedule: (i64, i64) = connection
                .query_row("SELECT reps,revision FROM card_schedules", [], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
                .map_err(CommandError::database)?;
            assert_eq!(schedule, (0, 1));
            let reviews: i64 = connection
                .query_row("SELECT COUNT(*) FROM review_events", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(reviews, 0);
            Ok(())
        })
        .unwrap();
}

#[test]
fn delete_and_undo_keep_card_identity_without_reviving_semantic_interpretations() {
    let database = fixture();
    let source = materialize_source(&database, &seed()).unwrap().remove(0);
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE notes SET deleted_at = 2 WHERE id = ?1",
                    ["b".repeat(32)],
                )
                .map_err(CommandError::database)?;
            assert!(!repository::source_is_current(connection, &source)?);
            connection
                .execute(
                    "UPDATE notes SET deleted_at = NULL WHERE id = ?1",
                    ["b".repeat(32)],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    let restored = materialize_source(&database, &seed()).unwrap().remove(0);
    assert_eq!(restored.id, source.id);
    database
        .with_connection(|connection| {
            assert!(repository::load_units(connection, &restored.id)?.is_empty());
            connection
                .execute("DELETE FROM notes WHERE id = ?1", ["b".repeat(32)])
                .map_err(CommandError::database)?;
            assert!(repository::load_source(connection, &source.id)?.is_none());
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM knowledge_cache_evictions WHERE source_id = ?1",
                    [source.id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(count, 1);
            Ok(())
        })
        .unwrap();
}

#[test]
fn a_same_content_republication_does_not_accept_an_old_owner_revision() {
    let database = fixture();
    let earlier = materialize_source(&database, &seed()).unwrap().remove(0);
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE notes SET revision = revision + 1 WHERE id = ?1",
                    ["b".repeat(32)],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    let current = materialize_source(&database, &seed()).unwrap().remove(0);
    assert_eq!(earlier.id, current.id);
    assert_eq!(earlier.content_hash, current.content_hash);
    assert_ne!(earlier.owner_revision, current.owner_revision);
    database
        .with_connection(|connection| {
            assert!(!repository::source_is_current(connection, &earlier)?);
            assert!(repository::source_is_current(connection, &current)?);
            Ok(())
        })
        .unwrap();
    assert!(
        repository::publish_units(&database, &earlier, &hash("obsolete request"), vec![]).is_err()
    );
}
