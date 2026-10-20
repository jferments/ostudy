//! A copy of a PDF with its annotations drawn into the pages.
//!
//! The source file's bytes are kept whole and an incremental update (ISO
//! 32000-1 §7.5.6) is appended after them, so the original pages, text,
//! links, outline and metadata stay as they were. oxidize-pdf draws each
//! annotated page's annotations onto a page of their own; that page's
//! content becomes a form XObject which the source page draws after its own
//! content, wrapped in `q`/`Q` so the source's graphics state cannot leak into
//! it. Sticky notes also become PDF text annotations holding their whole text,
//! so another reader can open them.
//!
//! Annotations are stored in points from the top-left corner of the page as
//! the annotator shows it: the crop box, turned by the page's `/Rotate`. Each
//! form's matrix maps that frame back to the page's own coordinates.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::Cursor;

use oxidize_pdf::graphics::{BlendMode, Color, ExtGState, LineCap, LineJoin};
use oxidize_pdf::parser::objects::{PdfDictionary, PdfObject};
use oxidize_pdf::parser::{PdfDocument, PdfReader};
use oxidize_pdf::text::Font;
use oxidize_pdf::{Document, Page};

use super::{AnnotationShape, Frame, HighlightStroke, InkStroke, Segment, TextNote};

/// Matches the annotator: line height as a multiple of the font size, padding in points.
const TEXT_LINE_HEIGHT: f64 = 1.3;
const TEXT_PADDING: f64 = 2.0;
const STICKY_HEIGHT: f64 = 72.0;
const STICKY_PADDING: f64 = 6.0;
const STICKY_FOLD: f64 = 12.0;
const HIGHLIGHT_OPACITY: f64 = 0.75;
/// The annotator's rough glyph width for wrapping, as a share of the font size.
const CHARACTER_WIDTH: f64 = 0.55;
const FORM_NAME: &str = "OstudyAnnotations";
const FONT_NAME: &str = "OstudyNoteFont";

/// One page's annotations, earliest first. `page_number` counts from 1.
pub struct PageAnnotations<'a> {
    pub page_number: u32,
    pub shapes: Vec<&'a AnnotationShape>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExportError {
    /// The PDF is password protected; its pages cannot be changed.
    Encrypted,
    /// The PDF could not be read, or its structure is damaged.
    Unreadable(String),
    /// An annotation names a page the PDF does not have.
    PageMissing(u32),
    /// Drawing or writing failed.
    Write(String),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encrypted => write!(formatter, "the PDF is encrypted"),
            Self::Unreadable(detail) => write!(formatter, "the PDF could not be read: {detail}"),
            Self::PageMissing(page) => write!(formatter, "the PDF has no page {page}"),
            Self::Write(detail) => write!(
                formatter,
                "the annotated PDF could not be written: {detail}"
            ),
        }
    }
}

fn unreadable(error: impl std::fmt::Display) -> ExportError {
    ExportError::Unreadable(error.to_string())
}

fn write_failed(error: impl std::fmt::Display) -> ExportError {
    ExportError::Write(error.to_string())
}

