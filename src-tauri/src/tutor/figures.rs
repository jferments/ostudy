//! Document figures a Tutor answer may show.
//!
//! An answer about pages that hold a figure oStudy has read and judged worth
//! studying is offered that figure, and the model may place it in the answer
//! as `![title](figure:<document id>/<figure id>)` where it helps. Most answers show none: only
//! figures on the retrieved pages, or explained there, are offered, and only
//! those a learner needs or that illustrate the passage. The reference lives in
//! the saved answer text; the figure itself is drawn from the PDF when the
//! answer is read, as the Figures page draws it, so nothing new is stored.

use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    database::Database,
    document_domain::SourceUnitKind,
    document_figures::{self, DocumentFigureDto, FigureBoxDto},
    error::CommandError,
    knowledge::contracts::{SourceKind, SourceRecord},
    util::validate_entity_id,
};

/// More would crowd a chat answer; Jay asked for images only when they help.
pub(crate) const MAX_SHOWN: usize = 2;
/// Offered at most, best first, so the request stays small.
const MAX_OFFERED: usize = 6;
/// A figure read as a helpful illustration (3) or more.
const MIN_IMPORTANCE: i64 = 3;
const SCHEME: &str = "figure:";

/// A figure offered to the model.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OfferedFigure {
    pub document_id: String,
    pub figure_id: String,
    pub title: String,
    pub shows: String,
    pub page_number: i64,
    importance: i64,
}

impl OfferedFigure {
    /// What an answer's image names: figure ids are only unique within
    /// their document.
    pub(crate) fn reference(&self) -> String {
        format!("{}/{}", self.document_id, self.figure_id)
    }

    pub(crate) fn prompt(&self) -> Value {
        json!({
            "figure": self.reference(),
            "title": self.title,
            "shows": self.shows,
            "page": self.page_number,
        })
    }
}

/// The read figures, worth studying, on or explained on the pages the
/// sources come from, most important first.
pub(crate) fn offered(
    connection: &Connection,
    sources: &[SourceRecord],
) -> Result<Vec<OfferedFigure>, CommandError> {
    let mut pages_by_document: Vec<(String, HashSet<i64>)> = Vec::new();
    for source in sources {
        if source.kind != SourceKind::Document {
            continue;
        }
        let (Some(document_id), Some(locator)) = (&source.document_id, &source.locator) else {
            continue;
        };
        if !matches!(
            locator.unit_kind,
            SourceUnitKind::Page | SourceUnitKind::Slide
        ) {
            continue;
        }
        match pages_by_document
            .iter_mut()
            .find(|(document, _)| document == document_id)
        {
            Some((_, pages)) => pages.extend(&locator.unit_numbers),
            None => pages_by_document.push((
                document_id.clone(),
                locator.unit_numbers.iter().copied().collect(),
            )),
        }
    }
    offered_on(connection, pages_by_document)
}

/// The figures offered for these pages of each document.
fn offered_on(
    connection: &Connection,
    pages_by_document: Vec<(String, HashSet<i64>)>,
) -> Result<Vec<OfferedFigure>, CommandError> {
    let mut offered = Vec::new();
    let mut seen = HashSet::new();
    for (document_id, pages) in pages_by_document {
        let document = document_figures::figures(connection, &document_id)?;
        if !document.use_images {
            continue;
        }
        for figure in document.figures {
            let reference = format!("{document_id}/{}", figure.figure_id);
            let Some(reading) = figure.reading.as_ref() else {
                continue;
            };
            let belongs = pages.contains(&figure.page_number)
                || reading.explained_on.iter().any(|page| pages.contains(page));
            if figure.hidden
                || !reading.worth_studying
                || reading.importance < MIN_IMPORTANCE
                || !belongs
                || !seen.insert(reference)
            {
                continue;
            }
            offered.push(OfferedFigure {
                title: reading.title.clone(),
                shows: reading.shows.chars().take(400).collect(),
                page_number: figure.page_number,
                importance: reading.importance,
                document_id: document_id.clone(),
                figure_id: figure.figure_id,
            });
        }
    }
    offered.sort_by_key(|figure| std::cmp::Reverse(figure.importance));
    offered.truncate(MAX_OFFERED);
    Ok(offered)
}

/// The answer with every figure image it may keep: offered figures, each
/// once, at most [`MAX_SHOWN`]. Any other image is left out, and with no
/// figures allowed every figure image is.
pub(crate) fn keep_offered(text: &str, offered: &[OfferedFigure]) -> String {
    let allowed: HashSet<String> = offered.iter().map(OfferedFigure::reference).collect();
    let mut shown = HashSet::new();
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("![") {
        out.push_str(&rest[..start]);
        let image = &rest[start..];
        let Some((length, address)) = image_at(image) else {
            out.push_str("![");
            rest = &image[2..];
            continue;
        };
        match address.strip_prefix(SCHEME) {
            Some(reference)
                if allowed.contains(reference)
                    && shown.len() < MAX_SHOWN
                    && shown.insert(reference.to_string()) =>
            {
                out.push_str(&image[..length]);
            }
            // A figure not offered, a repeat, or one too many is dropped.
            Some(_) => {}
            // Other images stay as written; the reader shows their words only.
            None => out.push_str(&image[..length]),
        }
        rest = &image[length..];
    }
    out.push_str(rest);
    out
}

