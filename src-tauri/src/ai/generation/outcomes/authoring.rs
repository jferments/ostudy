//! Observe native authoring returns, not artifact publication or factual quality.
//! The caller retains its ordinary Result and its existing recovery policy.
use std::future::Future;

use serde_json::json;

use super::{write, Context, Task};
use crate::error::CommandError;

pub(crate) mod candidates;

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Flashcards,
    GuideSection,
    GuideEditorial,
    CurationSelection,
    Lesson,
    LectureScript,
    PodcastScript,
}

impl Kind {
    pub(crate) fn stage(self) -> &'static str {
        match self {
            Self::Flashcards => "flashcard_authoring",
            Self::GuideSection => "guide_authoring",
            Self::GuideEditorial => "guide_editorial",
            Self::CurationSelection => "flashcard_curation",
            Self::Lesson => "lesson_authoring",
            Self::LectureScript => "lecture_authoring",
            Self::PodcastScript => "podcast_authoring",
        }
    }

    fn items(self) -> &'static str {
        match self {
            Self::Flashcards => "cards",
            Self::GuideSection => "guide_sections",
            Self::GuideEditorial => "guide_editorials",
            Self::CurationSelection => "curation_selections",
            Self::Lesson => "lessons",
            Self::LectureScript => "lecture_scripts",
            Self::PodcastScript => "podcast_scripts",
        }
    }
}

/// Box before constructing the outer async state machine. A failed observer
/// write cannot change the result of paid work or authorize another request.
pub(crate) fn observe<'a, T: 'a, F>(
    context: Option<&'a Context>,
    key: Option<&'a str>,
    kind: Kind,
    future: F,
    count: fn(&T) -> usize,
) -> impl Future<Output = Result<T, CommandError>> + 'a
where
    F: Future<Output = Result<T, CommandError>> + 'a,
{
    let future = Box::pin(future);
    async move {
        let mut task = Task::new(context, kind.stage(), key);
        let result = task.scope(future).await;
        if let Some(state) = task.state.as_ref().and_then(|state| state.lock().ok()) {
            if let Some(id) = &state.last_request {
                write(
                    &state.context,
                    id,
                    "$.nativeAuthoring",
                    json!({
                        "schemaVersion": 1,
                        "state": if result.is_ok() { "returned" } else { "failed" },
                        "itemKind": kind.items(),
                        "returnedItems": result.as_ref().ok().map(count),
                        "persistence": "not_observed",
                    }),
                );
            }
        }
        task.finish(&result);
        result
    }
}

#[cfg(test)]
mod tests;
