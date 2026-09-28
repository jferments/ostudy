//! A guide request reserves both attempts before dispatch. Hidden reasoning is
//! budgeted by the provider capability owner, separately from visible JSON.
//! Recovery is not an identical replay: it gets more headroom and a lean schema.

use serde_json::Value;

use super::{
    ensure_context_budget, protocol, AuthoringEvidence, CommandError, DocumentTaskContext,
    JsonTaskRequest,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StudyGuideCompletionPlan {
    pub(super) initial_visible_tokens: usize,
    pub(super) repair_visible_tokens: usize,
}

// A document section has the same rich wire contract as a collection section;
// it must not receive half the first-pass allowance merely because of its owner.
pub(super) const DOCUMENT_GUIDE_COMPLETION_PLAN: StudyGuideCompletionPlan =
    StudyGuideCompletionPlan {
        initial_visible_tokens: 6_144,
        repair_visible_tokens: 12_288,
    };

pub(super) const COLLECTION_GUIDE_COMPLETION_PLANS: [StudyGuideCompletionPlan; 4] = [
    DOCUMENT_GUIDE_COMPLETION_PLAN,
    StudyGuideCompletionPlan {
        initial_visible_tokens: 4_096,
        repair_visible_tokens: 8_192,
    },
    StudyGuideCompletionPlan {
        initial_visible_tokens: 3_072,
        repair_visible_tokens: 6_144,
    },
    StudyGuideCompletionPlan {
        initial_visible_tokens: 2_048,
        repair_visible_tokens: 4_096,
    },
];

impl StudyGuideCompletionPlan {
    pub(super) fn instructions(self) -> String {
        format!(
            "Initial output plan: the visible completion allowance is {} tokens. Complete the required JSON object before adding optional richness. Target at most 7,500 human-readable characters for this request, including all categories. Start with one to three short paragraphs of coherent narrative overview. keyPoints are optional and should be empty unless a short list adds distinct scanning value after the prose. Teach central concepts, causal relationships, useful comparisons and decision-relevant exceptions before isolated details. Add only optional study aids that contribute distinct teaching value; unused arrays must be []. Do not fill every category or restate the same fact in several fields. Preserve entire conditions, uncertainty, numerical signs, units, and exceptions; never cut a qualification to meet an editorial target. This writing target is not permission to claim exhaustive coverage or invent source support.",
            self.initial_visible_tokens
        )
    }

    pub(super) fn recovery_instructions(self) -> String {
        format!(
            "The prior attempt reached its completion limit. Recovery has {} visible output tokens, not the original ceiling. Return one complete JSON object using the compact recovery schema. Keep the same supported core concepts and all necessary qualifications in one to three short narrative paragraphs. keyPoints remain optional; use only a few if they add information structure that the prose does not. Keep essential defined importantTerms when useful. The other study-aid arrays must be empty in this recovery; do not repeat the same idea in multiple categories. Keep total human-readable content under 7,500 characters. Select only the supplied knowledge aliases actually explained. Do not add an explanation of this retry or try to mention every supplied unit.",
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
    choose_plan(evidence.knowledge.units.len(), |plan| {
        // Reserve the exact truncation-recovery suffix and the larger output
        // before assigning partitions. The ordinary request uses less of both.
        let planned_user = format!(
            "{user}\n\n{}\n\n{}",
            plan.instructions(),
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
    unit_count: usize,
    mut preflight: impl FnMut(StudyGuideCompletionPlan) -> Result<(), CommandError>,
) -> Result<StudyGuideCompletionPlan, CommandError> {
    // Split multi-unit work before reducing its reserved output. Smaller-context
    // models can still author one indivisible unit using a compatible allowance.
    let choices = if unit_count > 1 {
        1
    } else {
        COLLECTION_GUIDE_COMPLETION_PLANS.len()
    };
    for plan in COLLECTION_GUIDE_COMPLETION_PLANS.into_iter().take(choices) {
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

/// A stricter request schema for a truncated attempt, not a weaker reader.
/// Already-valid field repairs use the original schema and retain their siblings.
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
        compact["properties"]["content"]["properties"][field]["maxItems"] = Value::from(0);
    }
    compact
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn guide_recovery_has_real_preflighted_headroom_instead_of_identical_replay() {
        let mut observed = Vec::new();
        let plan = choose_plan(13, |plan| {
            observed.push(plan);
            Ok(())
        })
        .unwrap();
        assert_eq!(plan, DOCUMENT_GUIDE_COMPLETION_PLAN);
        assert_eq!(observed, vec![plan]);
        for plan in COLLECTION_GUIDE_COMPLETION_PLANS {
            assert_eq!(plan.repair_visible_tokens, plan.initial_visible_tokens * 2);
        }
    }

    #[test]
    fn small_context_fallback_is_only_for_indivisible_knowledge() {
        let fits = |plan: StudyGuideCompletionPlan| {
            if plan.repair_visible_tokens <= 6_144 {
                Ok(())
            } else {
                Err(CommandError::new("ai_document_context_budget", "split"))
            }
        };
        assert_eq!(choose_plan(1, fits).unwrap().repair_visible_tokens, 6_144);
        assert_eq!(
            choose_plan(2, fits).unwrap_err().code,
            "ai_document_context_budget"
        );
        assert_eq!(
            choose_plan(1, |_| Err(CommandError::new(
                "ai_profile_changed",
                "changed"
            )))
            .unwrap_err()
            .code,
            "ai_profile_changed"
        );
    }

    #[test]
    fn guide_first_pass_explains_joint_budget_and_preserves_qualifications() {
        let instruction = DOCUMENT_GUIDE_COMPLETION_PLAN.instructions();
        for required in [
            "6144 tokens",
            "7,500 human-readable characters",
            "including all categories",
            "keyPoints are optional",
            "unused arrays must be []",
            "never cut a qualification",
        ] {
            assert!(instruction.contains(required), "{required}");
        }
        assert!(DOCUMENT_GUIDE_COMPLETION_PLAN
            .recovery_instructions()
            .contains("12288"));
    }

    #[test]
    fn compact_recovery_keeps_required_content_and_support_but_bounds_optional_expansion() {
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
        assert_eq!(fields["keyPoints"]["maxItems"], 5);
        assert_eq!(fields["tables"]["maxItems"], 0);
        assert_eq!(fields["visualExplanations"]["maxItems"], 0);
        assert_eq!(
            compact["properties"]["content"]["additionalProperties"],
            json!(false)
        );
        assert_eq!(
            schema["properties"]["content"]["properties"]["tables"]["maxItems"],
            3
        );
    }
}
