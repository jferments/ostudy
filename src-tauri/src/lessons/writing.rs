//! Writing a subject's course with AI.
//!
//! The course follows the subject's outline: the same bounded windows of
//! retained knowledge that its study guide sections are written from, one
//! lesson for each. Lessons are written in parallel within the provider's
//! limits and saved together once every one of them is valid, so a failed or
//! stopped run leaves the course as it was. A subject that already has
//! lessons is refused; rewriting a course is a separate decision.

use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::State;

use super::{
    authoring::{create_generated_course, ensure_course_empty, MAX_LESSONS_PER_SUBJECT},
    content::{LessonContent, LESSON_CONTENT_VERSION},
    list_lessons, require_subject, SubjectLessonsDto,
};
use crate::{
    ai::{
        concurrency::{document_request_parallelism, run_bounded_provider_queue, QueueControl},
        document_intelligence::{
            generate_lesson, load_document_task_context, DocumentTaskContext, LessonAuthoring,
            LESSON_PROMPT_VERSION,
        },
        document_provider, generation, AiRuntimeState,
    },
    database::Database,
    error::CommandError,
    knowledge::collection_view::{self, CollectionGuideWindow, CollectionViewState},
    study_materials::composition::{collection_title, relationship_context},
    task_activity::ActivityGuard,
    task_cancellation::CancellationSignal,
    util::{unix_time_millis, validate_entity_id},
};

const PURPOSE: &str = "lesson_authoring";
/// How much of each part's concepts the course outline shows.
const OUTLINE_CONTEXT_CHARS: usize = 200;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteSubjectCourseInput {
    pub subject_id: String,
    pub operation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelSubjectCourseWritingInput {
    pub operation_id: String,
}

pub(crate) async fn write_subject_course(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: WriteSubjectCourseInput,
) -> Result<SubjectLessonsDto, CommandError> {
    write_course(database.inner(), runtime.inner(), input).await
}

#[tauri::command]
pub fn cancel_subject_course_writing(
    input: CancelSubjectCourseWritingInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.operation_id, "course writing operation")?;
    let active = ACTIVE.lock().map_err(|_| registry_error())?;
    let signal = active
        .values()
        .find(|(operation_id, _)| *operation_id == input.operation_id)
        .map(|(_, signal)| signal)
        .ok_or_else(|| {
            CommandError::new(
                "lesson_writing_not_running",
                "That course is no longer being written.",
            )
        })?;
    signal.cancel();
    Ok(())
}

pub(crate) async fn write_course(
    database: &Database,
    runtime: &AiRuntimeState,
    input: WriteSubjectCourseInput,
) -> Result<SubjectLessonsDto, CommandError> {
    validate_entity_id(&input.subject_id, "subject")?;
    validate_entity_id(&input.operation_id, "course writing operation")?;
    database.with_connection(|connection| {
        require_subject(connection, &input.subject_id)?;
        ensure_course_empty(connection, &input.subject_id)
    })?;
    let guard = Guard::begin(&input.subject_id, &input.operation_id)?;
    let signal = guard.signal();
    let mut diagnostics = generation::Operation::begin(
        database,
        Some(&input.operation_id),
        None,
        Some(&input.subject_id),
        PURPOSE,
    )?;
    let result = write_lessons(database, runtime, &input, diagnostics.context(), &signal).await;
    diagnostics.finish_result(&result);
    drop(guard);
    result?;
    database.with_connection(|connection| list_lessons(connection, &input.subject_id))
}

async fn write_lessons(
    database: &Database,
    runtime: &AiRuntimeState,
    input: &WriteSubjectCourseInput,
    diagnostics: generation::Context,
    signal: &CancellationSignal,
) -> Result<(), CommandError> {
    let subject_id = input.subject_id.as_str();
    let native = collection_view::refresh_native(database, subject_id)?;
    if native.synthesis.topics.is_empty() {
        return Err(nothing_to_teach());
    }
    let mut task = load_document_task_context(database)?;
    let capabilities = document_provider::load_document_provider_capabilities(
        database,
        &task.profile_id,
        &task.provider_kind,
        &task.model_id,
    )?;
    task.apply_provider_capabilities(&capabilities);
    task.diagnostics = Some(diagnostics);
    let view =
        collection_view::ensure_reduced(database, runtime, &task, subject_id, signal).await?;
    ensure_running(signal)?;
    let windows = collection_view::guide_section_windows(database, subject_id, &view.synthesis)?;
    if windows.is_empty() {
        return Err(nothing_to_teach());
    }
    if windows.len() > usize::try_from(MAX_LESSONS_PER_SUBJECT).unwrap_or(usize::MAX) {
        return Err(CommandError::new(
            "lesson_limit",
            format!("A subject can hold up to {MAX_LESSONS_PER_SUBJECT} lessons."),
        ));
    }
    let subject_title = collection_title(database, subject_id)?;
    let course = CourseRequest {
        database,
        runtime,
        task: &task,
        subject_title: &subject_title,
        view: &view,
        operation_id: &input.operation_id,
        windows: &windows,
        signal,
    };
    let parallelism = document_request_parallelism(&task.network_scope).clamp(1, windows.len());
    let mut lessons = vec![None; windows.len()];
    let mut first_error: Option<(usize, CommandError)> = None;
    run_bounded_provider_queue(
        windows.len(),
        parallelism,
        |index| {
            let course = &course;
            async move { (index, course.write(index).await) }
        },
        |(index, result)| {
            Ok::<_, CommandError>(match result {
                Ok(lesson) => {
                    lessons[index] = Some(lesson);
                    QueueControl::Continue
                }
                Err(error) => {
                    // Report the earliest failed lesson, as the guide does.
                    if first_error
                        .as_ref()
                        .is_none_or(|(failed, _)| index < *failed)
                    {
                        first_error = Some((index, error));
                    }
                    QueueControl::Stop
                }
            })
        },
    )
    .await?;
    if let Some((_, error)) = first_error {
        return Err(error);
    }
    ensure_running(signal)?;
    let lessons = lessons
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            CommandError::new(
                "lesson_writing_incomplete",
                "A lesson of this course was not written.",
            )
        })?;
    task.ensure_current(database)?;
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        // The source must still be the one the lessons were written from.
        collection_view::ensure_analysis_current(connection, subject_id, &view.analysis_hash)?;
        create_generated_course(connection, subject_id, &lessons, now).map(|_| ())
    })
}

