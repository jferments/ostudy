//! Scope selection is authorization: sources and their complete evidence sets
//! must remain within the selected union, with exact-scope conversation opt-in.
use super::{
    adapters,
    contracts::{hash, SourceKind, SourceRecord, StudyScope},
    repository, retrieval,
};
use crate::{database::Database, error::CommandError};
use rusqlite::params;
use serde_json::{json, Value};

fn other_document(database: &Database, original: &SourceRecord, digit: char) -> SourceRecord {
    let id = digit.to_string().repeat(32);
    database.with_connection(|c| {
        c.execute("INSERT INTO documents(id,title,original_filename,mime_type,byte_size,sha256,source_relative_path,lifecycle_state,created_at,updated_at)
            VALUES(?1,'Other document','other.pdf','application/pdf',1,?2,'documents/other.pdf','ready',1,1)",
            params![id, digit.to_string().repeat(64)]).map_err(CommandError::database)?;
        Ok(())
    }).unwrap();
    let mut source = original.clone();
    source.owner_id = id.clone();
    source.document_id = Some(id);
    source.refresh_identity().unwrap();
    database
        .with_connection(|c| repository::publish_source(c, &source, "0", 1))
        .unwrap();
    source
}
fn scope(members: Value) -> StudyScope {
    serde_json::from_value(json!({"kind":"selection","id":null,"members":members})).unwrap()
}
fn conversation(database: &Database, selected: &StudyScope, digit: char) -> String {
    let id = digit.to_string().repeat(32);
    let turn = format!("{digit}{}", "0".repeat(31));
    database.with_connection(|c| {
        c.execute("INSERT INTO tutor_conversations(id,operation_id,title,scope_json,retrieval_enabled,created_at,updated_at)
            VALUES(?1,?1,'Permitted chat',?2,1,1,1)",params![id,serde_json::to_string(selected).unwrap()]).map_err(CommandError::database)?;
        c.execute("INSERT INTO tutor_turns(id,conversation_id,operation_id,sequence,request_hash,request_json,mode,user_text,assistant_text,
            state,profile_revision,provider_kind,model_id,created_at,updated_at)
            VALUES(?1,?2,?1,1,?3,'{}','explain','Question','Answer','complete',1,'llama_cpp','fixture',1,1)",
            params![turn,id,hash("request")]).map_err(CommandError::database)?;
        Ok(())
    }).unwrap();
    adapters::materialize_source(
        database,
        &adapters::SourceSeed {
            kind: SourceKind::Conversation,
            owner_id: id,
            parent_key: turn,
        },
    )
    .unwrap()[0]
        .id
        .clone()
}
#[test]
fn union_uses_both_documents_excludes_others_and_requires_exact_conversation_permission() {
    let (database, first, _, _) = super::authoring::tests::fixture();
    let second = other_document(&database, &first, '4');
    let outside = other_document(&database, &first, '5');
    let selected = scope(
        json!([{"kind":"document","id":second.owner_id},{"kind":"document","id":first.owner_id}]),
    );
    let single = serde_json::from_value(json!({"kind":"document","id":first.owner_id})).unwrap();
    let inside_chat = conversation(&database, &selected, 'a');
    let other_chat = conversation(&database, &single, 'b');
    let keys = retrieval::eligible_entries(&database, &selected, false).unwrap();
    let ids = keys
        .iter()
        .map(|entry| entry.source_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert!(ids.contains(first.id.as_str()));
    assert!(ids.contains(second.id.as_str()));
    assert!(!ids.contains(outside.id.as_str()));
    assert!(!ids.contains(inside_chat.as_str()));
    let keys = retrieval::eligible_entries(&database, &selected, true).unwrap();
    assert!(keys.iter().any(|entry| entry.source_id == inside_chat));
    assert!(!keys.iter().any(|entry| entry.source_id == other_chat));
    database
        .with_connection(|c| {
            c.execute(
                "UPDATE tutor_conversations SET retrieval_enabled=0 WHERE id=?1",
                ["a".repeat(32)],
            )
            .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    assert!(!retrieval::eligible_entries(&database, &selected, true)
        .unwrap()
        .iter()
        .any(|entry| entry.source_id == inside_chat));
    database
        .with_connection(|c| {
            c.execute("DELETE FROM documents WHERE id=?1", [&second.owner_id])
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    // Missing members fail closed instead of turning into a library-wide query.
    assert!(retrieval::eligible_entries(&database, &selected, false).is_err());
}
#[test]
fn largest_selection_fits_existing_storage_and_legacy_scopes_remain_readable() {
    let selected = scope(Value::Array(
        (0..16)
            .map(|i| json!({"kind":"document","id":format!("{i:032x}")}))
            .collect(),
    ));
    assert!(serde_json::to_string(&selected).unwrap().len() <= 1024);
    let general: StudyScope = serde_json::from_value(json!({"kind":"general","id":null})).unwrap();
    let database = Database::open_in_memory().unwrap();
    assert!(retrieval::eligible_entries(&database, &general, true)
        .unwrap()
        .is_empty());
}
