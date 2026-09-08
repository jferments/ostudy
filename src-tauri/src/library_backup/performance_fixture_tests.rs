use std::{fs, path::PathBuf};

use rusqlite::params;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{
    cards::validate_content,
    database::Database,
    decks::{create_deck_in_connection, CreateDeckInput},
    document_domain::{SourceLocator, SourceUnitKind},
    document_learning::{
        ensure_document_learning_plan, DocumentLearningPlanInput, LearningEvidenceChunk,
        LearningTopicInput,
    },
    document_study_guides::{
        publish_document_study_guide, StudyGuideOverview, StudyGuideSectionContent,
        StudyGuideSectionDraft,
    },
    error::CommandError,
    library_backup::export_backup,
    paths::AppPaths,
    scheduler::insert_new_schedule,
    util::generate_id,
};

const FIXTURE_TIME: i64 = 1_700_000_000_000;

fn performance_fixture_app_data(scenario: &str) -> (PathBuf, bool) {
    if let Some(path) = std::env::var_os("OSTUDY_UI_PERFORMANCE_FIXTURE_APP_DATA") {
        let path = PathBuf::from(path);
        assert!(path.starts_with(std::env::temp_dir()));
        return (path, false);
    }

    let path = std::env::temp_dir().join(format!(
        "ostudy-ui-performance-{scenario}-{}-{}",
        std::process::id(),
        crate::util::unix_time_millis().expect("fixture clock")
    ));
    assert!(!path.exists(), "isolated fixture path must be new");
    (path, true)
}

fn cleanup_performance_fixture_app_data(path: &PathBuf, cleanup: bool) {
    if cleanup {
        fs::remove_dir_all(path).expect("remove isolated performance fixture state");
    }
}

fn rich_doc(text: &str) -> serde_json::Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
}

fn insert_cards(
    database: &Database,
    deck_id: &str,
    prefix: &str,
    count: usize,
) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        for index in 1..=count {
            let note_id = generate_id(connection)?;
            let card_id = generate_id(connection)?;
            let question = format!("{prefix} concept {index}?");
            let answer = format!("Deterministic {prefix} answer {index}.");
            let content = validate_content(
                rich_doc(&question),
                rich_doc(&answer),
                question.clone(),
                answer.clone(),
            )?;
            connection
                .execute(
                    "INSERT INTO notes (
                         id, deck_id, note_type, content_format, content_schema_version,
                         front_doc, back_doc, front_text, back_text, duplicate_hash,
                         created_at, updated_at, revision
                     ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, ?6, ?7, ?8, ?8, 1)",
                    params![
                        note_id,
                        deck_id,
                        content.front_doc_json,
                        content.back_doc_json,
                        content.front_text,
                        content.back_text,
                        format!("ui-performance-{prefix}-{index}"),
                        FIXTURE_TIME,
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO cards (
                         id, note_id, deck_id, direction, template_ordinal,
                         created_at, updated_at
                     ) VALUES (?1, ?2, ?3, 'forward', 0, ?4, ?4)",
                    params![card_id, note_id, deck_id, FIXTURE_TIME],
                )
                .map_err(CommandError::database)?;
            insert_new_schedule(connection, &card_id, FIXTURE_TIME - 10_000)?;
        }
        Ok(())
    })
}

