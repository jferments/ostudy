use std::{fs, path::PathBuf};

use rusqlite::params;
use serde_json::json;

use super::*;
use crate::collections::test_support::seed_document;

const DOCUMENT: &str = "dddddddddddddddddddddddddddddddd";
const OTHER_DOCUMENT: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const NOW: i64 = 1_700_000_000_000;

struct Fixture {
    database: Database,
    paths: AppPaths,
    root: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A library with one PDF whose pages render at these pixel sizes; `None`
/// records a render whose file is missing.
fn fixture(name: &str, pages: &[Option<(u32, u32)>]) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "ostudy-annotations-{name}-{}-{}",
        std::process::id(),
        unix_time_millis().unwrap()
    ));
    let paths = AppPaths::new(root.clone()).unwrap();
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            seed_document(connection, DOCUMENT, pages.len() as i64, 'a')?;
            seed_document(connection, OTHER_DOCUMENT, pages.len() as i64, 'b')?;
            for (index, size) in pages.iter().enumerate() {
                let page = index as i64 + 1;
                let relative = AppPaths::document_page_relative_path(DOCUMENT, page);
                let bytes = match size {
                    Some((width, height)) => {
                        let mut png = Vec::new();
                        image::DynamicImage::new_rgb8(*width, *height)
                            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                            .unwrap();
                        let path = paths.resolve_relative(&relative);
                        fs::create_dir_all(path.parent().unwrap()).unwrap();
                        fs::write(path, &png).unwrap();
                        png
                    }
                    None => vec![1],
                };
                connection
                    .execute(
                        "INSERT INTO document_source_assets(
                             document_id, asset_id, source_unit_number, block_index, role,
                             media_type, relative_path, byte_size, sha256, width, height,
                             alt_text, created_at, updated_at
                         ) VALUES (?1, ?2, ?3, NULL, 'unit_render', 'image/png', ?4, ?5, ?6,
                                   NULL, NULL, NULL, 1, 1)",
                        params![
                            DOCUMENT,
                            crate::document_canonical::unit_render_asset_id(page),
                            page,
                            relative,
                            bytes.len() as i64,
                            crate::document_canonical::sha256_hex(&bytes),
                        ],
                    )
                    .map_err(CommandError::database)?;
            }
            Ok(())
        })
        .unwrap();
    Fixture {
        database,
        paths,
        root,
    }
}

fn id(byte: char) -> String {
    byte.to_string().repeat(32)
}

fn arrow(x2: f64) -> AnnotationShape {
    AnnotationShape::Arrow(Segment {
        x1: 10.0,
        y1: 20.0,
        x2,
        y2: 40.0,
        color: "#d92d20".into(),
        stroke_width: 2.0,
    })
}

fn put(id: &str, page_number: i64, shape: AnnotationShape) -> AnnotationChange {
    AnnotationChange::Put {
        id: id.into(),
        page_number,
        shape,
    }
}

fn delete(id: &str) -> AnnotationChange {
    AnnotationChange::Delete { id: id.into() }
}

fn apply(
    fixture: &Fixture,
    document_id: &str,
    changes: Vec<AnnotationChange>,
    now: i64,
) -> Result<AppliedDocumentAnnotationChangesDto, CommandError> {
    fixture.database.with_connection(|connection| {
        apply_changes(
            connection,
            ApplyDocumentAnnotationChangesInput {
                document_id: document_id.into(),
                changes,
            },
            now,
        )
    })
}

fn stored(fixture: &Fixture) -> Vec<DocumentAnnotationDto> {
    load_document_annotations(&fixture.database, &fixture.paths, DOCUMENT.into())
        .unwrap()
        .annotations
}