/// The source PDF with every page in `pages` drawn over. `font` is a Unicode
/// TrueType font for typed text; without one, text uses Helvetica and
/// characters it lacks show as `?`.
pub fn annotated_pdf(
    source: &[u8],
    pages: &[PageAnnotations<'_>],
    font: Option<Vec<u8>>,
) -> Result<Vec<u8>, ExportError> {
    let prev = last_xref_offset(source)?;
    let reader = PdfReader::new(Cursor::new(source)).map_err(unreadable)?;
    if reader.is_encrypted() {
        return Err(ExportError::Encrypted);
    }
    let trailer = reader.trailer().dict.clone();
    let xref_is_stream = !source
        .get(prev..)
        .is_some_and(|rest| rest.starts_with(b"xref"));
    let document = PdfDocument::new(reader);
    let page_count = document.page_count().map_err(unreadable)?;

    let mut frames = Vec::with_capacity(pages.len());
    for page in pages {
        if page.page_number == 0 || page.page_number > page_count {
            return Err(ExportError::PageMissing(page.page_number));
        }
        let parsed = document
            .get_page(page.page_number - 1)
            .map_err(unreadable)?;
        frames.push(PageFrame::of(&parsed));
    }

    let overlay = draw_overlay(pages, &frames, font)?;
    let overlay_document =
        PdfDocument::new(PdfReader::new(Cursor::new(overlay.as_slice())).map_err(write_failed)?);

    let size = trailer
        .get("Size")
        .and_then(PdfObject::as_integer)
        .and_then(|size| u32::try_from(size).ok())
        .ok_or_else(|| unreadable("the trailer has no size"))?;
    let mut update = Update::new(source, size);
    for (index, page) in pages.iter().enumerate() {
        let parsed = document
            .get_page(page.page_number - 1)
            .map_err(unreadable)?;
        let frame = &frames[index];
        let overlay_page = overlay_document
            .get_page(u32::try_from(index).map_err(write_failed)?)
            .map_err(write_failed)?;
        let form = update.import_form(&overlay_document, &overlay_page, frame)?;
        let notes = sticky_notes(page, frame);
        update.replace_page(&document, &parsed, form, notes)?;
    }
    update.finish(&trailer, prev, xref_is_stream)
}

/// The byte offset in the file's last `startxref`.
fn last_xref_offset(source: &[u8]) -> Result<usize, ExportError> {
    let tail_start = source.len().saturating_sub(2048);
    let tail = &source[tail_start..];
    let at = tail
        .windows(9)
        .rposition(|window| window == b"startxref")
        .ok_or_else(|| unreadable("no startxref"))?;
    let digits: String = tail[at + 9..]
        .iter()
        .skip_while(|byte| byte.is_ascii_whitespace())
        .take_while(|byte| byte.is_ascii_digit())
        .map(|byte| char::from(*byte))
        .collect();
    let offset: usize = digits.parse().map_err(|_| unreadable("a bad startxref"))?;
    if offset >= source.len() {
        return Err(unreadable("startxref is past the end of the file"));
    }
    Ok(offset)
}

/// How the annotator's view of a page maps onto the page.
struct PageFrame {
    /// The page as shown, in points.
    width: f64,
    height: f64,
    /// Shown coordinates (x right, y down from the top-left) to the page's own
    /// coordinates: `[a b c d e f]` with `x' = a x + c y + e`, `y' = b x + d y + f`.
    shown_to_page: [f64; 6],
}

impl PageFrame {
    fn of(page: &oxidize_pdf::parser::page_tree::ParsedPage) -> Self {
        let media = page.media_box;
        let [x0, y0, x1, y1] = page
            .crop_box
            .map(|crop| {
                [
                    crop[0].max(media[0]),
                    crop[1].max(media[1]),
                    crop[2].min(media[2]),
                    crop[3].min(media[3]),
                ]
            })
            .filter(|crop| crop[2] > crop[0] && crop[3] > crop[1])
            .unwrap_or(media);
        let (width, height) = (x1 - x0, y1 - y0);
        match page.rotation.rem_euclid(360) {
            90 => Self {
                width: height,
                height: width,
                shown_to_page: [0.0, 1.0, 1.0, 0.0, x0, y0],
            },
            180 => Self {
                width,
                height,
                shown_to_page: [-1.0, 0.0, 0.0, 1.0, x1, y0],
            },
            270 => Self {
                width: height,
                height: width,
                shown_to_page: [0.0, -1.0, -1.0, 0.0, x1, y1],
            },
            _ => Self {
                width,
                height,
                shown_to_page: [1.0, 0.0, 0.0, -1.0, x0, y1],
            },
        }
    }

    fn to_page(&self, x: f64, y: f64) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.shown_to_page;
        (a * x + c * y + e, b * x + d * y + f)
    }

    /// The form matrix: the overlay page (y up from the bottom) to the page.
    fn form_matrix(&self) -> [f64; 6] {
        let [a, b, c, d, e, f] = self.shown_to_page;
        [a, b, -c, -d, c * self.height + e, d * self.height + f]
    }
}

// ── Drawing ──

fn draw_overlay(
    pages: &[PageAnnotations<'_>],
    frames: &[PageFrame],
    font: Option<Vec<u8>>,
) -> Result<Vec<u8>, ExportError> {
    let mut document = Document::new();
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
    for (page, frame) in pages.iter().zip(frames) {
        let mut canvas = Canvas {
            page: Page::new(frame.width, frame.height),
            height: frame.height,
            font: &text_font,
        };
        for shape in &page.shapes {
            canvas.draw(shape)?;
        }
        document.add_page(canvas.page);
    }
    document.to_bytes().map_err(write_failed)
}

struct Canvas<'a> {
    page: Page,
    /// The page's height, to turn the annotator's y-down points upward.
    height: f64,
    font: &'a Font,
}

