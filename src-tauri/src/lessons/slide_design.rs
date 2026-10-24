//! A lesson's lecture slides as designed pages.
//!
//! The same slides the lecture shows (a title slide, one per step, and the
//! sources) are laid out by Typst in one of the study guide themes and
//! returned as one SVG per slide. Each step slide shows its slide points and
//! at most one visual chosen from its blocks: a process becomes a step flow, a
//! comparison two panels, and so on. A step whose source pages hold a figure
//! that illustrates it shows that figure, cropped from its PDF, in place of
//! the drawn visual. Slides are compiled from the saved
//! revision on every read, never stored and never written by a model; the
//! reader's view of the lesson is the input, so a check's answer never reaches
//! a slide.

use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;

use super::{
    content::{CalloutKind, LessonBlock},
    lecture::{source_documents, step_slide_key, SOURCES_KEY, TITLE_KEY},
    read_lesson, GetLessonInput, LessonStepViewDto,
};
use crate::{
    database::Database,
    document_annotations::page_tiles::pdf_source_path,
    document_figures::{self, DocumentFigureDto},
    document_study_guide_export::{
        compile_template_svgs_with,
        figures::{choose_for, draw_png, FigurePlace},
        rich_runs, StudyGuidePdfTheme,
    },
    error::CommandError,
    paths::AppPaths,
    pdf_render_backend::PdfRenderRuntime,
};

const TEMPLATE: &str = include_str!("typst/slides.typ");
/// More rows than this do not fit a slide's table; the lesson still has them.
const MAX_TABLE_ROWS: usize = 8;
/// More boxes than this do not fit a slide's step flow.
const MAX_FLOW_STEPS: usize = 8;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetLessonSlideDesignsInput {
    pub lesson_id: String,
    /// An earlier revision to lay out; the latest when absent.
    pub revision_number: Option<i64>,
    #[serde(default)]
    pub theme: StudyGuidePdfTheme,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonSlideDesignsDto {
    pub lesson_id: String,
    /// The revision the slides were laid out from.
    pub revision_number: i64,
    pub theme: StudyGuidePdfTheme,
    pub slides: Vec<SlideDesignDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideDesignDto {
    /// The lecture slide's key: `title`, `step:<step key>` or `sources`.
    pub key: String,
    /// The slide as a standalone SVG document, 16:9.
    pub svg: String,
}

/// Draws one figure from the PDF at a document's stored relative path.
pub(super) type DrawFigure<'a> =
    &'a dyn Fn(&str, &DocumentFigureDto) -> Option<(Vec<u8>, u32, u32)>;

#[tauri::command]
pub async fn get_lesson_slide_designs(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PdfRenderRuntime>,
    input: GetLessonSlideDesignsInput,
) -> Result<LessonSlideDesignsDto, CommandError> {
    let database = database.inner().clone();
    let paths = paths.inner().clone();
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let draw = |relative: &str, figure: &DocumentFigureDto| {
            draw_png(&runtime, &paths.resolve_relative(relative), figure)
        };
        database.with_connection(|connection| {
            read_slide_designs_drawing(connection, input, Some(&draw))
        })
    })
    .await
    .map_err(|error| design_error(error.to_string()))?
}

/// The slides without figures.
#[cfg(test)]
pub(super) fn read_slide_designs(
    connection: &Connection,
    input: GetLessonSlideDesignsInput,
) -> Result<LessonSlideDesignsDto, CommandError> {
    read_slide_designs_drawing(connection, input, None)
}

pub(super) fn read_slide_designs_drawing(
    connection: &Connection,
    input: GetLessonSlideDesignsInput,
    draw: Option<DrawFigure<'_>>,
) -> Result<LessonSlideDesignsDto, CommandError> {
    let theme = input.theme;
    let (lesson_id, revision_number, keys, data, images) = slide_data(
        connection,
        GetLessonInput {
            lesson_id: input.lesson_id,
            revision_number: input.revision_number,
        },
        theme,
        draw,
    )?;
    let svgs = compile_template_svgs_with(TEMPLATE, &data, images).map_err(design_error)?;
    // The template fits each slide to one page; anything else is a layout bug.
    if svgs.len() != keys.len() {
        return Err(design_error(format!(
            "{} slides came out as {} pages",
            keys.len(),
            svgs.len()
        )));
    }
    Ok(LessonSlideDesignsDto {
        lesson_id,
        revision_number,
        theme,
        slides: keys
            .into_iter()
            .zip(svgs)
            .map(|(key, svg)| SlideDesignDto { key, svg })
            .collect(),
    })
}

