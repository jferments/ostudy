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
    PageAnnotations, PageFrame, PageLink, TextNote, Update, FONT_NAME, STICKY_HEIGHT,
    STICKY_ICON_SIZE, TEXT_LINE_HEIGHT, TEXT_PADDING,
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

/// Where a link's label (or a topic's box) sits, as `(x, top, width, height)` in shown points.
pub(super) fn link_box(note: &TextNote) -> (f64, f64, f64, f64) {
    if note.topic.is_some() {
        return (note.x, note.y, note.width, topic_height(note));
    }
    let lines = wrap(&note.text, note.width - 2.0 * TEXT_PADDING, note.font_size).len();
    let height = lines.max(1) as f64 * note.font_size * TEXT_LINE_HEIGHT + 2.0 * TEXT_PADDING;
    (note.x, note.y, note.width, height)
}

/// What a mark covers on its page, as `(left, top, right, bottom)` in points,
/// with half its line width around strokes. A text note's height follows its
/// wrapped lines, as the app lays them out.
pub fn shape_bounds(shape: &AnnotationShape) -> (f64, f64, f64, f64) {
    fn around(coordinates: impl Iterator<Item = (f64, f64)>, pad: f64) -> (f64, f64, f64, f64) {
        let (mut left, mut top, mut right, mut bottom) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for (x, y) in coordinates {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
        if left > right {
            return (0.0, 0.0, 0.0, 0.0);
        }
        (left - pad, top - pad, right + pad, bottom + pad)
    }
    match shape {
        AnnotationShape::Ink(stroke) => around(
            stroke
                .points
                .chunks_exact(3)
                .map(|point| (point[0], point[1])),
            stroke.stroke_width / 2.0,
        ),
        AnnotationShape::Highlight(stroke) => match &stroke.rects {
            Some(rects) if !rects.is_empty() => around(
                rects
                    .chunks_exact(4)
                    .flat_map(|rect| [(rect[0], rect[1]), (rect[0] + rect[2], rect[1] + rect[3])]),
                0.0,
            ),
            _ => around(
                stroke
                    .points
                    .chunks_exact(2)
                    .map(|point| (point[0], point[1])),
                stroke.stroke_width / 2.0,
            ),
        },
        AnnotationShape::Line(segment) | AnnotationShape::Arrow(segment) => {
            let controls = segment.curve.as_deref().unwrap_or(&[]);
            around(
                [(segment.x1, segment.y1), (segment.x2, segment.y2)]
                    .into_iter()
                    .chain(controls.chunks_exact(2).map(|point| (point[0], point[1]))),
                // Room for an arrow's head as well as the line.
                segment.stroke_width * 3.0,
            )
        }
        AnnotationShape::Rectangle(frame) | AnnotationShape::Ellipse(frame) => around(
            [
                (frame.x, frame.y),
                (frame.x + frame.width, frame.y + frame.height),
            ]
            .into_iter(),
            frame.stroke_width / 2.0,
        ),
        AnnotationShape::Text(note) if note.sticky == Some(true) => {
            let (width, height) = if note.collapsed == Some(true) {
                (STICKY_ICON_SIZE, STICKY_ICON_SIZE)
            } else {
                (note.width, STICKY_HEIGHT)
            };
            (note.x, note.y, note.x + width, note.y + height)
        }
        AnnotationShape::Text(note) => {
            let (x, top, width, height) = link_box(note);
            (x, top, x + width, top + height)
        }
        AnnotationShape::Image(picture) => (
            picture.x,
            picture.y,
            picture.x + picture.width,
            picture.y + picture.height,
        ),
    }
}

/// The mark moved `dx` points right and `dy` down.
pub fn shifted(shape: &AnnotationShape, dx: f64, dy: f64) -> AnnotationShape {
    fn pairs(values: &mut [f64], stride: usize, dx: f64, dy: f64) {
        for chunk in values.chunks_exact_mut(stride) {
            chunk[0] += dx;
            chunk[1] += dy;
        }
    }
    let mut moved = shape.clone();
    match &mut moved {
        AnnotationShape::Ink(stroke) => pairs(&mut stroke.points, 3, dx, dy),
        AnnotationShape::Highlight(stroke) => {
            pairs(&mut stroke.points, 2, dx, dy);
            if let Some(rects) = stroke.rects.as_mut() {
                pairs(rects, 4, dx, dy);
            }
        }
        AnnotationShape::Line(segment) | AnnotationShape::Arrow(segment) => {
            segment.x1 += dx;
            segment.y1 += dy;
            segment.x2 += dx;
            segment.y2 += dy;
            if let Some(curve) = segment.curve.as_mut() {
                pairs(curve, 2, dx, dy);
            }
        }
        AnnotationShape::Rectangle(frame) | AnnotationShape::Ellipse(frame) => {
            frame.x += dx;
            frame.y += dy;
        }
        AnnotationShape::Text(note) => {
            note.x += dx;
            note.y += dy;
        }
        AnnotationShape::Image(picture) => {
            picture.x += dx;
            picture.y += dy;
        }
    }
    moved
}

/// A mind-map topic's box, as the app draws it: padding inside an outline, in points.
const TOPIC_PADDING_X: f64 = 10.0;
const TOPIC_PADDING_Y: f64 = 6.0;
const TOPIC_BORDER: f64 = 2.0;
/// A topic's words, dark on its white box; a central topic's are white on its color.
const TOPIC_INK: &str = "#1f2328";

fn topic_lines(note: &TextNote) -> Vec<String> {
    wrap(
        &note.text,
        note.width - 2.0 * (TOPIC_PADDING_X + TOPIC_BORDER),
        note.font_size,
    )
}

fn topic_height(note: &TextNote) -> f64 {
    topic_lines(note).len().max(1) as f64 * note.font_size * TEXT_LINE_HEIGHT
        + 2.0 * (TOPIC_PADDING_Y + TOPIC_BORDER)
}

impl Canvas<'_> {
    /// A mind-map topic: a box outlined in its branch's color (the central
    /// topic filled with it) and its words centered in it.
    pub(super) fn topic(&mut self, note: &TextNote) -> Result<(), ExportError> {
        let central = note
            .topic
            .as_ref()
            .is_some_and(|topic| topic.parent.is_none());
        let color = super::parse_color(&note.color);
        let height = topic_height(note);
        let inset = TOPIC_BORDER / 2.0;
        let bottom = self.y(note.y + height);
        self.page
            .graphics()
            .save_state()
            .set_fill_color(if central { color } else { paper_white() })
            .set_stroke_color(color)
            .set_line_width(TOPIC_BORDER)
            .rect(
                note.x + inset,
                bottom + inset,
                note.width - TOPIC_BORDER,
                height - TOPIC_BORDER,
            )
            .fill_stroke()
            .restore_state();
        let ink = if central {
            paper_white()
        } else {
            super::parse_color(TOPIC_INK)
        };
        let top = note.y + TOPIC_BORDER + TOPIC_PADDING_Y;
        let line_height = note.font_size * TEXT_LINE_HEIGHT;
        for (index, line) in topic_lines(note).into_iter().enumerate() {
            // Centered by the same rough glyph width the lines were wrapped with.
            let width = line.chars().count() as f64 * note.font_size * super::CHARACTER_WIDTH;
            let x = note.x + ((note.width - width) / 2.0).max(TOPIC_BORDER + TOPIC_PADDING_X);
            self.write_lines(
                &[line],
                x,
                top + index as f64 * line_height,
                note.font_size,
                ink,
            )?;
        }
        Ok(())
    }
}

