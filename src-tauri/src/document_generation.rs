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

pub(crate) async fn execute_generation_windows(
    input: GenerationExecution<'_>,
) -> Result<Vec<Value>, CommandError> {
    let renders = if input.task.include_page_images {
        document_store::load_source_renders(input.database, input.document_id)?
    } else {
        Vec::new()
    };
    let mut cached = input.cached_cards;
    if cached.len() < input.windows.len() {
        cached.resize_with(input.windows.len(), || None);
    }

    let mut generated = Vec::new();
    for (position, window) in input.windows.iter().enumerate() {
        let cards = if let Some(cards) = cached.get_mut(position).and_then(Option::take) {
            crate::logging::info(
                "document_generation_window_reused",
                format!(
                    "document={} window={}/{} partition={} locator={} cards={}",
                    input.document_id,
                    window.window_index + 1,
                    input.windows.len(),
                    window.partition_index + 1,
                    window.source_locator.stable_identity(),
                    cards.len(),
                ),
            );
            cards
        } else {
            document_store::mark_window_running(
                input.database,
                input.document_id,
                input.job_id,
                window.window_index,
            )?;
            let images = if input.task.include_page_images {
                document_store::load_window_images(
                    input.paths,
                    &renders,
                    window,
                    input.plan.max_images_per_window,
                )?
            } else {
                Vec::new()
            };
            let cards = generate_window_with_recovery(
                input.runtime,
                input.task,
                input.document_title,
                input.outline,
                window,
                &images,
                input.plan.max_cards_per_window,
                input.diagnostic_prefix,
            )
            .await?;
            if let (Some(contract), false) = (input.checkpoint_contract, cards.is_empty()) {
                document_checkpoints::persist_generation_cards_best_effort(
                    input.database,
                    input.document_id,
                    contract,
                    window,
                    &cards,
                );
            } else if let Some(contract) = input.checkpoint_contract {
                // Empty output is still a completed result and is safe to cache;
                // retaining it prevents repeatedly paying for a source window
                // whose correct answer is no cards.
                document_checkpoints::persist_generation_cards_best_effort(
                    input.database,
                    input.document_id,
                    contract,
                    window,
                    &cards,
                );
            }
            cards
        };

        if generated.len().saturating_add(cards.len()) > MAX_GENERATED_CARDS {
            return Err(CommandError::new(
                "too_many_generated_cards",
                format!("The model generated more than {MAX_GENERATED_CARDS} cards from this document."),
            ));
        }
        document_store::mark_window_complete(
            input.database,
            input.document_id,
            input.job_id,
            window,
            cards.len(),
        )?;
        document_store::update_generation_progress(
            input.database,
            input.document_id,
            position + 1,
        )?;
        generated.extend(cards);
    }
    Ok(generated)
}

pub(crate) async fn generate_window_with_recovery(
    runtime: &AiRuntimeState,
    task: &document_intelligence::DocumentTaskContext,
    document_title: &str,
    outline: &document_intelligence::DocumentOutline,
    window: &SemanticWindow,
    images: &[document_intelligence::SourcePageImage],
    maximum_cards: usize,
    diagnostic_prefix: &str,
) -> Result<Vec<Value>, CommandError> {
    let first = document_intelligence::generate_window_flashcards(
        runtime,
        task,
        document_title,
        outline,
        window,
        images,
        maximum_cards.max(1),
    )
    .await;
    match first {
        Err(error)
            if maximum_cards > 2
                && (error.code == "ai_document_output_truncated"
                    || (error.code == "ai_document_output_invalid"
                        && error.message.contains("more than"))) =>
        {
            let reduced_budget = (maximum_cards / 2).max(2);
            crate::logging::info(
                "document_generation_window_compact_retry",
                format!(
                    "path={diagnostic_prefix} window={} partition={} locator={} old_budget={} new_budget={} code={}",
                    window.window_index + 1,
                    window.partition_index + 1,
                    window.source_locator.stable_identity(),
                    maximum_cards,
                    reduced_budget,
                    error.code,
                ),
            );
            document_intelligence::generate_window_flashcards(
                runtime,
                task,
                document_title,
                outline,
                window,
                images,
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
}
