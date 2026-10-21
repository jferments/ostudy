use serde_json::{json, Value};

use super::*;
use crate::collections::test_support::seed_document;

const SUBJECT: &str = "5555555555555555555555555555555a";
const DOCUMENT: &str = "dddddddddddddddddddddddddddddddd";
const DECK: &str = "cccccccccccccccccccccccccccccccc";
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
                    "INSERT INTO decks(id, name, created_at, updated_at) VALUES (?1, 'Cells', 1, 1)",
                    [DECK],
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

fn new_notebook(connection: &mut Connection, title: &str) -> NotebookContentsDto {
    create(
        connection,
        CreateNotebookInput {
            title: title.into(),
            subject_id: Some(SUBJECT.into()),
            paper: Paper::Grid,
            page_size: PageSize::Wide,
        },
        NOW,
    )
    .unwrap()
}

fn change(value: Value) -> NotebookItemChange {
    serde_json::from_value(value).unwrap()
}

fn arrow(item: &str, page: &str, x2: f64) -> NotebookItemChange {
    change(json!({
        "op": "put", "id": item, "pageId": page,
        "shape": {
            "kind": "arrow", "x1": 10, "y1": 10, "x2": x2, "y2": 40,
            "color": "#1f6feb", "strokeWidth": 2
        }
    }))
}

fn link(item: &str, page: &str, target: Value) -> NotebookItemChange {
    change(json!({
        "op": "put", "id": item, "pageId": page,
        "shape": {
            "kind": "text", "x": 20, "y": 20, "width": 160, "text": "Study the cells",
            "color": "#1f6feb", "fontSize": 14,
            "link": { "target": target, "button": true }
        }
    }))
}

fn apply(
    connection: &mut Connection,
    notebook: &str,
    changes: Vec<NotebookItemChange>,
) -> Result<AppliedNotebookItemChangesDto, CommandError> {
    apply_changes(
        connection,
        ApplyNotebookItemChangesInput {
            notebook_id: notebook.into(),
            changes,
        },
        NOW + 10,
    )
}

fn page_ids(contents: &NotebookContentsDto) -> Vec<String> {
    contents.pages.iter().map(|page| page.id.clone()).collect()
}

#[test]
fn a_new_notebook_has_one_page_of_its_paper_and_size() {
    let database = library();
    database
        .with_connection(|connection| {
            let contents = new_notebook(connection, "  Cell biology  ");
            assert_eq!(contents.notebook.title, "Cell biology");
            assert_eq!(contents.notebook.subject_id.as_deref(), Some(SUBJECT));
            assert_eq!(contents.pages.len(), 1);
            let page = &contents.pages[0];
            assert_eq!((page.position, page.paper), (1, Paper::Grid));
            assert_eq!((page.width, page.height), (1224.0, 792.0));
            assert!(contents.items.is_empty());

            let listed = list(connection)?;
            assert_eq!(listed.len(), 1);
            assert_eq!((listed[0].page_count, listed[0].link_count), (1, 0));
            assert_eq!(listed[0].first_page.as_ref().unwrap().id, page.id);

            for refused in ["   ", &"x".repeat(201), "tab\there"] {
                assert!(create(
                    connection,
                    CreateNotebookInput {
                        title: refused.into(),
                        subject_id: None,
                        paper: Paper::Blank,
                        page_size: PageSize::Letter,
                    },
                    NOW,
                )
                .is_err());
            }
            let missing_subject = create(
                connection,
                CreateNotebookInput {
                    title: "Lost".into(),
                    subject_id: Some(id('9')),
                    paper: Paper::Blank,
                    page_size: PageSize::Letter,
                },
                NOW,
            )
            .unwrap_err();
            assert_eq!(missing_subject.code, "subject_not_found");
            Ok(())
        })
        .unwrap();
}

#[test]
fn marks_are_put_replaced_and_deleted_and_a_repeat_changes_nothing() {
    let database = library();
    database
        .with_connection(|connection| {
            let contents = new_notebook(connection, "Cells");
            let notebook = contents.notebook.id.clone();
            let page = contents.pages[0].id.clone();

            let applied = apply(connection, &notebook, vec![arrow(&id('a'), &page, 90.0)])?;
            assert_eq!(applied.items[0].revision, 1);
            let repeated = apply(connection, &notebook, vec![arrow(&id('a'), &page, 90.0)])?;
            assert_eq!(repeated.items[0].revision, 1);
            let moved = apply(connection, &notebook, vec![arrow(&id('a'), &page, 120.0)])?;
            assert_eq!(moved.items[0].revision, 2);

            // Later marks draw on top, even within one request.
            apply(
                connection,
                &notebook,
                vec![arrow(&id('b'), &page, 1.0), arrow(&id('c'), &page, 2.0)],
            )?;
            let order = contents_ids(connection, &notebook);
            assert_eq!(order, vec![id('a'), id('b'), id('c')]);

            let deleted = apply(
                connection,
                &notebook,
                vec![change(json!({ "op": "delete", "id": id('b') }))],
            )?;
            assert_eq!(deleted.deleted_ids, vec![id('b')]);
            apply(
                connection,
                &notebook,
                vec![change(json!({ "op": "delete", "id": id('b') }))],
            )?;
            assert_eq!(contents_ids(connection, &notebook), vec![id('a'), id('c')]);
            Ok(())
        })
        .unwrap();
}

