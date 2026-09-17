use super::*;
use crate::cards::questions::ValidatedQuestion;
use serde_json::json;

fn rich(text: &str) -> Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
}

fn id(value: u32) -> String {
    format!("{value:032x}")
}

fn question_body(index: u32) -> Value {
    let a = id(index * 20 + 1);
    let b = id(index * 20 + 2);
    let c = id(index * 20 + 3);
    let d = id(index * 20 + 4);
    match index {
        1 => json!({"kind":"single_choice","prompt":rich("Pick one."),"options":[
            {"id":a,"content":rich("A"),"rationale":null},
            {"id":b,"content":rich("B"),"rationale":rich("Reason")}
        ],"correctOptionId":a,"explanation":null}),
        2 => json!({"kind":"select_all","prompt":rich("Pick all."),"options":[
            {"id":a,"content":rich("A"),"rationale":null},
            {"id":b,"content":rich("B"),"rationale":null}
        ],"correctOptionIds":[a,b],"explanation":rich("Both")}),
        3 => json!({"kind":"true_false","statement":rich("A proposition."),"correct":false,
            "explanation":null,"correctedStatement":rich("A correction.")}),
        4 => json!({"kind":"matching","prompt":rich("Match."),"pairs":[
            {"left":{"id":a,"content":rich("A")},"right":{"id":c,"content":rich("C")},"rationale":null},
            {"left":{"id":b,"content":rich("B")},"right":{"id":d,"content":rich("D")},"rationale":null}
        ],"explanation":null}),
        5 => json!({"kind":"grouping","prompt":rich("Group."),"categories":[
            {"id":a,"content":rich("First")},{"id":b,"content":rich("Second")}
        ],"items":[
            {"item":{"id":c,"content":rich("C")},"categoryId":a,"rationale":null},
            {"item":{"id":d,"content":rich("D")},"categoryId":b,"rationale":null}
        ],"explanation":null}),
        6 => json!({"kind":"ordering","prompt":rich("Order."),"items":[
            {"id":a,"content":rich("First")},{"id":b,"content":rich("Second")}
        ],"explanation":null}),
        7 => json!({"kind":"image_occlusion","imageAssetId":id(999),"masks":[
            {"id":a,"rect":{"x":0.1,"y":0.1,"width":0.2,"height":0.2}},
            {"id":b,"rect":{"x":0.5,"y":0.5,"width":0.2,"height":0.2}}
        ],"questions":[
            {"id":c,"prompt":rich("Name first."),"answer":rich("First answer"),
                "explanation":null,"maskIds":[a],"mode":"hide_target"},
            {"id":d,"prompt":rich("Name both."),"answer":rich("Both answers"),
                "explanation":rich("Explanation"),"maskIds":[a,b],"mode":"hide_all"}
        ]}),
        _ => unreachable!(),
    }
}

fn package(index: u32) -> PackageData {
    let deck_id = id(1000 + index);
    let note_id = id(2000 + index);
    let question = ValidatedQuestion::parse(
        &json!({"schemaVersion":1,"question":question_body(index)}).to_string(),
    )
    .unwrap();
    let text = question.search_text().unwrap();
    let keys = question.presentation_keys();
    let cards = keys
        .iter()
        .enumerate()
        .map(|(ordinal, key)| PackageCard {
            source_id: id(3000 + index * 10 + u32::try_from(ordinal).unwrap()),
            source_note_id: note_id.clone(),
            direction: "forward".to_string(),
            template_ordinal: i64::try_from(ordinal).unwrap(),
            question_key: Some(key.clone()),
            created_at: 1,
            updated_at: 1,
        })
        .collect();
    let mut data = PackageData {
        version: CURRENT_PACKAGE_VERSION,
        deck: PackageDeck {
            source_id: deck_id.clone(),
            name: format!("Typed {index}"),
            description: String::new(),
            sort_order: 0,
            created_at: 1,
            updated_at: 1,
            revision: 1,
        },
        notes: vec![PackageNote {
            source_id: note_id.clone(),
            source_deck_id: deck_id,
            note_type: "interactive".to_string(),
            content_format: "question-json".to_string(),
            content_schema_version: 1,
            front_doc: Value::Null,
            back_doc: Value::Null,
            front_text: text.prompt,
            back_text: text.answer,
            duplicate_hash: question.duplicate_hash().unwrap(),
            reverse_enabled: false,
            cloze_source: String::new(),
            cloze_mode: "one_at_a_time".to_string(),
            question_definition: Some(question.to_json().unwrap()),
            created_at: 1,
            updated_at: 1,
            revision: 1,
        }],
        cards,
        tags: Vec::new(),
        note_tags: Vec::new(),
        assets: Vec::new(),
        note_assets: Vec::new(),
    };
    if index == 7 {
        data.assets.push(PackageAsset {
            source_id: id(999),
            original_filename: "occlusion.png".to_string(),
            mime_type: "image/png".to_string(),
            byte_size: 12,
            data: vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0],
        });
        data.note_assets
            .push((note_id, id(999), "content".to_string()));
    }
    data
}

