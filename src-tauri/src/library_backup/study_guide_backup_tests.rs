mod study_guide_artifact_backup_tests {
    use std::fs;

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
            publish_document_study_guide, save_study_guide_overlay, SaveStudyGuideOverlayInput,
            StudyGuideOverlayKind, StudyGuideOverview, StudyGuideSectionContent,
            StudyGuideSectionDraft,
        },
        document_topic_links::link_generated_note_to_learning_plan,
        error::CommandError,
        library_backup::{
            apply_pending_restore, copy_and_sync, export_backup, pending_restore_path,
        },
        paths::AppPaths,
        util::{generate_id, unix_time_millis},
    };

    fn temp_paths(label: &str) -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "ostudy-study-guide-backup-{label}-{}-{}",
            std::process::id(),
            unix_time_millis().expect("clock")
        ));
        AppPaths::new(root).expect("paths")
    }

    fn source_bytes() -> &'static [u8] {
        b"topic,explanation\nPreload,Ventricular filling before systole\n"
    }

    fn guide_content() -> StudyGuideSectionContent {
        StudyGuideSectionContent {
            overview: "Preload is ventricular filling before systole.".to_string(),
            key_points: vec!["More filling can increase stroke volume.".to_string()],
            explanations: Vec::new(),
            important_terms: Vec::new(),
            process_steps: Vec::new(),
            comparisons: Vec::new(),
            common_misconceptions: Vec::new(),
            visual_explanations: Vec::new(),
        }
    }

    fn rich_doc(text: &str) -> serde_json::Value {
        json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": text}]
            }]
        })
    }

    #[test]
    fn study_guide_topic_graph_survives_complete_backup_restore() {
        let source_paths = temp_paths("source");
        let source = Database::open(source_paths.database_path()).expect("source database");
        let document_id = "d".repeat(32);
        let source_relative =
            AppPaths::document_source_relative_path_with_extension(&document_id, "csv");
        let source_path = source_paths.resolve_relative(&source_relative);
        fs::create_dir_all(source_path.parent().expect("source parent")).expect("source parent");
        fs::write(&source_path, source_bytes()).expect("managed CSV source");
        let source_hash = format!("{:x}", Sha256::digest(source_bytes()));
        let locator = SourceLocator::new(
            SourceUnitKind::Section,
            vec![1],
            Some("Preload".to_string()),
        )
        .expect("source locator");
        let locator_json = serde_json::to_string(&locator).expect("locator json");

        let note_id = source
            .with_connection(|connection| {
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Topic-linked cards".to_string(),
                        description: Some("Backup provenance fixture".to_string()),
                    },
                )?;
                let note_id = generate_id(connection)?;
                let card_id = generate_id(connection)?;
                let content = validate_content(
                    rich_doc("What is preload?"),
                    rich_doc("Ventricular filling before systole."),
                    "What is preload?".to_string(),
                    "Ventricular filling before systole.".to_string(),
                )?;
                connection
                    .execute(
                        "INSERT INTO notes (
                             id, deck_id, note_type, content_format, content_schema_version,
                             front_doc, back_doc, front_text, back_text, duplicate_hash,
                             created_at, updated_at, revision
                         ) VALUES (
                             ?1, ?2, 'basic', 'tiptap-json', 1,
                             ?3, ?4, ?5, ?6, 'study-guide-backup-note', 10, 10, 1
                         )",
                        params![
                            note_id,
                            deck.id,
                            content.front_doc_json,
                            content.back_doc_json,
                            content.front_text,
                            content.back_text,
                        ],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO cards (
                             id, note_id, deck_id, direction, template_ordinal,
                             created_at, updated_at
                         ) VALUES (?1, ?2, ?3, 'forward', 0, 10, 10)",
                        params![card_id, note_id, deck.id],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO documents (
                             id, title, original_filename, mime_type, byte_size, sha256,
                             source_relative_path, lifecycle_state, created_at, updated_at,
                             source_format, source_unit_kind, source_unit_count,
                             extracted_source_unit_count, source_media_type,
                             adapter_backend, extraction_backend
                         ) VALUES (
                             ?1, 'Hemodynamics', 'hemodynamics.csv', 'application/pdf', ?2, ?3,
                             ?4, 'failed', 10, 10,
                             'csv', 'section', 1, 1, 'text/csv',
                             'anydoc-canonical-adapter-v1', 'anydoc-0.2.3'
                         )",
                        params![
                            document_id,
                            i64::try_from(source_bytes().len()).unwrap_or(i64::MAX),
                            source_hash,
                            source_relative,
                        ],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO document_source_files (
                             document_id, file_ordinal, original_filename, media_type,
                             relative_path, byte_size, sha256, created_at, updated_at
                         ) VALUES (?1, 1, 'hemodynamics.csv', 'text/csv', ?2, ?3, ?4, 10, 10)",
                        params![
                            document_id,
                            source_relative,
                            i64::try_from(source_bytes().len()).unwrap_or(i64::MAX),
                            source_hash,
                        ],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO document_source_units (
                             document_id, unit_number, unit_kind, label, title,
                             plain_text, text_sha256, source_file_ordinal, locator_json,
                             extraction_confidence, created_at, updated_at
                         ) VALUES (
                             ?1, 1, 'section', 'Section 1', 'Preload',
                             ?2, ?3, 1, ?4, 1.0, 10, 10
                         )",
                        params![
                            document_id,
                            String::from_utf8_lossy(source_bytes()).to_string(),
                            source_hash,
                            locator_json,
                        ],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO document_semantic_chunk_sources (
                             document_id, chunk_index, text, full_text,
                             source_locator_json, visual_asset_ids_json,
                             element_types_json, heading_context, token_estimate,
                             is_oversized, content_sha256, canonical_schema_version,
                             created_at, updated_at
                         ) VALUES (
                             ?1, 0, ?2, ?2, ?3, '[]', '[\"paragraph\"]',
                             'Preload', 20, 0, ?4, 1, 10, 10
                         )",
                        params![
                            document_id,
                            String::from_utf8_lossy(source_bytes()).to_string(),
                            locator_json,
                            source_hash,
                        ],
                    )
                    .map_err(CommandError::database)?;
                Ok(note_id)
            })
            .expect("document and note fixture");

        let plan = ensure_document_learning_plan(
            &source,
            &document_id,
            DocumentLearningPlanInput {
                summary: "Hemodynamic filling and output.".to_string(),
                learning_objectives: vec!["Explain preload.".to_string()],
                key_concepts: vec!["Preload".to_string()],
                topics: vec![LearningTopicInput {
                    parent_index: None,
                    title: "Preload".to_string(),
                    summary: "Ventricular filling before systole.".to_string(),
                    learning_objectives: vec!["Relate preload to stroke volume.".to_string()],
                    key_concepts: vec!["Frank-Starling".to_string()],
                    source_locator: locator.clone(),
                }],
            },
            &[LearningEvidenceChunk {
                chunk_index: 0,
                source_locator: locator.clone(),
                content_hash: source_hash.clone(),
            }],
        )
        .expect("learning plan");
        let topic_id = plan.topics[0].id.clone();
        let guide = publish_document_study_guide(
            &source,
            &document_id,
            &plan.id,
            None,
            StudyGuideOverview {
                summary: plan.summary.clone(),
                learning_objectives: plan.learning_objectives.clone(),
                key_concepts: plan.key_concepts.clone(),
            },
            vec![StudyGuideSectionDraft {
                knowledge_support: Vec::new(),
                topic_id: topic_id.clone(),
                content: guide_content(),
            }],
        )
        .expect("study guide");
        let overlay = save_study_guide_overlay(
            &source,
            SaveStudyGuideOverlayInput {
                overlay_id: None,
                document_id: document_id.clone(),
                guide_id: None,
                topic_id: Some(topic_id.clone()),
                overlay_kind: StudyGuideOverlayKind::TopicNote,
                base_content_hash: None,
                rich_doc: rich_doc("Compare preload with afterload."),
                plain_text: "Compare preload with afterload.".to_string(),
                expected_revision: None,
            },
        )
        .expect("topic note");
        source
            .with_connection(|connection| {
                link_generated_note_to_learning_plan(
                    connection,
                    &document_id,
                    &note_id,
                    None,
                    &plan,
                    &locator,
                    40,
                )?;
                Ok(())
            })
            .expect("note-topic provenance");

        let backup_path = source_paths.app_data_dir().join("study-guide.ostudybackup");
        export_backup(&source, &source_paths, &backup_path).expect("backup");
        drop(source);

        let target_paths = temp_paths("target");
        {
            let target = Database::open(target_paths.database_path()).expect("target database");
            drop(target);
        }
        let pending = pending_restore_path(&target_paths);
        copy_and_sync(&backup_path, &pending).expect("pending backup");
        let guard = apply_pending_restore(&target_paths)
            .expect("restore should prepare")
            .expect("restore should exist");
        let restored = Database::open(target_paths.database_path()).expect("restored database");
        restored
            .with_connection(|connection| {
                let counts: (i64, i64, i64, i64, i64, i64, i64, i64) = connection
                    .query_row(
                        "SELECT
                             (SELECT COUNT(*) FROM documents WHERE id = ?1),
                             (SELECT COUNT(*) FROM document_learning_plans
                              WHERE id = ?2 AND state = 'active'),
                             (SELECT COUNT(*) FROM document_learning_topics WHERE id = ?3),
                             (SELECT COUNT(*) FROM document_learning_plan_topics
                              WHERE plan_id = ?2 AND topic_id = ?3),
                             (SELECT COUNT(*) FROM document_study_guides
                              WHERE id = ?4 AND state = 'active'),
                             (SELECT COUNT(*) FROM document_study_guide_sections
                              WHERE guide_id = ?4 AND topic_id = ?3),
                             (SELECT COUNT(*) FROM document_study_guide_overlays
                              WHERE id = ?5 AND state = 'active'),
                             (SELECT COUNT(*) FROM document_note_topics
                              WHERE document_id = ?1 AND note_id = ?6 AND topic_id = ?3)",
                        params![
                            document_id,
                            plan.id,
                            topic_id,
                            guide.id,
                            overlay.id,
                            note_id,
                        ],
                        |row| {
                            Ok((
                                row.get(0)?,
                                row.get(1)?,
                                row.get(2)?,
                                row.get(3)?,
                                row.get(4)?,
                                row.get(5)?,
                                row.get(6)?,
                                row.get(7)?,
                            ))
                        },
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(counts, (1, 1, 1, 1, 1, 1, 1, 1));

                let document_state: (String, Option<String>) = connection
                    .query_row(
                        "SELECT lifecycle_state, generated_deck_id FROM documents WHERE id = ?1",
                        [&document_id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(document_state, ("ready".to_string(), None));
                let overlay_text: String = connection
                    .query_row(
                        "SELECT plain_text FROM document_study_guide_overlays WHERE id = ?1",
                        [&overlay.id],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(overlay_text, "Compare preload with afterload.");
                let relationship: String = connection
                    .query_row(
                        "SELECT relationship_kind FROM document_note_topics
                         WHERE document_id = ?1 AND note_id = ?2 AND topic_id = ?3",
                        params![document_id, note_id, topic_id],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(relationship, "primary");
                Ok(())
            })
            .expect("study-guide graph should survive restore");
        assert_eq!(
            fs::read(target_paths.resolve_relative(&source_relative)).expect("restored source"),
            source_bytes()
        );
        drop(restored);
        guard.commit();
        assert!(!pending.exists());
        let _ = fs::remove_dir_all(source_paths.app_data_dir());
        let _ = fs::remove_dir_all(target_paths.app_data_dir());
    }
}
