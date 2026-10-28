//! General chat must work with no embedding profile and no material at all.
use super::*;
use crate::{ai::AiRuntimeState, paths::AppPaths, task_cancellation::CancellationSignal};

#[tokio::test]
async fn source_free_question_builds_publishes_and_evaluates_without_search_setup() {
    let f = fixture();
    let id = f
        .database
        .with_connection(|connection| generate_id(connection))
        .unwrap();
    let directory =
        std::env::temp_dir().join(format!("ostudy-source-free-{}-{id}", std::process::id()));
    let paths = AppPaths::new(directory.join("library")).unwrap();
    let runtime = AiRuntimeState::new(crate::platform::unavailable_secret_store()).unwrap();
    assert!(crate::ai::embeddings::load_default_embedding_context(&f.database).is_err());
    let mut question_request = input(&f, TutorMode::Explain);
    question_request.text = "Ask me one geometry question and wait for my answer.".into();
    let signal = CancellationSignal::new();
    let focus = focus::FocusedContent::default();
    let ctx = super::super::context::build(
        &f.database,
        &paths,
        &runtime,
        &f.task,
        super::super::context::StudyContextInput {
            scope: &f.conversation.scope,
            input: &question_request,
            focus: &focus,
            quiz: None,
            series: None,
            history: history::HistorySnapshot::default(),
            project_instructions: None,
            attachments: &[],
        },
        &signal,
    )
    .await
    .unwrap();
    assert!(ctx.sources.is_empty());
    assert!(ctx.units.is_empty());
    assert!(!paths
        .app_data_dir()
        .join("cache/semantic-v1.sqlite3")
        .exists());
    let answer = output::validate(
        json!({"text":"Let's begin. The answer is three.",
        "sourceIds":[],"feedback":null,"quiz":{"question":"How many sides does a triangle have?",
        "expectedAnswer":"Three","criteria":["Names three sides."],
        "explanation":"A triangle has three sides.","sourceIds":[],"knowledgeUnitIds":[]}}),
        TutorMode::Explain,
        &f.conversation.scope,
        &ctx,
        None,
    )
    .unwrap();
    let identity = turns::begin(&f.database, None, &question_request, &f.task).unwrap();
    let question = turns::complete(&f.database, &identity, &answer, &ctx, None).unwrap();
    assert!(question.pending_question);
    assert!(question.citations.is_empty());
    assert_eq!(
        question.assistant_text,
        "How many sides does a triangle have?"
    );
    assert_eq!(
        turns::replay(&f.database, &question_request)
            .unwrap()
            .unwrap()
            .id,
        question.id
    );
    let fixed = f
        .database
        .with_connection(|c| turns::load_question(c, &f.conversation.id, &question.id))
        .unwrap();
    let mut reply = input(&f, TutorMode::Answer);
    reply.text = "It has three sides.".into();
    reply.question_id = Some(question.id.clone());
    let ctx = super::super::context::build(
        &f.database,
        &paths,
        &runtime,
        &f.task,
        super::super::context::StudyContextInput {
            scope: &f.conversation.scope,
            input: &reply,
            focus: &focus,
            quiz: Some(&fixed),
            series: None,
            history: history::HistorySnapshot::default(),
            project_instructions: None,
            attachments: &[],
        },
        &signal,
    )
    .await
    .unwrap();
    let feedback = output::validate(json!({"text":"Feedback","sourceIds":[],"quiz":null,
        "feedback":{"outcome":"correct","explanation":"Correct.","criteriaFeedback":["Three sides."],"criteriaMet":[true]}}),
        TutorMode::Answer,&f.conversation.scope,&ctx,Some(&fixed)).unwrap();
    let identity = turns::begin(&f.database, None, &reply, &f.task).unwrap();
    let saved = turns::complete(&f.database, &identity, &feedback, &ctx, None).unwrap();
    assert!(saved.assistant_text.contains("Expected answer: Three"));
    assert!(
        !f.database
            .with_connection(|c| turns::load_public(c, &question.id))
            .unwrap()
            .pending_question
    );
    std::fs::remove_dir_all(directory).unwrap();
}
