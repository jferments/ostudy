use super::*;
use crate::{
    ai::AiRuntimeState,
    knowledge::{
        contracts::{hash, EvidenceImage},
        repository,
    },
    paths::AppPaths,
    task_cancellation::CancellationSignal,
};

#[tokio::test]
async fn excluded_focused_image_is_never_loaded_or_sent() {
    let mut f = fixture();
    f.task.include_page_images = true;
    let mut sources = materialize_source(
        &f.database,
        &SourceSeed {
            kind: SourceKind::Card,
            owner_id: "d".repeat(32),
            parent_key: "presentation".into(),
        },
    )
    .unwrap();
    // Deliberately nonexistent bytes: the old focus-first image path attempted
    // to load them even though General scope had already excluded this source.
    sources[0].images.push(EvidenceImage::DocumentAsset {
        document_id: "f".repeat(32),
        asset_id: "excluded-image-sentinel".into(),
    });
    let focus = focus::FocusedContent {
        sources,
        ..Default::default()
    };
    let directory = std::env::temp_dir().join(format!(
        "ostudy-context-image-{}-{}",
        std::process::id(),
        f.conversation.id
    ));
    let paths = AppPaths::new(directory.join("library")).unwrap();
    let runtime = AiRuntimeState::new(crate::platform::unavailable_secret_store()).unwrap();
    let request = input(&f, TutorMode::Explain);
    let ctx = super::super::context::build(
        &f.database,
        &paths,
        &runtime,
        &f.task,
        super::super::context::StudyContextInput {
            scope: &f.conversation.scope,
            input: &request,
            focus: &focus,
            quiz: None,
            series: None,
            history: history::HistorySnapshot::default(),
            project_instructions: None,
        },
        &CancellationSignal::new(),
    )
    .await
    .unwrap();
    assert!(ctx.sources.is_empty());
    assert!(ctx.images.is_empty());
    assert!(!ctx.request_text.contains("excluded-image-sentinel"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn late_reinterpretation_cannot_publish_a_tutor_turn_over_unchanged_source_bytes() {
    let mut f = fixture();
    f.conversation = conversations::create(
        &f.database,
        CreateConversationInput {
            operation_id: "9".repeat(32),
            scope: StudyScope {
                kind: ScopeKind::Deck,
                id: Some("b".repeat(32)),
                members: vec![],
            },
            title: "Retained knowledge".into(),
        },
    )
    .unwrap();
    let sources = materialize_source(
        &f.database,
        &SourceSeed {
            kind: SourceKind::Card,
            owner_id: "d".repeat(32),
            parent_key: "presentation".into(),
        },
    )
    .unwrap();
    for source in &sources {
        repository::publish_units(
            &f.database,
            source,
            &hash("original-empty-interpretation"),
            vec![],
        )
        .unwrap();
    }
    let focus = focus::FocusedContent {
        sources: sources.clone(),
        ..Default::default()
    };
    let directory = std::env::temp_dir().join(format!(
        "ostudy-context-revision-{}-{}",
        std::process::id(),
        f.conversation.id
    ));
    let paths = AppPaths::new(directory.join("library")).unwrap();
    let runtime = AiRuntimeState::new(crate::platform::unavailable_secret_store()).unwrap();
    let request = input(&f, TutorMode::Explain);
    let ctx = super::super::context::build(
        &f.database,
        &paths,
        &runtime,
        &f.task,
        super::super::context::StudyContextInput {
            scope: &f.conversation.scope,
            input: &request,
            focus: &focus,
            quiz: None,
            series: None,
            history: history::HistorySnapshot::default(),
            project_instructions: None,
        },
        &CancellationSignal::new(),
    )
    .await
    .unwrap();
    assert!(!ctx.sources.is_empty());
    let identity = turns::begin(&f.database, &request, &f.task).unwrap();
    repository::publish_units(
        &f.database,
        &sources[0],
        &hash("successor-interpretation"),
        vec![],
    )
    .unwrap();
    f.database
        .with_connection(|c| {
            assert!(repository::source_is_current(c, &sources[0])?);
            Ok(())
        })
        .unwrap();
    let answer = ProviderAnswer {
        text: "Late answer".into(),
        source_ids: vec![],
        quiz: None,
        feedback: None,
    };
    assert!(turns::complete(&f.database, &identity, &answer, &ctx, None).is_err());
    let retained = f
        .database
        .with_connection(|c| turns::load_public(c, &identity.id))
        .unwrap();
    assert_eq!(retained.state, "running");
    assert!(retained.assistant_text.is_empty());
    std::fs::remove_dir_all(directory).unwrap();
}