fn every_kind() -> Vec<AnnotationShape> {
    vec![
        AnnotationShape::Ink(InkStroke {
            points: vec![1.0, 2.0, 0.5, 3.25, 4.5, 0.75],
            color: "#1d4ed8".into(),
            stroke_width: 1.5,
            simulate_pressure: false,
        }),
        AnnotationShape::Highlight(HighlightStroke {
            points: vec![10.0, 12.0, 80.0, 12.5],
            color: "#facc15".into(),
            stroke_width: 12.0,
            rects: None,
            text: None,
        }),
        text_highlight(
            vec![72.0, 100.0, 200.5, 11.0, 72.0, 113.0, 90.0, 11.0],
            "cardiac output equals",
        ),
        AnnotationShape::Line(Segment {
            x1: 0.0,
            y1: 0.0,
            x2: 72.0,
            y2: 0.0,
            color: "#111111".into(),
            stroke_width: 1.0,
        }),
        arrow(30.0),
        AnnotationShape::Rectangle(Frame {
            x: 5.0,
            y: 6.0,
            width: 40.0,
            height: 20.0,
            color: "#16a34a".into(),
            stroke_width: 2.0,
        }),
        AnnotationShape::Ellipse(Frame {
            x: 5.0,
            y: 6.0,
            width: 40.0,
            height: 20.0,
            color: "#16a34a".into(),
            stroke_width: 2.0,
        }),
        AnnotationShape::Text(TextNote {
            x: 12.0,
            y: 30.0,
            width: 160.0,
            text: "Mitral valve\n\tcloses first".into(),
            color: "#111111".into(),
            font_size: 12.0,
            sticky: None,
            link: None,
        }),
        AnnotationShape::Text(TextNote {
            x: 300.0,
            y: 40.0,
            width: 120.0,
            text: "Ask about the second heart sound.\n\nIt splits on inspiration.".into(),
            color: "#fde047".into(),
            font_size: 10.0,
            sticky: Some(true),
            link: None,
        }),
    ]
}

#[test]
fn every_kind_is_stored_without_its_tag_and_reads_back() {
    for shape in every_kind() {
        shape.validate().unwrap();
        let payload = shape.payload_json().unwrap();
        assert!(!payload.contains("\"kind\""), "{payload}");
        assert_eq!(
            AnnotationShape::from_stored(shape.kind(), &payload).unwrap(),
            shape
        );
        let wire = serde_json::to_value(&shape).unwrap();
        assert_eq!(wire["kind"], shape.kind());
        assert_eq!(
            serde_json::from_value::<AnnotationShape>(wire).unwrap(),
            shape
        );
    }
}

