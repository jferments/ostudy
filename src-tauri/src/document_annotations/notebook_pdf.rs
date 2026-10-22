//! A notebook as an ordinary PDF.
//!
//! Each notebook page becomes a PDF page of its size: the paper's ruling,
//! then every mark in drawing order, drawn the way the annotated-PDF copy
//! draws a document's marks, with pictures embedded. A second pass appends
//! an incremental update that adds sticky notes as PDF notes and turns links
//! to pages of the same notebook into PDF links. Links to anything else (a
//! document, a study guide, a deck or another notebook) stay as their
//! drawn labels, since a reader of the file has none of those.

use std::collections::HashMap;
use std::io::Cursor;

use oxidize_pdf::graphics::{Color, Image};
use oxidize_pdf::parser::objects::PdfObject;
use oxidize_pdf::parser::{PdfDocument, PdfReader};
use oxidize_pdf::text::Font;
use oxidize_pdf::{Document, Page};

use super::{
    sticky_notes, unreadable, wrap, write_failed, AnnotationShape, Canvas, ExportError,
    PageAnnotations, PageFrame, PageLink, TextNote, Update, FONT_NAME, TEXT_LINE_HEIGHT,
    TEXT_PADDING,
};

/// Spacing of ruled lines, grid squares and dots, in points, as the app draws them.
const LINE_SPACING: f64 = 24.0;
const GRID_SPACING: f64 = 18.0;
const TOP_MARGIN: f64 = 72.0;
const LEFT_MARGIN: f64 = 64.0;
const RULE: &str = "#c4d3e6";
const MARGIN_RULE: &str = "#e8b4b8";

/// The ruling a page's paper has.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ruling {
    Blank,
    Lined,
    Grid,
    Dots,
}

/// One notebook page: its size in points, its paper, and its marks, earliest
/// first. `links` pairs each mark that is a link to a page of this notebook
/// with that page's index, from 0.
pub struct NotebookPdfPage<'a> {
    pub width: f64,
    pub height: f64,
    pub ruling: Ruling,
    pub shapes: Vec<&'a AnnotationShape>,
    pub links: Vec<(usize, usize)>,
}

