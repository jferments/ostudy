use super::detect::{self, FigureKind, PassageKind};
use super::*;
use crate::pdf_render_backend::{PageGraphic, PageGraphicKind, PageLayout, PageTextRun};

const DOCUMENT: &str = "dddddddddddddddddddddddddddddddd";

fn graphic(kind: PageGraphicKind, bounds: [f64; 4]) -> PageGraphic {
    PageGraphic {
        kind,
        bounds,
        boxy: false,
    }
}

fn run(text: &str, left: f64, top: f64, width: f64) -> PageTextRun {
    PageTextRun {
        text: text.to_string(),
        bounds: [left, top, width, 10.0],
    }
}

/// A letter page with `graphics` and `runs`, whose text is the runs in order.
fn page(page_number: usize, graphics: Vec<PageGraphic>, runs: Vec<PageTextRun>) -> PageLayout {
    let text = runs
        .iter()
        .map(|run| run.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    PageLayout {
        page_number,
        width: 612.0,
        height: 792.0,
        graphics,
        text_runs: runs,
        text,
    }
}

/// Enough body text that a page does not count as nearly empty.
fn body(top: f64) -> Vec<PageTextRun> {
    (0..12)
        .map(|line| {
            run(
                "The heart pumps blood through two circuits, one to the lungs and one to the body.",
                72.0,
                top + f64::from(line) * 14.0,
                460.0,
            )
        })
        .collect()
}

#[test]
fn captions_are_read_from_the_lines_beneath_a_picture() {
    let mut runs = vec![
        run(
            "Figure 3.2 The four chambers of the heart, seen from the",
            72.0,
            336.0,
            420.0,
        ),
        run("front, with the valves between them.", 72.0, 348.0, 260.0),
        run(
            "Blood returns from the body to the right atrium.",
            72.0,
            380.0,
            340.0,
        ),
    ];
    runs.extend(body(420.0));
    let figures = detect::detect_layouts(&[page(
        1,
        vec![graphic(PageGraphicKind::Image, [72.0, 100.0, 300.0, 220.0])],
        runs,
    )]);
    assert_eq!(figures.len(), 1);
    let figure = &figures[0];
    assert_eq!(figure.figure_id, "p00001-01");
    assert_eq!(figure.kind, FigureKind::Image);
    let caption = figure.caption.as_ref().unwrap();
    assert_eq!(caption.label, "Figure 3.2");
    assert_eq!(
        caption.text,
        "Figure 3.2 The four chambers of the heart, seen from the front, with the valves between them."
    );
    // The caption names its own figure; that is not a passage about it.
    assert!(figure.passages.is_empty());
}

#[test]
fn passages_that_name_a_figure_are_linked_from_any_page() {
    let mut first = vec![run(
        "Fig. 4-1: Blood pressure over one heartbeat.",
        72.0,
        330.0,
        320.0,
    )];
    first.extend(body(380.0));
    let mut third = body(72.0);
    third.push(run(
        "As figure 4–1 shows, pressure in the ventricle rises steeply. Later text follows.",
        72.0,
        300.0,
        460.0,
    ));
    let figures = detect::detect_layouts(&[
        page(
            1,
            vec![graphic(PageGraphicKind::Image, [72.0, 100.0, 300.0, 220.0])],
            first,
        ),
        page(2, Vec::new(), body(72.0)),
        page(3, Vec::new(), third),
    ]);
    assert_eq!(figures.len(), 1);
    assert_eq!(figures[0].caption.as_ref().unwrap().label, "Figure 4-1");
    assert_eq!(
        figures[0].passages,
        vec![detect::Passage {
            page_number: 3,
            kind: PassageKind::Mention,
            excerpt: Some(
                "As figure 4–1 shows, pressure in the ventricle rises steeply.".to_string()
            ),
        }]
    );
}

#[test]
fn a_figure_that_fills_its_page_brings_in_the_pages_beside_it() {
    let figures = detect::detect_layouts(&[
        page(1, Vec::new(), body(72.0)),
        page(
            2,
            vec![graphic(PageGraphicKind::Image, [40.0, 80.0, 530.0, 600.0])],
            vec![run("Figure 7 The nephron.", 40.0, 690.0, 160.0)],
        ),
        page(3, Vec::new(), body(72.0)),
    ]);
    assert_eq!(figures.len(), 1);
    let neighbours = figures[0]
        .passages
        .iter()
        .filter(|passage| passage.kind == PassageKind::Neighbour)
        .map(|passage| passage.page_number)
        .collect::<Vec<_>>();
    assert_eq!(neighbours, vec![1, 3]);
}

#[test]
fn page_furniture_small_marks_and_scanned_pages_are_not_figures() {
    let logo = graphic(PageGraphicKind::Image, [450.0, 300.0, 80.0, 80.0]);
    let pages = (1..=3)
        .map(|number| {
            page(
                number,
                vec![
                    logo.clone(),
                    // A header band banner and an icon too small to study.
                    graphic(PageGraphicKind::Image, [0.0, 0.0, 612.0, 40.0]),
                    graphic(PageGraphicKind::Image, [72.0, 600.0, 20.0, 20.0]),
                ],
                body(100.0),
            )
        })
        .chain([page(
            4,
            vec![graphic(PageGraphicKind::Image, [0.0, 0.0, 612.0, 792.0])],
            Vec::new(),
        )])
        .collect::<Vec<_>>();
    assert!(detect::detect_layouts(&pages).is_empty());
}

#[test]
fn drawings_are_clusters_of_paths_but_ruled_tables_are_not() {
    // A diagram: boxes joined by arrows, all touching.
    let mut diagram = Vec::new();
    for step in 0..4 {
        let left = 72.0 + f64::from(step) * 110.0;
        diagram.push(graphic(PageGraphicKind::Path, [left, 120.0, 80.0, 60.0]));
        diagram.push(graphic(
            PageGraphicKind::Path,
            [left + 78.0, 148.0, 34.0, 4.0],
        ));
    }
    // A ruled table: thin horizontal and vertical rules only.
    let mut table = Vec::new();
    for row in 0..8 {
        table.push(graphic(
            PageGraphicKind::Path,
            [72.0, 400.0 + f64::from(row) * 20.0, 460.0, 0.5],
        ));
    }
    for column in 0..4 {
        table.push(graphic(
            PageGraphicKind::Path,
            [72.0 + f64::from(column) * 150.0, 400.0, 0.5, 140.0],
        ));
    }
    let mut graphics = diagram;
    graphics.extend(table);
    let figures = detect::detect_layouts(&[page(1, graphics, body(600.0))]);
    assert_eq!(figures.len(), 1);
    assert_eq!(figures[0].kind, FigureKind::Drawing);
    assert_eq!(figures[0].bounds.top, 120.0);
}

#[test]
fn a_picture_and_the_drawing_around_it_are_one_figure() {
    let mut graphics = vec![graphic(
        PageGraphicKind::Image,
        [100.0, 100.0, 200.0, 150.0],
    )];
    for label in 0..6 {
        let top = 100.0 + f64::from(label) * 25.0;
        graphics.push(graphic(PageGraphicKind::Path, [300.0, top, 60.0, 3.0]));
        graphics.push(graphic(
            PageGraphicKind::Path,
            [358.0, top - 6.0, 30.0, 15.0],
        ));
    }
    let figures = detect::detect_layouts(&[page(1, graphics, body(400.0))]);
    assert_eq!(figures.len(), 1);
    assert_eq!(figures[0].kind, FigureKind::Image);
    assert_eq!(figures[0].bounds.left, 100.0);
    assert!((figures[0].bounds.width - 288.0).abs() < 1e-9);
}

/// A library with one three-page PDF document.
fn library() -> Connection {
    let connection =
        crate::database::subjects_fixture::release_library(crate::database::CURRENT_SCHEMA_VERSION);
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .unwrap();
    crate::collections::test_support::seed_document(&connection, DOCUMENT, 3, 'a').unwrap();
    connection
}

fn found(left: f64) -> DetectedFigure {
    DetectedFigure {
        figure_id: "p00001-01".to_string(),
        page_number: 1,
        page_width: 612.0,
        page_height: 792.0,
        kind: FigureKind::Image,
        bounds: Rect::from_bounds([left, 100.0, 300.0, 200.0]),
        caption: Some(detect::Caption {
            bounds: Rect::from_bounds([left, 310.0, 300.0, 22.0]),
            text: "Figure 1 The heart.".to_string(),
            label: "Figure 1".to_string(),
        }),
        passages: vec![
            detect::Passage {
                page_number: 2,
                kind: PassageKind::Mention,
                excerpt: Some("See Figure 1.".to_string()),
            },
            detect::Passage {
                page_number: 3,
                kind: PassageKind::Neighbour,
                excerpt: None,
            },
        ],
    }
}

#[test]
fn a_search_is_stored_and_read_back_in_order() {
    let mut connection = library();
    let before = figures(&connection, DOCUMENT).unwrap();
    assert_eq!(before.state, "none");
    assert!(before.use_images);
    assert!(before.figures.is_empty());

    replace_figures(&mut connection, DOCUMENT, &[found(72.0)], 5).unwrap();
    let stored = figures(&connection, DOCUMENT).unwrap();
    assert_eq!(stored.state, "found");
    assert_eq!(stored.scanned_at, Some(5));
    assert_eq!(stored.figures.len(), 1);
    let figure = &stored.figures[0];
    assert_eq!(figure.label.as_deref(), Some("Figure 1"));
    assert_eq!(figure.caption.as_deref(), Some("Figure 1 The heart."));
    assert_eq!(
        figure.caption_bounds,
        Some(FigureBoxDto {
            left: 72.0,
            top: 310.0,
            width: 300.0,
            height: 22.0
        })
    );
    assert_eq!(
        figure.passages,
        vec![
            FigurePassageDto {
                page_number: 2,
                kind: "mention".to_string(),
                excerpt: Some("See Figure 1.".to_string()),
            },
            FigurePassageDto {
                page_number: 3,
                kind: "neighbour".to_string(),
                excerpt: None,
            },
        ]
    );
}

#[test]
fn a_hidden_figure_stays_hidden_when_found_again_in_the_same_place() {
    let mut connection = library();
    replace_figures(&mut connection, DOCUMENT, &[found(72.0)], 5).unwrap();
    let hidden = set_hidden(
        &mut connection,
        SetDocumentFigureHiddenInput {
            document_id: DOCUMENT.to_string(),
            figure_id: "p00001-01".to_string(),
            hidden: true,
        },
        6,
    )
    .unwrap();
    assert!(hidden.figures[0].hidden);

    replace_figures(&mut connection, DOCUMENT, &[found(74.0)], 7).unwrap();
    assert!(figures(&connection, DOCUMENT).unwrap().figures[0].hidden);
    replace_figures(&mut connection, DOCUMENT, &[found(250.0)], 8).unwrap();
    assert!(!figures(&connection, DOCUMENT).unwrap().figures[0].hidden);

    let missing = set_hidden(
        &mut connection,
        SetDocumentFigureHiddenInput {
            document_id: DOCUMENT.to_string(),
            figure_id: "p00009-01".to_string(),
            hidden: true,
        },
        9,
    )
    .unwrap_err();
    assert_eq!(missing.code, "document_figure_not_found");
}

#[test]
fn turning_images_off_forgets_the_figures_and_the_import_choice_is_kept() {
    let mut connection = library();
    record_import_choice(&connection, DOCUMENT, false, 1).unwrap();
    assert!(!use_images(&connection, DOCUMENT).unwrap());
    assert_eq!(figures(&connection, DOCUMENT).unwrap().state, "pending");

    let on = set_use_images(
        &mut connection,
        SetDocumentFiguresUseImagesInput {
            document_id: DOCUMENT.to_string(),
            use_images: true,
        },
        2,
    )
    .unwrap();
    assert!(on.use_images);
    replace_figures(&mut connection, DOCUMENT, &[found(72.0)], 3).unwrap();

    let off = set_use_images(
        &mut connection,
        SetDocumentFiguresUseImagesInput {
            document_id: DOCUMENT.to_string(),
            use_images: false,
        },
        4,
    )
    .unwrap();
    assert!(!off.use_images);
    assert_eq!(off.state, "pending");
    assert!(off.figures.is_empty());
    let passages: i64 = connection
        .query_row("SELECT COUNT(*) FROM document_figure_passages", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(passages, 0);
}

#[test]
fn deleting_a_document_deletes_its_figures() {
    let mut connection = library();
    replace_figures(&mut connection, DOCUMENT, &[found(72.0)], 5).unwrap();
    connection
        .execute("DELETE FROM documents WHERE id = ?1", [DOCUMENT])
        .unwrap();
    let left: i64 = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM document_figure_scans)
                  + (SELECT COUNT(*) FROM document_figures)
                  + (SELECT COUNT(*) FROM document_figure_passages)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(left, 0);
}

/// A two-page PDF: a labelled diagram with a caption on page 1, and a page 2
/// that refers to it.
fn figure_pdf() -> std::path::PathBuf {
    use oxidize_pdf::{graphics::Color, Document, Font, Page};

    let mut document = Document::new();
    let mut first = Page::letter();
    first
        .text()
        .set_font(Font::Helvetica, 12.0)
        .at(72.0, 720.0)
        .write("Chapter 1. The heart")
        .unwrap();
    for step in 0..4 {
        let left = 72.0 + f64::from(step) * 115.0;
        first
            .graphics()
            .set_fill_color(Color::rgb(0.8, 0.85, 0.95))
            .rectangle(left, 520.0, 85.0, 70.0)
            .fill();
        first
            .graphics()
            .set_stroke_color(Color::rgb(0.2, 0.2, 0.4))
            .set_line_width(1.0)
            .rectangle(left, 520.0, 85.0, 70.0)
            .stroke();
        first
            .graphics()
            .set_fill_color(Color::rgb(0.2, 0.2, 0.4))
            .rectangle(left + 85.0, 553.0, 30.0, 4.0)
            .fill();
    }
    first
        .text()
        .set_font(Font::Helvetica, 10.0)
        .at(72.0, 500.0)
        .write("Figure 1.1 Blood flows through the four chambers in turn.")
        .unwrap();
    for line in 0..14 {
        first
            .text()
            .set_font(Font::Helvetica, 11.0)
            .at(72.0, 460.0 - f64::from(line) * 15.0)
            .write("The heart pumps blood through the lungs and then the rest of the body.")
            .unwrap();
    }
    document.add_page(first);
    let mut second = Page::letter();
    second
        .text()
        .set_font(Font::Helvetica, 11.0)
        .at(72.0, 700.0)
        .write("As Figure 1.1 shows, the right atrium fills first.")
        .unwrap();
    document.add_page(second);

    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "ostudy-figures-{}-{}.pdf",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    document.save(&path).unwrap();
    path
}

#[test]
fn pdfium_finds_a_drawn_figure_its_caption_and_the_page_that_refers_to_it() {
    if !crate::pdf_render_backend::PAGE_LAYOUTS_SUPPORTED {
        return;
    }
    let path = figure_pdf();
    let runtime = crate::pdf_render_backend::shared_test_runtime();
    let figures = search(runtime, &path).unwrap_or_else(|_| panic!("the search ran"));
    let _ = std::fs::remove_file(&path);

    assert_eq!(figures.len(), 1, "{figures:#?}");
    let figure = &figures[0];
    assert_eq!(figure.page_number, 1);
    assert_eq!(figure.kind, FigureKind::Drawing);
    // Top-left points: the boxes span 520..590 from the bottom of a 792pt page.
    assert!(
        (figure.bounds.top - 202.0).abs() < 2.0,
        "{:?}",
        figure.bounds
    );
    assert!(
        (figure.bounds.left - 72.0).abs() < 2.0,
        "{:?}",
        figure.bounds
    );
    let caption = figure.caption.as_ref().expect("the caption is found");
    assert_eq!(caption.label, "Figure 1.1");
    assert!(caption.text.starts_with("Figure 1.1 Blood flows"));
    assert_eq!(figure.passages.len(), 1);
    assert_eq!(figure.passages[0].page_number, 2);
    assert_eq!(
        figure.passages[0].excerpt.as_deref(),
        Some("As Figure 1.1 shows, the right atrium fills first.")
    );
}

fn boxy(bounds: [f64; 4]) -> PageGraphic {
    PageGraphic {
        kind: PageGraphicKind::Path,
        bounds,
        boxy: true,
    }
}

/// A row of boxed, labelled cells: a text panel or a boxed table.
fn text_panel(top: f64) -> (Vec<PageGraphic>, Vec<PageTextRun>) {
    let mut graphics = Vec::new();
    let mut runs = Vec::new();
    for cell in 0..8 {
        let left = 72.0 + f64::from(cell % 4) * 110.0;
        let cell_top = top + f64::from(cell / 4) * 40.0;
        graphics.push(boxy([left, cell_top, 110.0, 40.0]));
        runs.push(run("Proximal tubule", left + 8.0, cell_top + 14.0, 90.0));
    }
    (graphics, runs)
}

#[test]
fn a_boxed_text_panel_is_a_figure_only_when_captioned() {
    let (graphics, mut runs) = text_panel(120.0);
    runs.extend(body(400.0));
    assert!(detect::detect_layouts(&[page(1, graphics.clone(), runs.clone())]).is_empty());

    runs.push(run(
        "Figure 6.2 The segments of the nephron.",
        72.0,
        206.0,
        300.0,
    ));
    let figures = detect::detect_layouts(&[page(1, graphics, runs)]);
    assert_eq!(figures.len(), 1);
    assert_eq!(figures[0].caption.as_ref().unwrap().label, "Figure 6.2");
}

#[test]
fn a_bulleted_caption_is_read_and_a_reference_is_not_a_caption() {
    let picture = graphic(PageGraphicKind::Image, [72.0, 100.0, 300.0, 220.0]);
    let mut runs = vec![run("• Figure 1.3 The lungs.", 72.0, 330.0, 200.0)];
    runs.extend(body(400.0));
    let figures = detect::detect_layouts(&[page(1, vec![picture.clone()], runs)]);
    assert_eq!(figures[0].caption.as_ref().unwrap().label, "Figure 1.3");
    assert_eq!(
        figures[0].caption.as_ref().unwrap().text,
        "Figure 1.3 The lungs."
    );

    let mut runs = vec![run(
        "Figure 2), drawn earlier, shows the airways.",
        72.0,
        330.0,
        300.0,
    )];
    runs.extend(body(400.0));
    let figures = detect::detect_layouts(&[page(1, vec![picture], runs)]);
    assert_eq!(figures.len(), 1);
    assert!(figures[0].caption.is_none());
}

#[test]
fn panels_that_share_one_caption_are_one_figure() {
    let mut runs = vec![run(
        "Figure 5 A, filtration. B, reabsorption.",
        72.0,
        262.0,
        460.0,
    )];
    runs.extend(body(400.0));
    let figures = detect::detect_layouts(&[page(
        1,
        vec![
            graphic(PageGraphicKind::Image, [72.0, 100.0, 200.0, 150.0]),
            graphic(PageGraphicKind::Image, [330.0, 100.0, 200.0, 150.0]),
        ],
        runs,
    )]);
    assert_eq!(figures.len(), 1);
    assert_eq!(figures[0].bounds.left, 72.0);
    assert!((figures[0].bounds.width - 458.0).abs() < 1e-9);
}

#[test]
fn small_pieces_above_a_caption_are_gathered_into_one_figure() {
    let mut graphics = Vec::new();
    for piece in 0..4 {
        let left = 72.0 + f64::from(piece % 2) * 50.0;
        let top = 200.0 + f64::from(piece / 2) * 50.0;
        graphics.push(graphic(PageGraphicKind::Image, [left, top, 40.0, 40.0]));
    }
    let mut runs = vec![run("Figure 8.4 Four kinds of villi.", 72.0, 300.0, 200.0)];
    runs.extend(body(400.0));
    let figures = detect::detect_layouts(&[page(1, graphics.clone(), runs)]);
    assert_eq!(figures.len(), 1);
    assert_eq!(figures[0].caption.as_ref().unwrap().label, "Figure 8.4");
    assert_eq!(figures[0].bounds.top, 200.0);
    assert!((figures[0].bounds.width - 90.0).abs() < 1e-9);

    // Without a caption they stay too small to be figures.
    assert!(detect::detect_layouts(&[page(1, graphics, body(400.0))]).is_empty());
}

fn reading_of(figure_id: &str) -> crate::ai::document_intelligence::FigureReading {
    crate::ai::document_intelligence::FigureReading {
        figure_id: figure_id.to_string(),
        title: "The heart".to_string(),
        shows: "The four chambers and the valves between them.".to_string(),
        use_in_text: Some("Page 2 walks through one heartbeat with it.".to_string()),
        concepts: vec!["Cardiac cycle".to_string(), "Valves".to_string()],
        explained_on: vec![2],
        importance: 5,
        worth_studying: true,
    }
}

#[test]
fn what_the_model_read_is_stored_and_shown_with_its_figure() {
    let mut connection = library();
    replace_figures(&mut connection, DOCUMENT, &[found(72.0)], 5).unwrap();
    assert!(reading::waiting_to_be_read(&connection, DOCUMENT).unwrap());
    assert_eq!(figures(&connection, DOCUMENT).unwrap().reading, "none");

    reading::store_readings(
        &mut connection,
        DOCUMENT,
        // A figure a newer search no longer has is not stored.
        &[reading_of("p00001-01"), reading_of("p00009-01")],
        "openrouter",
        "vision-model",
        "read",
        6,
    )
    .unwrap();
    let stored = figures(&connection, DOCUMENT).unwrap();
    assert_eq!(stored.reading, "read");
    assert_eq!(stored.read_at, Some(6));
    assert_eq!(
        stored.figures[0].reading,
        Some(FigureReadingDto {
            title: "The heart".to_string(),
            shows: "The four chambers and the valves between them.".to_string(),
            use_in_text: Some("Page 2 walks through one heartbeat with it.".to_string()),
            concepts: vec!["Cardiac cycle".to_string(), "Valves".to_string()],
            explained_on: vec![2],
            importance: 5,
            worth_studying: true,
            model: "vision-model".to_string(),
        })
    );
    assert!(!reading::waiting_to_be_read(&connection, DOCUMENT).unwrap());
    let readings: i64 = connection
        .query_row("SELECT COUNT(*) FROM document_figure_readings", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(readings, 1);
}

#[test]
fn finding_again_keeps_what_was_read_of_a_figure_in_the_same_place() {
    let mut connection = library();
    replace_figures(&mut connection, DOCUMENT, &[found(72.0)], 5).unwrap();
    reading::store_readings(
        &mut connection,
        DOCUMENT,
        &[reading_of("p00001-01")],
        "openrouter",
        "vision-model",
        "read",
        6,
    )
    .unwrap();

    replace_figures(&mut connection, DOCUMENT, &[found(74.0)], 7).unwrap();
    let again = figures(&connection, DOCUMENT).unwrap();
    assert_eq!(again.reading, "read");
    assert_eq!(
        again.figures[0].reading.as_ref().unwrap().title,
        "The heart"
    );

    replace_figures(&mut connection, DOCUMENT, &[found(250.0)], 8).unwrap();
    let moved = figures(&connection, DOCUMENT).unwrap();
    assert!(moved.figures[0].reading.is_none());
    assert_eq!(moved.reading, "none");
    assert!(reading::waiting_to_be_read(&connection, DOCUMENT).unwrap());
}

#[test]
fn turning_images_off_forgets_what_was_read() {
    let mut connection = library();
    replace_figures(&mut connection, DOCUMENT, &[found(72.0)], 5).unwrap();
    reading::store_readings(
        &mut connection,
        DOCUMENT,
        &[reading_of("p00001-01")],
        "openrouter",
        "vision-model",
        "read",
        6,
    )
    .unwrap();
    let off = set_use_images(
        &mut connection,
        SetDocumentFiguresUseImagesInput {
            document_id: DOCUMENT.to_string(),
            use_images: false,
        },
        7,
    )
    .unwrap();
    assert_eq!(off.reading, "none");
    assert_eq!(off.read_at, None);
    let readings: i64 = connection
        .query_row("SELECT COUNT(*) FROM document_figure_readings", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(readings, 0);
}

#[test]
fn a_page_is_read_four_figures_at_a_time_with_the_page_beside_it() {
    let mut connection = library();
    let mut on_page = (0..5)
        .map(|index| {
            let mut figure = found(72.0);
            figure.figure_id = format!("p00002-{:02}", index + 1);
            figure.page_number = 2;
            figure.passages.clear();
            figure
        })
        .collect::<Vec<_>>();
    let mut filling = found(72.0);
    filling.figure_id = "p00003-01".to_string();
    filling.page_number = 3;
    filling.passages[1].page_number = 4;
    on_page.push(filling);
    replace_figures(&mut connection, DOCUMENT, &on_page, 5).unwrap();
    let stored = figures(&connection, DOCUMENT).unwrap().figures;
    assert_eq!(
        reading::reading_windows(&stored),
        vec![
            reading::ReadingWindow {
                page_number: 2,
                figures: vec![0, 1, 2, 3],
                neighbour: None,
            },
            reading::ReadingWindow {
                page_number: 2,
                figures: vec![4],
                neighbour: None,
            },
            reading::ReadingWindow {
                page_number: 3,
                figures: vec![5],
                neighbour: Some(4),
            },
        ]
    );
}

#[test]
fn crops_and_pages_are_drawn_within_one_tile() {
    let page = reading::whole_page_tile(612.0, 792.0);
    assert!(page.is_valid());
    assert_eq!((page.width, page.height), (791, 1024));

    // A small figure is drawn at about 200 dpi with a margin.
    let small = reading::crop_tile(
        612.0,
        792.0,
        FigureBoxDto {
            left: 72.0,
            top: 100.0,
            width: 144.0,
            height: 72.0,
        },
    );
    assert!(small.is_valid());
    assert_eq!(small.page_width, 1700);
    assert!((420..=430).contains(&small.width));

    // A figure filling the page is shrunk to fit one tile.
    let large = reading::crop_tile(
        612.0,
        792.0,
        FigureBoxDto {
            left: 0.0,
            top: 0.0,
            width: 612.0,
            height: 792.0,
        },
    );
    assert!(large.is_valid());
    assert!(large.width <= 1024 && large.height <= 1024);
}

/// A page of a scanned book: one picture of the whole page, with the text
/// layer its scanner wrote over it.
fn scanned_page(page_number: usize, runs: Vec<PageTextRun>) -> PageLayout {
    page(
        page_number,
        vec![graphic(PageGraphicKind::Image, [0.0, 0.0, 612.0, 792.0])],
        runs,
    )
}

/// Running text in a column: lines of many words, 12 points apart.
fn prose(left: f64, top: f64, width: f64, lines: usize) -> Vec<PageTextRun> {
    (0..lines)
        .map(|line| {
            run(
                "the drug lowers blood pressure by relaxing the smooth muscle of arteries",
                left,
                top + line as f64 * 12.0,
                width,
            )
        })
        .collect()
}

#[test]
fn a_scanned_pages_figure_is_found_above_its_garbled_caption() {
    // Left column: a running header, the figure's labels, its caption (whose
    // font map prints "Fig. 22.1" as "Figú 22ú1") and text under it; the right
    // column is text from top to bottom.
    let mut runs = vec![run("UNIT IV Drugs Affecting the Heart", 60.0, 30.0, 200.0)];
    runs.extend([
        run("Cortex", 90.0, 90.0, 40.0),
        run("Sympathetic", 180.0, 120.0, 70.0),
        run("Kidney", 100.0, 200.0, 40.0),
        run("Beta blockers", 150.0, 260.0, 80.0),
        run("Figú 22ú1", 60.0, 300.0, 40.0),
        run("Sites of action of the", 104.0, 300.0, 120.0),
        run("antihypertensive drugs.", 60.0, 312.0, 130.0),
    ]);
    runs.extend(prose(60.0, 340.0, 230.0, 20));
    runs.extend(prose(320.0, 80.0, 230.0, 50));
    let figures = detect::detect_layouts(&[scanned_page(28, runs)]);
    assert_eq!(figures.len(), 1, "{figures:?}");
    let figure = &figures[0];
    assert_eq!(figure.caption.as_ref().unwrap().label, "Figure 22.1");
    let detect::Rect {
        left,
        top,
        width,
        height,
    } = figure.bounds;
    // Under the header, over the labels, and within the left column.
    assert!(
        (792.0 * 0.09 - 1.0..=90.0).contains(&top),
        "{:?}",
        figure.bounds
    );
    assert!(top + height <= 300.0);
    assert!(left <= 60.0 && left + width < 320.0, "{:?}", figure.bounds);
}

#[test]
fn a_scanned_figure_across_the_page_ends_at_the_text_above_it() {
    // A paragraph, then a figure whose labels reach past its caption's ends.
    let mut runs = prose(60.0, 80.0, 490.0, 6);
    runs.extend([
        run("Preload", 50.0, 190.0, 50.0),
        run("Afterload", 500.0, 230.0, 60.0),
        run("Contractility", 260.0, 320.0, 70.0),
        run(
            "Figure 27.2 Factors that set the heart's output and how",
            90.0,
            400.0,
            430.0,
        ),
        run(
            "the drugs for heart failure act on them.",
            90.0,
            412.0,
            240.0,
        ),
    ]);
    runs.extend(prose(60.0, 440.0, 490.0, 20));
    let figures = detect::detect_layouts(&[scanned_page(113, runs)]);
    assert_eq!(figures.len(), 1, "{figures:?}");
    let detect::Rect {
        left,
        top,
        width,
        height,
    } = figures[0].bounds;
    assert!(
        top > 80.0 + 5.0 * 12.0 + 10.0 && top <= 190.0,
        "{:?}",
        figures[0].bounds
    );
    assert!(top + height <= 400.0);
    assert!(
        left <= 50.0 && left + width >= 560.0,
        "{:?}",
        figures[0].bounds
    );
}

#[test]
fn a_scanned_pages_text_that_names_a_figure_is_not_one() {
    // "Fig. 22.1" at the start of a line of running text, with text above it.
    let mut runs = prose(60.0, 80.0, 490.0, 10);
    runs.push(run(
        "Figú 22ú1 shows where each class of drug acts on the body",
        60.0,
        200.0,
        490.0,
    ));
    runs.extend(prose(60.0, 212.0, 490.0, 30));
    assert!(detect::detect_layouts(&[scanned_page(30, runs)]).is_empty());
}
