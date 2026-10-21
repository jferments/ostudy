use std::io::Cursor;

use oxidize_pdf::parser::{PdfDocument, PdfReader};

use super::*;

/// A fixture page: its media box, crop box and rotation.
type FixturePage = ([f64; 4], Option<[f64; 4]>, i32);

/// A small PDF whose pages each fill their media box with mid grey, so a
/// mark only shows if it is drawn over the page's own content.
fn fixture(pages: &[FixturePage]) -> Vec<u8> {
    let kids: Vec<String> = (0..pages.len())
        .map(|index| format!("{} 0 R", 3 + 2 * index))
        .collect();
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            pages.len()
        ),
    ];
    for (index, (media, crop, rotation)) in pages.iter().enumerate() {
        let crop = crop
            .map(|crop| format!(" /CropBox [{}]", crop.map(number).join(" ")))
            .unwrap_or_default();
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [{}]{crop} /Rotate {rotation} /Contents {} 0 R /Resources << >> >>",
            media.map(number).join(" "),
            4 + 2 * index
        ));
        let content = format!(
            "0.5 g {} {} {} {} re f",
            media[0],
            media[1],
            media[2] - media[0],
            media[3] - media[1]
        );
        objects.push(format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ));
    }
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (index, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", index + 1).as_bytes());
    }
    let start = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

fn shapes(json: &str) -> Vec<AnnotationShape> {
    serde_json::from_str(json).expect("test shapes should parse")
}

/// A black text highlight over the box 40..80 by 30..50 of the page as shown.
fn highlight() -> Vec<AnnotationShape> {
    shapes(
        r##"[{"kind":"highlight","points":[],"color":"#000000","strokeWidth":12,
              "rects":[40,30,40,20],"text":"marked"}]"##,
    )
}

fn export(source: &[u8], shapes: &[AnnotationShape]) -> Result<Vec<u8>, ExportError> {
    annotated_pdf(
        source,
        &[PageAnnotations {
            page_number: 1,
            shapes: shapes.iter().collect(),
        }],
        None,
    )
}

fn reread(bytes: &[u8]) -> PdfDocument<Cursor<&[u8]>> {
    PdfDocument::new(PdfReader::new(Cursor::new(bytes)).expect("the export should parse"))
}

/// Grey levels of page 1 drawn at one pixel per point, as shown.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn render(bytes: &[u8], name: &str) -> image::GrayImage {
    use crate::pdf_render_backend::{render_page_tile, shared_test_runtime, PageTile};
    let path = std::env::temp_dir().join(format!(
        "ostudy-annotated-pdf-{name}-{}.pdf",
        std::process::id()
    ));
    std::fs::write(&path, bytes).unwrap();
    let document = reread(bytes);
    let frame = PageFrame::of(&document.get_page(0).unwrap());
    let (width, height) = (frame.width, frame.height);
    let tile = PageTile {
        page_width: width.round() as u32,
        page_height: height.round() as u32,
        left: 0,
        top: 0,
        width: width.round() as u32,
        height: height.round() as u32,
    };
    let png = render_page_tile(shared_test_runtime(), &path, 1, tile).unwrap();
    std::fs::remove_file(&path).ok();
    image::load_from_memory(&png).unwrap().to_luma8()
}

