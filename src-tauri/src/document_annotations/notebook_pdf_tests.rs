use std::collections::HashMap;
use std::io::Cursor;

use oxidize_pdf::graphics::Image;
use oxidize_pdf::parser::objects::PdfObject;
use oxidize_pdf::parser::{PdfDocument, PdfReader};

use super::*;

fn shapes(json: &str) -> Vec<AnnotationShape> {
    serde_json::from_str(json).expect("test shapes should parse")
}

fn reread(bytes: &[u8]) -> PdfDocument<Cursor<&[u8]>> {
    PdfDocument::new(PdfReader::new(Cursor::new(bytes)).expect("the notebook PDF should parse"))
}

/// A 20 by 10 picture: red on the left half, see-through on the right.
fn picture() -> Image {
    let mut rgba = Vec::new();
    for _ in 0..10 {
        for x in 0..20 {
            rgba.extend_from_slice(if x < 10 {
                &[255, 0, 0, 255]
            } else {
                &[0, 0, 0, 0]
            });
        }
    }
    Image::from_rgba_data(rgba, 20, 10).unwrap()
}

/// Page `index` (from 0) drawn at one pixel per point, in color.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn render(bytes: &[u8], index: usize, name: &str, width: u32, height: u32) -> image::RgbImage {
    use crate::pdf_render_backend::{render_page_tile, shared_test_runtime, PageTile};
    let path = std::env::temp_dir().join(format!(
        "ostudy-notebook-pdf-{name}-{}.pdf",
        std::process::id()
    ));
    std::fs::write(&path, bytes).unwrap();
    let tile = PageTile {
        page_width: width,
        page_height: height,
        left: 0,
        top: 0,
        width,
        height,
    };
    let png = render_page_tile(shared_test_runtime(), &path, index + 1, tile).unwrap();
    std::fs::remove_file(&path).ok();
    image::load_from_memory(&png).unwrap().to_rgb8()
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[test]
fn a_notebook_page_is_drawn_with_its_paper_marks_and_pictures() {
    let marks = shapes(
        r##"[{"kind":"image","x":20,"y":120,"width":80,"height":40,"assetId":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
             {"kind":"rectangle","x":150,"y":20,"width":40,"height":40,"color":"#000000","strokeWidth":4,
              "fill":"#000000"},
             {"kind":"image","x":20,"y":170,"width":20,"height":20,"assetId":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}]"##,
    );
    let pictures = HashMap::from([("a".repeat(32), picture())]);
    let pages = [
        NotebookPdfPage {
            width: 200.0,
            height: 220.0,
            ruling: Ruling::Lined,
            shapes: marks.iter().collect(),
            links: Vec::new(),
        },
        NotebookPdfPage {
            width: 300.0,
            height: 200.0,
            ruling: Ruling::Blank,
            shapes: Vec::new(),
            links: Vec::new(),
        },
    ];
    let output = notebook_pdf(&pages, &pictures, None, "Cardiac cycle").unwrap();
    let document = reread(&output);
    assert_eq!(document.page_count().unwrap(), 2);
    assert_eq!(
        document.get_page(1).unwrap().media_box,
        [0.0, 0.0, 300.0, 200.0]
    );

    let shown = render(&output, 0, "marks", 200, 220);
    let at = |x: u32, y: u32| shown.get_pixel(x, y).0;
    // Ruled lines every 24 points from 72, in pale blue; the paper between them is white.
    let rule = at(100, 72);
    assert!(rule[2] > rule[0] + 10, "a ruled line is blue: {rule:?}");
    assert!(
        at(100, 84).iter().all(|channel| *channel > 245),
        "{:?}",
        at(100, 84)
    );
    // The margin rule is pink.
    let margin = at(64, 30);
    assert!(
        margin[0] > margin[2] + 10,
        "the margin rule is pink: {margin:?}"
    );
    // The filled box is black.
    assert!(
        at(170, 40).iter().all(|channel| *channel < 40),
        "{:?}",
        at(170, 40)
    );
    // The picture's left half is red; its see-through half shows the paper.
    let red = at(40, 140);
    assert!(red[0] > 200 && red[1] < 60 && red[2] < 60, "{red:?}");
    assert!(
        at(80, 130).iter().all(|channel| *channel > 200),
        "{:?}",
        at(80, 130)
    );
    // A picture whose image is missing is left out.
    assert!(
        at(30, 185).iter().all(|channel| *channel > 200),
        "{:?}",
        at(30, 185)
    );
}