fn design_error(detail: String) -> CommandError {
    CommandError::new(
        "lesson_slides_layout_failed",
        format!("The slides could not be laid out: {detail}"),
    )
}

/// Images the template reads, by absolute path.
type SlideImages = Vec<(String, Vec<u8>)>;
/// The template's data with the images it reads.
type SlideData = (String, i64, Vec<String>, Value, SlideImages);

/// The lesson's slide keys in order, the template's data and the figure
/// images it places, drawn with `draw` when given.
pub(super) fn slide_data(
    connection: &Connection,
    input: GetLessonInput,
    theme: StudyGuidePdfTheme,
    draw: Option<DrawFigure<'_>>,
) -> Result<SlideData, CommandError> {
    let lesson = read_lesson(connection, input)?;
    let documents = source_documents(connection, &lesson)?;
    let content = &lesson.content;
    let (figures, images) = match draw {
        Some(draw) => step_figures(connection, &content.steps, draw)?,
        None => (vec![Value::Null; content.steps.len()], Vec::new()),
    };
    let total = content.steps.len() + 1 + usize::from(!documents.is_empty());
    let mut keys = Vec::with_capacity(total);
    let mut slides = Vec::with_capacity(total);

    keys.push(TITLE_KEY.to_string());
    slides.push(json!({
        "kind": "title",
        "number": 1,
        "eyebrow": "Lesson",
        "title": rich_runs(&content.title),
        "points": content.objectives.iter().map(|item| rich_runs(item)).collect::<Vec<_>>(),
        "meta": format!("{} slides", total),
        "visual": Value::Null,
    }));
    for (index, step) in content.steps.iter().enumerate() {
        keys.push(step_slide_key(&step.key));
        slides.push(json!({
            "kind": "step",
            "number": index + 2,
            "eyebrow": format!("Step {} of {}", index + 1, content.steps.len()),
            "title": rich_runs(&step.title),
            "points": step.slide_points.iter().map(|item| rich_runs(item)).collect::<Vec<_>>(),
            "visual": visual(step),
            "figure": figures[index].clone(),
        }));
    }
    if !documents.is_empty() {
        keys.push(SOURCES_KEY.to_string());
        slides.push(json!({
            "kind": "sources",
            "number": total,
            "eyebrow": "Read more",
            "title": rich_runs("Sources"),
            "points": documents.iter().map(|item| rich_runs(item)).collect::<Vec<_>>(),
            "visual": Value::Null,
        }));
    }
    let data = json!({
        "theme": theme.key(),
        "title": content.title,
        "slides": slides,
    });
    Ok((lesson.lesson.id, lesson.revision_number, keys, data, images))
}

/// The figure each step's slide shows, or null, and the images they read.
/// A step's figure comes from the pages its knowledge was written from, and
/// is chosen as a guide section's is; each figure shows once a lesson.
fn step_figures(
    connection: &Connection,
    steps: &[LessonStepViewDto],
    draw: DrawFigure<'_>,
) -> Result<(Vec<Value>, SlideImages), CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT source.document_id, json_extract(source.record_json, '$.locator.unitNumbers')
             FROM knowledge_units unit
             JOIN knowledge_sources source ON source.id = unit.source_id
             WHERE unit.id = ?1 AND source.document_id IS NOT NULL",
        )
        .map_err(CommandError::database)?;
    // Each step's pages, by document.
    let mut pages: HashMap<String, Vec<HashSet<i64>>> = HashMap::new();
    for (index, step) in steps.iter().enumerate() {
        for unit_id in &step.knowledge_unit_ids {
            let found: Option<(String, Option<String>)> = statement
                .query_row([unit_id], |row| Ok((row.get(0)?, row.get(1)?)))
                .optional()
                .map_err(CommandError::database)?;
            let Some((document_id, numbers)) = found else {
                continue;
            };
            let numbers: Vec<i64> = numbers
                .and_then(|text| serde_json::from_str(&text).ok())
                .unwrap_or_default();
            pages
                .entry(document_id)
                .or_insert_with(|| vec![HashSet::new(); steps.len()])[index]
                .extend(numbers);
        }
    }
    let mut chosen: Vec<Value> = vec![Value::Null; steps.len()];
    let mut images = Vec::new();
    let mut documents = pages.into_iter().collect::<Vec<_>>();
    documents.sort_by(|a, b| a.0.cmp(&b.0));
    for (document_id, step_pages) in documents {
        let found = document_figures::figures(connection, &document_id)?;
        if !found.use_images || found.figures.is_empty() {
            continue;
        }
        let Ok(relative) = pdf_source_path(connection, &document_id) else {
            continue;
        };
        let places = steps
            .iter()
            .zip(step_pages)
            .map(|(step, pages)| FigurePlace {
                pages,
                text: format!(
                    "{} {} {}",
                    step.title,
                    step.slide_points.join(" "),
                    step.narration
                ),
            })
            .collect::<Vec<_>>();
        for (index, picks) in choose_for(&places, &found.figures, 1)
            .into_iter()
            .enumerate()
        {
            let Some(pick) = picks.first() else { continue };
            if !chosen[index].is_null() {
                continue;
            }
            let Some(figure) = found
                .figures
                .iter()
                .find(|figure| figure.figure_id == pick.figure_id)
            else {
                continue;
            };
            let Some((png, width, height)) = draw(&relative, figure) else {
                continue;
            };
            let path = format!("/figures/{}.png", images.len());
            images.push((path.clone(), png));
            chosen[index] = json!({
                "path": path,
                "caption": rich_runs(&pick.title),
                "tall": height > width,
            });
        }
    }
    Ok((chosen, images))
}