impl Canvas<'_> {
    fn y(&self, y: f64) -> f64 {
        self.height - y
    }

    fn draw(&mut self, shape: &AnnotationShape) -> Result<(), ExportError> {
        match shape {
            AnnotationShape::Ink(stroke) => self.ink(stroke),
            AnnotationShape::Highlight(stroke) => self.highlight(stroke),
            AnnotationShape::Line(segment) => self.line(segment, false),
            AnnotationShape::Arrow(segment) => self.line(segment, true),
            AnnotationShape::Rectangle(frame) => self.frame(frame, false),
            AnnotationShape::Ellipse(frame) => self.frame(frame, true),
            AnnotationShape::Text(note) if note.sticky == Some(true) => self.sticky(note),
            AnnotationShape::Text(note) => self.text(note),
        }
    }

    /// Starts a shape drawn like the annotator's marks: multiplied into the page.
    fn begin_mark(&mut self, alpha: Option<f64>) -> Result<(), ExportError> {
        let mut state = ExtGState::new().with_blend_mode(BlendMode::Multiply);
        if let Some(alpha) = alpha {
            state = state.with_alpha_stroke(alpha).with_alpha_fill(alpha);
        }
        let graphics = self.page.graphics();
        graphics.save_state();
        graphics.apply_extgstate(state).map_err(write_failed)?;
        Ok(())
    }

    fn end(&mut self) {
        self.page.graphics().restore_state();
    }

    fn ink(&mut self, stroke: &InkStroke) -> Result<(), ExportError> {
        let color = parse_color(&stroke.color);
        let points: Vec<(f64, f64, f64)> = stroke
            .points
            .chunks_exact(3)
            .map(|triple| (triple[0], self.y(triple[1]), triple[2]))
            .collect();
        self.begin_mark(None)?;
        let graphics = self.page.graphics();
        graphics
            .set_stroke_color(color)
            .set_fill_color(color)
            .set_line_cap(LineCap::Round)
            .set_line_join(LineJoin::Round);
        let pressured = !stroke.simulate_pressure
            && points
                .windows(2)
                .any(|pair| (pair[0].2 - pair[1].2).abs() > f64::EPSILON);
        match points.as_slice() {
            [] => {}
            [(x, y, pressure)] => {
                let radius = ink_width(stroke, *pressure, pressured) / 2.0;
                graphics.circle(*x, *y, radius).fill();
            }
            _ if pressured => {
                // Each piece takes the width its pressure gives; round caps join them.
                for pair in points.windows(2) {
                    let (x1, y1, p1) = pair[0];
                    let (x2, y2, p2) = pair[1];
                    graphics
                        .set_line_width(ink_width(stroke, (p1 + p2) / 2.0, true))
                        .move_to(x1, y1)
                        .line_to(x2, y2)
                        .stroke();
                }
            }
            _ => {
                graphics.set_line_width(stroke.stroke_width);
                smooth_path(
                    graphics,
                    &points.iter().map(|(x, y, _)| (*x, *y)).collect::<Vec<_>>(),
                );
                graphics.stroke();
            }
        }
        self.end();
        Ok(())
    }

    fn highlight(&mut self, stroke: &HighlightStroke) -> Result<(), ExportError> {
        let color = parse_color(&stroke.color);
        self.begin_mark(Some(HIGHLIGHT_OPACITY))?;
        if let Some(rects) = &stroke.rects {
            let boxes: Vec<[f64; 4]> = rects
                .chunks_exact(4)
                .map(|rect| [rect[0], self.y(rect[1] + rect[3]), rect[2], rect[3]])
                .collect();
            let graphics = self.page.graphics();
            graphics.set_fill_color(color);
            for [x, y, width, height] in boxes {
                graphics.rect(x, y, width, height);
            }
            graphics.fill();
        } else {
            let points: Vec<(f64, f64)> = stroke
                .points
                .chunks_exact(2)
                .map(|pair| (pair[0], self.y(pair[1])))
                .collect();
            let graphics = self.page.graphics();
            graphics
                .set_stroke_color(color)
                .set_line_width(stroke.stroke_width)
                .set_line_cap(LineCap::Butt)
                .set_line_join(LineJoin::Round);
            if let Some((first, rest)) = points.split_first() {
                graphics.move_to(first.0, first.1);
                if rest.is_empty() {
                    graphics.line_to(first.0 + 0.01, first.1);
                }
                for (x, y) in rest {
                    graphics.line_to(*x, *y);
                }
                graphics.stroke();
            }
        }
        self.end();
        Ok(())
    }

    fn line(&mut self, segment: &Segment, arrow: bool) -> Result<(), ExportError> {
        let color = parse_color(&segment.color);
        let head = arrow.then(|| arrow_head(segment));
        let (end_x, end_y) = head.map_or((segment.x2, segment.y2), |head| head.base);
        let (x1, y1, x2, y2) = (segment.x1, self.y(segment.y1), end_x, self.y(end_y));
        let head = head.map(|head| {
            [
                (segment.x2, self.y(segment.y2)),
                (head.left.0, self.y(head.left.1)),
                (head.right.0, self.y(head.right.1)),
            ]
        });
        self.begin_mark(None)?;
        let graphics = self.page.graphics();
        graphics
            .set_stroke_color(color)
            .set_fill_color(color)
            .set_line_width(segment.stroke_width)
            .set_line_cap(LineCap::Round)
            .move_to(x1, y1)
            .line_to(x2, y2)
            .stroke();
        if let Some([tip, left, right]) = head {
            graphics
                .set_line_width(segment.stroke_width.min(1.0))
                .set_line_join(LineJoin::Round)
                .move_to(tip.0, tip.1)
                .line_to(left.0, left.1)
                .line_to(right.0, right.1)
                .close_path()
                .fill_stroke();
        }
        self.end();
        Ok(())
    }

    fn frame(&mut self, frame: &Frame, ellipse: bool) -> Result<(), ExportError> {
        let color = parse_color(&frame.color);
        let bottom = self.y(frame.y + frame.height);
        self.begin_mark(None)?;
        let graphics = self.page.graphics();
        graphics
            .set_stroke_color(color)
            .set_line_width(frame.stroke_width)
            .set_line_join(LineJoin::Round);
        if ellipse {
            ellipse_path(graphics, frame.x, bottom, frame.width, frame.height);
        } else {
            graphics.rect(frame.x, bottom, frame.width, frame.height);
        }
        graphics.stroke();
        self.end();
        Ok(())
    }

    fn text(&mut self, note: &TextNote) -> Result<(), ExportError> {
        let lines = wrap(&note.text, note.width - 2.0 * TEXT_PADDING, note.font_size);
        self.write_lines(
            &lines,
            note.x + TEXT_PADDING,
            note.y + TEXT_PADDING,
            note.font_size,
            parse_color(&note.color),
        )
    }

    fn sticky(&mut self, note: &TextNote) -> Result<(), ExportError> {
        let paper = parse_color(&note.color);
        let (x, top, width) = (note.x, note.y, note.width);
        let bottom = self.y(top + STICKY_HEIGHT);
        let right = x + width;
        let graphics = self.page.graphics();
        graphics
            .save_state()
            .set_fill_color(paper)
            .rect(x, bottom, width, STICKY_HEIGHT)
            .fill();
        graphics
            .apply_extgstate(ExtGState::new().with_alpha_fill(0.2))
            .map_err(write_failed)?;
        graphics
            .set_fill_color(ink_black())
            .move_to(right, bottom + STICKY_FOLD)
            .line_to(right, bottom)
            .line_to(right - STICKY_FOLD, bottom)
            .close_path()
            .fill()
            .restore_state();
        let line_height = note.font_size * TEXT_LINE_HEIGHT;
        let shown = ((STICKY_HEIGHT - 2.0 * STICKY_PADDING) / line_height)
            .floor()
            .max(1.0) as usize;
        let mut lines = wrap(&note.text, width - 2.0 * STICKY_PADDING, note.font_size);
        if lines.len() > shown {
            lines.truncate(shown);
            if let Some(last) = lines.last_mut() {
                last.push('…');
            }
        }
        self.write_lines(
            &lines,
            x + STICKY_PADDING,
            top + STICKY_PADDING,
            note.font_size,
            ink_black(),
        )
    }

    /// Lines of text from `top`, each centred in its line height as CSS does.
    fn write_lines(
        &mut self,
        lines: &[String],
        x: f64,
        top: f64,
        size: f64,
        color: Color,
    ) -> Result<(), ExportError> {
        let line_height = size * TEXT_LINE_HEIGHT;
        for (index, line) in lines.iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            let baseline =
                top + index as f64 * line_height + (line_height - size) / 2.0 + size * 0.8;
            let y = self.y(baseline);
            let text = if matches!(self.font, Font::Custom(_)) {
                line.clone()
            } else {
                latin(line)
            };
            self.page
                .text()
                .set_font(self.font.clone(), size)
                .set_fill_color(color)
                .at(x, y)
                .write(&text)
                .map_err(write_failed)?;
        }
        Ok(())
    }
}