#[test]
fn the_export_appends_to_the_original_bytes_and_reads_back() {
    let source = fixture(&[([0.0, 0.0, 200.0, 220.0], None, 0); 2]);
    let output = annotated_pdf(
        &source,
        &[PageAnnotations {
            page_number: 2,
            shapes: highlight().iter().collect(),
        }],
        None,
    )
    .unwrap();
    assert!(output.starts_with(&source));
    let document = reread(&output);
    assert_eq!(document.page_count().unwrap(), 2);
    // The annotated page draws its own content inside q/Q, then the marks.
    let content = document
        .get_combined_page_content_stream(&document.get_page(1).unwrap())
        .unwrap();
    let content = String::from_utf8_lossy(&content);
    assert!(content.starts_with('q'), "{content}");
    assert!(content.contains("re f"), "{content}");
    assert!(
        content.trim_end().ends_with("/OstudyAnnotations Do Q"),
        "{content}"
    );
    // The other page is the original object, untouched.
    let untouched = document
        .get_combined_page_content_stream(&document.get_page(0).unwrap())
        .unwrap();
    assert!(!String::from_utf8_lossy(&untouched).contains("Ostudy"));
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[test]
fn marks_land_where_the_annotator_showed_them_on_turned_and_cropped_pages() {
    let pages = [
        ("upright", [0.0, 0.0, 200.0, 220.0], None, 0),
        (
            "cropped",
            [0.0, 0.0, 300.0, 300.0],
            Some([50.0, 30.0, 250.0, 250.0]),
            0,
        ),
        ("quarter", [0.0, 0.0, 220.0, 200.0], None, 90),
        ("upside-down", [10.0, 20.0, 210.0, 240.0], None, 180),
        ("three-quarter", [0.0, 0.0, 220.0, 200.0], None, 270),
    ];
    for (name, media, crop, rotation) in pages {
        let source = fixture(&[(media, crop, rotation)]);
        let output = export(&source, &highlight()).unwrap();
        let shown = render(&output, name);
        assert_eq!((shown.width(), shown.height()), (200, 220), "{name}");
        let grey = shown.get_pixel(150, 150).0[0];
        let marked = shown.get_pixel(60, 40).0[0];
        assert!(
            (120..=136).contains(&grey),
            "{name}: the page is grey ({grey})"
        );
        // Three-quarters black multiplied over grey leaves a quarter of it.
        assert!(
            marked < 50,
            "{name}: the mark is drawn over the page ({marked})"
        );
        for (x, y) in [(35, 40), (85, 40), (60, 25), (60, 55)] {
            let outside = shown.get_pixel(x, y).0[0];
            assert!(
                outside > 110,
                "{name}: ({x}, {y}) is outside the mark ({outside})"
            );
        }
    }
}

#[test]
fn xref_stream_sources_get_an_xref_stream_update() {
    let mut document = oxidize_pdf::Document::new();
    let mut page = Page::new(200.0, 220.0);
    page.graphics()
        .set_fill_color(Color::rgb(0.5, 0.5, 0.5))
        .rect(0.0, 0.0, 200.0, 220.0)
        .fill();
    document.add_page(page);
    let config = oxidize_pdf::writer::WriterConfig {
        use_xref_streams: true,
        use_object_streams: true,
        ..Default::default()
    };
    let source = document.to_bytes_with_config(config).unwrap();
    let output = export(&source, &highlight()).unwrap();
    let tail = String::from_utf8_lossy(&output[source.len()..]);
    assert!(tail.contains("/Type /XRef"), "{tail}");
    assert!(!tail.contains("\nxref\n"));
    let document = reread(&output);
    let content = document
        .get_combined_page_content_stream(&document.get_page(0).unwrap())
        .unwrap();
    assert!(String::from_utf8_lossy(&content).contains("/OstudyAnnotations Do"));
}

#[test]
fn sticky_notes_carry_their_whole_text_as_a_pdf_note() {
    let source = fixture(&[([0.0, 0.0, 200.0, 220.0], None, 0)]);
    let note = shapes(
        r##"[{"kind":"text","x":20,"y":20,"width":120,"text":"Kreb's cycle: π ≈ 3.14\nSecond line",
              "color":"#fff3b0","fontSize":10,"sticky":true}]"##,
    );
    let output = export(&source, &note).unwrap();
    let document = reread(&output);
    let annotations = document.get_page_annotations(0).unwrap();
    assert_eq!(annotations.len(), 1);
    let annotation = &annotations[0];
    assert_eq!(
        annotation
            .get("Subtype")
            .and_then(PdfObject::as_name)
            .map(|name| name.as_str()),
        Some("Text")
    );
    let contents = annotation
        .get("Contents")
        .and_then(PdfObject::as_string)
        .unwrap()
        .as_bytes();
    let units: Vec<u16> = contents[2..]
        .chunks_exact(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .collect();
    assert_eq!(&contents[..2], &[0xFE, 0xFF]);
    assert_eq!(
        String::from_utf16(&units).unwrap(),
        "Kreb's cycle: π ≈ 3.14\nSecond line"
    );
    // The note covers the paper drawn on the page: 20..140 by 20..92 shown.
    let rect: Vec<f64> = annotation
        .get("Rect")
        .and_then(PdfObject::as_array)
        .unwrap()
        .0
        .iter()
        .filter_map(PdfObject::as_real)
        .collect();
    assert_eq!(rect, vec![20.0, 128.0, 140.0, 200.0]);
}