struct CourseRequest<'a> {
    database: &'a Database,
    runtime: &'a AiRuntimeState,
    task: &'a DocumentTaskContext,
    subject_title: &'a str,
    view: &'a CollectionViewState,
    operation_id: &'a str,
    windows: &'a [CollectionGuideWindow],
    signal: &'a CancellationSignal,
}

impl CourseRequest<'_> {
    async fn write(&self, index: usize) -> Result<(LessonContent, Value), CommandError> {
        let window = &self.windows[index];
        ensure_running(self.signal)?;
        self.task.ensure_current(self.database)?;
        let catalog = window.catalog.fingerprint()?;
        let mut task = self.task.fork_usage();
        task.diagnostic_window_key = Some(format!(
            "{:x}",
            Sha256::digest(format!("lesson|{}|{catalog}", window.block_key).as_bytes())
        ));
        let relationships = relationship_context(self.view, &window.topic_ids);
        let outline = course_outline(self.windows, index);
        let lesson = generate_lesson(
            self.runtime,
            &task,
            LessonAuthoring {
                subject_title: self.subject_title,
                topic_title: &window.title,
                topic_summary: &window.topic_context,
                relationship_context: &relationships,
                course_outline: &outline,
                knowledge: &window.catalog,
            },
            self.signal,
        )
        .await?;
        let (total_tokens, usage_complete) = task.usage_summary();
        let provenance = json!({
            "provider": task.provider_kind,
            "model": task.model_id,
            "promptVersion": LESSON_PROMPT_VERSION,
            "lessonSchemaVersion": LESSON_CONTENT_VERSION,
            "operationId": self.operation_id,
            "outlineBlockKey": window.block_key,
            "catalogFingerprint": catalog,
            "totalTokens": total_tokens,
            "usageComplete": usage_complete,
        });
        Ok((lesson, provenance))
    }
}

/// Each lesson's title and the start of what its part covers, one per line,
/// with the lesson being written marked.
fn course_outline(windows: &[CollectionGuideWindow], current: usize) -> String {
    windows
        .iter()
        .enumerate()
        .map(|(index, window)| {
            let covers = window
                .topic_context
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let covers = covers
                .chars()
                .take(OUTLINE_CONTEXT_CHARS)
                .collect::<String>();
            let marker = if index == current {
                " <- this lesson"
            } else {
                ""
            };
            format!("{}. {}: {covers}{marker}", index + 1, window.title)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn ensure_running(signal: &CancellationSignal) -> Result<(), CommandError> {
    if signal.is_canceled() {
        Err(CommandError::new(
            "lesson_writing_canceled",
            "Writing the course was stopped.",
        ))
    } else {
        Ok(())
    }
}

fn nothing_to_teach() -> CommandError {
    CommandError::new(
        "lesson_course_empty",
        "This subject has no processed documents to write lessons from yet.",
    )
}

/// Subjects whose course is being written, with the operation writing it.
static ACTIVE: LazyLock<Mutex<HashMap<String, (String, CancellationSignal)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// One course writer per subject. Dropping the guard stops its requests.
struct Guard {
    subject_id: String,
    signal: CancellationSignal,
    _activity: ActivityGuard,
}

impl Guard {
    fn begin(subject_id: &str, operation_id: &str) -> Result<Self, CommandError> {
        let activity = ActivityGuard::begin()?;
        let signal = CancellationSignal::new();
        let mut active = ACTIVE.lock().map_err(|_| registry_error())?;
        if active.contains_key(subject_id)
            || active
                .values()
                .any(|(operation, _)| operation == operation_id)
        {
            return Err(CommandError::new(
                "lesson_writing_already_running",
                "This subject's course is already being written.",
            ));
        }
        active.insert(
            subject_id.to_string(),
            (operation_id.to_string(), signal.clone()),
        );
        Ok(Self {
            subject_id: subject_id.to_string(),
            signal,
            _activity: activity,
        })
    }

    fn signal(&self) -> CancellationSignal {
        self.signal.clone()
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.signal.cancel();
        if let Ok(mut active) = ACTIVE.lock() {
            active.remove(&self.subject_id);
        }
    }
}

fn registry_error() -> CommandError {
    CommandError::new(
        "lesson_writing_registry_unavailable",
        "Course writing is unavailable. Restart ostudy and try again.",
    )
}

#[cfg(test)]
#[path = "writing_tests.rs"]
mod tests;
