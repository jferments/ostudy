//! Instrument complete authoring tasks, including native result admission.
//! The original modules retain prompts, decoding, evidence checks and retries.
use std::future::Future;

use serde_json::Value;

use crate::{
    ai::generation::outcomes::authoring::{observe, Kind},
    document_learning::{DocumentLearningPlan, LearningTopic},
    document_semantics::SemanticWindow,
    document_study_guides::{
        editorial::StudyGuideEditorial, GroundedStudyGuideSection, StudyGuideOverview,
        StudyGuideSectionContent,
    },
    error::CommandError,
    lessons::{content::LessonContent, podcasts::PodcastScript, teaching::LectureScript},
    task_cancellation::CancellationSignal,
};

use super::{
    flashcards, lecture_script, lesson, podcast_script, study_guide, AiRuntimeState,
    AuthoringEvidence, CollectionStudyGuideAuthoring, DocumentOutline, DocumentTaskContext,
    FlashcardGenerationLimits, LectureScriptAuthoring, LessonAuthoring, PodcastScriptAuthoring,
};

pub(crate) fn generate_window_flashcards<'a>(
    runtime: &'a AiRuntimeState,
    task: &'a DocumentTaskContext,
    document_title: &'a str,
    outline: &'a DocumentOutline,
    window: &'a SemanticWindow,
    evidence: AuthoringEvidence<'a>,
    limits: FlashcardGenerationLimits,
) -> impl Future<Output = Result<Vec<Value>, CommandError>> + 'a {
    observe(
        task.diagnostics.as_ref(),
        task.diagnostic_window_key.as_deref(),
        Kind::Flashcards,
        flashcards::generate_window_flashcards(
            runtime,
            task,
            document_title,
            outline,
            window,
            evidence,
            limits,
        ),
        Vec::len,
    )
}

pub(crate) fn generate_study_guide_topic_section<'a>(
    runtime: &'a AiRuntimeState,
    task: &'a DocumentTaskContext,
    document_title: &'a str,
    plan: &'a DocumentLearningPlan,
    topic: &'a LearningTopic,
    source_text: &'a str,
    evidence: AuthoringEvidence<'a>,
) -> impl Future<Output = Result<GroundedStudyGuideSection, CommandError>> + 'a {
    observe(
        task.diagnostics.as_ref(),
        task.diagnostic_window_key.as_deref(),
        Kind::GuideSection,
        study_guide::generate_study_guide_topic_section(
            runtime,
            task,
            document_title,
            plan,
            topic,
            source_text,
            evidence,
        ),
        |_| 1,
    )
}

pub(crate) fn generate_document_study_guide_editorial<'a>(
    runtime: &'a AiRuntimeState,
    task: &'a DocumentTaskContext,
    document_title: &'a str,
    plan: &'a DocumentLearningPlan,
    sections: &'a [&'a StudyGuideSectionContent],
    cancellation: &'a CancellationSignal,
) -> impl Future<Output = Result<(StudyGuideOverview, StudyGuideEditorial), CommandError>> + 'a {
    observe(
        task.diagnostics.as_ref(),
        task.diagnostic_window_key.as_deref(),
        Kind::GuideEditorial,
        study_guide::generate_document_study_guide_editorial(
            runtime,
            task,
            document_title,
            plan,
            sections,
            cancellation,
        ),
        |_| 1,
    )
}

pub(crate) fn generate_collection_study_guide_topic_section<'a>(
    runtime: &'a AiRuntimeState,
    task: &'a DocumentTaskContext,
    collection_title: &'a str,
    authoring: CollectionStudyGuideAuthoring<'a>,
    cancellation: &'a CancellationSignal,
) -> impl Future<Output = Result<GroundedStudyGuideSection, CommandError>> + 'a {
    observe(
        task.diagnostics.as_ref(),
        task.diagnostic_window_key.as_deref(),
        Kind::GuideSection,
        study_guide::generate_collection_study_guide_topic_section(
            runtime,
            task,
            collection_title,
            authoring,
            cancellation,
        ),
        |_| 1,
    )
}

pub(crate) fn generate_lesson<'a>(
    runtime: &'a AiRuntimeState,
    task: &'a DocumentTaskContext,
    authoring: LessonAuthoring<'a>,
    cancellation: &'a CancellationSignal,
) -> impl Future<Output = Result<LessonContent, CommandError>> + 'a {
    observe(
        task.diagnostics.as_ref(),
        task.diagnostic_window_key.as_deref(),
        Kind::Lesson,
        lesson::generate_lesson(runtime, task, authoring, cancellation),
        |_| 1,
    )
}

pub(crate) fn generate_lecture_script<'a>(
    runtime: &'a AiRuntimeState,
    task: &'a DocumentTaskContext,
    authoring: LectureScriptAuthoring<'a>,
    cancellation: &'a CancellationSignal,
) -> impl Future<Output = Result<LectureScript, CommandError>> + 'a {
    observe(
        task.diagnostics.as_ref(),
        task.diagnostic_window_key.as_deref(),
        Kind::LectureScript,
        lecture_script::generate_lecture_script(runtime, task, authoring, cancellation),
        |_| 1,
    )
}

pub(crate) fn generate_podcast_script<'a>(
    runtime: &'a AiRuntimeState,
    task: &'a DocumentTaskContext,
    authoring: PodcastScriptAuthoring<'a>,
    cancellation: &'a CancellationSignal,
) -> impl Future<Output = Result<PodcastScript, CommandError>> + 'a {
    observe(
        task.diagnostics.as_ref(),
        task.diagnostic_window_key.as_deref(),
        Kind::PodcastScript,
        podcast_script::generate_podcast_script(runtime, task, authoring, cancellation),
        |_| 1,
    )
}