/// A notebook page's marks as they are drawn on paper, from `(id, mark)`
/// pairs in drawing order: topics folded away are left out, and each shown
/// branch of a mind map is drawn as a curve from its topic, under the marks.
pub fn mind_map_marks(items: &[(&str, &AnnotationShape)]) -> Vec<AnnotationShape> {
    let topics: HashMap<&str, &TextNote> = items
        .iter()
        .filter_map(|(id, shape)| match shape {
            AnnotationShape::Text(note) if note.topic.is_some() => Some((*id, note)),
            _ => None,
        })
        .collect();
    if topics.is_empty() {
        return items.iter().map(|(_, shape)| (*shape).clone()).collect();
    }
    let parent_of = |id: &str| -> Option<&str> {
        let topic = topics.get(id)?.topic.as_ref()?;
        let parent = topic.parent.as_deref()?;
        let parent_topic = topics.get(parent)?.topic.as_ref()?;
        (parent_topic.map == topic.map).then_some(parent)
    };
    // A topic is hidden when a topic above it is folded; a loop of parents hides nothing.
    let hidden = |id: &str| -> bool {
        let mut current = id;
        for _ in 0..topics.len() {
            let Some(parent) = parent_of(current) else {
                return false;
            };
            if topics
                .get(parent)
                .and_then(|note| note.topic.as_ref())
                .is_some_and(|topic| topic.folded == Some(true))
            {
                return true;
            }
            current = parent;
        }
        false
    };
    let mut branches = Vec::new();
    for (id, note) in &topics {
        if hidden(id) {
            continue;
        }
        let Some(parent_id) = parent_of(id) else {
            continue;
        };
        let Some(parent) = topics.get(parent_id) else {
            continue;
        };
        if parent
            .topic
            .as_ref()
            .is_some_and(|topic| topic.folded == Some(true))
        {
            continue;
        }
        let root = note
            .topic
            .as_ref()
            .and_then(|topic| topics.get(topic.map.as_str()));
        let down = root
            .and_then(|root| root.topic.as_ref())
            .is_some_and(|topic| {
                topic.layout == Some(crate::document_annotations::MapLayout::Down)
            });
        let (parent_height, child_height) = (topic_height(parent), topic_height(note));
        let (from, to, controls) = if down {
            let from = (parent.x + parent.width / 2.0, parent.y + parent_height);
            let to = (note.x + note.width / 2.0, note.y);
            let middle = (from.1 + to.1) / 2.0;
            (from, to, [from.0, middle, to.0, middle])
        } else {
            let leftward = note.x + note.width / 2.0 < parent.x + parent.width / 2.0;
            let from = (
                if leftward {
                    parent.x
                } else {
                    parent.x + parent.width
                },
                parent.y + parent_height / 2.0,
            );
            let to = (
                if leftward {
                    note.x + note.width
                } else {
                    note.x
                },
                note.y + child_height / 2.0,
            );
            let middle = (from.0 + to.0) / 2.0;
            (from, to, [middle, from.1, middle, to.1])
        };
        let from_root = parent
            .topic
            .as_ref()
            .is_some_and(|topic| topic.parent.is_none());
        branches.push((
            *id,
            AnnotationShape::Line(super::Segment {
                x1: from.0,
                y1: from.1,
                x2: to.0,
                y2: to.1,
                color: note.color.clone(),
                stroke_width: if from_root { 3.0 } else { 2.0 },
                dash: None,
                both_ends: None,
                curve: Some(controls.to_vec()),
                group: None,
            }),
        ));
    }
    // The same order every time, whatever the map's order of lookup.
    branches.sort_by(|a, b| a.0.cmp(b.0));
    branches
        .into_iter()
        .map(|(_, line)| line)
        .chain(
            items
                .iter()
                .filter(|(id, _)| !topics.contains_key(id) || !hidden(id))
                .map(|(_, shape)| (*shape).clone()),
        )
        .collect()
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