fn contents_ids(connection: &Connection, notebook: &str) -> Vec<String> {
    super::contents(connection, notebook)
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.id)
        .collect()
}

#[test]
fn a_mark_cannot_land_on_or_take_over_another_notebook() {
    let database = library();
    database
        .with_connection(|connection| {
            let first = new_notebook(connection, "First");
            let second = new_notebook(connection, "Second");
            let first_page = first.pages[0].id.clone();
            let second_page = second.pages[0].id.clone();
            apply(
                connection,
                &first.notebook.id,
                vec![arrow(&id('a'), &first_page, 50.0)],
            )?;

            let foreign_page = apply(
                connection,
                &first.notebook.id,
                vec![arrow(&id('b'), &second_page, 50.0)],
            )
            .unwrap_err();
            assert_eq!(foreign_page.code, "notebook_page_missing");
            let taken = apply(
                connection,
                &second.notebook.id,
                vec![arrow(&id('a'), &second_page, 50.0)],
            )
            .unwrap_err();
            assert_eq!(taken.code, "notebook_item_conflict");
            let deleted = apply(
                connection,
                &second.notebook.id,
                vec![change(json!({ "op": "delete", "id": id('a') }))],
            )
            .unwrap_err();
            assert_eq!(deleted.code, "notebook_item_conflict");

            // A rejected request writes nothing, even its valid changes.
            let partial = apply(
                connection,
                &first.notebook.id,
                vec![
                    arrow(&id('c'), &first_page, 50.0),
                    arrow(&id('d'), &second_page, 50.0),
                ],
            );
            assert!(partial.is_err());
            assert_eq!(contents_ids(connection, &first.notebook.id), vec![id('a')]);

            let twice = apply(
                connection,
                &first.notebook.id,
                vec![
                    arrow(&id('e'), &first_page, 50.0),
                    change(json!({ "op": "delete", "id": id('e') })),
                ],
            );
            assert!(twice.is_err());
            Ok(())
        })
        .unwrap();
}

#[test]
fn text_highlights_and_sticky_links_are_refused_on_paper() {
    let database = library();
    database
        .with_connection(|connection| {
            let contents = new_notebook(connection, "Cells");
            let notebook = contents.notebook.id.clone();
            let page = contents.pages[0].id.clone();
            let text_highlight = change(json!({
                "op": "put", "id": id('a'), "pageId": page,
                "shape": {
                    "kind": "highlight", "points": [],
                    "color": "#ffd33d", "strokeWidth": 12,
                    "rects": [10, 5, 30, 10], "text": "cell"
                }
            }));
            assert!(apply(connection, &notebook, vec![text_highlight]).is_err());

            let sticky_link = change(json!({
                "op": "put", "id": id('b'), "pageId": page,
                "shape": {
                    "kind": "text", "x": 20, "y": 20, "width": 160, "text": "Cells",
                    "color": "#fff3b0", "fontSize": 14, "sticky": true,
                    "link": { "target": { "kind": "deck", "deckId": DECK } }
                }
            }));
            assert!(apply(connection, &notebook, vec![sticky_link]).is_err());

            let page_zero = link(
                &id('c'),
                &page,
                json!({ "kind": "documentPage", "documentId": DOCUMENT, "pageNumber": 0 }),
            );
            assert!(apply(connection, &notebook, vec![page_zero]).is_err());
            Ok(())
        })
        .unwrap();
}

#[test]
fn links_to_missing_targets_stay_and_are_reported_unavailable() {
    let database = library();
    database
        .with_connection(|connection| {
            let contents = new_notebook(connection, "Cells");
            let notebook = contents.notebook.id.clone();
            let page = contents.pages[0].id.clone();
            let gone = id('9');
            apply(
                connection,
                &notebook,
                vec![
                    link(
                        &id('1'),
                        &page,
                        json!({ "kind": "documentPage", "documentId": DOCUMENT, "pageNumber": 3 }),
                    ),
                    link(
                        &id('2'),
                        &page,
                        json!({ "kind": "documentPage", "documentId": DOCUMENT, "pageNumber": 4 }),
                    ),
                    link(
                        &id('3'),
                        &page,
                        json!({ "kind": "documentPage", "documentId": gone, "pageNumber": 1 }),
                    ),
                    link(&id('4'), &page, json!({ "kind": "deck", "deckId": DECK })),
                    link(&id('5'), &page, json!({ "kind": "deck", "deckId": gone })),
                    link(
                        &id('6'),
                        &page,
                        json!({ "kind": "studyGuide", "documentId": DOCUMENT }),
                    ),
                    link(
                        &id('7'),
                        &page,
                        json!({ "kind": "notebookPage", "notebookId": notebook, "pageId": page }),
                    ),
                    link(
                        &id('8'),
                        &page,
                        json!({ "kind": "notebookPage", "notebookId": notebook, "pageId": gone }),
                    ),
                ],
            )?;
            let opened = super::contents(connection, &notebook)?;
            assert_eq!(
                opened.unavailable_link_ids,
                vec![id('2'), id('3'), id('5'), id('6'), id('8')]
            );
            assert_eq!(list(connection)?[0].link_count, 8);

            connection
                .execute("UPDATE decks SET deleted_at = 5 WHERE id = ?1", [DECK])
                .map_err(CommandError::database)?;
            assert!(super::contents(connection, &notebook)?
                .unavailable_link_ids
                .contains(&id('4')));
            Ok(())
        })
        .unwrap();
}

