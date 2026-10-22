//! What a learner draws or types over a document's pages.
//!
//! Importing a PDF already renders every page to a managed PNG, recorded as a
//! `unit_render` source asset. The annotator shows those images and draws each
//! annotation over them, so the webview never parses a document file and the
//! document itself is never rewritten.
//!
//! Each annotation is one `document_annotations` row (release schema 8): an ink
//! or highlighter stroke, a line, an arrow, a rectangle, an ellipse or a typed
//! text note, in PDF points from the top-left corner of its page. A write
//! carries the whole new state of every annotation it changes, so repeating a
//! request after a lost response changes nothing further. Annotations stay on
//! the device that made them: whole-library backups keep them, device sync
//! packages leave them out.

use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    paths::AppPaths,
    pdf_render_backend::render_pixels_to_points,
    util::{unix_time_millis, validate_entity_id},
};

/// The largest page side a PDF allows (200 inches); no coordinate lies beyond it.
const MAX_COORDINATE: f64 = 14_400.0;
const MIN_STROKE_WIDTH: f64 = 0.25;
const MAX_STROKE_WIDTH: f64 = 64.0;
const MAX_STROKE_POINTS: usize = 10_000;
const MIN_FONT_SIZE: f64 = 6.0;
const MAX_FONT_SIZE: f64 = 96.0;
const MAX_TEXT_CHARS: usize = 5_000;
/// Lines one text highlight may cover: a few dense pages' worth.
const MAX_TEXT_RECTS: usize = 2_000;
/// Sticky notes hold long-form text behind their preview.
const MAX_STICKY_CHARS: usize = 20_000;
/// Matches the table's own payload check.
const MAX_PAYLOAD_BYTES: usize = 262_144;
const MAX_CHANGES_PER_REQUEST: usize = 500;
const MAX_ANNOTATIONS_PER_DOCUMENT: i64 = 20_000;
/// A page whose render cannot be measured is laid out as US Letter.
const FALLBACK_PAGE_SIZE: (f64, f64) = (612.0, 792.0);

/// One annotation's geometry and look. Colors are `#rrggbb`; lengths are PDF
/// points.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
/// Every kind may carry `group`: marks on one page sharing a group are
/// selected and moved together.
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AnnotationShape {
    Ink(InkStroke),
    Highlight(HighlightStroke),
    Line(Segment),
    Arrow(Segment),
    Rectangle(Frame),
    Ellipse(Frame),
    Text(TextNote),
    Image(ImageMark),
}

/// A pen stroke. `points` holds `x, y, pressure` triples, pressure from 0 to 1.
/// Input without pressure (a mouse, a finger, a pen WebKitGTK reports as a
/// mouse) records 0.5 and sets `simulate_pressure`, so the stroke is drawn from
/// its speed instead, the same way every time.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InkStroke {
    pub points: Vec<f64>,
    pub color: String,
    pub stroke_width: f64,
    pub simulate_pressure: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// A translucent marker. A free stroke has `x, y` pairs in `points`; a
/// highlight of the PDF's own text has empty `points`, one `x, y, width,
/// height` box per line in `rects`, and the words it covers in `text`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HighlightStroke {
    pub points: Vec<f64>,
    pub color: String,
    pub stroke_width: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rects: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// A straight line or an arrow from (x1, y1) to (x2, y2); an arrow's head is at
/// the second point, and at the first too when `both_ends` is set.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Segment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub color: String,
    pub stroke_width: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash: Option<LineDash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub both_ends: Option<bool>,
    /// A curve: the two control points `[x1, y1, x2, y2]` of a cubic Bézier
    /// from the first end to the second.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// The outline of a rectangle or of the ellipse inside it, optionally filled
/// with `fill`. A filled frame may drop its outline with `border: false`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub color: String,
    pub stroke_width: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash: Option<LineDash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// How a line or outline is broken up; a solid line has none. Dashes and dots