/// `![words](address)` at the start of `text`: its length and address.
fn image_at(text: &str) -> Option<(usize, &str)> {
    let close = text.find("](")?;
    if text[2..close].contains(['\n', '[', ']']) {
        return None;
    }
    let after = close + 2;
    let end = after + text[after..].find(')')?;
    let address = text[after..end].trim();
    if address.contains(char::is_whitespace) {
        return None;
    }
    Some((end + 1, address))
}

/// The figures a saved answer shows, as `<document id>/<figure id>`, in order.
pub(crate) fn shown_in(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("![") {
        let image = &rest[start..];
        match image_at(image) {
            Some((length, address)) => {
                if let Some(id) = address.strip_prefix(SCHEME) {
                    ids.push(id.to_string());
                }
                rest = &image[length..];
            }
            None => rest = &image[2..],
        }
    }
    ids
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorFigureInput {
    pub turn_id: String,
    pub document_id: String,
    pub figure_id: String,
}

/// Where a figure an answer shows sits, for the reader to draw it from its
/// document's page.
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorFigureDto {
    pub document_id: String,
    pub figure_id: String,
    pub title: String,
    pub page_number: i64,
    pub page_width: f64,
    pub page_height: f64,
    pub bounds: FigureBoxDto,
}

/// A figure that a finished answer shows, while the learner keeps it: a
/// figure hidden since, or a document whose images were turned off, is not
/// drawn.
pub(crate) fn tutor_figure(
    database: &Database,
    input: TutorFigureInput,
) -> Result<TutorFigureDto, CommandError> {
    validate_entity_id(&input.turn_id, "Tutor reply")?;
    validate_entity_id(&input.document_id, "Document")?;
    document_figures::validate_figure_id(&input.figure_id)?;
    database.with_connection(|connection| figure_for(connection, input))
}

fn figure_for(
    connection: &Connection,
    input: TutorFigureInput,
) -> Result<TutorFigureDto, CommandError> {
    let reference = format!("{}/{}", input.document_id, input.figure_id);
    let missing = || {
        CommandError::new(
            "tutor_figure_missing",
            "This figure is no longer available.",
        )
    };
    let text: Option<String> = connection
        .query_row(
            "SELECT assistant_text FROM tutor_turns WHERE id = ?1 AND state = 'complete'",
            params![input.turn_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    if !text.is_some_and(|text| shown_in(&text).contains(&reference)) {
        return Err(missing());
    }
    let document = document_figures::figures(connection, &input.document_id)?;
    if !document.use_images {
        return Err(missing());
    }
    let figure: DocumentFigureDto = document
        .figures
        .into_iter()
        .find(|figure| figure.figure_id == input.figure_id && !figure.hidden)
        .ok_or_else(missing)?;
    Ok(TutorFigureDto {
        document_id: input.document_id,
        title: figure
            .reading
            .map(|reading| reading.title)
            .or(figure.label)
            .unwrap_or_default(),
        figure_id: figure.figure_id,
        page_number: figure.page_number,
        page_width: figure.page_width,
        page_height: figure.page_height,
        bounds: figure.bounds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ai::document_intelligence::FigureReading, document_figures::detect};

    const DOCUMENT: &str = "0123456789abcdef0123456789abcdef";

    fn figure(id: &str) -> OfferedFigure {
        OfferedFigure {
            document_id: DOCUMENT.to_string(),
            figure_id: id.to_string(),
            title: "Nephron".to_string(),
            shows: "A nephron".to_string(),
            page_number: 3,
            importance: 4,
        }
    }

    #[test]
    fn only_offered_figures_are_kept_each_once_and_at_most_two() {
        let offered = [
            figure("p00003-01"),
            figure("p00003-02"),
            figure("p00004-01"),
        ];
        let [a, b, c] = offered.clone().map(|figure| figure.reference());
        // The same figure id in another document was not offered.
        let other = format!("{}/p00003-01", "f".repeat(32));
        let text = format!(
            "Look:\n\n![Nephron](figure:{a})\n\nAgain ![x](figure:{a}) and ![y](figure:{other}) \
             then ![B](figure:{b}) ![C](figure:{c}) and ![photo](https://example.com/p.png)."
        );
        let kept = keep_offered(&text, &offered);
        assert_eq!(
            kept,
            format!(
                "Look:\n\n![Nephron](figure:{a})\n\nAgain  and  then ![B](figure:{b})  and \
                 ![photo](https://example.com/p.png)."
            )
        );
        assert_eq!(shown_in(&kept), vec![a, b]);
    }

    /// A library with a three-page document whose page 1 holds two figures:
    /// one read as worth studying and one read as incidental.
    fn library() -> Connection {
        let mut connection = crate::database::subjects_fixture::release_library(
            crate::database::CURRENT_SCHEMA_VERSION,
        );
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .unwrap();
        crate::collections::test_support::seed_document(&connection, DOCUMENT, 3, 'a').unwrap();
        let found = |figure_id: &str, left: f64| detect::DetectedFigure {
            figure_id: figure_id.to_string(),
            page_number: 1,
            page_width: 612.0,
            page_height: 792.0,
            kind: detect::FigureKind::Image,
            bounds: detect::Rect::from_bounds([left, 100.0, 200.0, 200.0]),
            caption: None,
            passages: Vec::new(),
        };
        document_figures::replace_figures(
            &mut connection,
            DOCUMENT,
            &[found("p00001-01", 72.0), found("p00001-02", 320.0)],
            5,
        )
        .unwrap();
        let read = |figure_id: &str, importance: i64, worth_studying: bool| FigureReading {
            figure_id: figure_id.to_string(),
            title: format!("Figure {figure_id}"),
            shows: "The nephron's loop.".to_string(),
            use_in_text: None,
            concepts: Vec::new(),
            explained_on: vec![2],
            importance,
            worth_studying,
        };
        document_figures::reading::store_readings(
            &mut connection,
            DOCUMENT,
            &[read("p00001-01", 4, true), read("p00001-02", 2, false)],
            "openrouter",
            "vision",
            "read",
            6,
        )
        .unwrap();
        connection
    }

    fn pages(pages: &[i64]) -> Vec<(String, HashSet<i64>)> {
        vec![(DOCUMENT.to_string(), pages.iter().copied().collect())]
    }

    fn hide(connection: &mut Connection, figure_id: &str) {
        document_figures::set_hidden(
            connection,
            document_figures::SetDocumentFigureHiddenInput {
                document_id: DOCUMENT.to_string(),
                figure_id: figure_id.to_string(),
                hidden: true,
            },
            7,
        )
        .unwrap();
    }

    fn offered_ids(connection: &Connection, pages_seen: &[i64]) -> Vec<String> {
        offered_on(connection, pages(pages_seen))
            .unwrap()
            .into_iter()
            .map(|figure| figure.figure_id)
            .collect()
    }

    #[test]
    fn only_worthwhile_figures_on_or_explained_on_the_pages_are_offered() {
        let mut connection = library();
        assert_eq!(offered_ids(&connection, &[1]), vec!["p00001-01"]);
        // Page 2's text explains it.
        assert_eq!(offered_ids(&connection, &[2]), vec!["p00001-01"]);
        assert!(offered_ids(&connection, &[3]).is_empty());
        hide(&mut connection, "p00001-01");
        assert!(offered_ids(&connection, &[1]).is_empty());
    }

    #[test]
    fn a_figure_is_drawn_only_for_the_finished_answer_that_shows_it() {
        let mut connection = library();
        let conversation = "c".repeat(32);
        let turn = "e".repeat(32);
        connection
            .execute(
                "INSERT INTO tutor_conversations(id, operation_id, title, scope_json, created_at, updated_at)
                 VALUES (?1, ?1, 'Chat', '{}', 1, 1)",
                params![conversation],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO tutor_turns(
                     id, conversation_id, operation_id, sequence, request_hash, request_json, mode,
                     user_text, assistant_text, state, profile_revision, provider_kind, model_id,
                     created_at, updated_at
                 ) VALUES (?1, ?2, ?1, 1, ?3, '{}', 'explain', 'Why?', ?4, 'complete', 1,
                           'openrouter', 'model', 1, 1)",
                params![
                    turn,
                    conversation,
                    "0".repeat(64),
                    format!("See ![Loop](figure:{DOCUMENT}/p00001-01).")
                ],
            )
            .unwrap();
        let ask = |connection: &Connection, figure_id: &str| {
            figure_for(
                connection,
                TutorFigureInput {
                    turn_id: turn.clone(),
                    document_id: DOCUMENT.to_string(),
                    figure_id: figure_id.to_string(),
                },
            )
        };
        let shown = ask(&connection, "p00001-01").unwrap();
        assert_eq!(shown.title, "Figure p00001-01");
        assert_eq!(shown.page_number, 1);
        // A figure of the document the answer does not show is not handed out.
        assert_eq!(
            ask(&connection, "p00001-02").unwrap_err().code,
            "tutor_figure_missing"
        );
        hide(&mut connection, "p00001-01");
        assert_eq!(
            ask(&connection, "p00001-01").unwrap_err().code,
            "tutor_figure_missing"
        );
    }

    #[test]
    fn with_nothing_offered_no_figure_is_kept() {
        let text = format!("See ![Nephron](figure:{DOCUMENT}/p00003-01) here.");
        assert_eq!(keep_offered(&text, &[]), "See  here.");
        assert_eq!(keep_offered("A ![ broken", &[]), "A ![ broken");
    }
}