#[test]
fn pages_are_added_moved_and_deleted_in_order() {
    let database = library();
    database
        .with_connection(|connection| {
            let first = new_notebook(connection, "Cells");
            let notebook = first.notebook.id.clone();
            let one = first.pages[0].id.clone();
            let add = |connection: &mut Connection, position: i64| {
                add_page(
                    connection,
                    AddNotebookPageInput {
                        notebook_id: notebook.clone(),
                        position,
                        paper: Some(Paper::Lined),
                        page_size: None,
                    },
                    NOW,
                )
                .unwrap()
            };
            let two = add(connection, 99);
            assert_eq!(two.pages.len(), 2);
            assert_eq!(two.pages[1].paper, Paper::Lined);
            assert_eq!(two.pages[1].width, 1224.0);
            let three = add(connection, 1);
            let ids = page_ids(&three);
            assert_eq!(ids[1], one);
            assert_eq!(
                three
                    .pages
                    .iter()
                    .map(|page| page.position)
                    .collect::<Vec<_>>(),
                vec![1, 2, 3]
            );

            let moved = move_page(
                connection,
                MoveNotebookPageInput {
                    notebook_id: notebook.clone(),
                    page_id: ids[0].clone(),
                    position: 3,
                },
                NOW,
            )?;
            assert_eq!(
                page_ids(&moved),
                vec![ids[1].clone(), ids[2].clone(), ids[0].clone()]
            );
            let back = move_page(
                connection,
                MoveNotebookPageInput {
                    notebook_id: notebook.clone(),
                    page_id: ids[0].clone(),
                    position: 1,
                },
                NOW,
            )?;
            assert_eq!(page_ids(&back), ids);

            apply(connection, &notebook, vec![arrow(&id('a'), &one, 50.0)])?;
            let input = || NotebookPageInput {
                notebook_id: notebook.clone(),
                page_id: one.clone(),
            };
            let deleted = delete_page(connection, input(), NOW)?;
            assert_eq!(page_ids(&deleted), vec![ids[0].clone(), ids[2].clone()]);
            assert_eq!(
                deleted
                    .pages
                    .iter()
                    .map(|page| page.position)
                    .collect::<Vec<_>>(),
                vec![1, 2]
            );
            assert!(deleted.items.is_empty());
            // Deleting it again is a repeated request, not an error.
            delete_page(connection, input(), NOW)?;

            delete_page(
                connection,
                NotebookPageInput {
                    notebook_id: notebook.clone(),
                    page_id: ids[0].clone(),
                },
                NOW,
            )?;
            let last = delete_page(
                connection,
                NotebookPageInput {
                    notebook_id: notebook.clone(),
                    page_id: ids[2].clone(),
                },
                NOW,
            )
            .unwrap_err();
            assert_eq!(last.code, "notebook_last_page");
            Ok(())
        })
        .unwrap();
}

#[test]
fn deleting_the_subject_files_notebooks_under_no_subject() {
    let database = library();
    database
        .with_connection(|connection| {
            let contents = new_notebook(connection, "Cells");
            let notebook = contents.notebook.id.clone();
            let renamed = rename(
                connection,
                RenameNotebookInput {
                    notebook_id: notebook.clone(),
                    title: "Cell structure".into(),
                },
                NOW + 1,
            )?;
            assert_eq!(
                (renamed.title.as_str(), renamed.revision),
                ("Cell structure", 2)
            );

            connection
                .execute("DELETE FROM document_collections WHERE id = ?1", [SUBJECT])
                .map_err(CommandError::database)?;
            assert_eq!(
                super::contents(connection, &notebook)?.notebook.subject_id,
                None
            );

            delete(connection, &notebook)?;
            delete(connection, &notebook)?;
            for table in ["notebooks", "notebook_pages", "notebook_items"] {
                let rows: i64 = connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })
                    .map_err(CommandError::database)?;
                assert_eq!(rows, 0, "{table}");
            }
            Ok(())
        })
        .unwrap();
}
