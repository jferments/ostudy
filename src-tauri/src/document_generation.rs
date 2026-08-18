//! Shared execution engine for initial and additional document card generation.
//!
//! Initial generation may reuse validated window checkpoints; additional
//! generation normally starts with empty cache input. Both paths otherwise use
//! exactly the same image loading, bounded retry, job transitions, progress,
//! and total-card safety rules.

use serde_json::Value;

use crate::{
    ai::{document_intelligence, AiRuntimeState},
    database::Database,
    document_checkpoints,
    document_scale::DocumentScalePlan,
    document_semantics::SemanticWindow,
    document_store,
    error::CommandError,
    paths::AppPaths,
    task_cancellation::set_current_task_phase,
};

const MAX_GENERATED_CARDS: usize = 10_000;

pub(crate) struct GenerationExecution<'a> {
    pub database: &'a Database,
    pub paths: &'a AppPaths,
    pub runtime: &'a AiRuntimeState,
    pub task: &'a document_intelligence::DocumentTaskContext,
    pub document_id: &'a str,
    pub document_title: &'a str,
    pub outline: &'a document_intelligence::DocumentOutline,
    pub plan: DocumentScalePlan,
    pub windows: &'a [SemanticWindow],
    pub job_id: &'a str,
    pub cached_cards: Vec<Option<Vec<Value>>>,
    pub checkpoint_contract: Option<&'a str>,
    pub diagnostic_prefix: &'a str,
}

struct WindowGenerationRequest<'a> {
    runtime: &'a AiRuntimeState,
    task: &'a document_intelligence::DocumentTaskContext,
    document_title: &'a str,
    outline: &'a document_intelligence::DocumentOutline,
    window: &'a SemanticWindow,
    images: &'a [document_intelligence::SourcePageImage],
    maximum_cards: usize,
    diagnostic_prefix: &'a str,
}