/// perfect-freehand's width for a pressure: thinner when light, wider when hard.
fn ink_width(stroke: &InkStroke, pressure: f64, pressured: bool) -> f64 {
    if !pressured {
        return stroke.stroke_width;
    }
    (stroke.stroke_width * (1.0 - 0.6 * (1.0 - 2.0 * pressure))).max(0.25)
}

/// A path through the midpoints of the points, curving at each point, as the
/// annotator's outline does.
fn smooth_path(graphics: &mut oxidize_pdf::graphics::GraphicsContext, points: &[(f64, f64)]) {
    let Some(&(x, y)) = points.first() else {
        return;
    };
    graphics.move_to(x, y);
    if points.len() == 2 {
        graphics.line_to(points[1].0, points[1].1);
        return;
    }
    let mut current = (x, y);
    for index in 1..points.len() - 1 {
        let control = points[index];
        let next = points[index + 1];
        let end = if index + 2 == points.len() {
            next
        } else {
            ((control.0 + next.0) / 2.0, (control.1 + next.1) / 2.0)
        };
        // A quadratic curve as the cubic PDF draws.
        let c1 = (
            current.0 + 2.0 / 3.0 * (control.0 - current.0),
            current.1 + 2.0 / 3.0 * (control.1 - current.1),
        );
        let c2 = (
            end.0 + 2.0 / 3.0 * (control.0 - end.0),
            end.1 + 2.0 / 3.0 * (control.1 - end.1),
        );
        graphics.curve_to(c1.0, c1.1, c2.0, c2.1, end.0, end.1);
        current = end;
    }
}

fn ellipse_path(
    graphics: &mut oxidize_pdf::graphics::GraphicsContext,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) {
    const KAPPA: f64 = 0.552_284_749_8;
    let (rx, ry) = (width / 2.0, height / 2.0);
    let (cx, cy) = (x + rx, y + ry);
    let (ox, oy) = (rx * KAPPA, ry * KAPPA);
    graphics
        .move_to(cx + rx, cy)
        .curve_to(cx + rx, cy + oy, cx + ox, cy + ry, cx, cy + ry)
        .curve_to(cx - ox, cy + ry, cx - rx, cy + oy, cx - rx, cy)
        .curve_to(cx - rx, cy - oy, cx - ox, cy - ry, cx, cy - ry)
        .curve_to(cx + ox, cy - ry, cx + rx, cy - oy, cx + rx, cy)
        .close_path();
}

