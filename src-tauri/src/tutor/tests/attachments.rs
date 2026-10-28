//! Pictures and text files attached to a question: what is admitted, how a
//! picture's asset is kept with its message, and what goes to the model.
use super::*;
use crate::{ai::AiRuntimeState, paths::AppPaths};

use super::super::contracts::{TutorAttachment, TutorAttachmentInput};

fn library_paths(name: &str) -> AppPaths {
    let root = std::env::temp_dir().join(format!(
        "ostudy-tutor-attachments-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    AppPaths::new(root).unwrap()
}

fn picture(database: &Database, paths: &AppPaths) -> (String, String) {
    database
        .with_connection(|connection| {
            let (draft, asset) =
                crate::asset_lifecycle::stage_test_image(connection, paths, "import")?;
            Ok((draft.id, asset.id))
        })
        .unwrap()
}

fn text_file(name: &str, text: &str) -> TutorAttachmentInput {
    TutorAttachmentInput::Text {
        name: name.to_string(),
        text: text.to_string(),
    }
}

fn count(database: &Database, sql: &str) -> i64 {
    database
        .with_connection(|connection| {
            connection
                .query_row(sql, [], |row| row.get(0))
                .map_err(CommandError::database)
        })
        .unwrap()
}

#[test]
fn only_a_question_carries_attachments_within_their_limits() {
    let f = fixture();
    let mut question = input(&f, TutorMode::Explain);
    question.text = String::new();
    assert!(question.validate().is_err());
    // A picture or file can be the whole question.
    question.attachments = vec![text_file("notes.md", "# Krebs cycle")];
    question.validate().unwrap();

    let mut quiz = input(&f, TutorMode::Quiz);
    quiz.attachments = vec![text_file("notes.md", "# Krebs cycle")];
    assert!(quiz.validate().is_err());

    let image = |id: char| TutorAttachmentInput::Image {
        asset_id: id.to_string().repeat(32),
        draft_id: None,
    };
    let mut many = input(&f, TutorMode::Explain);
    many.attachments = ['1', '2', '3', '4', '5'].into_iter().map(image).collect();
    assert!(many.validate().is_err());
    many.attachments.truncate(4);
    many.validate().unwrap();
    many.attachments[3] = image('1');
    assert!(many.validate().is_err());

    let mut long = input(&f, TutorMode::Explain);
    long.attachments = vec![text_file("long.txt", &"a".repeat(30_001))];
    assert!(long.validate().is_err());
    long.attachments = vec![
        text_file("one.txt", &"a".repeat(30_000)),
        text_file("two.txt", &"b".repeat(30_000)),
        text_file("three.txt", "c"),
    ];
    assert!(long.validate().is_err());
    long.attachments = vec![text_file("bad\nname.txt", "c")];
    assert!(long.validate().is_err());
}

#[test]
fn a_question_keeps_its_pictures_and_files_until_its_chat_is_deleted() {
    let f = fixture();
    let paths = library_paths("kept");
    let (draft, asset) = picture(&f.database, &paths);

    // A new picture names the import draft it was staged in.
    let mut stray = input(&f, TutorMode::Explain);
    stray.attachments = vec![TutorAttachmentInput::Image {
        asset_id: asset.clone(),
        draft_id: None,
    }];
    let refused = turns::begin(&f.database, Some(&paths), &stray, &f.task).unwrap_err();
    assert_eq!(refused.code, "asset_draft_mismatch");
    assert_eq!(count(&f.database, "SELECT COUNT(*) FROM tutor_turns"), 0);

    let mut question = input(&f, TutorMode::Explain);
    question.attachments = vec![
        TutorAttachmentInput::Image {
            asset_id: asset.clone(),
            draft_id: Some(draft),
        },
        text_file(
            "notes.md",
            "Citrate forms from acetyl-CoA and oxaloacetate.",
        ),
    ];
    let identity = turns::begin(&f.database, Some(&paths), &question, &f.task).unwrap();
    assert_eq!(
        count(
            &f.database,
            "SELECT COUNT(*) FROM assets WHERE lifecycle_state = 'ready'"
        ),
        1
    );
    let saved = f
        .database
        .with_connection(|connection| turns::load_public(connection, &identity.id))
        .unwrap();
    assert!(matches!(
        &saved.attachments[..],
        [
            TutorAttachment::Image { asset_id, .. },
            TutorAttachment::Text { name, .. },
        ] if asset_id == &asset && name == "notes.md"
    ));

    // Asking again sends the kept picture with no draft.
    let failed = turns::fail(
        &f.database,
        &identity,
        &CommandError::new("ai_request_failed", "Failed"),
        None,
    )
    .unwrap();
    let mut again = input(&f, TutorMode::Explain);
    again.attachments = vec![TutorAttachmentInput::Image {
        asset_id: asset.clone(),
        draft_id: None,
    }];
    let retried = turns::begin(&f.database, Some(&paths), &again, &f.task).unwrap();
    assert_ne!(retried.id, failed.id);
    assert_eq!(
        count(
            &f.database,
            "SELECT COUNT(*) FROM tutor_turn_attachments WHERE kind = 'image'"
        ),
        2
    );
    turns::fail(
        &f.database,
        &retried,
        &CommandError::new("ai_request_failed", "Failed"),
        None,
    )
    .unwrap();

    conversations::delete(
        &f.database,
        DeleteConversationInput {
            operation_id: "2".repeat(32),
            conversation_id: f.conversation.id.clone(),
            expected_revision: input(&f, TutorMode::Explain).expected_revision,
            expected_organization_revision: f.conversation.organization_revision,
        },
    )
    .unwrap();
    assert_eq!(
        count(&f.database, "SELECT COUNT(*) FROM tutor_turn_attachments"),
        0
    );
    let _ = std::fs::remove_dir_all(paths.app_data_dir());
}

#[tokio::test]
async fn pictures_are_refused_before_sending_when_the_model_cannot_see_them() {
    let f = fixture();
    let paths = library_paths("refused");
    let runtime = AiRuntimeState::new(crate::platform::unavailable_secret_store()).unwrap();
    let (draft, asset) = picture(&f.database, &paths);
    assert!(!super::super::service::accepts_images(
        &f.database,
        Some(&"a".repeat(32))
    ));
    let mut question = input(&f, TutorMode::Explain);
    question.profile_id = Some("a".repeat(32));
    question.attachments = vec![TutorAttachmentInput::Image {
        asset_id: asset,
        draft_id: Some(draft),
    }];
    let error = super::super::service::send(&f.database, &paths, &runtime, question)
        .await
        .unwrap_err();
    assert_eq!(error.code, "tutor_images_unsupported");
    assert_eq!(count(&f.database, "SELECT COUNT(*) FROM tutor_turns"), 0);
    assert_eq!(
        count(
            &f.database,
            "SELECT COUNT(*) FROM assets WHERE lifecycle_state = 'staged'"
        ),
        1
    );
    let _ = std::fs::remove_dir_all(paths.app_data_dir());
}

#[test]
fn attached_pictures_go_to_the_model_by_name_in_the_order_attached() {
    let f = fixture();
    let paths = library_paths("sent");
    let (first_draft, first) = picture(&f.database, &paths);
    let (second_draft, second) = picture(&f.database, &paths);
    let mut question = input(&f, TutorMode::Explain);
    question.attachments = vec![
        TutorAttachmentInput::Image {
            asset_id: first,
            draft_id: Some(first_draft),
        },
        text_file("notes.md", "Citrate"),
        TutorAttachmentInput::Image {
            asset_id: second,
            draft_id: Some(second_draft),
        },
    ];
    let identity = turns::begin(&f.database, Some(&paths), &question, &f.task).unwrap();
    let attachments = f
        .database
        .with_connection(|connection| turns::load_attachments(connection, &identity.id))
        .unwrap();
    let images = super::super::context::attached_images(&f.database, &paths, &attachments).unwrap();
    assert_eq!(
        images
            .iter()
            .map(|image| (image.page_number, image.shows.clone()))
            .collect::<Vec<_>>(),
        attachments
            .iter()
            .filter_map(|attachment| match attachment {
                TutorAttachment::Image { name, .. } => Some(name.clone()),
                TutorAttachment::Text { .. } => None,
            })
            .enumerate()
            .map(|(index, name)| (
                index as i64 + 1,
                crate::ai::document_intelligence::ImageShows::Attached(name)
            ))
            .collect::<Vec<_>>()
    );
    let _ = std::fs::remove_dir_all(paths.app_data_dir());
}