/// The one picture a step's slide shows, chosen from its blocks: a process
/// first, then a comparison, a table, a misconception, a term, a callout and
/// a worked example. A step with none of them shows its points alone.
fn visual(step: &LessonStepViewDto) -> Value {
    let rank = |block: &LessonBlock| match block {
        LessonBlock::Process { .. } => Some(0),
        LessonBlock::Comparison { .. } => Some(1),
        LessonBlock::Table { .. } => Some(2),
        LessonBlock::Misconception { .. } => Some(3),
        LessonBlock::Term { .. } => Some(4),
        LessonBlock::Callout { .. } => Some(5),
        LessonBlock::WorkedExample { .. } => Some(6),
        LessonBlock::Explanation { .. } | LessonBlock::KeyPoints { .. } => None,
    };
    let Some(block) = step
        .blocks
        .iter()
        .filter_map(|block| rank(block).map(|rank| (rank, block)))
        .min_by_key(|(rank, _)| *rank)
        .map(|(_, block)| block)
    else {
        return Value::Null;
    };
    let list = |items: &[String]| items.iter().map(|item| rich_runs(item)).collect::<Vec<_>>();
    match block {
        LessonBlock::Process { title, steps } => json!({
            "kind": "flow",
            "title": rich_runs(title),
            "steps": list(&steps[..steps.len().min(MAX_FLOW_STEPS)]),
        }),
        LessonBlock::Comparison {
            subject_a,
            subject_b,
            relationship,
        } => json!({
            "kind": "compare",
            "a": rich_runs(subject_a),
            "b": rich_runs(subject_b),
            "relationship": rich_runs(relationship),
        }),
        LessonBlock::Table {
            title,
            columns,
            rows,
            caption,
        } => json!({
            "kind": "table",
            "title": rich_runs(title),
            "columns": list(columns),
            "rows": rows
                .iter()
                .take(MAX_TABLE_ROWS)
                .map(|row| {
                    // Every row has as many cells as there are columns.
                    let mut cells = list(row);
                    cells.resize(columns.len().max(1), Value::Array(Vec::new()));
                    cells
                })
                .collect::<Vec<_>>(),
            "caption": rich_runs(caption.as_deref().unwrap_or_default()),
        }),
        LessonBlock::Misconception {
            misconception,
            correction,
        } => json!({
            "kind": "myth",
            "myth": rich_runs(misconception),
            "fact": rich_runs(correction),
        }),
        LessonBlock::Term { term, explanation } => json!({
            "kind": "term",
            "term": rich_runs(term),
            "explanation": rich_runs(explanation),
        }),
        LessonBlock::Callout {
            callout,
            title,
            body,
        } => json!({
            "kind": "callout",
            "callout": match callout {
                CalloutKind::KeyIdea => "key_idea",
                CalloutKind::Warning => "warning",
                CalloutKind::Tip => "tip",
                CalloutKind::Note => "note",
            },
            "title": rich_runs(title),
            "body": rich_runs(body),
        }),
        LessonBlock::WorkedExample {
            title,
            prompt,
            steps,
            answer,
            ..
        } => json!({
            "kind": "example",
            "title": rich_runs(title),
            "prompt": rich_runs(prompt),
            "steps": list(steps),
            "answer": rich_runs(answer),
        }),
        LessonBlock::Explanation { .. } | LessonBlock::KeyPoints { .. } => Value::Null,
    }
}