fn insert_document(
    database: &Database,
    paths: &AppPaths,
    document_id: &str,
    title: &str,
    archived: bool,
    bytes: &[u8],
) -> Result<(String, SourceLocator), CommandError> {
    let relative = AppPaths::document_source_relative_path(document_id);
    let source_path = paths.resolve_relative(&relative);
    fs::create_dir_all(source_path.parent().expect("fixture source parent"))
        .map_err(CommandError::io)?;
    fs::write(&source_path, bytes).map_err(CommandError::io)?;
    let hash = format!("{:x}", Sha256::digest(bytes));
    let locator = SourceLocator::new(SourceUnitKind::Page, vec![1], Some(title.to_string()))?;
    let locator_json = serde_json::to_string(&locator).map_err(CommandError::serialization)?;
    database.with_connection(|connection| {
        connection
            .execute(
                "INSERT INTO documents (
                     id, title, original_filename, mime_type, byte_size, sha256,
                     source_relative_path, lifecycle_state, page_count, extracted_page_count,
                     generated_note_count, generated_card_count, created_at, updated_at,
                     completed_at, archived_at, revision, extraction_backend, render_backend,
                     outline_state, source_format, source_unit_kind, source_unit_count,
                     extracted_source_unit_count, source_media_type, adapter_backend
                 ) VALUES (
                     ?1, ?2, ?3, 'application/pdf', ?4, ?5, ?6, ?7, 1, 1,
                     0, 0, ?8, ?8, ?8, ?9, 1, 'pdfium-text-v1', 'pdfium-render-v1',
                     'ready', 'pdf', 'page', 1, 1, 'application/pdf', 'native-pdf-adapter-v1'
                 )",
                params![
                    document_id,
                    title,
                    format!("{}.pdf", title.to_lowercase().replace(' ', "-")),
                    i64::try_from(bytes.len()).unwrap_or(i64::MAX),
                    hash,
                    relative,
                    if archived { "archived" } else { "ready" },
                    FIXTURE_TIME,
                    archived.then_some(FIXTURE_TIME),
                ],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO document_source_files (
                     document_id, file_ordinal, original_filename, media_type,
                     relative_path, byte_size, sha256, created_at, updated_at
                 ) VALUES (?1, 1, ?2, 'application/pdf', ?3, ?4, ?5, ?6, ?6)",
                params![
                    document_id,
                    format!("{}.pdf", title.to_lowercase().replace(' ', "-")),
                    relative,
                    i64::try_from(bytes.len()).unwrap_or(i64::MAX),
                    hash,
                    FIXTURE_TIME,
                ],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO document_source_units (
                     document_id, unit_number, unit_kind, label, title, plain_text,
                     text_sha256, source_file_ordinal, locator_json, extraction_confidence,
                     created_at, updated_at
                 ) VALUES (?1, 1, 'page', 'Page 1', ?2, ?3, ?4, 1, ?5, 1.0, ?6, ?6)",
                params![
                    document_id,
                    title,
                    "Synthetic cardiopulmonary source.",
                    hash,
                    locator_json,
                    FIXTURE_TIME
                ],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO document_semantic_chunk_sources (
                     document_id, chunk_index, text, full_text, source_locator_json,
                     visual_asset_ids_json, element_types_json, heading_context,
                     token_estimate, is_oversized, content_sha256, canonical_schema_version,
                     created_at, updated_at
                 ) VALUES (?1, 0, ?2, ?2, ?3, '[]', '[\"paragraph\"]', ?4, 12, 0, ?5, 1, ?6, ?6)",
                params![
                    document_id,
                    "Synthetic cardiopulmonary source.",
                    locator_json,
                    title,
                    hash,
                    FIXTURE_TIME
                ],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })?;
    Ok((hash, locator))
}