#[derive(Clone, Copy)]
struct ArrowHead {
    base: (f64, f64),
    left: (f64, f64),
    right: (f64, f64),
}

/// The annotator's arrow head (`arrowHead` in annotationModel.ts).
fn arrow_head(segment: &Segment) -> ArrowHead {
    let length = (segment.stroke_width * 4.0).max(6.0);
    let angle = (segment.y2 - segment.y1).atan2(segment.x2 - segment.x1);
    let spread = std::f64::consts::PI / 7.0;
    let back =
        (length * spread.cos() * 0.8).min((segment.x2 - segment.x1).hypot(segment.y2 - segment.y1));
    ArrowHead {
        base: (
            segment.x2 - angle.cos() * back,
            segment.y2 - angle.sin() * back,
        ),
        left: (
            segment.x2 - (angle - spread).cos() * length,
            segment.y2 - (angle - spread).sin() * length,
        ),
        right: (
            segment.x2 - (angle + spread).cos() * length,
            segment.y2 - (angle + spread).sin() * length,
        ),
    }
}

/// Lines that fit `width`, estimated as the annotator does; line breaks stay.
fn wrap(text: &str, width: f64, size: f64) -> Vec<String> {
    let per_line = ((width / (size * CHARACTER_WIDTH)).floor() as usize).max(1);
    let mut lines = Vec::new();
    for paragraph in text.replace('\t', "    ").split('\n') {
        let mut current = String::new();
        let mut count = 0;
        for word in paragraph.split(' ') {
            let length = word.chars().count();
            if count > 0 && count + 1 + length > per_line {
                lines.push(std::mem::take(&mut current));
                count = 0;
            }
            if count > 0 {
                current.push(' ');
                count += 1;
            }
            for character in word.chars() {
                if count == per_line {
                    lines.push(std::mem::take(&mut current));
                    count = 0;
                }
                current.push(character);
                count += 1;
            }
        }
        lines.push(current);
    }
    lines
}

/// Text Helvetica can show: anything outside Latin-1 becomes `?`.
fn latin(text: &str) -> String {
    text.chars()
        .map(|character| {
            if u32::from(character) < 0x100 {
                character
            } else {
                '?'
            }
        })
        .collect()
}

fn parse_color(color: &str) -> Color {
    let channel = |range: std::ops::Range<usize>| {
        color
            .get(range)
            .and_then(|hex| u8::from_str_radix(hex, 16).ok())
            .map_or(0.0, |value| f64::from(value) / 255.0)
    };
    Color::rgb(channel(1..3), channel(3..5), channel(5..7))
}

/// The annotator's near-black for sticky-note text.
fn ink_black() -> Color {
    Color::rgb(0.039, 0.039, 0.039)
}

// ── Sticky notes as PDF text annotations ──

struct StickyNote {
    rect: [f64; 4],
    text: String,
    color: [f64; 3],
}

fn sticky_notes(page: &PageAnnotations<'_>, frame: &PageFrame) -> Vec<StickyNote> {
    page.shapes
        .iter()
        .filter_map(|shape| match shape {
            AnnotationShape::Text(note) if note.sticky == Some(true) => Some(note),
            _ => None,
        })
        .map(|note| {
            // The note covers the paper drawn on the page, so selecting it opens its text.
            let corners = [
                frame.to_page(note.x, note.y),
                frame.to_page(note.x + note.width, note.y + STICKY_HEIGHT),
            ];
            let color = parse_color(&note.color);
            StickyNote {
                rect: [
                    corners[0].0.min(corners[1].0),
                    corners[0].1.min(corners[1].1),
                    corners[0].0.max(corners[1].0),
                    corners[0].1.max(corners[1].1),
                ],
                text: note.text.clone(),
                color: [color.r(), color.g(), color.b()],
            }
        })
        .collect()
}

// ── The incremental update ──

struct Update<'a> {
    source: &'a [u8],
    out: Vec<u8>,
    next_id: u32,
    /// Object number to (offset, generation) for everything this update writes.
    offsets: BTreeMap<u32, (usize, u16)>,
}

impl<'a> Update<'a> {
    fn new(source: &'a [u8], size: u32) -> Self {
        let mut out = Vec::with_capacity(source.len() + 64 * 1024);
        out.extend_from_slice(source);
        if !source.ends_with(b"\n") && !source.ends_with(b"\r") {
            out.push(b'\n');
        }
        Self {
            source,
            out,
            next_id: size.max(1),
            offsets: BTreeMap::new(),
        }
    }