#[test]
fn a_collapsed_sticky_note_is_a_small_icon_that_keeps_its_text() {
    let source = fixture(&[([0.0, 0.0, 200.0, 220.0], None, 0)]);
    let note = shapes(
        r##"[{"kind":"text","x":20,"y":20,"width":120,"text":"Folded away",
              "color":"#fff3b0","fontSize":10,"sticky":true,"collapsed":true}]"##,
    );
    let output = export(&source, &note).unwrap();
    let document = reread(&output);
    let annotations = document.get_page_annotations(0).unwrap();
    assert_eq!(annotations.len(), 1);
    // The note covers only the 18-point icon: 20..38 by 20..38 shown.
    let rect: Vec<f64> = annotations[0]
        .get("Rect")
        .and_then(PdfObject::as_array)
        .unwrap()
        .0
        .iter()
        .filter_map(PdfObject::as_real)
        .collect();
    assert_eq!(rect, vec![20.0, 182.0, 38.0, 200.0]);
    let contents = annotations[0]
        .get("Contents")
        .and_then(PdfObject::as_string)
        .unwrap()
        .as_bytes();
    let units: Vec<u16> = contents[2..]
        .chunks_exact(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .collect();
    assert_eq!(String::from_utf16(&units).unwrap(), "Folded away");
}

#[test]
fn every_kind_of_mark_is_written() {
    let source = fixture(&[([0.0, 0.0, 200.0, 220.0], None, 0)]);
    let all = shapes(
        r##"[
          {"kind":"ink","points":[10,10,0.5],"color":"#1f2328","strokeWidth":3,"simulatePressure":true},
          {"kind":"ink","points":[10,30,0.5,30,40,0.5,50,30,0.5,70,45,0.5],"color":"#1f2328","strokeWidth":3,"simulatePressure":true},
          {"kind":"ink","points":[10,60,0.1,30,65,0.5,50,60,0.9],"color":"#d1242f","strokeWidth":3,"simulatePressure":false},
          {"kind":"highlight","points":[10,80,60,80],"color":"#ffd400","strokeWidth":12},
          {"kind":"line","x1":10,"y1":100,"x2":60,"y2":100,"color":"#0969da","strokeWidth":2},
          {"kind":"arrow","x1":10,"y1":110,"x2":60,"y2":130,"color":"#0969da","strokeWidth":2},
          {"kind":"rectangle","x":100,"y":10,"width":50,"height":30,"color":"#8250df","strokeWidth":1},
          {"kind":"ellipse","x":100,"y":50,"width":50,"height":30,"color":"#1a7f37","strokeWidth":2},
          {"kind":"text","x":100,"y":100,"width":90,"text":"A typed note that wraps","color":"#1f2328","fontSize":10}
        ]"##,
    );
    let output = export(&source, &all).unwrap();
    let document = reread(&output);
    let content = document
        .get_combined_page_content_stream(&document.get_page(0).unwrap())
        .unwrap();
    assert!(String::from_utf8_lossy(&content).contains("/OstudyAnnotations Do"));
    // Without a Unicode font, text falls back to Helvetica.
    assert!(String::from_utf8_lossy(&output).contains("/Helvetica"));
}

