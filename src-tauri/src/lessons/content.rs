//! The text of one lesson revision, as it is stored and validated.
//!
//! A lesson is an ordered list of steps. Each step holds teaching blocks (the
//! same kinds a study guide section uses), the narration a voice reads for it,
//! the points its slide shows, an optional check, and the knowledge units it
//! explains. They are written together so the text, narration and slides of a
//! step cannot drift apart. Every text field is plain text; a check is a native
//! question definition and is validated by the question owner.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    cards::questions::{QuestionDefinition, QuestionKind, ValidatedQuestion},
    error::CommandError,
    util::validate_entity_id,
};

pub(crate) const LESSON_CONTENT_VERSION: u32 = 1;

pub(crate) const MAX_TITLE_CHARS: usize = 160;
const MAX_SUMMARY_CHARS: usize = 600;
const MAX_OBJECTIVES: usize = 6;
const MAX_OBJECTIVE_CHARS: usize = 260;
pub(crate) const MAX_STEPS: usize = 24;
const MAX_STEP_KEY_CHARS: usize = 40;
const MAX_BLOCKS_PER_STEP: usize = 12;
const MAX_HEADING_CHARS: usize = 500;
const MAX_BODY_CHARS: usize = 16 * 1024;
const MAX_LIST_ITEMS: usize = 32;
const MAX_ITEM_CHARS: usize = 2_000;
const MAX_TABLE_COLUMNS: usize = 8;
const MAX_TABLE_ROWS: usize = 32;
const MAX_TABLE_CELL_CHARS: usize = 1_000;
const MAX_NARRATION_CHARS: usize = 8_000;
const MAX_SLIDE_POINTS: usize = 6;
const MAX_SLIDE_POINT_CHARS: usize = 200;
const MAX_UNITS_PER_STEP: usize = 32;
/// Matches the revision table's own content check.
pub(crate) const MAX_CONTENT_BYTES: usize = 1_048_576;
/// Reading speed used for a lesson's estimated length.
const WORDS_PER_MINUTE: usize = 200;

