use serde_json::{json, Value};

use super::*;
use crate::collections::test_support::seed_document;

const SUBJECT: &str = "5555555555555555555555555555555a";
const DOCUMENT: &str = "dddddddddddddddddddddddddddddddd";
const LESSON: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const NOW: i64 = 1_700_000_000_000;

fn library() -> Database {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO document_collections(
                         id, name, sort_order, created_at, updated_at, revision
                     ) VALUES (?1, 'Biology', 1000, 1, 1, 1)",
                    [SUBJECT],
                )
                .map_err(CommandError::database)?;
            seed_document(connection, DOCUMENT, 3, 'a')?;
            connection
                .execute(
                    "INSERT INTO lessons(id, subject_id, sort_order, title, revision, created_at, updated_at)
                     VALUES (?1, ?2, 0, 'Cells', 1, 1, 1)",
                    [LESSON, SUBJECT],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    database
}

fn id(seed: char) -> String {
    seed.to_string().repeat(32)
}

fn lesson() -> InkMaterial {
    InkMaterial::Lesson {
        lesson_id: LESSON.into(),
    }
}

fn guide() -> InkMaterial {
    InkMaterial::StudyGuide {
        document_id: DOCUMENT.into(),
    }
}

fn shape(value: Value) -> AnnotationShape {
    serde_json::from_value(value).unwrap()
}

fn ink() -> AnnotationShape {
    shape(
        json!({ "kind": "ink", "points": [10, 10, 0.5, 40, 30, 0.6], "color": "#d64545", "strokeWidth": 3, "simulatePressure": false }),
    )
}

fn put(id: String, page_key: &str, shape: AnnotationShape) -> InkChange {
    InkChange::Put {
        id,
        page_key: page_key.into(),
        shape,
    }
}

fn apply(
    connection: &mut Connection,
    material: InkMaterial,
    changes: Vec<InkChange>,
) -> Result<AppliedInkChangesDto, CommandError> {
    apply_changes(
        connection,
        ApplyMaterialInkChangesInput { material, changes },
        NOW,
    )
}

#[test]
fn marks_on_slides_and_guide_pages_are_kept_apart_and_repeat_safely() {
    let database = library();
    database
        .with_connection(|connection| {
            let first = apply(
                connection,
                lesson(),
                vec![put(id('1'), "step:cells", ink())],
            )?;
            assert_eq!(first.marks[0].revision, 1);
            // The same request again changes nothing.
            let again = apply(
                connection,
                lesson(),
                vec![put(id('1'), "step:cells", ink())],
            )?;
            assert_eq!(again.marks[0].revision, 1);
            apply(connection, guide(), vec![put(id('2'), &id('7'), ink())])?;

            let slides = read(connection, &lesson())?;
            assert_eq!(slides.marks.len(), 1);
            assert_eq!(slides.marks[0].page_key, "step:cells");
            let pages = read(connection, &guide())?;
            assert_eq!(pages.marks.len(), 1);
            assert_eq!(pages.marks[0].page_key, id('7'));

            // A mark cannot be moved to another material.
            let error = apply(connection, guide(), vec![put(id('1'), "title", ink())]).unwrap_err();
            assert_eq!(error.code, "material_ink_conflict");
            let moved = apply(connection, lesson(), vec![put(id('1'), "title", ink())])?;
            assert_eq!(moved.marks[0].revision, 2);

            let deleted = apply(
                connection,
                lesson(),
                vec![
                    InkChange::Delete { id: id('1') },
                    InkChange::Delete { id: id('9') },
                ],
            )?;
            assert_eq!(deleted.deleted_ids.len(), 2);
            assert!(read(connection, &lesson())?.marks.is_empty());
            Ok(())
        })
        .unwrap();
}

#[test]
fn text_highlights_links_and_odd_page_keys_are_refused() {
    let database = library();
    database
        .with_connection(|connection| {
            let link = shape(json!({
                "kind": "text", "x": 10, "y": 10, "width": 120, "text": "Deck", "color": "#111111",
                "fontSize": 14, "link": { "target": { "kind": "deck", "deckId": id('c') }, "button": true }
            }));
            for (page_key, shape) in [("title", link), ("bad key", ink()), ("", ink())] {
                let error = apply(connection, lesson(), vec![put(id('3'), page_key, shape)]).unwrap_err();
                assert_eq!(error.code, "material_ink_invalid", "{page_key}");
            }
            let missing = InkMaterial::Lesson {
                lesson_id: id('f'),
            };
            let error = apply(connection, missing, vec![put(id('3'), "title", ink())]).unwrap_err();
            assert_eq!(error.code, "material_ink_material_missing");
            Ok(())
        })
        .unwrap();
}

#[test]
fn deleting_the_lesson_or_document_deletes_its_marks() {
    let database = library();
    database
        .with_connection(|connection| {
            apply(connection, lesson(), vec![put(id('1'), "title", ink())])?;
            apply(connection, guide(), vec![put(id('2'), &id('7'), ink())])?;
            connection
                .execute("DELETE FROM lessons WHERE id = ?1", [LESSON])
                .map_err(CommandError::database)?;
            connection
                .execute("DELETE FROM documents WHERE id = ?1", [DOCUMENT])
                .map_err(CommandError::database)?;
            let left: i64 = connection
                .query_row("SELECT COUNT(*) FROM material_ink", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(left, 0);
            Ok(())
        })
        .unwrap();
}