#[test]
fn links_to_the_notebooks_own_pages_open_them_and_sticky_notes_hold_their_text() {
    let first = shapes(
        r##"[{"kind":"text","x":40,"y":40,"width":120,"text":"Valves","color":"#2563eb","fontSize":14,
              "link":{"target":{"kind":"notebookPage","notebookId":"cccccccccccccccccccccccccccccccc",
                                "pageId":"dddddddddddddddddddddddddddddddd"}}},
             {"kind":"text","x":40,"y":80,"width":120,"text":"Lecture 7 · p. 9","color":"#2563eb","fontSize":14,
              "link":{"target":{"kind":"documentPage","documentId":"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                                "pageNumber":9},"button":true}},
             {"kind":"text","x":40,"y":140,"width":120,"text":"Check the second sound","color":"#fde68a",
              "fontSize":12,"sticky":true}]"##,
    );
    let pages = [
        NotebookPdfPage {
            width: 200.0,
            height: 220.0,
            ruling: Ruling::Grid,
            shapes: first.iter().collect(),
            // The first note opens the second page; the document link stays a label.
            links: vec![(0, 1)],
        },
        NotebookPdfPage {
            width: 200.0,
            height: 220.0,
            ruling: Ruling::Dots,
            shapes: Vec::new(),
            links: Vec::new(),
        },
    ];
    let output = notebook_pdf(&pages, &HashMap::new(), None, "Cardiac cycle").unwrap();
    let document = reread(&output);
    let page = document.get_page(0).unwrap();
    let second = document.get_page(1).unwrap().obj_ref;
    let annots = match document.resolve(page.dict.get("Annots").unwrap()).unwrap() {
        PdfObject::Array(array) => array.0,
        other => panic!("Annots is an array, not {other:?}"),
    };
    let mut kinds = Vec::new();
    for annot in &annots {
        let dict = document.resolve(annot).unwrap().as_dict().cloned().unwrap();
        let subtype = dict
            .get("Subtype")
            .and_then(PdfObject::as_name)
            .unwrap()
            .0
            .clone();
        if subtype == "Link" {
            let destination = match dict.get("Dest").unwrap() {
                PdfObject::Array(array) => array.0.clone(),
                other => panic!("a link names its page: {other:?}"),
            };
            assert_eq!(destination[0], PdfObject::Reference(second.0, second.1));
            // Over the label's line, in the page's own coordinates.
            let rect = match dict.get("Rect").unwrap() {
                PdfObject::Array(array) => array
                    .0
                    .iter()
                    .filter_map(PdfObject::as_real)
                    .collect::<Vec<_>>(),
                other => panic!("{other:?}"),
            };
            assert_eq!(rect.len(), 4);
            assert!(
                (rect[0] - 40.0).abs() < 0.01 && (rect[3] - 180.0).abs() < 0.01,
                "{rect:?}"
            );
        }
        kinds.push(subtype);
    }
    kinds.sort();
    assert_eq!(kinds, ["Link", "Text"]);
    // The second page has nothing to open.
    assert!(document.get_page(1).unwrap().dict.get("Annots").is_none());
}

#[test]
fn a_notebook_without_notes_or_links_is_written_in_one_pass() {
    let ink = shapes(
        r##"[{"kind":"ink","points":[10,10,0.5,50,50,0.5],"color":"#1f2328","strokeWidth":3,
              "simulatePressure":false}]"##,
    );
    let pages = [NotebookPdfPage {
        width: 100.0,
        height: 100.0,
        ruling: Ruling::Blank,
        shapes: ink.iter().collect(),
        links: Vec::new(),
    }];
    let output = notebook_pdf(&pages, &HashMap::new(), None, "Ink").unwrap();
    assert_eq!(
        output
            .windows(9)
            .filter(|window| window == b"startxref")
            .count(),
        1
    );
    assert!(notebook_pdf(&[], &HashMap::new(), None, "None").is_err());
}
