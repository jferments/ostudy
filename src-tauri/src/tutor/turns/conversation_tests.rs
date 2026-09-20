use super::*;
use crate::knowledge::contracts::{ScopeKind, StudyScope};
use crate::tutor::{
    contracts::{CreateConversationInput, RenameConversationInput},
    output,
};
use serde_json::json;

fn context() -> StudyContext {
    StudyContext {
        learning_stamp: None,
        question_hint: None,
        sources: vec![],
        units: vec![],
        history: history::HistorySnapshot::default(),
        inherited_turns: vec![],
        request_text: String::new(),
        notice: String::new(),
        images: vec![],
        completion_tokens: 3000,
    }
}
fn scope() -> StudyScope {
    StudyScope {
        kind: ScopeKind::General,
        id: None,
        members: vec![],
    }
}
fn quiz_response() -> serde_json::Value {
    json!({"text":"How many sides does a triangle have?", "sourceIds":[], "feedback":null,
        "quiz":{"question":"How many sides does a triangle have?", "expectedAnswer":"Three",
        "criteria":["Names the number of sides"], "explanation":"Triangles have three sides.",
        "sourceIds":[],"knowledgeUnitIds":[]}})
}

#[test]
fn ordinary_send_accepts_questions_without_relaxing_quiz_or_hint_privacy() {
    assert!(output::validate(
        quiz_response(),
        TutorMode::Explain,
        &scope(),
        &context(),
        None
    )
    .is_ok());
    assert!(
        output::validate(quiz_response(), TutorMode::Hint, &scope(), &context(), None).is_err()
    );
    let mut invalid = quiz_response();
    invalid["quiz"]["sourceIds"] = json!(["f".repeat(32)]);
    assert!(output::validate(invalid, TutorMode::Explain, &scope(), &context(), None).is_err());
    let questions = json!({"text":"1. Describe filtration.\n2. Describe reabsorption.","sourceIds":[],"quiz":null,"feedback":null});
    assert!(output::validate(questions, TutorMode::Explain, &scope(), &context(), None).is_ok());
}

#[test]
fn conversational_question_publication_preserves_request_identity_and_private_rubric() {
    let db = Database::open_in_memory().unwrap();
    let conversation = conversations::create(
        &db,
        CreateConversationInput {
            operation_id: "a".repeat(32),
            scope: scope(),
            title: "Geometry".into(),
        },
    )
    .unwrap();
    let request = TutorTurnInput {
        operation_id: "b".repeat(32),
        conversation_id: conversation.id.clone(),
        expected_revision: conversation.revision,
        mode: TutorMode::Explain,
        text: "Test my knowledge".into(),
        focus: None,
        profile_id: None,
        include_conversations: false,
        question_id: None,
    };
    let identity = TurnIdentity {
        id: "c".repeat(32),
        conversation_id: conversation.id.clone(),
        request_hash: request_hash(&request).unwrap(),
        accepted_revision: conversation.revision,
    };
    // Arrange an already-admitted request. Publication must change its output
    // kind without changing the request hash used for acknowledgement retries.
    db.with_connection(|c| {
        c.execute("INSERT INTO tutor_turns(id,conversation_id,operation_id,sequence,request_hash,request_json,
            mode,user_text,state,profile_revision,provider_kind,model_id,created_at,updated_at)
            VALUES(?1,?2,?3,1,?4,?5,'explain','Test my knowledge','running',1,'llama_cpp','fixture',1,1)",
            params![identity.id, conversation.id, request.operation_id, identity.request_hash,
            serde_json::to_string(&request).unwrap()]).map_err(CommandError::database)?;
        Ok(())
    }).unwrap();
    let mut ctx = context();
    ctx.notice = "Analysis is partial: internal diagnostic".into();
    let answer =
        output::validate(quiz_response(), TutorMode::Explain, &scope(), &ctx, None).unwrap();
    let public = complete(&db, &identity, &answer, &ctx, None).unwrap();
    assert_eq!(public.mode, TutorMode::Quiz);
    assert!(public.pending_question);
    assert_eq!(public.assistant_text, answer.text);
    let serialized = serde_json::to_string(&public).unwrap();
    for hidden in [
        "expectedAnswer",
        "Three",
        "Names the number of sides",
        "internal diagnostic",
    ] {
        assert!(!serialized.contains(hidden));
    }
    assert_eq!(replay(&db, &request).unwrap().unwrap().id, public.id);
    db.with_connection(|c| {
        assert_eq!(
            load_question(c, &conversation.id, &public.id)?.expected_answer,
            "Three"
        );
        Ok(())
    })
    .unwrap();
}


#[test]
fn organization_changes_do_not_invalidate_an_admitted_response() {
    let db = Database::open_in_memory().unwrap();
    let conversation = conversations::create(
        &db,
        CreateConversationInput {
            operation_id: "d".repeat(32),
            scope: scope(),
            title: "Original title".into(),
        },
    )
    .unwrap();
    let request = TutorTurnInput {
        operation_id: "e".repeat(32),
        conversation_id: conversation.id.clone(),
        expected_revision: conversation.revision,
        mode: TutorMode::Explain,
        text: "Explain triangles".into(),
        focus: None,
        profile_id: None,
        include_conversations: false,
        question_id: None,
    };
    let identity = TurnIdentity {
        id: "f".repeat(32),
        conversation_id: conversation.id.clone(),
        request_hash: request_hash(&request).unwrap(),
        accepted_revision: conversation.revision,
    };
    db.with_connection(|connection| {
        connection.execute(
            "INSERT INTO tutor_turns(
                 id,conversation_id,operation_id,sequence,request_hash,request_json,
                 mode,user_text,state,profile_revision,provider_kind,model_id,created_at,updated_at
             ) VALUES(?1,?2,?3,1,?4,?5,'explain','Explain triangles','running',1,'llama_cpp','fixture',1,1)",
            params![
                identity.id,
                conversation.id,
                request.operation_id,
                identity.request_hash,
                serde_json::to_string(&json!({
                    "operationId":request.operation_id,
                    "conversationId":conversation.id,
                    "expectedRevision":conversation.revision,
                    "mode":"explain",
                    "text":"Explain triangles",
                    "focus":null,
                    "profileId":null,
                    "includeConversations":false,
                    "questionId":null,
                    "capturedScope":scope()
                })).unwrap()
            ],
        ).map_err(CommandError::database)?;
        Ok(())
    }).unwrap();

    let renamed = conversations::rename(
        &db,
        RenameConversationInput {
            operation_id: "9".repeat(32),
            conversation_id: conversation.id.clone(),
            expected_organization_revision: conversation.organization_revision,
            title: "Renamed while answering".into(),
        },
    )
    .unwrap();
    assert_eq!(renamed.revision, conversation.revision);
    assert_eq!(
        renamed.organization_revision,
        conversation.organization_revision + 1
    );

    let answer = ProviderAnswer {
        text: "Triangles have three sides.".into(),
        source_ids: vec![],
        quiz: None,
        feedback: None,
    };
    let published = complete(&db, &identity, &answer, &context(), None).unwrap();
    assert_eq!(published.state, "complete");
    db.with_connection(|connection| {
        let saved = conversations::load_summary(connection, &conversation.id)?;
        assert_eq!(saved.title, "Renamed while answering");
        assert_eq!(saved.revision, conversation.revision + 1);
        assert_eq!(
            saved.organization_revision,
            conversation.organization_revision + 1
        );
        Ok(())
    })
    .unwrap();
}