#[test]
pub fn writes_stage_0_ui_performance_pending_restore() {
    let (app_data, cleanup) = performance_fixture_app_data("stage-0");
    let paths = AppPaths::new(app_data.clone()).expect("isolated fixture paths");
    let database = Database::open(paths.database_path()).expect("fixture database");

    let (primary_deck, secondary_deck) = database
        .with_connection(|connection| {
            let primary = create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: "Cardiopulmonary foundations".to_string(),
                    description: Some("Stage 0 performance fixture".to_string()),
                },
            )?;
            let secondary = create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: "Hemodynamic essentials".to_string(),
                    description: Some("Stage 0 performance fixture".to_string()),
                },
            )?;
            let folder_id = generate_id(connection)?;
            connection
                .execute(
                    "INSERT INTO deck_folders (id, name, sort_order, created_at, updated_at, revision)
                     VALUES (?1, 'Clinical foundations', 0, ?2, ?2, 1)",
                    params![folder_id, FIXTURE_TIME],
                )
                .map_err(CommandError::database)?;
            connection
                .execute("UPDATE decks SET folder_id = ?1 WHERE id = ?2", params![folder_id, primary.id])
                .map_err(CommandError::database)?;
            for (key, value) in [
                ("theme", json!("light")),
                ("uiScale", json!(100)),
                ("motionPreference", json!("full")),
                ("materialPreference", json!("rich")),
                ("practiceShuffle", json!(false)),
                ("tutorialsEnabled", json!(false)),
            ] {
                connection
                    .execute(
                        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
                        params![key, value.to_string(), FIXTURE_TIME],
                    )
                    .map_err(CommandError::database)?;
            }
            Ok((primary.id, secondary.id))
        })
        .expect("decks and settings");
    insert_cards(&database, &primary_deck, "cardiopulmonary", 8).expect("primary cards");
    insert_cards(&database, &secondary_deck, "hemodynamic", 8).expect("secondary cards");

    let active_id = "11111111111111111111111111111111";
    let archived_id = "22222222222222222222222222222222";
    let (content_hash, locator) = insert_document(
        &database,
        &paths,
        active_id,
        "Cardiopulmonary source",
        false,
        b"%PDF-1.4 synthetic active performance fixture",
    )
    .expect("active document");
    insert_document(
        &database,
        &paths,
        archived_id,
        "Archived physiology source",
        true,
        b"%PDF-1.4 synthetic archived performance fixture",
    )
    .expect("archived document");
    let plan = ensure_document_learning_plan(
        &database,
        active_id,
        DocumentLearningPlanInput {
            summary: "Cardiopulmonary physiology foundations.".to_string(),
            learning_objectives: vec!["Explain pulmonary circulation.".to_string()],
            key_concepts: vec!["Preload".to_string()],
            topics: vec![LearningTopicInput {
                parent_index: None,
                title: "Pulmonary circulation".to_string(),
                summary: "Flow through the pulmonary circuit.".to_string(),
                learning_objectives: vec!["Relate pressure and flow.".to_string()],
                key_concepts: vec!["Resistance".to_string()],
                source_locator: locator.clone(),
            }],
        },
        &[LearningEvidenceChunk {
            chunk_index: 0,
            source_locator: locator,
            content_hash,
        }],
    )
    .expect("learning plan");
    publish_document_study_guide(
        &database,
        active_id,
        &plan.id,
        None,
        StudyGuideOverview {
            summary: plan.summary.clone(),
            learning_objectives: plan.learning_objectives.clone(),
            key_concepts: plan.key_concepts.clone(),
        },
        vec![StudyGuideSectionDraft {
            knowledge_support: Vec::new(),
            topic_id: plan.topics[0].id.clone(),
            content: StudyGuideSectionContent {
                overview: "Pulmonary blood flow supports gas exchange.".to_string(),
                key_points: vec![
                    "Right ventricular output enters the pulmonary arteries.".to_string()
                ],
                explanations: Vec::new(),
                important_terms: Vec::new(),
                process_steps: Vec::new(),
                comparisons: Vec::new(),
                common_misconceptions: Vec::new(),
                visual_explanations: Vec::new(),
            },
        }],
    )
    .expect("study guide");

    let pending = paths
        .app_data_dir()
        .join("pending-library-restore.ostudybackup");
    export_backup(&database, &paths, &pending).expect("validated pending restore fixture");
    assert!(pending.is_file());
    let counts = database
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM documents),
                            (SELECT COUNT(*) FROM decks WHERE deleted_at IS NULL),
                            (SELECT COUNT(*) FROM cards WHERE deleted_at IS NULL),
                            (SELECT COUNT(*) FROM document_study_guides WHERE state = 'active')",
                    [],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                        ))
                    },
                )
                .map_err(CommandError::database)
        })
        .expect("fixture counts");
    assert_eq!(counts, (2, 2, 16, 1));
    drop(database);
    cleanup_performance_fixture_app_data(&app_data, cleanup);
}

#[test]
pub fn writes_stage_4a_large_library_drag_pending_restore() {
    let (app_data, cleanup) = performance_fixture_app_data("stage-4a");
    let paths = AppPaths::new(app_data.clone()).expect("isolated fixture paths");
    let database = Database::open(paths.database_path()).expect("fixture database");

    database
        .with_connection(|connection| {
            let folder_id = generate_id(connection)?;
            connection
                .execute(
                    "INSERT INTO deck_folders (id, name, sort_order, created_at, updated_at, revision)
                     VALUES (?1, 'Large library folder', 0, ?2, ?2, 1)",
                    params![folder_id, FIXTURE_TIME],
                )
                .map_err(CommandError::database)?;
            for index in 0..120 {
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: if index == 0 {
                            "Large library drag source".to_string()
                        } else {
                            format!("Large library deck {:03}", index + 1)
                        },
                        description: Some(
                            "Stage 4A large-library drag candidate fixture".to_string(),
                        ),
                    },
                )?;
                if index % 2 == 0 {
                    connection
                        .execute(
                            "UPDATE decks SET folder_id = ?1 WHERE id = ?2",
                            params![folder_id, deck.id],
                        )
                        .map_err(CommandError::database)?;
                }
            }
            for (key, value) in [
                ("theme", json!("light")),
                ("uiScale", json!(100)),
                ("motionPreference", json!("full")),
                ("materialPreference", json!("rich")),
                ("tutorialsEnabled", json!(false)),
            ] {
                connection
                    .execute(
                        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
                        params![key, value.to_string(), FIXTURE_TIME],
                    )
                    .map_err(CommandError::database)?;
            }
            Ok(())
        })
        .expect("large-library decks and settings");

    let pending = paths
        .app_data_dir()
        .join("pending-library-restore.ostudybackup");
    export_backup(&database, &paths, &pending).expect("validated Stage 4A pending restore fixture");
    assert!(pending.is_file());
    let deck_count = database
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM decks WHERE deleted_at IS NULL",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(CommandError::database)
        })
        .expect("large-library fixture count");
    assert_eq!(deck_count, 120);
    drop(database);
    cleanup_performance_fixture_app_data(&app_data, cleanup);
}
