use super::*;
use crate::tutor::history;

fn fixture() -> Database {
    let database = Database::open_in_memory().unwrap();
    database.with_connection(|connection| {
        connection.execute(
            "INSERT INTO tutor_conversations (id,operation_id,title,scope_json,created_at,updated_at)
             VALUES (?1,?1,'Memory test','{\"kind\":\"general\",\"id\":null}',1,1)",
            ["a".repeat(32)],
        ).map_err(CommandError::database)?;
        for sequence in 1..=3 {
            connection.execute(
                "INSERT INTO tutor_turns (id,conversation_id,operation_id,sequence,request_hash,request_json,
                     mode,user_text,assistant_text,state,profile_revision,provider_kind,model_id,created_at,updated_at)
                 VALUES (?1,?2,?1,?3,?4,'{}','explain','Earlier question','Earlier response','complete',1,'llama_cpp','fixture',1,1)",
                params![format!("{sequence:032x}"),"a".repeat(32),sequence,"b".repeat(64)],
            ).map_err(CommandError::database)?;
        }
        Ok(())
    }).unwrap();
    database
}

fn input(id: char, revision: i64, action: MemoryAction) -> ChangeTutorMemoryInput {
    ChangeTutorMemoryInput {
        operation_id: id.to_string().repeat(32),
        conversation_id: "a".repeat(32),
        expected_revision: revision,
        action,
    }
}

#[test]
fn reset_does_not_delete_transcript_and_never_replays_over_new_messages() {
    let database = fixture();
    assert_eq!(
        history::load(&database, &"a".repeat(32))
            .unwrap()
            .turns
            .len(),
        3
    );
    let request = input('c', 1, MemoryAction::ForgetEarlier);
    let reset = change(&database, request.clone()).unwrap();
    assert_eq!(reset.start_sequence, 3);
    assert_eq!(reset.summary_through_sequence, 3);
    assert!(history::load(&database, &"a".repeat(32))
        .unwrap()
        .turns
        .is_empty());
    database.with_connection(|connection| {
        connection.execute(
            "INSERT INTO tutor_turns (id,conversation_id,operation_id,sequence,request_hash,request_json,
                 mode,user_text,assistant_text,state,profile_revision,provider_kind,model_id,created_at,updated_at)
             VALUES (?1,?2,?1,4,?3,'{}','explain','New question','New response','complete',1,'llama_cpp','fixture',1,1)",
            params!["d".repeat(32),"a".repeat(32),"b".repeat(64)],
        ).map_err(CommandError::database)?;
        Ok(())
    }).unwrap();
    assert_eq!(change(&database, request).unwrap().start_sequence, 3);
    let history = history::load(&database, &"a".repeat(32)).unwrap();
    assert_eq!(history.turns.len(), 1);
    assert_eq!(history.turns[0].sequence, 4);
    database
        .with_connection(|connection| {
            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM tutor_turns", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(count, 4);
            Ok(())
        })
        .unwrap();
}

#[test]
fn disabling_history_returns_no_context_and_operation_identity_is_strict() {
    let database = fixture();
    let request = input('c', 1, MemoryAction::DisableHistory);
    assert!(!change(&database, request.clone()).unwrap().history_enabled);
    assert!(history::load(&database, &"a".repeat(32))
        .unwrap()
        .turns
        .is_empty());
    assert!(change(&database, input('c', 1, MemoryAction::EnableHistory)).is_err());
    assert!(change(&database, input('d', 1, MemoryAction::EnableHistory)).is_err());
    assert!(
        change(&database, input('d', 2, MemoryAction::EnableHistory))
            .unwrap()
            .history_enabled
    );
    assert_eq!(
        history::load(&database, &"a".repeat(32))
            .unwrap()
            .turns
            .len(),
        3
    );
}

#[test]
fn sharing_is_opt_in_and_revocation_removes_all_historical_source_projections() {
    let database = fixture();
    assert!(!get(&database, &"a".repeat(32)).unwrap().retrieval_enabled);
    change(&database, input('c', 1, MemoryAction::AllowRetrieval)).unwrap();
    database
        .with_connection(|connection| {
            connection.execute(
            "INSERT INTO knowledge_sources (id,kind,owner_id,segment_key,parent_key,owner_revision,
                 content_hash,record_json,conversation_id,turn_id,created_at,updated_at)
             VALUES (?1,'conversation',?2,'turn:0','turn','1',?3,'{}',?2,?4,1,1)",
            params!["e".repeat(32),"a".repeat(32),"b".repeat(64),format!("{:032x}",1)],
        ).map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    change(&database, input('d', 2, MemoryAction::DenyRetrieval)).unwrap();
    database
        .with_connection(|connection| {
            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM knowledge_sources", [], |row| {
                    row.get(0)
                })
                .map_err(CommandError::database)?;
            assert_eq!(count, 0);
            let evictions: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM knowledge_cache_evictions",
                    [],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(evictions, 1);
            Ok(())
        })
        .unwrap();
}

#[test]
fn running_turn_prevents_memory_mutation_even_without_runtime_admission() {
    let database = fixture();
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE tutor_turns SET state = 'running' WHERE sequence = 3",
                    [],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    assert!(change(&database, input('c', 1, MemoryAction::ForgetEarlier)).is_err());
    assert_eq!(get(&database, &"a".repeat(32)).unwrap().start_sequence, 0);
}