fn verify_typed_library(database: &Database, expected_notes: i64, expected_cards: i64) {
    database
        .with_connection(|connection| {
            let typed_notes: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM notes WHERE note_type='interactive' AND front_doc IS NULL
                     AND back_doc IS NULL AND question_definition IS NOT NULL",
                    [],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(typed_notes, expected_notes);
            let presentations: i64 = connection
                .query_row("SELECT COUNT(*) FROM card_question_presentations", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(presentations, expected_cards);
            let schedule_state: (i64, i64, i64) = connection
                .query_row(
                    "SELECT MIN(reps), MAX(reps), MAX(lapses) FROM card_schedules",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(schedule_state, (0, 0, 0));
            let history: i64 = connection
                .query_row("SELECT COUNT(*) FROM review_events", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(history, 0);
            let mut statement = connection
                .prepare("SELECT question_definition FROM notes WHERE note_type='interactive'")
                .map_err(CommandError::database)?;
            for row in statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(CommandError::database)?
            {
                ValidatedQuestion::parse(&row.map_err(CommandError::database)?)?;
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn every_interactive_kind_round_trips_through_version_three_with_fresh_schedules() {
    let paths = super::tests::temp_paths("typed-roundtrip");
    let database = Database::open(paths.database_path()).unwrap();
    let receiver_paths = super::tests::temp_paths("typed-roundtrip-receiver");
    let receiver = Database::open(receiver_paths.database_path()).unwrap();
    let mut expected_cards = 0_i64;
    for index in 1..=7 {
        let data = package(index);
        read::validate_package_data(&data, true).unwrap();
        let expected = data.cards.len();
        expected_cards += i64::try_from(expected).unwrap();
        let result = database
            .with_connection(|connection| {
                import::import_package(
                    connection,
                    &paths,
                    &id(5000 + index),
                    &format!("{:064x}", index),
                    data,
                )
            })
            .unwrap();
        assert_eq!(usize::try_from(result.card_count).unwrap(), expected);

        let exported = paths
            .app_data_dir()
            .join(format!("typed-{index}.studydeck"));
        database
            .with_connection(|connection| {
                export::export_deck_from_connection(
                    connection,
                    &paths,
                    &result.deck.id,
                    &exported,
                )?;
                Ok(())
            })
            .unwrap();
        let package_connection =
            Connection::open_with_flags(&exported, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        assert_eq!(metadata_value(&package_connection, "formatVersion").unwrap(), "3");
        drop(package_connection);
        let imported = read::read_package(&exported).unwrap();
        assert_eq!(imported.cards.len(), expected);
        let fingerprint = hash_file(&exported).unwrap();
        receiver
            .with_connection(|connection| {
                import::import_package(
                    connection,
                    &receiver_paths,
                    &id(7000 + index),
                    &fingerprint,
                    imported,
                )?;
                Ok(())
            })
            .unwrap();
    }
    verify_typed_library(&database, 7, expected_cards);
    verify_typed_library(&receiver, 7, expected_cards);
    let _ = fs::remove_dir_all(paths.app_data_dir());
    let _ = fs::remove_dir_all(receiver_paths.app_data_dir());
}

#[test]
fn invalid_or_downgraded_typed_graph_is_rejected_before_mutation() {
    let paths = super::tests::temp_paths("typed-reject");
    let database = Database::open(paths.database_path()).unwrap();
    let mut invalid = package(1);
    invalid.cards[0].question_key = Some("not-the-canonical-key".to_string());
    assert!(read::validate_package_data(&invalid, true).is_err());
    invalid.version = CORE_PACKAGE_VERSION;
    assert!(read::validate_package_data(&invalid, true).is_err());
    database
        .with_connection(|connection| {
            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM decks", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
    let _ = fs::remove_dir_all(paths.app_data_dir());
}

#[test]
fn asset_identity_collision_is_remapped_inside_occlusion_definition() {
    let paths = super::tests::temp_paths("typed-asset-remap");
    let database = Database::open(paths.database_path()).unwrap();
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO assets (id,original_filename,mime_type,byte_size,relative_path,created_at)
                     VALUES (?1,'existing.png','image/png',1,?2,1)",
                    params![id(999), format!("assets/images/{}.png", id(999))],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    let data = package(7);
    database
        .with_connection(|connection| {
            import::import_package(
                connection,
                &paths,
                &id(6007),
                &"7".repeat(64),
                data,
            )?;
            let definition: String = connection
                .query_row(
                    "SELECT question_definition FROM notes WHERE note_type='interactive'",
                    [],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            let question = ValidatedQuestion::parse(&definition)?;
            assert!(!question.asset_ids().contains(&id(999)));
            let linked: String = connection
                .query_row(
                    "SELECT asset_id FROM note_assets JOIN notes ON notes.id=note_assets.note_id
                     WHERE notes.note_type='interactive'",
                    [],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(question.asset_ids(), &[linked]);
            Ok(())
        })
        .unwrap();
    let _ = fs::remove_dir_all(paths.app_data_dir());
}
