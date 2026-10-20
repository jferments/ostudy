//! Writing a subject's course with AI.
//!
//! The course follows the subject's documents in order and, within each, its
//! learning plan: one lesson for each top-level topic, together with its
//! subtopics, so lessons match the parts of the study guide the learner
//! reads. A knowledge unit that two lessons would share is taught by the one
//! drawn from less of the source, the rule guide sections follow, so lessons
//! do not repeat each other. Lessons are written in parallel within the
//! provider's limits and saved together once every one of them is valid, so
//! a failed or stopped run leaves the course as it was. A subject that
//! already has lessons is refused; rewriting a course is a separate decision.

use std::{
    collections::{BTreeSet, HashMap},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, LazyLock, Mutex,
    },
};

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
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
    document_learning::{load_active_document_learning_plan, DocumentLearningPlan, LearningTopic},
    document_study_guide_generation::ownership,
    error::CommandError,
    knowledge::{
        authoring::{catalog_for_chunks, KnowledgeCatalog, PreparationGaps},
        collection_scope,
        contracts::{ScopeKind, StudyScope},
    },
    study_materials::composition::collection_title,
    task_activity::ActivityGuard,
    task_cancellation::CancellationSignal,
    util::{unix_time_millis, validate_entity_id},
};

const PURPOSE: &str = "lesson_authoring";
/// How much of each part's summary the course outline shows.
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetSubjectCourseWritingProgressInput {
    pub operation_id: String,
}

/// How far a course being written is. Advisory: the course is saved only
/// when every lesson is written.
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseWritingProgressDto {
    /// The lessons the course will have; absent while the parts are planned.
    pub planned: Option<usize>,
    pub written: usize,
}

#[tauri::command]
pub fn get_subject_course_writing_progress(
    input: GetSubjectCourseWritingProgressInput,
) -> Result<CourseWritingProgressDto, CommandError> {
    validate_entity_id(&input.operation_id, "course writing operation")?;
    let active = ACTIVE.lock().map_err(|_| registry_error())?;
    let writing = active
        .values()
        .find(|writing| writing.operation_id == input.operation_id)
        .ok_or_else(not_running)?;
    Ok(writing.progress.snapshot())
}

#[tauri::command]
pub fn cancel_subject_course_writing(
    input: CancelSubjectCourseWritingInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.operation_id, "course writing operation")?;
    let active = ACTIVE.lock().map_err(|_| registry_error())?;
    let writing = active
        .values()
        .find(|writing| writing.operation_id == input.operation_id)
        .ok_or_else(not_running)?;
    writing.signal.cancel();
    Ok(())
}

