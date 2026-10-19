//! Artifact-owned demand for optional structured support. Ordinary extraction,
//! explanations and mixed recall requests do not pay to repair optional debt.
use std::future::Future;

use super::{
    assessment::policy::{GenerationPolicy, QuestionFamily},
    authoring::{self, DocumentPreparation},
};
use crate::error::CommandError;

tokio::task_local! {
    static CURRENT: Option<GenerationPolicy>;
}

pub(crate) fn current() -> Option<GenerationPolicy> {
    CURRENT.try_with(Clone::clone).ok().flatten()
}

pub(crate) fn scope<F: Future>(
    policy: &GenerationPolicy,
    future: F,
) -> impl Future<Output = F::Output> {
    // Weights are preferences, not a requirement to manufacture an ordering or
    // classification board. An interactive-only request cannot use the admitted
    // core as a recall fallback and therefore has an actual support obligation.
    let demand = (!policy.allowed_types.contains(&QuestionFamily::Recall)).then(|| policy.clone());
    CURRENT.scope(demand, Box::pin(future))
}

/// Complete this phase before allocating artifact catalogs/checkpoints or
/// polling parallel card/guide futures. Annotation changes legitimately change
/// unit content hashes; no sibling may still be authoring against the old ones.
pub(crate) async fn prepare_document(input: DocumentPreparation<'_>) -> Result<(), CommandError> {
    input.task.question_policy.validate()?;
    if input
        .task
        .question_policy
        .allowed_types
        .contains(&QuestionFamily::Recall)
    {
        return Ok(());
    }
    let policy = input.task.question_policy.clone();
    scope(&policy, authoring::prepare_document(input))
        .await
        .map(|_| ())
}