#[derive(Clone, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LessonContent {
    pub schema_version: u32,
    pub title: String,
    pub summary: String,
    pub objectives: Vec<String>,
    pub steps: Vec<LessonStep>,
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LessonStep {
    /// Unique within the lesson; an author keeps it when a step is rewritten.
    pub key: String,
    pub title: String,
    pub blocks: Vec<LessonBlock>,
    /// What a voice reads for this step. It never reveals the check's answer.
    pub narration: String,
    /// The bullets of this step's slide.
    pub slide_points: Vec<String>,
    pub check: Option<QuestionDefinition>,
    pub knowledge_unit_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CalloutKind {
    KeyIdea,
    Warning,
    Tip,
    Note,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum LessonBlock {
    Explanation {
        heading: Option<String>,
        body: String,
    },
    KeyPoints {
        items: Vec<String>,
    },
    Term {
        term: String,
        explanation: String,
    },
    Process {
        title: String,
        steps: Vec<String>,
    },
    Comparison {
        subject_a: String,
        subject_b: String,
        relationship: String,
    },
    Callout {
        callout: CalloutKind,
        title: String,
        body: String,
    },
    Table {
        title: String,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        caption: Option<String>,
    },
    WorkedExample {
        title: String,
        prompt: String,
        steps: Vec<String>,
        answer: String,
        takeaway: Option<String>,
    },
    Misconception {
        misconception: String,
        correction: String,
    },
}

/// Content that passed validation.
pub(crate) struct ValidatedLesson {
    pub content: LessonContent,
}

impl ValidatedLesson {
    pub(crate) fn validate(content: LessonContent) -> Result<Self, CommandError> {
        if content.schema_version != LESSON_CONTENT_VERSION {
            return Err(CommandError::new(
                "lesson_version_unsupported",
                "This lesson was written by a newer version of ostudy. Update ostudy to open it.",
            ));
        }
        text("A lesson title", &content.title, MAX_TITLE_CHARS)?;
        if !content.summary.is_empty() {
            text("A lesson summary", &content.summary, MAX_SUMMARY_CHARS)?;
        }
        count("objectives", content.objectives.len(), 1, MAX_OBJECTIVES)?;
        for objective in &content.objectives {
            text("An objective", objective, MAX_OBJECTIVE_CHARS)?;
        }
        count("steps", content.steps.len(), 1, MAX_STEPS)?;
        let mut keys = BTreeSet::new();
        for step in &content.steps {
            validate_step(step)?;
            if !keys.insert(step.key.as_str()) {
                return Err(invalid("Two steps of a lesson have the same key."));
            }
        }
        if canonical_json(&content)?.len() > MAX_CONTENT_BYTES {
            return Err(invalid("The lesson is too long to save."));
        }
        Ok(Self { content })
    }

    /// Reads a stored revision, refusing anything validation would refuse.
    pub(crate) fn parse(json: &str) -> Result<Self, CommandError> {
        if json.len() > MAX_CONTENT_BYTES {
            return Err(invalid("The lesson is too long to open."));
        }
        let version = serde_json::from_str::<serde_json::Value>(json)
            .ok()
            .and_then(|value| value.get("schemaVersion")?.as_u64());
        if version.is_some_and(|version| version != u64::from(LESSON_CONTENT_VERSION)) {
            return Err(CommandError::new(
                "lesson_version_unsupported",
                "This lesson was written by a newer version of ostudy. Update ostudy to open it.",
            ));
        }
        let content = serde_json::from_str(json)
            .map_err(|_| invalid("The saved lesson has an unsupported or damaged structure."))?;
        Self::validate(content)
    }

    /// Reading time at an ordinary pace, at least one minute.
    pub(crate) fn estimated_minutes(&self) -> u32 {
        let words: usize = self
            .content
            .steps
            .iter()
            .map(|step| step.blocks.iter().map(block_words).sum::<usize>() + words(&step.title))
            .sum();
        u32::try_from(words.div_ceil(WORDS_PER_MINUTE).max(1)).unwrap_or(u32::MAX)
    }
}

/// The stored form of a lesson: its fields in declaration order.
pub(crate) fn canonical_json(content: &LessonContent) -> Result<String, CommandError> {
    serde_json::to_string(content).map_err(|error| {
        CommandError::new(
            "lesson_invalid",
            format!("Could not save the lesson: {error}"),
        )
    })
}

fn validate_step(step: &LessonStep) -> Result<(), CommandError> {
    if step.key.is_empty()
        || step.key.len() > MAX_STEP_KEY_CHARS
        || !step
            .key
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(invalid(format!(
            "A step key is 1 to {MAX_STEP_KEY_CHARS} lowercase letters, digits or hyphens."
        )));
    }
    text("A step title", &step.title, MAX_TITLE_CHARS)?;
    count(
        "blocks in a step",
        step.blocks.len(),
        1,
        MAX_BLOCKS_PER_STEP,
    )?;
    for block in &step.blocks {
        validate_block(block)?;
    }
    text("A step's narration", &step.narration, MAX_NARRATION_CHARS)?;
    count("slide points", step.slide_points.len(), 0, MAX_SLIDE_POINTS)?;
    for point in &step.slide_points {
        text("A slide point", point, MAX_SLIDE_POINT_CHARS)?;
    }
    if let Some(check) = &step.check {
        // Image occlusion needs managed images, which lessons do not own.
        if check.question.kind() == QuestionKind::ImageOcclusion {
            return Err(invalid(
                "A lesson check cannot be an image occlusion question.",
            ));
        }
        ValidatedQuestion::validate(check.clone())?;
    }
    count(
        "knowledge units in a step",
        step.knowledge_unit_ids.len(),
        0,
        MAX_UNITS_PER_STEP,
    )?;
    let mut units = BTreeSet::new();
    for unit in &step.knowledge_unit_ids {
        validate_entity_id(unit, "knowledge unit")?;
        if !units.insert(unit) {
            return Err(invalid("A step lists the same knowledge unit twice."));
        }
    }
    Ok(())
}

fn validate_block(block: &LessonBlock) -> Result<(), CommandError> {
    match block {
        LessonBlock::Explanation { heading, body } => {
            if let Some(heading) = heading {
                text("A heading", heading, MAX_HEADING_CHARS)?;
            }
            text("An explanation", body, MAX_BODY_CHARS)
        }
        LessonBlock::KeyPoints { items } => list("key points", items, 1),
        LessonBlock::Term { term, explanation } => {
            text("A term", term, MAX_HEADING_CHARS)?;
            text("A term's explanation", explanation, MAX_BODY_CHARS)
        }
        LessonBlock::Process { title, steps } => {
            text("A process title", title, MAX_HEADING_CHARS)?;
            list("process steps", steps, 2)
        }
        LessonBlock::Comparison {
            subject_a,
            subject_b,
            relationship,
        } => {
            text("A compared item", subject_a, MAX_HEADING_CHARS)?;
            text("A compared item", subject_b, MAX_HEADING_CHARS)?;
            text("A comparison", relationship, MAX_BODY_CHARS)
        }
        LessonBlock::Callout { title, body, .. } => {
            text("A callout title", title, MAX_HEADING_CHARS)?;
            text("A callout", body, MAX_BODY_CHARS)
        }
        LessonBlock::Table {
            title,
            columns,
            rows,
            caption,
        } => {
            text("A table title", title, MAX_HEADING_CHARS)?;
            count("table columns", columns.len(), 2, MAX_TABLE_COLUMNS)?;
            for column in columns {
                text("A column heading", column, MAX_TABLE_CELL_CHARS)?;
            }
            count("table rows", rows.len(), 1, MAX_TABLE_ROWS)?;
            for row in rows {
                if row.len() != columns.len() {
                    return Err(invalid("Every table row needs one cell per column."));
                }
                for cell in row {
                    if !cell.is_empty() {
                        text("A table cell", cell, MAX_TABLE_CELL_CHARS)?;
                    }
                }
            }
            if let Some(caption) = caption {
                text("A table caption", caption, MAX_HEADING_CHARS)?;
            }
            Ok(())
        }
        LessonBlock::WorkedExample {
            title,
            prompt,
            steps,
            answer,
            takeaway,
        } => {
            text("A worked example title", title, MAX_HEADING_CHARS)?;
            text("A worked example prompt", prompt, MAX_BODY_CHARS)?;
            list("worked example steps", steps, 1)?;
            text("A worked example answer", answer, MAX_BODY_CHARS)?;
            if let Some(takeaway) = takeaway {
                text("A takeaway", takeaway, MAX_BODY_CHARS)?;
            }
            Ok(())
        }
        LessonBlock::Misconception {
            misconception,
            correction,
        } => {
            text("A misconception", misconception, MAX_BODY_CHARS)?;
            text("A correction", correction, MAX_BODY_CHARS)
        }
    }
}

fn block_words(block: &LessonBlock) -> usize {
    match block {
        LessonBlock::Explanation { heading, body } => {
            heading.as_deref().map_or(0, words) + words(body)
        }
        LessonBlock::KeyPoints { items } => items.iter().map(|item| words(item)).sum(),
        LessonBlock::Term { term, explanation } => words(term) + words(explanation),
        LessonBlock::Process { title, steps } => {
            words(title) + steps.iter().map(|step| words(step)).sum::<usize>()
        }
        LessonBlock::Comparison {
            subject_a,
            subject_b,
            relationship,
        } => words(subject_a) + words(subject_b) + words(relationship),
        LessonBlock::Callout { title, body, .. } => words(title) + words(body),
        LessonBlock::Table {
            title,
            columns,
            rows,
            caption,
        } => {
            words(title)
                + columns.iter().map(|cell| words(cell)).sum::<usize>()
                + rows.iter().flatten().map(|cell| words(cell)).sum::<usize>()
                + caption.as_deref().map_or(0, words)
        }
        LessonBlock::WorkedExample {
            title,
            prompt,
            steps,
            answer,
            takeaway,
        } => {
            words(title)
                + words(prompt)
                + steps.iter().map(|step| words(step)).sum::<usize>()
                + words(answer)
                + takeaway.as_deref().map_or(0, words)
        }
        LessonBlock::Misconception {
            misconception,
            correction,
        } => words(misconception) + words(correction),
    }
}

fn words(value: &str) -> usize {
    value.split_whitespace().count()
}

fn list(label: &str, items: &[String], minimum: usize) -> Result<(), CommandError> {
    count(label, items.len(), minimum, MAX_LIST_ITEMS)?;
    for item in items {
        text("A list item", item, MAX_ITEM_CHARS)?;
    }
    Ok(())
}

fn count(label: &str, len: usize, minimum: usize, maximum: usize) -> Result<(), CommandError> {
    if (minimum..=maximum).contains(&len) {
        Ok(())
    } else {
        Err(invalid(format!(
            "A lesson needs {minimum} to {maximum} {label}."
        )))
    }
}

/// Plain text: not blank, no control characters other than newlines and tabs,
/// no surrounding whitespace, and at most `maximum` characters.
fn text(label: &str, value: &str, maximum: usize) -> Result<(), CommandError> {
    if value.trim().is_empty() {
        return Err(invalid(format!("{label} cannot be empty.")));
    }
    if value.trim() != value {
        return Err(invalid(format!("{label} cannot start or end with spaces.")));
    }
    if value.chars().count() > maximum {
        return Err(invalid(format!(
            "{label} can hold up to {maximum} characters."
        )));
    }
    if value
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(invalid(format!(
            "{label} cannot contain control characters."
        )));
    }
    Ok(())
}

pub(crate) fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::new("lesson_invalid", message)
}