fn not_running() -> CommandError {
    CommandError::new(
        "lesson_writing_not_running",
        "That course is no longer being written.",
    )
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
    let progress = guard.progress();
    let mut diagnostics = generation::Operation::begin(
        database,
        Some(&input.operation_id),
        None,
        Some(&input.subject_id),
        PURPOSE,
    )?;
    let result = write_lessons(
        database,
        runtime,
        &input,
        diagnostics.context(),
        &signal,
        &progress,
    )
    .await;
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
    progress: &CourseProgress,
) -> Result<(), CommandError> {
    let subject_id = input.subject_id.as_str();
    let plans = database.with_connection(|connection| subject_plans(connection, subject_id))?;
    if plans.is_empty() {
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
    let mut parts = Vec::new();
    for (document_id, plan_id) in &plans {
        let plan = load_active_document_learning_plan(database, document_id)?
            .filter(|plan| plan.id == *plan_id)
            .ok_or_else(source_changed)?;
        parts.extend(document_parts(database, &task, &plan)?);
    }
    ensure_running(signal)?;
    if parts.is_empty() {
        return Err(nothing_to_teach());
    }
    if parts.len() > usize::try_from(MAX_LESSONS_PER_SUBJECT).unwrap_or(usize::MAX) {
        return Err(CommandError::new(
            "lesson_limit",
            format!("A subject can hold up to {MAX_LESSONS_PER_SUBJECT} lessons."),
        ));
    }
    progress.plan(parts.len());
    let subject_title = collection_title(database, subject_id)?;
    let course = CourseRequest {
        database,
        runtime,
        task: &task,
        subject_title: &subject_title,
        operation_id: &input.operation_id,
        parts: &parts,
        signal,
    };
    let parallelism = document_request_parallelism(&task.network_scope).clamp(1, parts.len());
    let mut lessons = vec![None; parts.len()];
    let mut first_error: Option<(usize, CommandError)> = None;
    run_bounded_provider_queue(
        parts.len(),
        parallelism,
        |index| {
            let course = &course;
            async move { (index, course.write(index).await) }
        },
        |(index, result)| {
            Ok::<_, CommandError>(match result {
                Ok(lesson) => {
                    lessons[index] = Some(lesson);
                    progress.wrote_one();
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
        // The documents and their plans must still be the ones the lessons
        // were written from.
        if subject_plans(connection, subject_id)? != plans {
            return Err(source_changed());
        }
        create_generated_course(connection, subject_id, &lessons, now).map(|_| ())
    })
}

/// The subject's documents in order, each with its active learning plan.
/// Every document needs one: a course is never written from part of a subject.
fn subject_plans(
    connection: &Connection,
    subject_id: &str,
) -> Result<Vec<(String, String)>, CommandError> {
    let scope = StudyScope::single(ScopeKind::Collection, Some(subject_id.to_string()));
    collection_scope::documents(connection, &scope)?
        .into_iter()
        .map(|document_id| {
            let plan_id = connection
                .query_row(
                    "SELECT id FROM document_learning_plans
                     WHERE document_id = ?1 AND state = 'active'",
                    [&document_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(CommandError::database)?
                .ok_or_else(|| {
                    CommandError::new(
                        "lesson_documents_unprepared",
                        "Finish processing every document in this subject before writing its lessons.",
                    )
                })?;
            Ok((document_id, plan_id))
        })
        .collect()
}

/// One part of the course: a top-level topic of a document's plan with its
/// subtopics, and the knowledge it teaches.
struct CoursePart {
    document_id: String,
    plan_id: String,
    topic_ids: Vec<String>,
    title: String,
    summary: String,
    objectives: String,
    catalog: KnowledgeCatalog,
}

/// A document's parts in plan order. Parts with no extracted knowledge are
/// left out; shared units go to the part drawn from fewer source chunks.
fn document_parts(
    database: &Database,
    task: &DocumentTaskContext,
    plan: &DocumentLearningPlan,
) -> Result<Vec<CoursePart>, CommandError> {
    let groups = topic_groups(plan);
    let chunks = groups
        .iter()
        .map(|topics| {
            topics
                .iter()
                .flat_map(|topic| topic.evidence_chunk_indices.iter())
                .map(|index| i64::try_from(*index).map_err(|_| source_changed()))
                .collect::<Result<BTreeSet<_>, _>>()
                .map(|indices| indices.into_iter().collect::<Vec<_>>())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let gaps = PreparationGaps::default();
    let catalogs = chunks
        .iter()
        .map(|indices| catalog_for_chunks(database, &plan.document_id, indices, task, &gaps))
        .collect::<Result<Vec<_>, _>>()?;
    let sizes = chunks.iter().map(Vec::len).collect::<Vec<_>>();
    let (catalogs, _) = ownership::by_size(&sizes, catalogs);
    Ok(groups
        .into_iter()
        .zip(catalogs)
        .filter(|(_, catalog)| !catalog.units.is_empty())
        .map(|(topics, catalog)| {
            let root = topics[0];
            let subtopics = topics[1..]
                .iter()
                .map(|topic| topic.title.as_str())
                .collect::<Vec<_>>();
            let summary = if subtopics.is_empty() {
                root.summary.clone()
            } else {
                format!("{} It also covers: {}.", root.summary, subtopics.join("; "))
            };
            let mut seen = BTreeSet::new();
            let objectives = topics
                .iter()
                .flat_map(|topic| topic.learning_objectives.iter())
                .filter(|objective| seen.insert(objective.as_str()))
                .map(|objective| format!("- {objective}"))
                .collect::<Vec<_>>()
                .join("\n");
            CoursePart {
                document_id: plan.document_id.clone(),
                plan_id: plan.id.clone(),
                topic_ids: topics.iter().map(|topic| topic.id.clone()).collect(),
                title: root.title.clone(),
                summary,
                objectives,
                catalog,
            }
        })
        .collect())
}

/// The plan's topics grouped under their top-level topic, in plan order,
/// each group starting with that top-level topic.
fn topic_groups(plan: &DocumentLearningPlan) -> Vec<Vec<&LearningTopic>> {
    let by_id = plan
        .topics
        .iter()
        .map(|topic| (topic.id.as_str(), topic))
        .collect::<HashMap<_, _>>();
    let root = |topic: &LearningTopic| {
        let mut current = topic;
        // The plan compiler forbids cycles; the bound keeps a damaged plan
        // from looping.
        for _ in 0..plan.topics.len() {
            match current
                .parent_topic_id
                .as_deref()
                .and_then(|id| by_id.get(id))
            {
                Some(parent) => current = parent,
                None => break,
            }
        }
        current.id.clone()
    };
    let mut topics = plan.topics.iter().collect::<Vec<_>>();
    topics.sort_by_key(|topic| topic.sort_index);
    let mut groups = Vec::<Vec<&LearningTopic>>::new();
    let mut group_of = HashMap::<String, usize>::new();
    for topic in topics.iter().filter(|topic| root(topic) == topic.id) {
        group_of.insert(topic.id.clone(), groups.len());
        groups.push(vec![*topic]);
    }
    for topic in topics.iter().filter(|topic| root(topic) != topic.id) {
        groups[group_of[&root(topic)]].push(topic);
    }
    groups
}

struct CourseRequest<'a> {
    database: &'a Database,
    runtime: &'a AiRuntimeState,
    task: &'a DocumentTaskContext,
    subject_title: &'a str,
    operation_id: &'a str,
    parts: &'a [CoursePart],
    signal: &'a CancellationSignal,
}

impl CourseRequest<'_> {
    async fn write(&self, index: usize) -> Result<(LessonContent, Value), CommandError> {
        let part = &self.parts[index];
        ensure_running(self.signal)?;
        self.task.ensure_current(self.database)?;
        let catalog = part.catalog.fingerprint()?;
        let mut task = self.task.fork_usage();
        task.diagnostic_window_key = Some(format!(
            "{:x}",
            Sha256::digest(
                format!("lesson|{}|{}|{catalog}", part.plan_id, part.topic_ids[0]).as_bytes()
            )
        ));
        let outline = course_outline(self.parts, index);
        let lesson = generate_lesson(
            self.runtime,
            &task,
            LessonAuthoring {
                subject_title: self.subject_title,
                topic_title: &part.title,
                topic_summary: &part.summary,
                objectives: &part.objectives,
                course_outline: &outline,
                knowledge: &part.catalog,
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
            "documentId": part.document_id,
            "learningPlanId": part.plan_id,
            "topicIds": part.topic_ids,
            "catalogFingerprint": catalog,
            "totalTokens": total_tokens,
            "usageComplete": usage_complete,
        });
        Ok((lesson, provenance))
    }
}

/// Each lesson's title and the start of what its part covers, one per line,
/// with the lesson being written marked.
fn course_outline(parts: &[CoursePart], current: usize) -> String {
    parts
        .iter()
        .enumerate()
        .map(|(index, part)| {
            let covers = part
                .summary
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
            format!("{}. {}: {covers}{marker}", index + 1, part.title)
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

fn source_changed() -> CommandError {
    CommandError::new(
        "lesson_source_changed",
        "This subject's documents changed while its lessons were being written. Try again.",
    )
}

fn nothing_to_teach() -> CommandError {
    CommandError::new(
        "lesson_course_empty",
        "This subject has no processed documents to write lessons from yet.",
    )
}

/// Subjects whose course is being written, with the operation writing it.
static ACTIVE: LazyLock<Mutex<HashMap<String, Writing>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

struct Writing {
    operation_id: String,
    signal: CancellationSignal,
    progress: Arc<CourseProgress>,
}

/// Lessons planned and written so far; zero planned until the parts are known.
#[derive(Default)]
struct CourseProgress {
    planned: AtomicUsize,
    written: AtomicUsize,
}

impl CourseProgress {
    fn plan(&self, lessons: usize) {
        self.planned.store(lessons, Ordering::Relaxed);
    }

    fn wrote_one(&self) {
        self.written.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> CourseWritingProgressDto {
        let planned = self.planned.load(Ordering::Relaxed);
        CourseWritingProgressDto {
            planned: (planned > 0).then_some(planned),
            written: self.written.load(Ordering::Relaxed).min(planned),
        }
    }
}

/// One course writer per subject. Dropping the guard stops its requests.
struct Guard {
    subject_id: String,
    signal: CancellationSignal,
    progress: Arc<CourseProgress>,
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
                .any(|writing| writing.operation_id == operation_id)
        {
            return Err(CommandError::new(
                "lesson_writing_already_running",
                "This subject's course is already being written.",
            ));
        }
        let progress = Arc::new(CourseProgress::default());
        active.insert(
            subject_id.to_string(),
            Writing {
                operation_id: operation_id.to_string(),
                signal: signal.clone(),
                progress: Arc::clone(&progress),
            },
        );
        Ok(Self {
            subject_id: subject_id.to_string(),
            signal,
            progress,
            _activity: activity,
        })
    }

    fn signal(&self) -> CancellationSignal {
        self.signal.clone()
    }

    fn progress(&self) -> Arc<CourseProgress> {
        Arc::clone(&self.progress)
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