/// are spaced by the line's width.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LineDash {
    Dashed,
    Dotted,
}

/// A picture placed on a notebook page: a managed image asset drawn into the
/// box at (x, y) of `width` by `height`. Only notebook pages hold images; the
/// notebook records which asset each one uses so the asset store keeps it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageMark {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub asset_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// A picture clipped from a document's page names that page, so the
    /// notebook links back to it and the page lists the notes it is in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ClipSource>,
}

/// The document page a clipped picture came from. It names the page rather
/// than copying anything about it, so deleting the document never rewrites a
/// notebook.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipSource {
    pub document_id: String,
    pub page_number: i64,
}

/// Typed text whose lines wrap at `width`; its height follows the text. A
/// sticky note (`sticky: true`) is drawn as a small note of `width` showing
/// the start of its text, opens to all of it, and may hold longer text;
/// `color` is then the note's paper. A sticky note may be `collapsed: true`,
/// shown as a small icon that keeps all of its text.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextNote {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub text: String,
    pub color: String,
    pub font_size: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sticky: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collapsed: Option<bool>,
    /// Only on a notebook page: the note is a link, and `text` is its label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<NoteLink>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// Where a link on a notebook page goes. `button` shows it as a button that
/// does the thing (a deck's Study starts Practice) rather than a small link.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NoteLink {
    pub target: LinkTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub button: Option<bool>,
}

/// What a link names. Targets are looked up when a notebook opens, so a link
/// whose target was deleted stays on the page and opens nothing.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum LinkTarget {
    #[serde(rename_all = "camelCase")]
    DocumentPage {
        document_id: String,
        page_number: i64,
    },
    #[serde(rename_all = "camelCase")]
    StudyGuide { document_id: String },
    #[serde(rename_all = "camelCase")]
    Deck { deck_id: String },
    #[serde(rename_all = "camelCase")]
    NotebookPage {
        notebook_id: String,
        page_id: String,
    },
}

/// A link's label is one short line.
pub(crate) const MAX_LINK_LABEL_CHARS: usize = 200;