#[test]
fn typed_text_embeds_the_bundled_font() {
    let source = fixture(&[([0.0, 0.0, 200.0, 220.0], None, 0)]);
    let note = shapes(
        r##"[{"kind":"text","x":20,"y":20,"width":150,"text":"Δp = ρgh, ≈ 3 kPa","color":"#1f2328","fontSize":10}]"##,
    );
    let output = annotated_pdf(
        &source,
        &[PageAnnotations {
            page_number: 1,
            shapes: note.iter().collect(),
        }],
        Some(crate::document_study_guide_export::plain_sans_font().to_vec()),
    )
    .unwrap();
    let text = String::from_utf8_lossy(&output);
    // Embedded as a Unicode (Type 0) font rather than drawn in Helvetica.
    assert!(text.contains("/FontFile2"), "the font is embedded");
    assert!(text.contains("/Subtype /Type0"));
}

#[test]
fn a_page_the_pdf_lacks_is_refused() {
    let source = fixture(&[([0.0, 0.0, 200.0, 220.0], None, 0)]);
    let marks = highlight();
    let result = annotated_pdf(
        &source,
        &[PageAnnotations {
            page_number: 2,
            shapes: marks.iter().collect(),
        }],
        None,
    );
    assert_eq!(result, Err(ExportError::PageMissing(2)));
    assert!(matches!(
        export(b"not a pdf", &marks),
        Err(ExportError::Unreadable(_))
    ));
}

#[test]
fn a_form_name_the_page_already_uses_is_not_reused() {
    let mut names = PdfDictionary::new();
    assert_eq!(free_name(&names), FORM_NAME);
    names.insert(FORM_NAME.to_string(), PdfObject::Reference(1, 0));
    assert_eq!(free_name(&names), format!("{FORM_NAME}2"));
}

#[test]
fn text_wraps_like_the_annotator() {
    // 100 points at 10-point text holds 18 characters a line.
    assert_eq!(
        wrap("one two three four five six", 100.0, 10.0),
        vec!["one two three four", "five six"]
    );
    assert_eq!(wrap("a\n\nb", 100.0, 10.0), vec!["a", "", "b"]);
    assert_eq!(wrap("abcdefghij", 22.0, 10.0), vec!["abcd", "efgh", "ij"]);
    assert_eq!(latin("café π"), "café ?");
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[test]
fn fills_dashes_and_second_arrow_heads_are_drawn() {
    let source = fixture(&[([0.0, 0.0, 200.0, 220.0], None, 0)]);
    let styled = shapes(
        r##"[
          {"kind":"rectangle","x":40,"y":30,"width":40,"height":20,"color":"#ffffff","strokeWidth":1,
           "fill":"#000000","border":false},
          {"kind":"line","x1":10,"y1":150,"x2":190,"y2":150,"color":"#000000","strokeWidth":4,"dash":"dashed"},
          {"kind":"arrow","x1":20,"y1":190,"x2":180,"y2":190,"color":"#000000","strokeWidth":3,"bothEnds":true}
        ]"##,
    );
    let shown = render(&export(&source, &styled).unwrap(), "styled");
    assert!(shown.get_pixel(60, 40).0[0] < 50, "the box is filled");
    let along: Vec<u8> = (20..180).map(|x| shown.get_pixel(x, 150).0[0]).collect();
    assert!(along.iter().any(|&level| level < 50), "the dashes are drawn");
    assert!(along.iter().any(|&level| level > 110), "the dashes have gaps");
    // Each head is wider than the 3-point line: 10 points from a tip it reaches 4 points off the line.
    assert!(shown.get_pixel(29, 186).0[0] < 50, "the first end has a head");
    assert!(shown.get_pixel(170, 186).0[0] < 50, "the second end has a head");
}
