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

/// Managed files for tests that place pictures; marks without pictures never touch them.
fn test_paths() -> AppPaths {
    let root = std::env::temp_dir().join(format!(
        "ostudy-notebook-assets-{}-{}",
        std::process::id(),
        std::thread::current()
            .name()
            .unwrap_or("main")
            .replace("::", "-")
    ));
    AppPaths::new(root).unwrap()
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
            layout: PageLayout::Page,
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
        &test_paths(),
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
                        layout: PageLayout::Page,
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
                    layout: PageLayout::Page,
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
            let listed = list(connection)?;
            assert_eq!(listed[0].link_count, 8);
            // Every link here has the same label, so the tile names it once.
            assert_eq!(listed[0].link_labels, vec!["Study the cells".to_string()]);
            assert_eq!(
                (listed[0].folder_id.clone(), listed[0].pinned_at),
                (None, None)
            );

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
                        layout: PageLayout::Page,
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

fn picture(item: &str, page: &str, asset: &str, draft: Option<&str>) -> NotebookItemChange {
    let mut value = json!({
        "op": "put", "id": item, "pageId": page,
        "shape": { "kind": "image", "x": 40, "y": 40, "width": 120, "height": 80, "assetId": asset }
    });
    if let Some(draft) = draft {
        value["assetDraftId"] = json!(draft);
    }
    change(value)
}

fn asset_state(connection: &Connection, asset: &str) -> (String, i64) {
    connection
        .query_row(
            "SELECT lifecycle_state,
                    (SELECT COUNT(*) FROM notebook_item_assets WHERE asset_id = ?1)
             FROM assets WHERE id = ?1",
            [asset],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

#[test]
fn a_picture_keeps_its_image_until_the_picture_is_deleted() {
    let database = library();
    let paths = test_paths();
    database
        .with_connection(|connection| {
            let notebook = new_notebook(connection, "Cells");
            let page = page_ids(&notebook)[0].clone();
            let (draft, asset) =
                crate::asset_lifecycle::stage_test_image(connection, &paths, "import")?;
            let item = id('1');

            // A new picture must name the draft its image was added to.
            let refused = apply_changes(
                connection,
                &paths,
                ApplyNotebookItemChangesInput {
                    notebook_id: notebook.notebook.id.clone(),
                    changes: vec![picture(&item, &page, &asset.id, None)],
                },
                NOW + 10,
            );
            assert_eq!(refused.unwrap_err().code, "asset_draft_mismatch");
            assert_eq!(asset_state(connection, &asset.id), ("staged".into(), 0));

            apply_changes(
                connection,
                &paths,
                ApplyNotebookItemChangesInput {
                    notebook_id: notebook.notebook.id.clone(),
                    changes: vec![picture(&item, &page, &asset.id, Some(&draft.id))],
                },
                NOW + 10,
            )?;
            assert_eq!(asset_state(connection, &asset.id), ("ready".into(), 1));

            // Moving it later needs no draft, and keeps one record of its image.
            let mut moved = picture(&item, &page, &asset.id, None);
            if let NotebookItemChange::Put { shape, .. } = &mut moved {
                if let AnnotationShape::Image(image) = &mut **shape {
                    image.x = 60.0;
                }
            }
            apply_changes(
                connection,
                &paths,
                ApplyNotebookItemChangesInput {
                    notebook_id: notebook.notebook.id.clone(),
                    changes: vec![moved],
                },
                NOW + 20,
            )?;
            assert_eq!(asset_state(connection, &asset.id), ("ready".into(), 1));

            let reopened = contents(connection, &notebook.notebook.id)?;
            assert!(matches!(
                reopened.items[0].shape,
                AnnotationShape::Image(ref image) if image.asset_id == asset.id && image.x == 60.0
            ));

            apply_changes(
                connection,
                &paths,
                ApplyNotebookItemChangesInput {
                    notebook_id: notebook.notebook.id.clone(),
                    changes: vec![NotebookItemChange::Delete { id: item.clone() }],
                },
                NOW + 30,
            )?;
            assert_eq!(asset_state(connection, &asset.id).1, 0);
            Ok(())
        })
        .unwrap();
    let _ = std::fs::remove_dir_all(paths.app_data_dir());
}

#[test]
fn pictures_go_only_on_notebook_pages_and_only_pictures_name_a_draft() {
    let mut text = serde_json::from_value::<AnnotationShape>(json!({
        "kind": "image", "x": 1, "y": 1, "width": 10, "height": 10, "assetId": "not an id"
    }))
    .unwrap();
    assert!(text.validate().is_err());
    text = serde_json::from_value(json!({
        "kind": "image", "x": 1, "y": 1, "width": 10, "height": 10, "assetId": id('a')
    }))
    .unwrap();
    text.validate().unwrap();
    assert_eq!(text.image_asset(), Some(id('a').as_str()));

    let database = library();
    database
        .with_connection(|connection| {
            let notebook = new_notebook(connection, "Cells");
            let page = page_ids(&notebook)[0].clone();
            let mut with_draft = arrow(&id('2'), &page, 90.0);
            if let NotebookItemChange::Put { asset_draft_id, .. } = &mut with_draft {
                *asset_draft_id = Some(id('d'));
            }
            assert!(apply(connection, &notebook.notebook.id, vec![with_draft]).is_err());
            Ok(())
        })
        .unwrap();
}

#[test]
fn a_notebook_is_saved_as_a_pdf_with_its_pictures() {
    let database = library();
    let paths = test_paths();
    let destination = paths.app_data_dir().join("notebook-test.pdf");
    database
        .with_connection(|connection| {
            let notebook = new_notebook(connection, "Cardiac cycle");
            let page = page_ids(&notebook)[0].clone();
            let (draft, asset) =
                crate::asset_lifecycle::stage_test_image(connection, &paths, "import")?;
            apply_changes(
                connection,
                &paths,
                ApplyNotebookItemChangesInput {
                    notebook_id: notebook.notebook.id.clone(),
                    changes: vec![picture(&id('1'), &page, &asset.id, Some(&draft.id))],
                },
                NOW + 10,
            )?;
            add_page(
                connection,
                AddNotebookPageInput {
                    notebook_id: notebook.notebook.id.clone(),
                    position: 2,
                    paper: Some(Paper::Dots),
                    page_size: Some(PageSize::Wide),
                    layout: PageLayout::Page,
                },
                NOW + 20,
            )?;
            let opened = contents(connection, &notebook.notebook.id)?;

            let files: Vec<(String, std::path::PathBuf)> =
                save_pdf::picture_files(connection, &opened)?
                    .into_iter()
                    .map(|(asset_id, relative)| (asset_id, paths.resolve_relative(&relative)))
                    .collect();
            assert_eq!(files.len(), 1);
            // The test image is only a PNG's signature; give it a real one.
            let mut png = Vec::new();
            image::RgbaImage::from_pixel(4, 2, image::Rgba([200, 30, 30, 255]))
                .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                .unwrap();
            std::fs::write(&files[0].1, png).map_err(CommandError::io)?;
            assert_eq!(
                save_pdf::write_notebook(
                    &opened,
                    &files,
                    &destination,
                    save_pdf::BoardPrint::OnePage
                )?,
                (2, 0)
            );
            let bytes = std::fs::read(&destination).map_err(CommandError::io)?;
            let reader =
                oxidize_pdf::parser::PdfReader::new(std::io::Cursor::new(bytes.as_slice()))
                    .unwrap();
            assert_eq!(
                oxidize_pdf::parser::PdfDocument::new(reader)
                    .page_count()
                    .unwrap(),
                2
            );

            // A picture whose image cannot be read is left out, and counted.
            std::fs::remove_file(&files[0].1).map_err(CommandError::io)?;
            assert_eq!(
                save_pdf::write_notebook(
                    &opened,
                    &files,
                    &destination,
                    save_pdf::BoardPrint::OnePage
                )?,
                (2, 1)
            );
            Ok(())
        })
        .unwrap();
    let _ = std::fs::remove_dir_all(paths.app_data_dir());
}

fn clip_input(x: f64, y: f64, width: f64, height: f64) -> StageDocumentClipInput {
    StageDocumentClipInput {
        draft_id: id('d'),
        document_id: DOCUMENT.into(),
        page_number: 2,
        page_width: 612.0,
        page_height: 792.0,
        x,
        y,
        width,
        height,
    }
}

#[test]
fn a_clip_is_drawn_sharply_within_one_tile_of_its_page() {
    // A small clip draws at three pixels per point.
    let (page, tile) = clips::clip_tile(&clip_input(100.0, 200.0, 150.0, 60.0)).unwrap();
    assert_eq!(page, 2);
    assert_eq!(
        (
            tile.page_width,
            tile.page_height,
            tile.left,
            tile.top,
            tile.width,
            tile.height
        ),
        (1836, 2376, 300, 600, 450, 180)
    );
    // A whole page fits one tile on its longer side.
    let (_, whole) = clips::clip_tile(&clip_input(0.0, 0.0, 612.0, 792.0)).unwrap();
    assert_eq!((whole.width, whole.height), (791, 1024));
    assert!(whole.is_valid());

    for refused in [
        clip_input(100.0, 200.0, 2.0, 60.0),
        clip_input(-20.0, 200.0, 100.0, 60.0),
        clip_input(600.0, 200.0, 100.0, 60.0),
        clip_input(f64::NAN, 200.0, 100.0, 60.0),
        StageDocumentClipInput {
            page_number: 0,
            ..clip_input(10.0, 10.0, 50.0, 50.0)
        },
        StageDocumentClipInput {
            page_width: 20_000.0,
            ..clip_input(10.0, 10.0, 50.0, 50.0)
        },
    ] {
        assert_eq!(
            clips::clip_tile(&refused).unwrap_err().code,
            "invalid_document_clip"
        );
    }
}

#[test]
fn a_document_page_lists_the_notebooks_its_clips_are_in() {
    let clipped = |source: Value| {
        serde_json::from_value::<AnnotationShape>(json!({
            "kind": "image", "x": 1, "y": 1, "width": 10, "height": 10, "assetId": id('a'),
            "source": source
        }))
    };
    clipped(json!({ "documentId": DOCUMENT, "pageNumber": 2 }))
        .unwrap()
        .validate()
        .unwrap();
    for refused in [
        json!({ "documentId": "not an id", "pageNumber": 2 }),
        json!({ "documentId": DOCUMENT, "pageNumber": 0 }),
    ] {
        assert!(clipped(refused).unwrap().validate().is_err());
    }
    assert!(clipped(json!({ "documentId": DOCUMENT, "pageNumber": 2, "rect": [] })).is_err());

    let database = library();
    let paths = test_paths();
    database
        .with_connection(|connection| {
            let cells = new_notebook(connection, "Cells");
            let atlas = new_notebook(connection, "atlas");
            let mut place = |notebook: &NotebookContentsDto, item: char, page_number: i64| {
                let (draft, asset) =
                    crate::asset_lifecycle::stage_test_image(connection, &paths, "import")?;
                apply_changes(
                    connection,
                    &paths,
                    ApplyNotebookItemChangesInput {
                        notebook_id: notebook.notebook.id.clone(),
                        changes: vec![change(json!({
                            "op": "put", "id": id(item), "pageId": page_ids(notebook)[0],
                            "assetDraftId": draft.id,
                            "shape": {
                                "kind": "image", "x": 40, "y": 40, "width": 120, "height": 80,
                                "assetId": asset.id,
                                "source": { "documentId": DOCUMENT, "pageNumber": page_number }
                            }
                        }))],
                    },
                    NOW + 10,
                )
            };
            place(&cells, '1', 2)?;
            place(&cells, '2', 2)?;
            place(&atlas, '3', 2)?;
            place(&atlas, '4', 1)?;

            let listed = clips::clips_for_document(connection, DOCUMENT)?
                .into_iter()
                .map(|clip| (clip.page_number, clip.notebook_title))
                .collect::<Vec<_>>();
            assert_eq!(
                listed,
                [
                    (1, "atlas".to_string()),
                    (2, "atlas".into()),
                    (2, "Cells".into())
                ]
            );
            assert!(clips::clips_for_document(connection, &id('e'))?.is_empty());
            // An open notebook names the documents its clips came from.
            let title: String = connection
                .query_row(
                    "SELECT title FROM documents WHERE id = ?1",
                    [DOCUMENT],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            let opened = contents(connection, &cells.notebook.id)?;
            assert_eq!(
                opened.clip_documents.into_iter().collect::<Vec<_>>(),
                [(DOCUMENT.to_string(), title)]
            );
            let empty = new_notebook(connection, "Empty");
            assert!(contents(connection, &empty.notebook.id)?
                .clip_documents
                .is_empty());

            delete(connection, &atlas.notebook.id)?;
            assert_eq!(clips::clips_for_document(connection, DOCUMENT)?.len(), 1);
            Ok(())
        })
        .unwrap();
    let _ = std::fs::remove_dir_all(paths.app_data_dir());
}

#[test]
fn a_drawn_clip_is_staged_in_the_notebooks_image_draft() {
    let database = library();
    let paths = test_paths();
    let draft = database
        .with_connection(|connection| {
            Ok(crate::asset_lifecycle::stage_test_image(connection, &paths, "import")?.0)
        })
        .unwrap();
    let png = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0];
    let asset = crate::asset_lifecycle::stage_generated_png(
        &database,
        &paths,
        &draft.id,
        &png,
        "Page 2 clip.png",
    )
    .unwrap();
    assert_eq!(
        (asset.mime_type.as_str(), asset.original_filename.as_str()),
        ("image/png", "Page 2 clip.png")
    );
    database
        .with_connection(|connection| {
            assert_eq!(asset_state(connection, &asset.id), ("staged".into(), 0));
            Ok(())
        })
        .unwrap();
    // A draft that is gone takes no clip.
    assert!(crate::asset_lifecycle::stage_generated_png(
        &database,
        &paths,
        &id('f'),
        &png,
        "x.png"
    )
    .is_err());
    let _ = std::fs::remove_dir_all(paths.app_data_dir());
}

fn ink(item: &str, page: &str, points: &[(f64, f64)]) -> NotebookItemChange {
    let flat: Vec<f64> = points.iter().flat_map(|(x, y)| [*x, *y, 0.5]).collect();
    change(json!({
        "op": "put", "id": item, "pageId": page,
        "shape": {
            "kind": "ink", "points": flat, "color": "#1f2328",
            "strokeWidth": 2, "simulatePressure": true
        }
    }))
}

fn new_board(connection: &mut Connection) -> NotebookContentsDto {
    create(
        connection,
        CreateNotebookInput {
            title: "Cardiac cycle map".into(),
            subject_id: None,
            paper: Paper::Dots,
            page_size: PageSize::Letter,
            layout: PageLayout::Board,
        },
        NOW,
    )
    .unwrap()
}

#[test]
fn a_board_is_a_page_of_the_largest_size_that_goes_with_its_page() {
    let database = library();
    database
        .with_connection(|connection| {
            let notebook = new_board(connection);
            let board = &notebook.pages[0];
            assert_eq!(board.layout, PageLayout::Board);
            assert_eq!((board.width, board.height), (BOARD_SIZE, BOARD_SIZE));
            // Later pages are pages of the notebook's size unless asked otherwise.
            let added = add_page(
                connection,
                AddNotebookPageInput {
                    notebook_id: notebook.notebook.id.clone(),
                    position: 2,
                    paper: None,
                    page_size: None,
                    layout: PageLayout::Page,
                },
                NOW + 1,
            )?;
            assert_eq!(added.pages[1].layout, PageLayout::Page);
            assert_eq!(
                (added.pages[1].width, added.pages[1].height),
                (612.0, 792.0)
            );
            let added = add_page(
                connection,
                AddNotebookPageInput {
                    notebook_id: notebook.notebook.id.clone(),
                    position: 1,
                    paper: Some(Paper::Grid),
                    page_size: None,
                    layout: PageLayout::Board,
                },
                NOW + 2,
            )?;
            let layouts: Vec<PageLayout> = added.pages.iter().map(|page| page.layout).collect();
            assert_eq!(
                layouts,
                [PageLayout::Board, PageLayout::Board, PageLayout::Page]
            );
            assert_eq!(added.pages[0].paper, Paper::Grid);

            // A board far from its corner keeps its marks like any page.
            let first = added.pages[0].id.clone();
            apply(
                connection,
                &notebook.notebook.id,
                vec![ink(&id('1'), &first, &[(7000.0, 7000.0), (7400.0, 7300.0)])],
            )?;
            delete_page(
                connection,
                NotebookPageInput {
                    notebook_id: notebook.notebook.id.clone(),
                    page_id: first,
                },
                NOW + 3,
            )?;
            let boards: i64 = connection
                .query_row("SELECT COUNT(*) FROM notebook_boards", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(boards, 1);
            let listed = list(connection)?;
            // The list counts pages of paper and boards apart.
            assert_eq!((listed[0].page_count, listed[0].board_count), (1, 1));
            assert_eq!(
                listed[0].first_page.as_ref().unwrap().layout,
                PageLayout::Board
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn a_board_is_saved_cut_to_its_marks_whole_or_in_printable_pieces() {
    let database = library();
    let paths = test_paths();
    let destination = paths.app_data_dir().join("board-test.pdf");
    database
        .with_connection(|connection| {
            let notebook = new_board(connection);
            let board = notebook.pages[0].id.clone();
            // An empty board saves as one blank page.
            let opened = contents(connection, &notebook.notebook.id)?;
            assert_eq!(
                save_pdf::write_notebook(
                    &opened,
                    &[],
                    &destination,
                    save_pdf::BoardPrint::OnePage
                )?,
                (1, 0)
            );
            // Marks reaching about 1000 by 700 points: one page of that size
            // and a margin, or two Letter pages side by side.
            apply(
                connection,
                &notebook.notebook.id,
                vec![
                    ink(&id('1'), &board, &[(6000.0, 7000.0), (6500.0, 7300.0)]),
                    ink(&id('2'), &board, &[(6900.0, 6700.0), (7000.0, 7400.0)]),
                ],
            )?;
            let opened = contents(connection, &notebook.notebook.id)?;
            assert_eq!(
                save_pdf::write_notebook(
                    &opened,
                    &[],
                    &destination,
                    save_pdf::BoardPrint::OnePage
                )?,
                (1, 0)
            );
            let bytes = std::fs::read(&destination).map_err(CommandError::io)?;
            let reader =
                oxidize_pdf::parser::PdfReader::new(std::io::Cursor::new(bytes.as_slice()))
                    .unwrap();
            let document = oxidize_pdf::parser::PdfDocument::new(reader);
            let page = document.get_page(0).unwrap();
            assert!((page.width() - 1074.0).abs() < 0.5, "{}", page.width());
            assert!((page.height() - 774.0).abs() < 0.5, "{}", page.height());
            assert_eq!(
                save_pdf::write_notebook(
                    &opened,
                    &[],
                    &destination,
                    save_pdf::BoardPrint::Split(PageSize::Letter)
                )?,
                (2, 0)
            );
            Ok(())
        })
        .unwrap();
    let _ = std::fs::remove_dir_all(paths.app_data_dir());
}