    fn allocate(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn write_object(&mut self, id: u32, generation: u16, body: &[u8]) {
        self.offsets.insert(id, (self.out.len(), generation));
        self.out
            .extend_from_slice(format!("{id} {generation} obj\n").as_bytes());
        self.out.extend_from_slice(body);
        self.out.extend_from_slice(b"\nendobj\n");
    }

    fn write_stream(&mut self, id: u32, mut dict: Vec<(String, Vec<u8>)>, data: &[u8]) {
        dict.retain(|(key, _)| key != "Length");
        dict.push(("Length".into(), data.len().to_string().into_bytes()));
        let mut body = dictionary_bytes(&dict);
        body.extend_from_slice(b"\nstream\n");
        body.extend_from_slice(data);
        body.extend_from_slice(b"\nendstream");
        self.write_object(id, 0, &body);
    }

    /// Copies an overlay page's drawing into this file as a form XObject,
    /// with every object its resources reach. Returns the form's object number.
    fn import_form<R: std::io::Read + std::io::Seek>(
        &mut self,
        overlay: &PdfDocument<R>,
        page: &oxidize_pdf::parser::page_tree::ParsedPage,
        frame: &PageFrame,
    ) -> Result<u32, ExportError> {
        let content = overlay
            .get_combined_page_content_stream(page)
            .map_err(write_failed)?;
        let resources = match page.dict.get("Resources") {
            Some(resources) => overlay.resolve(resources).map_err(write_failed)?,
            None => PdfObject::Dictionary(PdfDictionary::new()),
        };
        let mut importer = Importer::default();
        let resources = importer.copy(&resources, self);
        let form = self.allocate();
        let matrix = frame.form_matrix();
        self.write_stream(
            form,
            vec![
                ("Type".into(), b"/XObject".to_vec()),
                ("Subtype".into(), b"/Form".to_vec()),
                (
                    "BBox".into(),
                    format!("[0 0 {} {}]", number(frame.width), number(frame.height)).into_bytes(),
                ),
                (
                    "Matrix".into(),
                    format!("[{}]", matrix.map(number).join(" ")).into_bytes(),
                ),
                ("Resources".into(), resources),
            ],
            &content,
        );
        while let Some((source, target)) = importer.queue.pop_front() {
            let object = overlay
                .get_object(source.0, source.1)
                .map_err(write_failed)?;
            match &object {
                PdfObject::Stream(stream) => {
                    let dict = stream
                        .dict
                        .0
                        .iter()
                        .map(|(key, value)| (key.0.clone(), importer.copy(value, self)))
                        .collect();
                    let data = stream.raw_data().to_vec();
                    self.write_stream(target, dict, &data);
                }
                other => {
                    let body = importer.copy(other, self);
                    self.write_object(target, 0, &body);
                }
            }
        }
        Ok(form)
    }

    /// Writes a new revision of a page that draws `form` over its content.
    fn replace_page<R: std::io::Read + std::io::Seek>(
        &mut self,
        document: &PdfDocument<R>,
        page: &oxidize_pdf::parser::page_tree::ParsedPage,
        form: u32,
        notes: Vec<StickyNote>,
    ) -> Result<(), ExportError> {
        let mut dict: BTreeMap<String, Vec<u8>> = page
            .dict
            .0
            .iter()
            .map(|(key, value)| (key.0.clone(), object_bytes(value)))
            .collect();

        // Resources, with the form under a name the page does not use.
        let mut resources = match page.dict.get("Resources") {
            Some(resources) => document
                .resolve(resources)
                .map_err(unreadable)?
                .as_dict()
                .cloned()
                .unwrap_or_default(),
            None => page.inherited_resources.clone().unwrap_or_default(),
        };
        let mut xobjects = match resources.get("XObject") {
            Some(xobjects) => document
                .resolve(xobjects)
                .map_err(unreadable)?
                .as_dict()
                .cloned()
                .unwrap_or_default(),
            None => PdfDictionary::new(),
        };
        let name = free_name(&xobjects);
        xobjects.insert(name.clone(), PdfObject::Reference(form, 0));
        resources.insert("XObject".into(), PdfObject::Dictionary(xobjects));
        dict.insert(
            "Resources".into(),
            object_bytes(&PdfObject::Dictionary(resources)),
        );

        // Contents: the page's own streams inside q/Q, then the form.
        let mut contents = Vec::new();
        match page.dict.get("Contents") {
            Some(PdfObject::Reference(number, generation)) => {
                match document
                    .get_object(*number, *generation)
                    .map_err(unreadable)?
                {
                    PdfObject::Array(array) => contents.extend(array.0.iter().map(object_bytes)),
                    _ => contents.push(format!("{number} {generation} R").into_bytes()),
                }
            }
            Some(PdfObject::Array(array)) => contents.extend(array.0.iter().map(object_bytes)),
            _ => {}
        }
        let open = self.allocate();
        self.write_stream(open, Vec::new(), b"q");
        let close = self.allocate();
        self.write_stream(close, Vec::new(), format!("Q\nq /{name} Do Q").as_bytes());
        let mut array = format!("[{open} 0 R ").into_bytes();
        for content in contents {
            array.extend_from_slice(&content);
            array.push(b' ');
        }
        array.extend_from_slice(format!("{close} 0 R]").as_bytes());
        dict.insert("Contents".into(), array);

        // Sticky notes join the page's annotations.
        if !notes.is_empty() {
            let mut annots: Vec<Vec<u8>> = match page.dict.get("Annots") {
                Some(existing) => match document.resolve(existing).map_err(unreadable)? {
                    PdfObject::Array(array) => array.0.iter().map(object_bytes).collect(),
                    _ => Vec::new(),
                },
                None => Vec::new(),
            };
            let page_ref = format!("{} {} R", page.obj_ref.0, page.obj_ref.1);
            for note in notes {
                // An empty appearance: the page already shows the note.
                let appearance = self.allocate();
                let (width, height) = (note.rect[2] - note.rect[0], note.rect[3] - note.rect[1]);
                self.write_stream(
                    appearance,
                    vec![
                        ("Type".into(), b"/XObject".to_vec()),
                        ("Subtype".into(), b"/Form".to_vec()),
                        (
                            "BBox".into(),
                            format!("[0 0 {} {}]", number(width), number(height)).into_bytes(),
                        ),
                    ],
                    b"",
                );
                let id = self.allocate();
                let body = dictionary_bytes(&[
                    ("Type".into(), b"/Annot".to_vec()),
                    ("Subtype".into(), b"/Text".to_vec()),
                    (
                        "Rect".into(),
                        format!("[{}]", note.rect.map(number).join(" ")).into_bytes(),
                    ),
                    ("Contents".into(), text_string(&note.text)),
                    (
                        "C".into(),
                        format!("[{}]", note.color.map(number).join(" ")).into_bytes(),
                    ),
                    ("Name".into(), b"/Note".to_vec()),
                    ("F".into(), b"4".to_vec()),
                    ("P".into(), page_ref.clone().into_bytes()),
                    ("AP".into(), format!("<</N {appearance} 0 R>>").into_bytes()),
                ]);
                self.write_object(id, 0, &body);
                annots.push(format!("{id} 0 R").into_bytes());
            }
            let mut array = b"[".to_vec();
            array.extend_from_slice(&annots.join(&b' '));
            array.push(b']');
            dict.insert("Annots".into(), array);
        }

        let entries: Vec<(String, Vec<u8>)> = dict.into_iter().collect();
        self.write_object(page.obj_ref.0, page.obj_ref.1, &dictionary_bytes(&entries));
        Ok(())
    }

    /// Writes the cross-reference section and trailer, in the same form as the
    /// file's last one.
    fn finish(
        mut self,
        trailer: &PdfDictionary,
        prev: usize,
        as_stream: bool,
    ) -> Result<Vec<u8>, ExportError> {
        let mut entries: Vec<(String, Vec<u8>)> = ["Root", "Info", "ID"]
            .into_iter()
            .filter_map(|key| {
                trailer
                    .get(key)
                    .map(|value| (key.to_string(), object_bytes(value)))
            })
            .collect();
        entries.push(("Prev".into(), prev.to_string().into_bytes()));
        if as_stream {
            let id = self.allocate();
            let start = self.out.len();
            self.offsets.insert(id, (start, 0));
            let mut rows = Vec::new();
            for (offset, generation) in self.offsets.values() {
                rows.push(1u8);
                rows.extend_from_slice(
                    &u32::try_from(*offset).map_err(write_failed)?.to_be_bytes(),
                );
                rows.extend_from_slice(&generation.to_be_bytes());
            }
            entries.push(("Type".into(), b"/XRef".to_vec()));
            entries.push(("Size".into(), self.next_id.to_string().into_bytes()));
            entries.push(("W".into(), b"[1 4 2]".to_vec()));
            entries.push(("Index".into(), index_ranges(&self.offsets)));
            self.offsets.remove(&id);
            self.write_stream(id, entries, &rows);
            self.out
                .extend_from_slice(format!("startxref\n{start}\n%%EOF\n").as_bytes());
        } else {
            let start = self.out.len();
            let mut table = b"xref\n".to_vec();
            let ids: Vec<u32> = self.offsets.keys().copied().collect();
            for run in runs(&ids) {
                table.extend_from_slice(format!("{} {}\n", run.0, run.1).as_bytes());
                for id in run.0..run.0 + run.1 {
                    let (offset, generation) = self.offsets[&id];
                    table
                        .extend_from_slice(format!("{offset:010} {generation:05} n \n").as_bytes());
                }
            }
            entries.push(("Size".into(), self.next_id.to_string().into_bytes()));
            table.extend_from_slice(b"trailer\n");
            table.extend_from_slice(&dictionary_bytes(&entries));
            table.extend_from_slice(format!("\nstartxref\n{start}\n%%EOF\n").as_bytes());
            self.out.extend_from_slice(&table);
        }
        let _ = self.source;
        Ok(self.out)
    }
}

/// The form's resource name, numbered when the page already uses it.
fn free_name(xobjects: &PdfDictionary) -> String {
    let mut name = FORM_NAME.to_string();
    let mut suffix = 1;
    while xobjects.contains_key(&name) {
        suffix += 1;
        name = format!("{FORM_NAME}{suffix}");
    }
    name
}

/// Contiguous runs of object numbers, as `(first, count)`.
fn runs(ids: &[u32]) -> Vec<(u32, u32)> {
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for &id in ids {
        match runs.last_mut() {
            Some((first, count)) if *first + *count == id => *count += 1,
            _ => runs.push((id, 1)),
        }
    }
    runs
}

fn index_ranges(offsets: &BTreeMap<u32, (usize, u16)>) -> Vec<u8> {
    let ids: Vec<u32> = offsets.keys().copied().collect();
    let parts: Vec<String> = runs(&ids)
        .into_iter()
        .map(|(first, count)| format!("{first} {count}"))
        .collect();
    format!("[{}]", parts.join(" ")).into_bytes()
}

/// Renumbers objects copied from the overlay PDF into this file.
#[derive(Default)]
struct Importer {
    numbers: HashMap<(u32, u16), u32>,
    queue: VecDeque<((u32, u16), u32)>,
}

impl Importer {
    fn copy(&mut self, object: &PdfObject, update: &mut Update<'_>) -> Vec<u8> {
        match object {
            PdfObject::Reference(number, generation) => {
                let key = (*number, *generation);
                let target = match self.numbers.get(&key) {
                    Some(target) => *target,
                    None => {
                        let target = update.allocate();
                        self.numbers.insert(key, target);
                        self.queue.push_back((key, target));
                        target
                    }
                };
                format!("{target} 0 R").into_bytes()
            }
            PdfObject::Array(array) => {
                let mut bytes = b"[".to_vec();
                for (index, item) in array.0.iter().enumerate() {
                    if index > 0 {
                        bytes.push(b' ');
                    }
                    bytes.extend_from_slice(&self.copy(item, update));
                }
                bytes.push(b']');
                bytes
            }
            PdfObject::Dictionary(dict) => {
                let entries: Vec<(String, Vec<u8>)> = dict
                    .0
                    .iter()
                    .map(|(key, value)| (key.0.clone(), self.copy(value, update)))
                    .collect();
                dictionary_bytes(&entries)
            }
            other => object_bytes(other),
        }
    }
}

// ── Serialising ──

fn dictionary_bytes(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut sorted: Vec<&(String, Vec<u8>)> = entries.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut bytes = b"<<".to_vec();
    for (key, value) in sorted {
        bytes.extend_from_slice(&name_bytes(key));
        bytes.push(b' ');
        bytes.extend_from_slice(value);
        bytes.push(b'\n');
    }
    bytes.extend_from_slice(b">>");
    bytes
}

/// An object as it appears inside another; streams only appear by reference.
fn object_bytes(object: &PdfObject) -> Vec<u8> {
    match object {
        PdfObject::Null => b"null".to_vec(),
        PdfObject::Boolean(value) => value.to_string().into_bytes(),
        PdfObject::Integer(value) => value.to_string().into_bytes(),
        PdfObject::Real(value) => number(*value).into_bytes(),
        PdfObject::String(string) => hex_string(string.as_bytes()),
        PdfObject::Name(name) => name_bytes(&name.0),
        PdfObject::Array(array) => {
            let mut bytes = b"[".to_vec();
            for (index, item) in array.0.iter().enumerate() {
                if index > 0 {
                    bytes.push(b' ');
                }
                bytes.extend_from_slice(&object_bytes(item));
            }
            bytes.push(b']');
            bytes
        }
        PdfObject::Dictionary(dict) => {
            let entries: Vec<(String, Vec<u8>)> = dict
                .0
                .iter()
                .map(|(key, value)| (key.0.clone(), object_bytes(value)))
                .collect();
            dictionary_bytes(&entries)
        }
        // A direct stream cannot be written inside another object.
        PdfObject::Stream(_) => b"null".to_vec(),
        PdfObject::Reference(number, generation) => format!("{number} {generation} R").into_bytes(),
    }
}

fn name_bytes(name: &str) -> Vec<u8> {
    let mut bytes = b"/".to_vec();
    for &byte in name.as_bytes() {
        let regular = (0x21..=0x7e).contains(&byte) && !b"()<>[]{}/%#".contains(&byte);
        if regular {
            bytes.push(byte);
        } else {
            bytes.extend_from_slice(format!("#{byte:02X}").as_bytes());
        }
    }
    bytes
}

fn hex_string(data: &[u8]) -> Vec<u8> {
    let mut bytes = b"<".to_vec();
    for byte in data {
        bytes.extend_from_slice(format!("{byte:02X}").as_bytes());
    }
    bytes.push(b'>');
    bytes
}

/// A text string as UTF-16BE with a byte-order mark.
fn text_string(text: &str) -> Vec<u8> {
    let mut data = vec![0xFE, 0xFF];
    for unit in text.encode_utf16() {
        data.extend_from_slice(&unit.to_be_bytes());
    }
    hex_string(&data)
}

fn number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return format!("{value:.0}");
    }
    let text = format!("{value:.4}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
#[path = "pdf_export_tests.rs"]
mod tests;