impl AnnotationShape {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Ink(_) => "ink",
            Self::Highlight(_) => "highlight",
            Self::Line(_) => "line",
            Self::Arrow(_) => "arrow",
            Self::Rectangle(_) => "rectangle",
            Self::Ellipse(_) => "ellipse",
            Self::Text(_) => "text",
            Self::Image(_) => "image",
        }
    }

    /// The group the mark belongs to, if any.
    pub(crate) fn group(&self) -> Option<&str> {
        match self {
            Self::Ink(stroke) => stroke.group.as_deref(),
            Self::Highlight(stroke) => stroke.group.as_deref(),
            Self::Line(segment) | Self::Arrow(segment) => segment.group.as_deref(),
            Self::Rectangle(frame) | Self::Ellipse(frame) => frame.group.as_deref(),
            Self::Text(note) => note.group.as_deref(),
            Self::Image(image) => image.group.as_deref(),
        }
    }

    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        if let Some(group) = self.group() {
            if !is_identifier(group) {
                return Err(invalid("A group is named by a 32-character identifier."));
            }
        }
        self.validate_shape()
    }

    fn validate_shape(&self) -> Result<(), CommandError> {
        match self {
            Self::Ink(stroke) => {
                validate_points(&stroke.points, 3)?;
                for triple in stroke.points.chunks_exact(3) {
                    if !(0.0..=1.0).contains(&triple[2]) {
                        return Err(invalid("Pen pressure must be between 0 and 1."));
                    }
                }
                validate_color(&stroke.color)?;
                validate_stroke_width(stroke.stroke_width)
            }
            Self::Highlight(stroke) => {
                match (&stroke.rects, &stroke.text) {
                    (None, None) => validate_points(&stroke.points, 2)?,
                    (Some(rects), Some(text)) if stroke.points.is_empty() => {
                        validate_text_rects(rects)?;
                        validate_quoted_text(text)?;
                    }
                    _ => {
                        return Err(invalid(
                            "A highlight is either a free stroke or boxes over the page's text.",
                        ))
                    }
                }
                validate_color(&stroke.color)?;
                validate_stroke_width(stroke.stroke_width)
            }
            Self::Line(segment) | Self::Arrow(segment) => {
                for value in [segment.x1, segment.y1, segment.x2, segment.y2] {
                    validate_coordinate(value)?;
                }
                if segment.x1 == segment.x2 && segment.y1 == segment.y2 {
                    return Err(invalid("A line needs two different end points."));
                }
                validate_color(&segment.color)?;
                validate_stroke_width(segment.stroke_width)?;
                if let Some(curve) = &segment.curve {
                    if curve.len() != 4 {
                        return Err(invalid("A curve has two control points."));
                    }
                    for value in curve {
                        validate_coordinate(*value)?;
                    }
                }
                match segment.both_ends {
                    None => Ok(()),
                    Some(true) if matches!(self, Self::Arrow(_)) => Ok(()),
                    Some(true) => Err(invalid("Only an arrow can have a head at both ends.")),
                    Some(false) => Err(invalid("A one-headed arrow leaves bothEnds out.")),
                }
            }
            Self::Rectangle(frame) | Self::Ellipse(frame) => {
                validate_coordinate(frame.x)?;
                validate_coordinate(frame.y)?;
                validate_extent(frame.x, frame.width)?;
                validate_extent(frame.y, frame.height)?;
                validate_color(&frame.color)?;
                validate_stroke_width(frame.stroke_width)?;
                if let Some(fill) = &frame.fill {
                    validate_color(fill)?;
                }
                match frame.border {
                    None => Ok(()),
                    Some(false) if frame.fill.is_some() => Ok(()),
                    Some(false) => Err(invalid("A shape without a border needs a fill.")),
                    Some(true) => Err(invalid("A shape with a border leaves border out.")),
                }
            }
            Self::Image(image) => {
                validate_coordinate(image.x)?;
                validate_coordinate(image.y)?;
                validate_extent(image.x, image.width)?;
                validate_extent(image.y, image.height)?;
                if !is_identifier(&image.asset_id) {
                    return Err(invalid("A picture names its image by its identifier."));
                }
                match &image.source {
                    Some(source)
                        if !is_identifier(&source.document_id) || source.page_number < 1 =>
                    {
                        Err(invalid("A clip names the document page it came from."))
                    }
                    _ => Ok(()),
                }
            }
            Self::Text(note) => {
                validate_coordinate(note.x)?;
                validate_coordinate(note.y)?;
                validate_extent(note.x, note.width)?;
                if note.text.trim().is_empty() {
                    return Err(invalid("A text note needs some text."));
                }
                let limit = match note.sticky {
                    None if note.link.is_some() => MAX_LINK_LABEL_CHARS,
                    None => MAX_TEXT_CHARS,
                    Some(true) if note.link.is_some() => {
                        return Err(invalid("A sticky note cannot also be a link."))
                    }
                    Some(true) => MAX_STICKY_CHARS,
                    Some(false) => {
                        return Err(invalid(
                            "A text note is either a sticky note or not marked at all.",
                        ))
                    }
                };
                match (note.collapsed, note.sticky) {
                    (None, _) | (Some(true), Some(true)) => {}
                    _ => {
                        return Err(invalid(
                            "Only a sticky note can be collapsed, and only marked when it is.",
                        ))
                    }
                }
                if note.text.chars().count() > limit {
                    return Err(invalid(format!(
                        "A text note can hold up to {limit} characters."
                    )));
                }
                if note
                    .text
                    .chars()
                    .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
                {
                    return Err(invalid("A text note cannot contain control characters."));
                }
                if let Some(link) = &note.link {
                    link.validate()?;
                }
                validate_color(&note.color)?;
                if !note.font_size.is_finite()
                    || !(MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&note.font_size)
                {
                    return Err(invalid(format!(
                        "Text size must be between {MIN_FONT_SIZE} and {MAX_FONT_SIZE} points."
                    )));
                }
                Ok(())
            }
        }
    }

    /// The stored payload: the shape's own fields, without its kind.
    pub(crate) fn payload_json(&self) -> Result<String, CommandError> {
        let json = match self {
            Self::Ink(stroke) => serde_json::to_string(stroke),
            Self::Highlight(stroke) => serde_json::to_string(stroke),
            Self::Line(segment) | Self::Arrow(segment) => serde_json::to_string(segment),
            Self::Rectangle(frame) | Self::Ellipse(frame) => serde_json::to_string(frame),
            Self::Text(note) => serde_json::to_string(note),
            Self::Image(image) => serde_json::to_string(image),
        }
        .map_err(CommandError::serialization)?;
        if json.len() > MAX_PAYLOAD_BYTES {
            return Err(invalid(
                "This annotation is too large to save. Split it into smaller strokes.",
            ));
        }
        Ok(json)
    }

    pub(crate) fn from_stored(kind: &str, payload: &str) -> Result<Self, serde_json::Error> {
        Ok(match kind {
            "ink" => Self::Ink(serde_json::from_str(payload)?),
            "highlight" => Self::Highlight(serde_json::from_str(payload)?),
            "line" => Self::Line(serde_json::from_str(payload)?),
            "arrow" => Self::Arrow(serde_json::from_str(payload)?),
            "rectangle" => Self::Rectangle(serde_json::from_str(payload)?),
            "ellipse" => Self::Ellipse(serde_json::from_str(payload)?),
            "text" => Self::Text(serde_json::from_str(payload)?),
            "image" => Self::Image(serde_json::from_str(payload)?),
            other => {
                return Err(serde::de::Error::custom(format!(
                    "unknown annotation kind {other}"
                )))
            }
        })
    }
}

