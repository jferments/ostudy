//! Format-neutral orchestration for bounded document-intelligence work.
//!
//! This module is the stable application-facing facade. It owns concurrency,
//! checkpoint reuse, deterministic source-order persistence, and coordination.
//! Domain reasoning and provider wire transport live behind explicit private
//! modules so new document formats and learning artifacts can reuse the same
//! source-grounded contracts.

use std::time::Instant;

use crate::{
    document_semantics::SemanticWindow, error::CommandError,
    task_cancellation::set_current_task_phase_range,
};

use super::AiRuntimeState;

mod context;
mod contracts;
mod curation;
mod flashcards;
mod outline;
mod schemas;
mod study_guide;
mod support;
mod transport;

pub(crate) use context::{load_document_task_context, DocumentTaskContext};
pub(crate) use contracts::{DocumentOutline, SourcePageImage};
pub(crate) use curation::curate_document_cards;
pub(crate) use flashcards::{
    flashcard_completion_token_ceiling, generate_window_flashcards, FlashcardGenerationLimits,
    FLASHCARD_PROMPT_VERSION, FLASHCARD_SCHEMA_VERSION,
};
pub(crate) use outline::{OUTLINE_PROMPT_VERSION, OUTLINE_SCHEMA_VERSION};
pub(crate) use study_guide::generate_study_guide_topic_section;

const VISUAL_ONLY_MARKER: &str = "has no extractable structured text";

