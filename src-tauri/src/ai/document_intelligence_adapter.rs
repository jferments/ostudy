use crate::{
    document_semantics::SemanticWindow,
    error::CommandError,
    task_cancellation::set_current_task_phase,
};

use super::{catalog, endpoint_policy, gateway, profiles, secrets, AiRuntimeState};

#[path = "document_intelligence.rs"]
mod implementation;

pub(crate) use implementation::{
    curate_document_cards, generate_window_flashcards, load_document_task_context, DocumentOutline,
    DocumentTaskContext, OutlineBuildSources, SourcePageImage, OUTLINE_PROMPT_VERSION,
    OUTLINE_SCHEMA_VERSION,
};

const VISUAL_ONLY_MARKER: &str = "has no extractable structured text";

/// Build the reusable global outline used by all later generation windows.
///
/// Text-bearing PDFs request independent local outline fragments with bounded
/// provider concurrency. The first OpenRouter request is deliberately warmed on
/// its own so the stable document `session_id` can establish sticky routing
/// before later windows fan out. Completed fragments are persisted serially,
/// then the existing deterministic Rust assembly/global-synthesis path consumes
/// them as cached fragments so request completion order cannot change hierarchy.
pub(crate) async fn build_document_outline<F>(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    page_count: i64,
    windows: &[SemanticWindow],
    cached_fragments: &[Option<DocumentOutline>],
    mut persist_fragment: F,
) -> Result<DocumentOutline, CommandError>
where
    F: FnMut(&SemanticWindow, &DocumentOutline) -> Result<(), CommandError>,
{
    let visual_only = !windows.is_empty()
        && windows
            .iter()
            .all(|window| window.source_text.contains(VISUAL_ONLY_MARKER));

    if visual_only {
        if !task.include_page_images {
            return Err(CommandError::new(
                "document_requires_image_input",
                "This PDF has no extractable text. Choose a verified image-capable llama.cpp flashcard-generation profile and retry.",
            ));
        }
        return Ok(DocumentOutline {
            summary: format!(
                "{document_title} is an image-only PDF. Rendered pages are the authoritative source for flashcard generation."
            ),
            learning_objectives: vec![
                "Identify and understand the important concepts, relationships, diagrams, tables, and equations shown in the rendered document pages."
                    .to_string(),
            ],
            key_concepts: Vec::new(),
            sections: Vec::new(),
        });
    }

    if windows.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The PDF did not contain source windows that ostudy could analyze.",
        ));
    }

    let mut resolved = cached_fragments.to_vec();
    resolved.resize_with(windows.len(), || None);
    let missing = resolved
        .iter()
        .enumerate()
        .filter_map(|(index, fragment)| fragment.is_none().then_some(index))
        .collect::<Vec<_>>();
    let parallelism = super::concurrency::document_request_parallelism(&task.provider_kind)
        .clamp(1, windows.len());
    crate::logging::info(
        "document_outline_execution_plan",
        format!(
            "provider={} model={} windows={} cached={} parallelism={} sticky_warmup={}",
            task.provider_kind,
            task.model_id,
            windows.len(),
            windows.len().saturating_sub(missing.len()),
            parallelism,
            task.provider_kind == "openrouter" && missing.len() > 1,
        ),
    );

    let mut completed = windows.len().saturating_sub(missing.len());
    let mut cursor = 0_usize;
    let warm_first = task.provider_kind == "openrouter" && missing.len() > 1;
    while cursor < missing.len() {
        let batch_size = if warm_first && cursor == 0 {
            1
        } else {
            parallelism
        };
        let end = (cursor + batch_size).min(missing.len());
        let batch = &missing[cursor..end];
        let first = &windows[batch[0]];
        let last = &windows[*batch.last().unwrap_or(&batch[0])];
        set_current_task_phase(
            "outline",
            Some(batch[0] + 1),
            completed,
            windows.len(),
            Some(first.page_start),
            Some(last.page_end),
            Some(first.partition_index + 1),
        );

        let i0 = batch.first().copied();
        let i1 = batch.get(1).copied();
        let i2 = batch.get(2).copied();
        let i3 = batch.get(3).copied();
        let (r0, r1, r2, r3) = tokio::join!(
            outline_slot(runtime, task, document_title, page_count, windows, i0),
            outline_slot(runtime, task, document_title, page_count, windows, i1),
            outline_slot(runtime, task, document_title, page_count, windows, i2),
            outline_slot(runtime, task, document_title, page_count, windows, i3),
        );

        let mut first_error = None;
        for (index, result) in [r0, r1, r2, r3].into_iter().flatten() {
            match result {
                Ok(fragment) => {
                    persist_fragment(&windows[index], &fragment)?;
                    resolved[index] = Some(fragment);
                    completed = completed.saturating_add(1);
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        cursor = end;
    }

    // All local fragments are now durable and ordered by source-window index.
    // Feed them through the existing deterministic assembler as cache hits; it
    // performs no local-fragment provider requests and retains the bounded global
    // synthesis/fallback behavior.
    let images = vec![Vec::<SourcePageImage>::new(); windows.len()];
    implementation::build_document_outline(
        runtime,
        task,
        document_title,
        page_count,
        OutlineBuildSources {
            windows,
            cached_fragments: &resolved,
            window_images: &images,
        },
        |_window, _fragment| Ok(()),
    )
    .await
}

async fn outline_slot(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    page_count: i64,
    windows: &[SemanticWindow],
    index: Option<usize>,
) -> Option<(usize, Result<DocumentOutline, CommandError>)> {
    let index = index?;
    let window = &windows[index];
    crate::logging::info(
        "document_outline_window_started",
        format!(
            "provider={} model={} window={}/{} partition={} pages={}-{} source_tokens={}",
            task.provider_kind,
            task.model_id,
            index + 1,
            windows.len(),
            window.partition_index + 1,
            window.page_start,
            window.page_end,
            window.token_estimate,
        ),
    );
    let result = match implementation::request_outline_fragment(
        runtime,
        task,
        document_title,
        page_count,
        window,
        &[],
    )
    .await
    {
        Ok(fragment) => Ok(fragment),
        Err(error)
            if windows.len() > 1 && implementation::recoverable_outline_window_error(&error) =>
        {
            crate::logging::info(
                "document_outline_window_native_fallback",
                format!(
                    "window={}/{} partition={} pages={}-{} code={} detail={}",
                    index + 1,
                    windows.len(),
                    window.partition_index + 1,
                    window.page_start,
                    window.page_end,
                    error.code,
                    error.message.chars().take(400).collect::<String>(),
                ),
            );
            Ok(implementation::native_outline_fallback(
                document_title,
                window,
            ))
        }
        Err(error) => {
            crate::logging::error(
                "document_outline_window_failed",
                format!(
                    "provider={} model={} window={}/{} partition={} pages={}-{} source_tokens={} code={} detail={}",
                    task.provider_kind,
                    task.model_id,
                    index + 1,
                    windows.len(),
                    window.partition_index + 1,
                    window.page_start,
                    window.page_end,
                    window.token_estimate,
                    error.code,
                    error.message.chars().take(600).collect::<String>(),
                ),
            );
            Err(CommandError::new(
                error.code,
                format!(
                    "Outline pass failed in source window {}/{} (partition {}, PDF pages {}–{}, about {} source tokens): {}",
                    index + 1,
                    windows.len(),
                    window.partition_index + 1,
                    window.page_start,
                    window.page_end,
                    window.token_estimate,
                    error.message,
                ),
            ))
        }
    };
    Some((index, result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_only_marker_matches_extraction_fallback_contract() {
        assert!(
            "Rendered PDF page 2 has no extractable structured text.".contains(VISUAL_ONLY_MARKER)
        );
    }
}