impl AnnotationShape {
    /// Whether this is a link, which only a notebook page holds.
    pub(crate) fn is_link(&self) -> bool {
        matches!(self, Self::Text(TextNote { link: Some(_), .. }))
    }

    /// The managed asset a picture shows; only notebook pages hold pictures.
    pub(crate) fn image_asset(&self) -> Option<&str> {
        match self {
            Self::Image(image) => Some(&image.asset_id),
            _ => None,
        }
    }
}

impl NoteLink {
    fn validate(&self) -> Result<(), CommandError> {
        if self.button == Some(false) {
            return Err(invalid("A link is either a button or not marked at all."));
        }
        let ids: Vec<&str> = match &self.target {
            LinkTarget::DocumentPage {
                document_id,
                page_number,
            } => {
                if *page_number < 1 {
                    return Err(invalid("A link to a page needs its page number."));
                }
                vec![document_id]
            }
            LinkTarget::StudyGuide { document_id } => vec![document_id],
            LinkTarget::Deck { deck_id } => vec![deck_id],
            LinkTarget::NotebookPage {
                notebook_id,
                page_id,
            } => vec![notebook_id, page_id],
        };
        for id in ids {
            validate_entity_id(id, "link target")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentAnnotationDto {
    pub id: String,
    pub page_number: i64,
    pub shape: AnnotationShape,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// One page the annotator can show.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotatedPageDto {
    pub page_number: i64,
    /// The page image, read through `read_document_source_asset`.
    pub asset_id: String,
    /// The page size in PDF points.
    pub width: f64,
    pub height: f64,
    /// False when the page image could not be measured: the page is laid out
    /// at a default size and its annotations still show.
    pub image_available: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentAnnotationsDto {
    pub document_id: String,
    pub pages: Vec<AnnotatedPageDto>,
    /// In drawing order on each page: earlier annotations first.
    pub annotations: Vec<DocumentAnnotationDto>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedDocumentAnnotationChangesDto {
    /// The stored state of every annotation the request put.
    pub annotations: Vec<DocumentAnnotationDto>,
    pub deleted_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetDocumentAnnotationsInput {
    pub document_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyDocumentAnnotationChangesInput {
    pub document_id: String,
    pub changes: Vec<AnnotationChange>,
}

/// The new state of one annotation. A put creates the annotation or replaces
/// all of it; a delete of an annotation that is already gone succeeds.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub enum AnnotationChange {
    #[serde(rename_all = "camelCase")]
    Put {
        id: String,
        page_number: i64,
        shape: AnnotationShape,
    },
    Delete {
        id: String,
    },
}

impl AnnotationChange {
    fn id(&self) -> &str {
        match self {
            Self::Put { id, .. } | Self::Delete { id } => id,
        }
    }
}

/// A page render as the library records it.
#[derive(Debug)]
struct PageRender {
    page_number: i64,
    asset_id: String,
    relative_path: String,
    width_px: Option<i64>,
    height_px: Option<i64>,
}

#[tauri::command]
pub fn get_document_annotations(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: GetDocumentAnnotationsInput,
) -> Result<DocumentAnnotationsDto, CommandError> {
    load_document_annotations(database.inner(), paths.inner(), input.document_id)
}

#[tauri::command]
pub fn apply_document_annotation_changes(
    database: State<'_, Database>,
    input: ApplyDocumentAnnotationChangesInput,
) -> Result<AppliedDocumentAnnotationChangesDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| apply_changes(connection, input, now))
}

pub(crate) fn load_document_annotations(
    database: &Database,
    paths: &AppPaths,
    document_id: String,
) -> Result<DocumentAnnotationsDto, CommandError> {
    validate_entity_id(&document_id, "document")?;
    let (renders, annotations) = database.with_connection(|connection| {
        require_document(connection, &document_id)?;
        Ok((
            page_renders(connection, &document_id)?,
            load_annotations(connection, &document_id)?,
        ))
    })?;
    Ok(DocumentAnnotationsDto {
        document_id,
        pages: measure_pages(paths, renders),
        annotations,
    })
}

pub(crate) fn apply_changes(
    connection: &mut Connection,
    input: ApplyDocumentAnnotationChangesInput,
    now: i64,
) -> Result<AppliedDocumentAnnotationChangesDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    if input.changes.is_empty() || input.changes.len() > MAX_CHANGES_PER_REQUEST {
        return Err(invalid(format!(
            "Save between 1 and {MAX_CHANGES_PER_REQUEST} annotation changes at a time."
        )));
    }
    let mut ids = BTreeSet::new();
    for change in &input.changes {
        validate_annotation_id(change.id())?;
        if !ids.insert(change.id()) {
            return Err(invalid("A request changes the same annotation twice."));
        }
        if let AnnotationChange::Put {
            page_number, shape, ..
        } = change
        {
            if *page_number < 1 {
                return Err(page_missing());
            }
            shape.validate()?;
            if shape.is_link() {
                return Err(invalid("Links go on notebook pages, not on a document."));
            }
            if shape.image_asset().is_some() {
                return Err(invalid("Pictures go on notebook pages, not on a document."));
            }
        }
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    require_document(&transaction, &input.document_id)?;
    let pages = page_renders(&transaction, &input.document_id)?
        .into_iter()
        .map(|render| render.page_number)
        .collect::<BTreeSet<_>>();
    // New annotations draw on top of the document's existing ones, in request
    // order, even when they share a request or the clock has gone backwards.
    let latest_created_at: Option<i64> = transaction
        .query_row(
            "SELECT MAX(created_at) FROM document_annotations WHERE document_id = ?1",
            [&input.document_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let mut next_created_at = latest_created_at.map_or(now, |latest| now.max(latest + 1));
    let mut annotations = Vec::new();
    let mut deleted_ids = Vec::new();
    for change in input.changes {
        match change {
            AnnotationChange::Put {
                id,
                page_number,
                shape,
            } => {
                if !pages.contains(&page_number) {
                    return Err(page_missing());
                }
                annotations.push(put_annotation(
                    &transaction,
                    &input.document_id,
                    id,
                    page_number,
                    shape,
                    now,
                    &mut next_created_at,
                )?);
            }
            AnnotationChange::Delete { id } => {
                delete_annotation(&transaction, &input.document_id, &id)?;
                deleted_ids.push(id);
            }
        }
    }
    let count: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM document_annotations WHERE document_id = ?1",
            [&input.document_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if count > MAX_ANNOTATIONS_PER_DOCUMENT {
        return Err(CommandError::new(
            "document_annotation_limit",
            format!(
                "A document can hold up to {MAX_ANNOTATIONS_PER_DOCUMENT} annotations. Erase some before adding more."
            ),
        ));
    }
    transaction.commit().map_err(CommandError::database)?;
    Ok(AppliedDocumentAnnotationChangesDto {
        annotations,
        deleted_ids,
    })
}

/// Create the annotation, replace it when its content differs, or leave an
/// identical one (and its revision) as it is. A new annotation takes
/// `next_created_at`, which then moves on, so annotations keep the order they
/// were drawn in.
fn put_annotation(
    connection: &Connection,
    document_id: &str,
    id: String,
    page_number: i64,
    shape: AnnotationShape,
    now: i64,
    next_created_at: &mut i64,
) -> Result<DocumentAnnotationDto, CommandError> {
    let kind = shape.kind();
    let payload = shape.payload_json()?;
    let existing = connection
        .query_row(
            "SELECT document_id, page_number, kind, payload_json, revision, created_at, updated_at
             FROM document_annotations WHERE id = ?1",
            [&id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((owner, stored_page, stored_kind, stored_payload, revision, created_at, updated_at)) =
        existing
    else {
        let created_at = *next_created_at;
        *next_created_at += 1;
        connection
            .execute(
                "INSERT INTO document_annotations(
                     id, document_id, page_number, kind, payload_json,
                     revision, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)",
                params![id, document_id, page_number, kind, payload, created_at],
            )
            .map_err(CommandError::database)?;
        return Ok(DocumentAnnotationDto {
            id,
            page_number,
            shape,
            revision: 1,
            created_at,
            updated_at: created_at,
        });
    };
    if owner != document_id {
        return Err(other_document());
    }
    if stored_page == page_number && stored_kind == kind && stored_payload == payload {
        return Ok(DocumentAnnotationDto {
            id,
            page_number,
            shape,
            revision,
            created_at,
            updated_at,
        });
    }
    // A clock that went backwards must not break `updated_at >= created_at`.
    let updated_at = now.max(created_at);
    connection
        .execute(
            "UPDATE document_annotations
             SET page_number = ?2, kind = ?3, payload_json = ?4,
                 revision = revision + 1, updated_at = ?5
             WHERE id = ?1",
            params![id, page_number, kind, payload, updated_at],
        )
        .map_err(CommandError::database)?;
    Ok(DocumentAnnotationDto {
        id,
        page_number,
        shape,
        revision: revision + 1,
        created_at,
        updated_at,
    })
}

fn delete_annotation(
    connection: &Connection,
    document_id: &str,
    id: &str,
) -> Result<(), CommandError> {
    let owner: Option<String> = connection
        .query_row(
            "SELECT document_id FROM document_annotations WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    match owner {
        None => Ok(()),
        Some(owner) if owner != document_id => Err(other_document()),
        Some(_) => connection
            .execute("DELETE FROM document_annotations WHERE id = ?1", [id])
            .map(|_| ())
            .map_err(CommandError::database),
    }
}

fn require_document(connection: &Connection, document_id: &str) -> Result<(), CommandError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM documents WHERE id = ?1)",
            [document_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if exists {
        Ok(())
    } else {
        Err(CommandError::new(
            "document_not_found",
            "That document is no longer in the library.",
        ))
    }
}

fn page_renders(
    connection: &Connection,
    document_id: &str,
) -> Result<Vec<PageRender>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT source_unit_number, asset_id, relative_path, width, height
             FROM document_source_assets
             WHERE document_id = ?1 AND role = 'unit_render'
               AND media_type = 'image/png' AND source_unit_number IS NOT NULL
             ORDER BY source_unit_number, asset_id",
        )
        .map_err(CommandError::database)?;
    let mut renders = statement
        .query_map([document_id], |row| {
            Ok(PageRender {
                page_number: row.get(0)?,
                asset_id: row.get(1)?,
                relative_path: row.get(2)?,
                width_px: row.get(3)?,
                height_px: row.get(4)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    renders.dedup_by_key(|render| render.page_number);
    Ok(renders)
}

fn load_annotations(
    connection: &Connection,
    document_id: &str,
) -> Result<Vec<DocumentAnnotationDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, page_number, kind, payload_json, revision, created_at, updated_at
             FROM document_annotations
             WHERE document_id = ?1
             ORDER BY page_number, created_at, id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([document_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut annotations = Vec::with_capacity(rows.len());
    for (id, page_number, kind, payload, revision, created_at, updated_at) in rows {
        // Every write is validated, so a row that no longer parses was changed
        // outside oStudy. Skipping it keeps the rest of the document usable.
        match AnnotationShape::from_stored(&kind, &payload) {
            Ok(shape) => annotations.push(DocumentAnnotationDto {
                id,
                page_number,
                shape,
                revision,
                created_at,
                updated_at,
            }),
            Err(error) => crate::logging::warn_target(
                "documents.annotations",
                "document_annotation_unreadable",
                "A stored document annotation could not be read and was skipped.",
                serde_json::json!({ "annotationId": id, "error": error.to_string() }),
            ),
        }
    }
    Ok(annotations)
}

/// Page sizes in points. Ingestion records no pixel size for PDF page renders,
/// so each PNG header is read; that happens after the database lock is released.
fn measure_pages(paths: &AppPaths, renders: Vec<PageRender>) -> Vec<AnnotatedPageDto> {
    let mut last_size = FALLBACK_PAGE_SIZE;
    renders
        .into_iter()
        .map(|render| {
            let pixels = match (render.width_px, render.height_px) {
                (Some(width), Some(height)) if width > 0 && height > 0 => {
                    u32::try_from(width).ok().zip(u32::try_from(height).ok())
                }
                _ => image::image_dimensions(paths.resolve_relative(&render.relative_path)).ok(),
            };
            let size = pixels.map(|(width, height)| {
                (
                    render_pixels_to_points(width),
                    render_pixels_to_points(height),
                )
            });
            if let Some(size) = size {
                last_size = size;
            }
            let (width, height) = size.unwrap_or(last_size);
            AnnotatedPageDto {
                page_number: render.page_number,
                asset_id: render.asset_id,
                width,
                height,
                image_available: size.is_some(),
            }
        })
        .collect()
}

fn is_identifier(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn validate_annotation_id(id: &str) -> Result<(), CommandError> {
    if is_identifier(id) {
        Ok(())
    } else {
        Err(CommandError::new(
            "invalid_identifier",
            "The annotation identifier is invalid.",
        ))
    }
}

fn validate_points(points: &[f64], stride: usize) -> Result<(), CommandError> {
    if points.is_empty() || !points.len().is_multiple_of(stride) {
        return Err(invalid("A stroke's points are incomplete."));
    }
    if points.len() / stride > MAX_STROKE_POINTS {
        return Err(invalid(format!(
            "A stroke can have up to {MAX_STROKE_POINTS} points."
        )));
    }
    for point in points.chunks_exact(stride) {
        validate_coordinate(point[0])?;
        validate_coordinate(point[1])?;
    }
    Ok(())
}

fn validate_text_rects(rects: &[f64]) -> Result<(), CommandError> {
    if rects.is_empty() || !rects.len().is_multiple_of(4) {
        return Err(invalid("A text highlight's boxes are incomplete."));
    }
    if rects.len() / 4 > MAX_TEXT_RECTS {
        return Err(invalid(format!(
            "A text highlight can cover up to {MAX_TEXT_RECTS} lines."
        )));
    }
    for rect in rects.chunks_exact(4) {
        validate_coordinate(rect[0])?;
        validate_coordinate(rect[1])?;
        validate_extent(rect[0], rect[2])?;
        validate_extent(rect[1], rect[3])?;
    }
    Ok(())
}

/// The words a text highlight covers, as one line of text.
fn validate_quoted_text(text: &str) -> Result<(), CommandError> {
    if text.trim().is_empty() {
        return Err(invalid("A text highlight needs the text it covers."));
    }
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(invalid(format!(
            "A text highlight can cover up to {MAX_TEXT_CHARS} characters."
        )));
    }
    if text.chars().any(char::is_control) {
        return Err(invalid(
            "A text highlight's text cannot contain control characters.",
        ));
    }
    Ok(())
}

fn validate_coordinate(value: f64) -> Result<(), CommandError> {
    if value.is_finite() && (0.0..=MAX_COORDINATE).contains(&value) {
        Ok(())
    } else {
        Err(invalid("An annotation lies outside its page."))
    }
}

/// A box side: positive, and ending inside the largest page.
fn validate_extent(start: f64, length: f64) -> Result<(), CommandError> {
    if length.is_finite() && length > 0.0 && start + length <= MAX_COORDINATE {
        Ok(())
    } else {
        Err(invalid(
            "An annotation box needs a positive size inside its page.",
        ))
    }
}

fn validate_stroke_width(width: f64) -> Result<(), CommandError> {
    if width.is_finite() && (MIN_STROKE_WIDTH..=MAX_STROKE_WIDTH).contains(&width) {
        Ok(())
    } else {
        Err(invalid(format!(
            "Line width must be between {MIN_STROKE_WIDTH} and {MAX_STROKE_WIDTH} points."
        )))
    }
}

fn validate_color(color: &str) -> Result<(), CommandError> {
    let bytes = color.as_bytes();
    if bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..]
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(invalid("Annotation colors are written as #rrggbb."))
    }
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::new("document_annotation_invalid", message)
}

fn page_missing() -> CommandError {
    CommandError::new(
        "document_annotation_page_missing",
        "That page is not part of this document.",
    )
}

fn other_document() -> CommandError {
    CommandError::new(
        "document_annotation_conflict",
        "That annotation belongs to another document.",
    )
}

#[path = "document_annotations/page_tiles.rs"]
mod page_tiles;
pub(crate) use page_tiles::{
    pdf_source_path, render_document_page_tile, RenderDocumentPageTileInput,
};

#[path = "document_annotations/page_text.rs"]
mod page_text;
pub(crate) use page_text::{get_document_page_text, DocumentPageTextDto, GetDocumentPageTextInput};

#[path = "document_annotations/pdf_export.rs"]
mod pdf_export;
pub(crate) use pdf_export::{notebook_pdf, NotebookPdfPage, Ruling};

#[path = "document_annotations/annotated_pdf.rs"]
mod annotated_pdf;
pub(crate) use annotated_pdf::{
    prepare_annotated_pdf, PrepareAnnotatedPdfInput, PreparedAnnotatedPdfDto,
};

#[cfg(test)]
#[path = "document_annotations/tests.rs"]
mod tests;