/// Build the reusable global outline used by later learning-artifact windows.
///
/// Text-bearing documents request independent local outline fragments with
/// bounded provider concurrency. Remote windows deliberately do not share a
/// sticky provider session because each call has distinct canonical source
/// material. Completed fragments are persisted serially in source-window order;
/// deterministic Rust assembly and optional global synthesis run only after all
/// local windows have resolved.
pub(crate) async fn build_document_outline<F>(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    source_unit_count: i64,
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
                "This document has no extractable text. Choose a verified image-capable document-generation profile and retry.",
            ));
        }
        return Ok(DocumentOutline {
            summary: format!(
                "{document_title} is image-only. Rendered visual evidence is the authoritative source for learning-artifact generation."
            ),
            learning_objectives: vec![
                "Identify and understand the important concepts, relationships, diagrams, tables, and equations shown in the rendered document source units."
                    .to_string(),
            ],
            key_concepts: Vec::new(),
            sections: Vec::new(),
        });
    }

    if windows.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The document did not contain canonical source windows that ostudy could analyze.",
        ));
    }

    let mut resolved = cached_fragments.to_vec();
    resolved.resize_with(windows.len(), || None);
    let missing = resolved
        .iter()
        .enumerate()
        .filter_map(|(index, fragment)| fragment.is_none().then_some(index))
        .collect::<Vec<_>>();
    let parallelism = super::concurrency::document_request_parallelism(&task.network_scope)
        .clamp(1, windows.len());
    crate::logging::debug_target(
        "ai.document.outline",
        "document_outline_execution_plan",
        "Prepared bounded outline-provider work without logging document content.",
        serde_json::json!({
            "provider": task.provider_kind.as_str(),
            "model": task.model_id.as_str(),
            "windows": windows.len(),
            "cachedWindows": windows.len().saturating_sub(missing.len()),
            "providerWindows": missing.len(),
            "parallelism": parallelism,
        }),
    );

    let mut completed = windows.len().saturating_sub(missing.len());
    let mut cursor = 0_usize;
    while cursor < missing.len() {
        let batch_size = parallelism;
        let end = (cursor + batch_size).min(missing.len());
        let batch = &missing[cursor..end];
        let first = &windows[batch[0]];
        let last = &windows[*batch.last().unwrap_or(&batch[0])];
        let first_locator = first.source_locator();
        let last_locator = last.source_locator();
        set_current_task_phase_range(
            "outline",
            (Some(batch[0] + 1), batch.last().map(|index| *index + 1)),
            completed,
            windows.len(),
            Some(first_locator.first()),
            Some(last_locator.last()),
            Some(first.partition_index + 1),
        );

        let i0 = batch.first().copied();
        let i1 = batch.get(1).copied();
        let i2 = batch.get(2).copied();
        let i3 = batch.get(3).copied();
        let (r0, r1, r2, r3) = tokio::join!(
            outline_slot(runtime, task, document_title, source_unit_count, windows, i0),
            outline_slot(runtime, task, document_title, source_unit_count, windows, i1),
            outline_slot(runtime, task, document_title, source_unit_count, windows, i2),
            outline_slot(runtime, task, document_title, source_unit_count, windows, i3),
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

    let fragments = resolved
        .into_iter()
        .enumerate()
        .map(|(index, fragment)| {
            fragment.ok_or_else(|| {
                CommandError::new(
                    "document_outline_assembly_incomplete",
                    format!(
                        "Canonical source window {} did not produce or reuse an outline fragment.",
                        index + 1
                    ),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    outline::assemble_document_outline(
        runtime,
        task,
        document_title,
        source_unit_count,
        windows,
        fragments,
    )
    .await
}

async fn outline_slot(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    source_unit_count: i64,
    windows: &[SemanticWindow],
    index: Option<usize>,
) -> Option<(usize, Result<DocumentOutline, CommandError>)> {
    let index = index?;
    let window = &windows[index];
    let locator = window.source_locator();
    crate::logging::debug_target(
        "ai.document.outline",
        "document_outline_window_started",
        "A bounded outline fragment started without logging document content.",
        serde_json::json!({
            "provider": task.provider_kind.as_str(),
            "model": task.model_id.as_str(),
            "window": index + 1,
            "totalWindows": windows.len(),
            "partition": window.partition_index + 1,
            "sourceUnitStart": locator.first(),
            "sourceUnitEnd": locator.last(),
            "sourceTokens": window.token_estimate,
        }),
    );
    let started = Instant::now();
    let result = match outline::request_outline_fragment(
        runtime,
        task,
        document_title,
        source_unit_count,
        window,
        &[],
    )
    .await
    {
        Ok(fragment) => Ok(fragment),
        Err(error) if windows.len() > 1 && outline::recoverable_outline_window_error(&error) => {
            crate::logging::warn_target(
                "ai.document.outline",
                "document_outline_window_native_fallback",
                "Provider outline output remained invalid after bounded repair; ostudy used a deterministic source-derived fallback for this window.",
                serde_json::json!({
                    "window": index + 1,
                    "totalWindows": windows.len(),
                    "partition": window.partition_index + 1,
                    "sourceUnitStart": locator.first(),
                    "sourceUnitEnd": locator.last(),
                    "code": error.code.as_str(),
                }),
            );
            Ok(outline::native_outline_fallback(document_title, window))
        }
        Err(error) => {
            crate::logging::error(
                "document_outline_window_failed",
                format!(
                    "provider={} model={} window={}/{} partition={} locator={} source_tokens={} code={} detail={}",
                    task.provider_kind,
                    task.model_id,
                    index + 1,
                    windows.len(),
                    window.partition_index + 1,
                    locator.stable_identity(),
                    window.token_estimate,
                    error.code,
                    error.message.chars().take(600).collect::<String>(),
                ),
            );
            Err(CommandError::new(
                error.code,
                format!(
                    "Outline pass failed in source window {}/{} (partition {}, source units {}–{}, about {} source tokens): {}",
                    index + 1,
                    windows.len(),
                    window.partition_index + 1,
                    locator.first(),
                    locator.last(),
                    window.token_estimate,
                    error.message,
                ),
            ))
        }
    };
    if let Ok(fragment) = &result {
        crate::logging::debug_target(
            "ai.document.outline",
            "document_outline_window_completed",
            "A bounded outline fragment completed without logging source or model output.",
            serde_json::json!({
                "provider": task.provider_kind.as_str(),
                "model": task.model_id.as_str(),
                "window": index + 1,
                "totalWindows": windows.len(),
                "partition": window.partition_index + 1,
                "sourceUnitStart": locator.first(),
                "sourceUnitEnd": locator.last(),
                "sourceTokens": window.token_estimate,
                "sections": fragment.sections.len(),
                "objectives": fragment.learning_objectives.len(),
                "concepts": fragment.key_concepts.len(),
                "durationMs": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            }),
        );
    }
    Some((index, result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_only_marker_matches_adapter_fallback_contract() {
        assert!(
            "Rendered PDF page 2 has no extractable structured text.".contains(VISUAL_ONLY_MARKER)
        );
    }

    #[test]
    fn public_document_intelligence_facade_has_no_provider_wire_vocabulary() {
        let source = include_str!("mod.rs");
        let production = source
            .split_once("#[cfg(test)]")
            .map_or(source, |(production, _)| production);
        for forbidden in [
            "response_format",
            "tool_choice",
            "chat_template_kwargs",
            "prompt_tokens",
        ] {
            assert!(
                !production.contains(forbidden),
                "unexpected wire term: {forbidden}"
            );
        }
    }
}