#[test]
fn requests_name_their_operation_and_refuse_unknown_fields() {
    let change: AnnotationChange = serde_json::from_value(json!({
        "op": "put",
        "id": id('a'),
        "pageNumber": 1,
        "shape": {
            "kind": "arrow", "x1": 1, "y1": 2, "x2": 3, "y2": 4,
            "color": "#000000", "strokeWidth": 2
        }
    }))
    .unwrap();
    assert!(matches!(
        change,
        AnnotationChange::Put { page_number: 1, .. }
    ));
    let change: AnnotationChange =
        serde_json::from_value(json!({ "op": "delete", "id": id('a') })).unwrap();
    assert!(matches!(change, AnnotationChange::Delete { .. }));

    for refused in [
        json!({ "op": "delete", "id": id('a'), "revision": 3 }),
        json!({ "op": "move", "id": id('a') }),
        json!({ "id": id('a') }),
        json!({
            "op": "put", "id": id('a'), "pageNumber": 1,
            "shape": { "kind": "polygon", "points": [] }
        }),
        json!({
            "op": "put", "id": id('a'), "pageNumber": 1,
            "shape": {
                "kind": "line", "x1": 1, "y1": 2, "x2": 3, "y2": 4,
                "color": "#000000", "strokeWidth": 2, "dash": [2, 2]
            }
        }),
        json!({
            "op": "put", "id": id('a'), "pageNumber": 1,
            "shape": { "x1": 1, "y1": 2, "x2": 3, "y2": 4, "color": "#000000", "strokeWidth": 2 }
        }),
    ] {
        assert!(
            serde_json::from_value::<AnnotationChange>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}

#[test]
fn shapes_outside_their_limits_are_refused() {
    let ink = |points: Vec<f64>| {
        AnnotationShape::Ink(InkStroke {
            points,
            color: "#000000".into(),
            stroke_width: 2.0,
            simulate_pressure: true,
        })
    };
    let text = |text: &str, font_size: f64| {
        AnnotationShape::Text(TextNote {
            x: 1.0,
            y: 1.0,
            width: 100.0,
            text: text.into(),
            color: "#000000".into(),
            font_size,
            sticky: None,
            link: None,
        })
    };
    let frame = |width: f64, height: f64| {
        AnnotationShape::Rectangle(Frame {
            x: 1.0,
            y: 1.0,
            width,
            height,
            color: "#000000".into(),
            stroke_width: 1.0,
        })
    };
    let mut line = arrow(30.0);
    if let AnnotationShape::Arrow(segment) = &mut line {
        segment.x2 = segment.x1;
        segment.y2 = segment.y1;
    }
    let mut upper = arrow(30.0);
    if let AnnotationShape::Arrow(segment) = &mut upper {
        segment.color = "#D92D20".into();
    }
    let mut thin = arrow(30.0);
    if let AnnotationShape::Arrow(segment) = &mut thin {
        segment.stroke_width = 0.1;
    }
    for refused in [
        ink(vec![]),
        ink(vec![1.0, 2.0]),
        ink(vec![1.0, 2.0, 1.5]),
        ink(vec![f64::NAN, 2.0, 0.5]),
        ink(vec![-1.0, 2.0, 0.5]),
        ink(vec![1.0, MAX_COORDINATE + 1.0, 0.5]),
        ink([1.0, 2.0, 0.5].repeat(MAX_STROKE_POINTS + 1)),
        text("   \n ", 12.0),
        text("bell\u{7}", 12.0),
        text(&"x".repeat(MAX_TEXT_CHARS + 1), 12.0),
        text("Too small", 4.0),
        frame(0.0, 10.0),
        frame(10.0, f64::INFINITY),
        frame(MAX_COORDINATE, 10.0),
        line,
        upper,
        thin,
    ] {
        assert_eq!(
            refused.validate().unwrap_err().code,
            "document_annotation_invalid",
            "{refused:?}"
        );
    }
    ink([1.0, 2.0, 0.5].repeat(MAX_STROKE_POINTS))
        .validate()
        .unwrap();
}

#[test]
fn the_workspace_lists_measured_pages_and_annotations_in_drawing_order() {
    let fixture = fixture("workspace", &[Some((120, 156)), Some((156, 120)), None]);
    apply(
        &fixture,
        DOCUMENT,
        vec![
            put(&id('b'), 2, arrow(30.0)),
            put(&id('d'), 1, arrow(30.0)),
            put(&id('a'), 1, arrow(40.0)),
        ],
        NOW,
    )
    .unwrap();
    // A clock that went backwards still puts the newest annotation on top.
    apply(
        &fixture,
        DOCUMENT,
        vec![put(&id('c'), 1, arrow(50.0))],
        NOW - 5,
    )
    .unwrap();

    let workspace =
        load_document_annotations(&fixture.database, &fixture.paths, DOCUMENT.into()).unwrap();
    assert_eq!(
        workspace.pages,
        vec![
            AnnotatedPageDto {
                page_number: 1,
                asset_id: "unit-render:000001".into(),
                width: 72.0,
                height: 93.6,
                image_available: true,
            },
            AnnotatedPageDto {
                page_number: 2,
                asset_id: "unit-render:000002".into(),
                width: 93.6,
                height: 72.0,
                image_available: true,
            },
            // The missing render keeps its page, at the previous page's size.
            AnnotatedPageDto {
                page_number: 3,
                asset_id: "unit-render:000003".into(),
                width: 93.6,
                height: 72.0,
                image_available: false,
            },
        ]
    );
    let order = workspace
        .annotations
        .iter()
        .map(|annotation| (annotation.page_number, annotation.id.clone()))
        .collect::<Vec<_>>();
    // Annotations drawn in one request keep the order they were drawn in.
    assert_eq!(
        order,
        vec![(1, id('d')), (1, id('a')), (1, id('c')), (2, id('b'))]
    );

    let missing = load_document_annotations(&fixture.database, &fixture.paths, id('f'));
    assert_eq!(missing.unwrap_err().code, "document_not_found");
}

#[test]
fn a_put_creates_replaces_or_leaves_an_identical_annotation_alone() {
    let fixture = fixture("revisions", &[Some((120, 156)), Some((120, 156))]);
    let created = apply(&fixture, DOCUMENT, vec![put(&id('a'), 1, arrow(30.0))], NOW).unwrap();
    assert_eq!(created.annotations[0].revision, 1);

    // Repeating the request after a lost response changes nothing.
    let repeated = apply(
        &fixture,
        DOCUMENT,
        vec![put(&id('a'), 1, arrow(30.0))],
        NOW + 10,
    )
    .unwrap();
    assert_eq!(repeated.annotations, created.annotations);

    let moved = apply(
        &fixture,
        DOCUMENT,
        vec![put(&id('a'), 2, arrow(60.0))],
        NOW + 20,
    )
    .unwrap();
    assert_eq!(
        moved.annotations[0],
        DocumentAnnotationDto {
            id: id('a'),
            page_number: 2,
            shape: arrow(60.0),
            revision: 2,
            created_at: NOW,
            updated_at: NOW + 20,
        }
    );
    assert_eq!(stored(&fixture), moved.annotations);

    // A clock that went backwards still leaves updated_at >= created_at.
    let earlier = apply(
        &fixture,
        DOCUMENT,
        vec![put(&id('a'), 2, arrow(70.0))],
        NOW - 1_000,
    )
    .unwrap();
    assert_eq!(earlier.annotations[0].revision, 3);
    assert_eq!(earlier.annotations[0].updated_at, NOW);

    let deleted = apply(&fixture, DOCUMENT, vec![delete(&id('a'))], NOW + 30).unwrap();
    assert_eq!(deleted.deleted_ids, vec![id('a')]);
    assert!(stored(&fixture).is_empty());
    apply(&fixture, DOCUMENT, vec![delete(&id('a'))], NOW + 40).unwrap();
}

#[test]
fn a_refused_change_leaves_the_whole_request_unapplied() {
    let fixture = fixture("atomic", &[Some((120, 156))]);
    apply(&fixture, DOCUMENT, vec![put(&id('a'), 1, arrow(30.0))], NOW).unwrap();

    let missing_page = apply(
        &fixture,
        DOCUMENT,
        vec![
            put(&id('b'), 1, arrow(30.0)),
            delete(&id('a')),
            put(&id('c'), 2, arrow(30.0)),
        ],
        NOW,
    );
    assert_eq!(
        missing_page.unwrap_err().code,
        "document_annotation_page_missing"
    );
    let ids = stored(&fixture)
        .into_iter()
        .map(|annotation| annotation.id)
        .collect::<Vec<_>>();
    assert_eq!(ids, vec![id('a')]);

    // OTHER_DOCUMENT has no page renders, so it cannot take an annotation, and
    // an annotation of this document cannot be moved or deleted through it.
    assert_eq!(
        apply(
            &fixture,
            OTHER_DOCUMENT,
            vec![put(&id('b'), 1, arrow(30.0))],
            NOW
        )
        .unwrap_err()
        .code,
        "document_annotation_page_missing"
    );
    assert_eq!(
        apply(&fixture, OTHER_DOCUMENT, vec![delete(&id('a'))], NOW)
            .unwrap_err()
            .code,
        "document_annotation_conflict"
    );
    assert_eq!(
        apply(&fixture, &id('f'), vec![delete(&id('a'))], NOW)
            .unwrap_err()
            .code,
        "document_not_found"
    );
    for refused in [
        vec![],
        vec![put(&id('b'), 1, arrow(30.0)), delete(&id('b'))],
        vec![put("ABABABABABABABABABABABABABABABAB", 1, arrow(30.0))],
        vec![put(&id('b'), 0, arrow(30.0))],
    ] {
        assert!(apply(&fixture, DOCUMENT, refused, NOW).is_err());
    }
    assert_eq!(stored(&fixture).len(), 1);
}

#[test]
fn a_document_holds_a_bounded_number_of_annotations() {
    let fixture = fixture("limit", &[Some((120, 156))]);
    let payload = arrow(30.0).payload_json().unwrap();
    fixture
        .database
        .with_connection(|connection| {
            connection
                .execute(
                    "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < ?3)
                     INSERT INTO document_annotations(
                         id, document_id, page_number, kind, payload_json,
                         revision, created_at, updated_at
                     )
                     SELECT printf('%032x', i), ?1, 1, 'arrow', ?2, 1, 1, 1 FROM n",
                    params![DOCUMENT, payload, MAX_ANNOTATIONS_PER_DOCUMENT],
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    let over = apply(&fixture, DOCUMENT, vec![put(&id('f'), 1, arrow(30.0))], NOW);
    assert_eq!(over.unwrap_err().code, "document_annotation_limit");
    // Changing or erasing existing annotations still works at the limit.
    apply(
        &fixture,
        DOCUMENT,
        vec![
            put(&format!("{:032x}", 1), 1, arrow(40.0)),
            delete(&format!("{:032x}", 2)),
        ],
        NOW,
    )
    .unwrap();
}

#[test]
fn an_unreadable_stored_annotation_is_skipped() {
    let fixture = fixture("unreadable", &[Some((120, 156))]);
    apply(
        &fixture,
        DOCUMENT,
        vec![put(&id('a'), 1, arrow(30.0)), put(&id('b'), 1, arrow(40.0))],
        NOW,
    )
    .unwrap();
    fixture
        .database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE document_annotations SET payload_json = '{\"x\":1}' WHERE id = ?1",
                    [id('a')],
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    let ids = stored(&fixture)
        .into_iter()
        .map(|annotation| annotation.id)
        .collect::<Vec<_>>();
    assert_eq!(ids, vec![id('b')]);
}

#[test]
fn deleting_the_document_removes_its_annotations() {
    let fixture = fixture("lifetime", &[Some((120, 156))]);
    apply(&fixture, DOCUMENT, vec![put(&id('a'), 1, arrow(30.0))], NOW).unwrap();
    let remaining: i64 = fixture
        .database
        .with_connection(|connection| {
            connection
                .execute("DELETE FROM documents WHERE id = ?1", [DOCUMENT])
                .map_err(CommandError::database)?;
            connection
                .query_row("SELECT COUNT(*) FROM document_annotations", [], |row| {
                    row.get(0)
                })
                .map_err(CommandError::database)
        })
        .unwrap();
    assert_eq!(remaining, 0);
}

fn tile_request(page_number: i64, left: u32, width: u32) -> RenderDocumentPageTileInput {
    RenderDocumentPageTileInput {
        document_id: DOCUMENT.to_string(),
        page_number,
        page_width: 2_000,
        page_height: 2_600,
        left,
        top: 0,
        width,
        height: 512,
    }
}

#[test]
fn page_tile_requests_name_a_page_and_a_tile_on_it() {
    let (page, tile) = page_tiles::validated_request(&tile_request(2, 1_488, 512)).unwrap();
    assert_eq!(page, 2);
    assert_eq!(
        (tile.left, tile.width, tile.page_width),
        (1_488, 512, 2_000)
    );
    assert_eq!(
        page_tiles::validated_request(&tile_request(0, 0, 512))
            .unwrap_err()
            .code,
        "document_annotation_page_missing"
    );
    assert_eq!(
        page_tiles::validated_request(&tile_request(1, 1_489, 512))
            .unwrap_err()
            .code,
        "document_annotation_invalid"
    );
    let mut bad_id = tile_request(1, 0, 512);
    bad_id.document_id = "../x".to_string();
    assert!(page_tiles::validated_request(&bad_id).is_err());
}

#[test]
fn page_tiles_draw_only_from_a_stored_pdf() {
    let fixture = fixture("tiles", &[Some((1_020, 1_320))]);
    fixture
        .database
        .with_connection(|connection| {
            assert!(page_tiles::pdf_source_path(connection, DOCUMENT)?.ends_with(".pdf"));
            connection
                .execute(
                    "UPDATE documents SET source_format = 'docx' WHERE id = ?1",
                    [OTHER_DOCUMENT],
                )
                .map_err(CommandError::database)?;
            assert_eq!(
                page_tiles::pdf_source_path(connection, OTHER_DOCUMENT)
                    .unwrap_err()
                    .code,
                "document_annotation_invalid"
            );
            assert_eq!(
                page_tiles::pdf_source_path(connection, "ffffffffffffffffffffffffffffffff")
                    .unwrap_err()
                    .code,
                "document_not_found"
            );
            Ok(())
        })
        .unwrap();
}

fn text_highlight(rects: Vec<f64>, text: &str) -> AnnotationShape {
    AnnotationShape::Highlight(HighlightStroke {
        points: Vec::new(),
        color: "#facc15".into(),
        stroke_width: 12.0,
        rects: Some(rects),
        text: Some(text.into()),
    })
}

#[test]
fn text_highlights_carry_boxes_and_the_words_they_cover() {
    assert!(text_highlight(vec![1.0, 2.0, 30.0, 10.0], "heart rate")
        .validate()
        .is_ok());
    for bad in [
        text_highlight(Vec::new(), "heart rate"),
        text_highlight(vec![1.0, 2.0, 30.0], "heart rate"),
        text_highlight(vec![1.0, 2.0, 0.0, 10.0], "heart rate"),
        text_highlight(vec![1.0, 2.0, 30.0, 10.0], "  "),
        text_highlight(vec![1.0, 2.0, 30.0, 10.0], "heart\nrate"),
    ] {
        assert_eq!(
            bad.validate().unwrap_err().code,
            "document_annotation_invalid"
        );
    }
    // A highlight is a free stroke or a text highlight, never half of each.
    let AnnotationShape::Highlight(mut both) = text_highlight(vec![1.0, 2.0, 30.0, 10.0], "rate")
    else {
        unreachable!()
    };
    both.points = vec![1.0, 2.0, 3.0, 4.0];
    assert!(AnnotationShape::Highlight(both.clone()).validate().is_err());
    both.points.clear();
    both.text = None;
    assert!(AnnotationShape::Highlight(both).validate().is_err());
    // A free stroke stores no text fields, so its payload is unchanged.
    let free = &every_kind()[1];
    assert!(!free.payload_json().unwrap().contains("rects"));
}

#[test]
fn page_text_pairs_each_character_with_its_box() {
    use crate::pdf_render_backend::{PageCharacter, PageCharacters};
    let dto = page_text::to_dto(
        3,
        PageCharacters {
            width: 612.0,
            height: 792.0,
            characters: vec![
                PageCharacter {
                    text: 'é',
                    bounds: Some([72.004, 100.0, 5.5, 11.0]),
                },
                PageCharacter {
                    text: ' ',
                    bounds: None,
                },
                PageCharacter {
                    text: '𝑥',
                    bounds: Some([90.0, 100.0, 6.0, 11.0]),
                },
            ],
        },
    );
    assert_eq!(dto.characters.chars().count(), 3);
    assert_eq!(
        dto.boxes,
        vec![72.0, 100.0, 5.5, 11.0, 0.0, 0.0, 0.0, 0.0, 90.0, 100.0, 6.0, 11.0]
    );
    assert_eq!((dto.page_number, dto.width), (3, 612.0));
}

#[test]
fn sticky_notes_hold_longer_text_and_are_marked_only_one_way() {
    let note = |text: String, sticky: Option<bool>| {
        AnnotationShape::Text(TextNote {
            x: 1.0,
            y: 1.0,
            width: 120.0,
            text,
            color: "#fde047".into(),
            font_size: 10.0,
            sticky,
            link: None,
        })
    };
    let long = "a".repeat(MAX_TEXT_CHARS + 1);
    assert!(note(long.clone(), None).validate().is_err());
    note(long, Some(true)).validate().unwrap();
    assert!(note("a".repeat(MAX_STICKY_CHARS + 1), Some(true))
        .validate()
        .is_err());
    assert!(note("Marked false".into(), Some(false)).validate().is_err());
    // A plain note stores no marker, so builds before sticky notes still read it.
    let plain = note("Plain".into(), None).payload_json().unwrap();
    assert!(!plain.contains("sticky"), "{plain}");
}
