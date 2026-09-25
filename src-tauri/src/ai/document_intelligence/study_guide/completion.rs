//! First-pass output feasibility. Use the already reserved recovery headroom
//! initially instead of deliberately making the model hit a smaller ceiling.
//! These are existing maximum allowances, not claims of measured token savings.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StudyGuideCompletionPlan {
    pub(super) initial_visible_tokens: usize,
    pub(super) repair_visible_tokens: usize,
}

pub(super) const DOCUMENT_GUIDE_COMPLETION_PLAN: StudyGuideCompletionPlan =
    StudyGuideCompletionPlan {
        initial_visible_tokens: 3_200,
        repair_visible_tokens: 3_200,
    };

pub(super) const COLLECTION_GUIDE_COMPLETION_PLANS: [StudyGuideCompletionPlan; 4] = [
    StudyGuideCompletionPlan {
        initial_visible_tokens: 6_144,
        repair_visible_tokens: 6_144,
    },
    StudyGuideCompletionPlan {
        initial_visible_tokens: 4_096,
        repair_visible_tokens: 4_096,
    },
    StudyGuideCompletionPlan {
        initial_visible_tokens: 3_072,
        repair_visible_tokens: 3_072,
    },
    StudyGuideCompletionPlan {
        initial_visible_tokens: 2_048,
        repair_visible_tokens: 2_048,
    },
];

impl StudyGuideCompletionPlan {
    pub(super) fn instructions(self) -> String {
        format!(
            "Initial output plan: the visible completion allowance is {} tokens. Complete the required JSON object before adding optional richness. Target at most 4,200 human-readable characters for this request, including all categories. Start with a narrative overview and nonempty supported keyPoints. Add only the optional study aids that contribute distinct teaching value; unused arrays must be []. Do not fill every category or restate the same fact in several fields. Preserve entire conditions, uncertainty, numerical signs, units, and exceptions; never cut a qualification to meet an editorial target. This writing target is not permission to claim exhaustive coverage or invent source support.",
            self.initial_visible_tokens
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_guide_first_pass_receives_previously_reserved_headroom() {
        assert_eq!(DOCUMENT_GUIDE_COMPLETION_PLAN.initial_visible_tokens, 3_200);
        assert_eq!(DOCUMENT_GUIDE_COMPLETION_PLAN.repair_visible_tokens, 3_200);
        assert_eq!(
            COLLECTION_GUIDE_COMPLETION_PLANS.map(|plan| plan.initial_visible_tokens),
            [6_144, 4_096, 3_072, 2_048]
        );
        for plan in COLLECTION_GUIDE_COMPLETION_PLANS {
            assert_eq!(plan.initial_visible_tokens, plan.repair_visible_tokens);
        }
    }

    #[test]
    fn document_guide_first_pass_explains_joint_budget_and_preserves_qualifications() {
        let instruction = DOCUMENT_GUIDE_COMPLETION_PLAN.instructions();
        for required in [
            "3200 tokens",
            "4,200 human-readable characters",
            "including all categories",
            "nonempty supported keyPoints",
            "unused arrays must be []",
            "never cut a qualification",
        ] {
            assert!(instruction.contains(required), "{required}");
        }
    }

    #[test]
    fn document_guide_wire_contract_requires_native_required_key_points() {
        let schema = super::super::study_guide_section_schema();
        assert_eq!(schema["properties"]["keyPoints"]["minItems"], 1);
        assert_eq!(schema["properties"]["keyPoints"]["maxItems"], 7);
        assert_eq!(schema["properties"]["processSteps"].get("minItems"), None);
        assert_eq!(schema["additionalProperties"], false);
    }
}