/// Execute independent generation windows with bounded provider concurrency.
///
/// Provider calls are concurrent, but all SQLite/checkpoint writes happen after
/// a batch resolves and in source-window order. This keeps durable progress and
/// generated-card ordering deterministic regardless of network completion order.
pub(crate) async fn execute_generation_windows(
    mut input: GenerationExecution<'_>,
) -> Result<Vec<Value>, CommandError> {
    let renders = if input.task.include_page_images {
        document_store::load_source_renders(input.database, input.document_id)?
    } else {
        Vec::new()
    };
    let mut cached = std::mem::take(&mut input.cached_cards);
    if cached.len() < input.windows.len() {
        cached.resize_with(input.windows.len(), || None);
    }

    let parallelism =
        crate::ai::concurrency::document_request_parallelism(&input.task.provider_kind)
            .clamp(1, input.windows.len().max(1));
    crate::logging::info(
        "document_generation_execution_plan",
        format!(
            "document={} provider={} model={} windows={} parallelism={} sticky_warmup={}",
            input.document_id,
            input.task.provider_kind,
            input.task.model_id,
            input.windows.len(),
            parallelism,
            input.task.provider_kind == "openrouter",
        ),
    );

    let mut by_window = vec![None; input.windows.len()];
    let mut missing = Vec::new();
    let mut completed = 0_usize;
    for (position, window) in input.windows.iter().enumerate() {
        if let Some(cards) = cached.get_mut(position).and_then(Option::take) {
            crate::logging::info(
                "document_generation_window_reused",
                format!(
                    "document={} window={}/{} partition={} locator={} cards={}",
                    input.document_id,
                    window.window_index + 1,
                    input.windows.len(),
                    window.partition_index + 1,
                    window.source_locator().stable_identity(),
                    cards.len(),
                ),
            );
            document_store::mark_window_complete(
                input.database,
                input.document_id,
                input.job_id,
                window,
                cards.len(),
            )?;
            by_window[position] = Some(cards);
            completed = completed.saturating_add(1);
        } else {
            missing.push(position);
        }
    }
    document_store::update_generation_progress(input.database, input.document_id, completed)?;

    let warm_first = input.task.provider_kind == "openrouter" && missing.len() > 1;
    let mut cursor = 0_usize;
    while cursor < missing.len() {
        let batch_size = if warm_first && cursor == 0 {
            1
        } else {
            parallelism
        };
        let end = (cursor + batch_size).min(missing.len());
        let batch = &missing[cursor..end];
        let first = &input.windows[batch[0]];
        let last = &input.windows[*batch.last().unwrap_or(&batch[0])];
        set_current_task_phase(
            "generating",
            Some(batch[0] + 1),
            completed,
            input.windows.len(),
            Some(first.page_start),
            Some(last.page_end),
            Some(first.partition_index + 1),
        );

        let mut images = Vec::with_capacity(batch.len());
        for position in batch.iter().copied() {
            let window = &input.windows[position];
            document_store::mark_window_running(
                input.database,
                input.document_id,
                input.job_id,
                window.window_index,
            )?;
            images.push(if input.task.include_page_images {
                document_store::load_window_images(
                    input.paths,
                    &renders,
                    window,
                    input.plan.max_images_per_window,
                )?
            } else {
                Vec::new()
            });
        }

        let p0 = batch.first().copied();
        let p1 = batch.get(1).copied();
        let p2 = batch.get(2).copied();
        let p3 = batch.get(3).copied();
        let empty: &[document_intelligence::SourcePageImage] = &[];
        let i0 = images.first().map(Vec::as_slice).unwrap_or(empty);
        let i1 = images.get(1).map(Vec::as_slice).unwrap_or(empty);
        let i2 = images.get(2).map(Vec::as_slice).unwrap_or(empty);
        let i3 = images.get(3).map(Vec::as_slice).unwrap_or(empty);
        let (r0, r1, r2, r3) = tokio::join!(
            generation_slot(&input, p0, i0),
            generation_slot(&input, p1, i1),
            generation_slot(&input, p2, i2),
            generation_slot(&input, p3, i3),
        );

        let mut first_error = None;
        for (position, result) in [r0, r1, r2, r3].into_iter().flatten() {
            match result {
                Ok(cards) => {
                    let window = &input.windows[position];
                    if let Some(contract) = input.checkpoint_contract {
                        // Empty output is also a completed result. Caching it
                        // avoids paying again for a window whose correct answer
                        // is no cards.
                        document_checkpoints::persist_generation_cards_best_effort(
                            input.database,
                            input.document_id,
                            contract,
                            window,
                            &cards,
                        );
                    }
                    document_store::mark_window_complete(
                        input.database,
                        input.document_id,
                        input.job_id,
                        window,
                        cards.len(),
                    )?;
                    by_window[position] = Some(cards);
                    completed = completed.saturating_add(1);
                    document_store::update_generation_progress(
                        input.database,
                        input.document_id,
                        completed,
                    )?;
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

    let mut generated = Vec::new();
    for cards in by_window.into_iter().flatten() {
        if generated.len().saturating_add(cards.len()) > MAX_GENERATED_CARDS {
            return Err(CommandError::new(
                "too_many_generated_cards",
                format!(
                    "The model generated more than {MAX_GENERATED_CARDS} cards from this document."
                ),
            ));
        }
        generated.extend(cards);
    }
    set_current_task_phase(
        "curating",
        None,
        input.windows.len(),
        input.windows.len(),
        None,
        None,
        None,
    );
    Ok(generated)
}

async fn generation_slot(
    input: &GenerationExecution<'_>,
    position: Option<usize>,
    images: &[document_intelligence::SourcePageImage],
) -> Option<(usize, Result<Vec<Value>, CommandError>)> {
    let position = position?;
    let window = &input.windows[position];
    Some((
        position,
        generate_window_with_recovery(WindowGenerationRequest {
            runtime: input.runtime,
            task: input.task,
            document_title: input.document_title,
            outline: input.outline,
            window,
            images,
            maximum_cards: input.plan.max_cards_per_window,
            diagnostic_prefix: input.diagnostic_prefix,
        })
        .await,
    ))
}

async fn generate_window_with_recovery(
    input: WindowGenerationRequest<'_>,
) -> Result<Vec<Value>, CommandError> {
    let first = document_intelligence::generate_window_flashcards(
        input.runtime,
        input.task,
        input.document_title,
        input.outline,
        input.window,
        input.images,
        input.maximum_cards.max(1),
    )
    .await;
    match first {
        Err(error)
            if input.maximum_cards > 2
                && (error.code == "ai_document_output_truncated"
                    || (error.code == "ai_document_output_invalid"
                        && error.message.contains("more than"))) =>
        {
            let reduced_budget = (input.maximum_cards / 2).max(2);
            crate::logging::info(
                "document_generation_window_compact_retry",
                format!(
                    "path={} window={} partition={} locator={} old_budget={} new_budget={} code={}",
                    input.diagnostic_prefix,
                    input.window.window_index + 1,
                    input.window.partition_index + 1,
                    input.window.source_locator().stable_identity(),
                    input.maximum_cards,
                    reduced_budget,
                    error.code,
                ),
            );
            document_intelligence::generate_window_flashcards(
                input.runtime,
                input.task,
                input.document_title,
                input.outline,
                input.window,
                input.images,
                reduced_budget,
            )
            .await
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_vector_can_represent_explicit_empty_completed_windows() {
        let cached = [Some(Vec::<Value>::new()), None];
        assert!(cached[0].as_ref().is_some_and(Vec::is_empty));
        assert!(cached[1].is_none());
    }

    #[test]
    fn provider_parallelism_is_bounded_before_window_execution() {
        assert_eq!(
            crate::ai::concurrency::document_request_parallelism("openrouter"),
            4
        );
        assert_eq!(
            crate::ai::concurrency::document_request_parallelism("deepseek"),
            2
        );
        assert_eq!(
            crate::ai::concurrency::document_request_parallelism("llama_cpp"),
            1
        );
    }
}
