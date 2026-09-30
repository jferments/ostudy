//! Plan visible output from this topic's evidence, then reserve recovery against
//! the provider's actual context budget. No page-count or style admission cap.
use serde_json::Value;

use super::{
    ensure_context_budget, protocol, AuthoringEvidence, CommandError, DocumentTaskContext,
    JsonTaskRequest, WritingBudget,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StudyGuideCompletionPlan {
    pub(super) initial_visible_tokens: usize,
    pub(super) repair_visible_tokens: usize,
}

pub(super) const DOCUMENT_GUIDE_COMPLETION_PLAN: StudyGuideCompletionPlan =
    StudyGuideCompletionPlan {
        initial_visible_tokens: 4_096,
        repair_visible_tokens: 6_144,
    };

pub(super) const COLLECTION_GUIDE_COMPLETION_PLANS: [StudyGuideCompletionPlan; 4] = [
    DOCUMENT_GUIDE_COMPLETION_PLAN,
    StudyGuideCompletionPlan {
        initial_visible_tokens: 3_072,
        repair_visible_tokens: 4_096,
    },
    StudyGuideCompletionPlan {
        initial_visible_tokens: 2_048,
        repair_visible_tokens: 3_072,
    },
    StudyGuideCompletionPlan {
        initial_visible_tokens: 1_536,
        repair_visible_tokens: 2_048,
    },
];

impl StudyGuideCompletionPlan {
    fn for_writing_budget(budget: WritingBudget) -> Self {
        let estimated = budget.target_chars.div_ceil(2).saturating_add(768);
        let initial_visible_tokens = [2_048, 3_072, 4_096, 6_144]
            .into_iter()
            .find(|tokens| *tokens >= estimated)
            .unwrap_or(6_144);
        Self {
            initial_visible_tokens,
            repair_visible_tokens: initial_visible_tokens + 2_048,
        }
    }

    pub(super) fn instructions(self) -> String {
        format!(
            "Initial output plan: the visible completion allowance is {} tokens. Complete the required JSON object before adding optional richness. keyPoints are optional; unused arrays must be []. Teach supported mechanisms, relationships and necessary exceptions without repeating facts across categories. Use the accompanying source-aware writing target as guidance, never a reason to remove a condition. Never cut a qualification, numerical sign, unit or exception to meet an editorial target.",
            self.initial_visible_tokens
        )
    }

    pub(super) fn recovery_instructions(self) -> String {
        format!(
            "The prior attempt reached its completion limit. Recovery has {} visible output tokens, not the original ceiling. Return one complete JSON object using the compact recovery schema. Preserve the supported core concepts and their complete qualifications in coherent prose. Use only study aids that carry distinct necessary meaning, including supplied visual evidence when needed; do not fill every optional category. Select only supplied knowledge aliases actually explained. Do not add commentary about this retry or invent support.",
            self.repair_visible_tokens
        )
    }
}

pub(super) fn fit_guide_completion_plan(
    task: &DocumentTaskContext,
    system: &str,
    user: &str,
    evidence: AuthoringEvidence<'_>,
) -> Result<StudyGuideCompletionPlan, CommandError> {
    let schema = protocol::schema(evidence.knowledge, !evidence.images.is_empty());
    let writing = WritingBudget::for_knowledge(evidence.knowledge);
    let preferred = StudyGuideCompletionPlan::for_writing_budget(writing);
    choose_plan(preferred, evidence.knowledge.units.len(), |plan| {
        let planned_user = format!(
            "{user}\n\n{}\n\n{}\n\n{}",
            plan.instructions(),
            writing
                .fit_output(plan.initial_visible_tokens)
                .instructions(),
            plan.recovery_instructions()
        );
        ensure_context_budget(
            task,
            &JsonTaskRequest {
                system_prompt: system,
                user_text: &planned_user,
                schema: &schema,
                max_tokens: task.completion_token_ceiling(plan.repair_visible_tokens),
                images: evidence.images,
            },
        )
    })
}

fn choose_plan(
    preferred: StudyGuideCompletionPlan,
    unit_count: usize,
    mut preflight: impl FnMut(StudyGuideCompletionPlan) -> Result<(), CommandError>,
) -> Result<StudyGuideCompletionPlan, CommandError> {
    // Split multi-unit input rather than starving output to force it into a
    // smaller context. The caller retains every unit and its evidence.
    let choices =
        std::iter::once(preferred).chain(COLLECTION_GUIDE_COMPLETION_PLANS.into_iter().filter(
            |plan| unit_count <= 1 && plan.repair_visible_tokens < preferred.repair_visible_tokens,
        ));
    for plan in choices {
        match preflight(plan) {
            Ok(()) => return Ok(plan),
            Err(error) if error.code == "ai_document_context_budget" => continue,
            Err(error) => return Err(error),
        }
    }
    Err(CommandError::new(
        "ai_document_context_budget",
        "This complete guide task must be partitioned before authoring; no knowledge units were discarded and no provider request was sent.",
    ))
}

/// Reduce optional breadth after truncation, not evidence or field capacity.
/// A necessary visual/mechanism explanation must remain representable.
pub(super) fn compact_recovery_schema(schema: &Value) -> Value {
    let mut compact = schema.clone();
    for field in [
        "explanations",
        "processSteps",
        "comparisons",
        "commonMisconceptions",
        "callouts",
        "tables",
        "workedExamples",
        "retrievalChecks",
        "visualExplanations",
    ] {
        let bound = &mut compact["properties"]["content"]["properties"][field]["maxItems"];
        if let Some(current) = bound.as_u64() {
            *bound = Value::from(current.min(2));
        }
    }
    compact
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn output_plan_scales_with_evidence_and_reserves_recovery() {
        let small =
            StudyGuideCompletionPlan::for_writing_budget(WritingBudget::from_signals(1, 120));
        let dense =
            StudyGuideCompletionPlan::for_writing_budget(WritingBudget::from_signals(13, 24_000));
        assert!(dense.initial_visible_tokens > small.initial_visible_tokens);
        for plan in [small, dense, DOCUMENT_GUIDE_COMPLETION_PLAN] {
            assert!(plan.repair_visible_tokens > plan.initial_visible_tokens);
            assert!(plan.repair_visible_tokens <= 8_192);
        }
    }

    #[test]
    fn small_context_fallback_is_only_for_indivisible_knowledge() {
        let fits = |plan: StudyGuideCompletionPlan| {
            if plan.repair_visible_tokens <= 3_072 {
                Ok(())
            } else {
                Err(CommandError::new("ai_document_context_budget", "split"))
            }
        };
        assert_eq!(
            choose_plan(DOCUMENT_GUIDE_COMPLETION_PLAN, 1, fits)
                .unwrap()
                .repair_visible_tokens,
            3_072
        );
        assert_eq!(
            choose_plan(DOCUMENT_GUIDE_COMPLETION_PLAN, 2, fits)
                .unwrap_err()
                .code,
            "ai_document_context_budget"
        );
        assert_eq!(
            choose_plan(DOCUMENT_GUIDE_COMPLETION_PLAN, 1, |_| {
                Err(CommandError::new("ai_profile_changed", "changed"))
            })
            .unwrap_err()
            .code,
            "ai_profile_changed"
        );
    }

    #[test]
    fn compact_recovery_keeps_required_content_and_support_and_visual_capacity() {
        let catalog = crate::knowledge::authoring::tests::catalog_fixture();
        let schema = protocol::schema(&catalog, true);
        let compact = compact_recovery_schema(&schema);
        assert_eq!(compact["required"], schema["required"]);
        assert_eq!(
            compact["properties"]["knowledgeUnitIds"],
            schema["properties"]["knowledgeUnitIds"]
        );
        let fields = &compact["properties"]["content"]["properties"];
        assert!(fields["keyPoints"].get("minItems").is_none());
        assert_eq!(fields["tables"]["maxItems"], 2);
        assert_eq!(fields["visualExplanations"]["maxItems"], 2);
        assert_eq!(
            compact["properties"]["content"]["additionalProperties"],
            json!(false)
        );
        assert_eq!(
            compact_recovery_schema(&protocol::schema(&catalog, false))["properties"]["content"]
                ["properties"]["visualExplanations"]["maxItems"],
            0
        );
        assert_eq!(
            fields["overview"]["maxLength"],
            schema["properties"]["content"]["properties"]["overview"]["maxLength"]
        );
    }
}