/// The notebook's pages as a PDF. `pictures` holds each picture's image by
/// its asset id, decoded; a picture whose image is missing is left out.
/// `font` is as for `annotated_pdf`.
pub fn notebook_pdf(
    pages: &[NotebookPdfPage<'_>],
    pictures: &HashMap<String, Image>,
    font: Option<Vec<u8>>,
    title: &str,
) -> Result<Vec<u8>, ExportError> {
    if pages.is_empty() {
        return Err(write_failed("a notebook has at least one page"));
    }
    let drawn = draw_pages(pages, pictures, font, title)?;

    // The second pass: notes and links, which name pages by their objects.
    let reader = PdfReader::new(Cursor::new(drawn.as_slice())).map_err(write_failed)?;
    let trailer = reader.trailer().dict.clone();
    let prev = super::last_xref_offset(&drawn)?;
    let xref_is_stream = !drawn
        .get(prev..)
        .is_some_and(|rest| rest.starts_with(b"xref"));
    let document = PdfDocument::new(reader);
    let mut parsed = Vec::with_capacity(pages.len());
    for index in 0..pages.len() {
        parsed.push(
            document
                .get_page(u32::try_from(index).map_err(write_failed)?)
                .map_err(write_failed)?,
        );
    }
    let size = trailer
        .get("Size")
        .and_then(PdfObject::as_integer)
        .and_then(|size| u32::try_from(size).ok())
        .ok_or_else(|| unreadable("the trailer has no size"))?;
    let mut update = Update::new(&drawn, size);
    let mut changed = false;
    for (page, parsed_page) in pages.iter().zip(&parsed) {
        let frame = upright(page);
        let notes = sticky_notes(
            &PageAnnotations {
                page_number: 1,
                shapes: page.shapes.clone(),
            },
            &frame,
        );
        let links: Vec<PageLink> = page
            .links
            .iter()
            .filter_map(|(shape, target)| {
                let AnnotationShape::Text(note) = page.shapes.get(*shape)? else {
                    return None;
                };
                let target = parsed.get(*target)?.obj_ref;
                let (x, top, width, height) = link_box(note);
                Some(PageLink {
                    rect: [x, page.height - top - height, x + width, page.height - top],
                    target,
                })
            })
            .collect();
        let Some(annots) = update.add_annotations(&document, parsed_page, notes, &links)? else {
            continue;
        };
        let mut dict: Vec<(String, Vec<u8>)> = parsed_page
            .dict
            .0
            .iter()
            .filter(|(key, _)| key.0 != "Annots")
            .map(|(key, value)| (key.0.clone(), super::object_bytes(value)))
            .collect();
        dict.push(("Annots".into(), annots));
        update.write_object(
            parsed_page.obj_ref.0,
            parsed_page.obj_ref.1,
            &super::dictionary_bytes(&dict),
        );
        changed = true;
    }
    if !changed {
        return Ok(drawn);
    }
    update.finish(&trailer, prev, xref_is_stream)
}

/// A notebook page's frame: shown as it is drawn, y down from the top.
fn upright(page: &NotebookPdfPage<'_>) -> PageFrame {
    PageFrame {
        width: page.width,
        height: page.height,
        shown_to_page: [1.0, 0.0, 0.0, -1.0, 0.0, page.height],
    }
}

/// Where a link's label sits, as `(x, top, width, height)` in shown points.
pub(super) fn link_box(note: &TextNote) -> (f64, f64, f64, f64) {
    let lines = wrap(&note.text, note.width - 2.0 * TEXT_PADDING, note.font_size).len();
    let height = lines.max(1) as f64 * note.font_size * TEXT_LINE_HEIGHT + 2.0 * TEXT_PADDING;
    (note.x, note.y, note.width, height)
}

fn draw_pages(
    pages: &[NotebookPdfPage<'_>],
    pictures: &HashMap<String, Image>,
    font: Option<Vec<u8>>,
    title: &str,
) -> Result<Vec<u8>, ExportError> {
    let mut document = Document::new();
    document.set_title(title);
    let has_text = pages
        .iter()
        .flat_map(|page| page.shapes.iter())
        .any(|shape| matches!(shape, AnnotationShape::Text(_)));
    let text_font = match font {
        Some(bytes) if has_text => {
            document
                .add_font_from_bytes(FONT_NAME, bytes)
                .map_err(write_failed)?;
            Font::Custom(FONT_NAME.to_string())
        }
        _ => Font::Helvetica,
    };
    for page in pages {
        let mut canvas = Canvas {
            page: Page::new(page.width, page.height),
            height: page.height,
            font: &text_font,
            pictures: Some(pictures),
        };
        rule(&mut canvas, page);
        for shape in &page.shapes {
            canvas.draw(shape)?;
        }
        document.add_page(canvas.page);
    }
    document.to_bytes().map_err(write_failed)
}

/// Draws the paper's ruling under the page's marks.
fn rule(canvas: &mut Canvas<'_>, page: &NotebookPdfPage<'_>) {
    let (width, height) = (page.width, page.height);
    let graphics = canvas.page.graphics();
    match page.ruling {
        Ruling::Blank => {}
        Ruling::Lined => {
            graphics
                .save_state()
                .set_stroke_color(super::parse_color(RULE))
                .set_line_width(0.75);
            let mut y = TOP_MARGIN;
            while y < height - LINE_SPACING / 2.0 {
                graphics.move_to(0.0, height - y).line_to(width, height - y);
                y += LINE_SPACING;
            }
            graphics.stroke();
            graphics
                .set_stroke_color(super::parse_color(MARGIN_RULE))
                .move_to(LEFT_MARGIN, 0.0)
                .line_to(LEFT_MARGIN, height)
                .stroke()
                .restore_state();
        }
        Ruling::Grid => {
            graphics
                .save_state()
                .set_stroke_color(super::parse_color(RULE))
                .set_line_width(0.6);
            let mut y = 0.0;
            while y < height {
                graphics.move_to(0.0, height - y).line_to(width, height - y);
                y += GRID_SPACING;
            }
            let mut x = 0.0;
            while x < width {
                graphics.move_to(x, 0.0).line_to(x, height);
                x += GRID_SPACING;
            }
            graphics.stroke().restore_state();
        }
        Ruling::Dots => {
            graphics
                .save_state()
                .set_fill_color(super::parse_color(RULE));
            let mut y = GRID_SPACING / 2.0;
            while y < height {
                let mut x = GRID_SPACING / 2.0;
                while x < width {
                    graphics.circle(x, height - y, 0.9);
                    x += GRID_SPACING;
                }
                y += GRID_SPACING;
            }
            graphics.fill().restore_state();
        }
    }
}

/// White, for a button link's label on its colored face.
pub(super) fn paper_white() -> Color {
    Color::rgb(1.0, 1.0, 1.0)
}

#[cfg(test)]
#[path = "notebook_pdf_tests.rs"]
mod tests;
