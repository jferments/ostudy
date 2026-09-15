//! Versioned authored data. Rich fields retain the existing canonical document
//! format; an interaction is never encoded as HTML, template code, or front/back.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuestionKind {
    SingleChoice,
    SelectAll,
    TrueFalse,
    Matching,
    Grouping,
    Ordering,
    ImageOcclusion,
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuestionDefinition {
    pub schema_version: u32,
    pub question: QuestionBody,
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum QuestionBody {
    SingleChoice {
        prompt: Value,
        options: Vec<ChoiceOption>,
        correct_option_id: String,
        explanation: Option<Value>,
    },
    SelectAll {
        prompt: Value,
        options: Vec<ChoiceOption>,
        correct_option_ids: Vec<String>,
        explanation: Option<Value>,
    },
    TrueFalse {
        statement: Value,
        correct: bool,
        explanation: Option<Value>,
        corrected_statement: Option<Value>,
    },
    Matching {
        prompt: Value,
        pairs: Vec<MatchingPair>,
        explanation: Option<Value>,
    },
    Grouping {
        prompt: Value,
        categories: Vec<RichItem>,
        items: Vec<GroupedItem>,
        explanation: Option<Value>,
    },
    Ordering {
        prompt: Value,
        /// Authored order is the key. Study order is captured independently.
        items: Vec<RichItem>,
        explanation: Option<Value>,
    },
    ImageOcclusion {
        image_asset_id: String,
        masks: Vec<ImageMask>,
        questions: Vec<OcclusionQuestion>,
    },
}

impl QuestionBody {
    pub fn kind(&self) -> QuestionKind {
        match self {
            Self::SingleChoice { .. } => QuestionKind::SingleChoice,
            Self::SelectAll { .. } => QuestionKind::SelectAll,
            Self::TrueFalse { .. } => QuestionKind::TrueFalse,
            Self::Matching { .. } => QuestionKind::Matching,
            Self::Grouping { .. } => QuestionKind::Grouping,
            Self::Ordering { .. } => QuestionKind::Ordering,
            Self::ImageOcclusion { .. } => QuestionKind::ImageOcclusion,
        }
    }
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RichItem {
    pub id: String,
    pub content: Value,
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChoiceOption {
    pub id: String,
    pub content: Value,
    pub rationale: Option<Value>,
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MatchingPair {
    pub left: RichItem,
    pub right: RichItem,
    pub rationale: Option<Value>,
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupedItem {
    pub item: RichItem,
    pub category_id: String,
    pub rationale: Option<Value>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NormalizedRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageMask {
    pub id: String,
    pub rect: NormalizedRect,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OcclusionMode {
    HideTarget,
    HideAll,
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OcclusionQuestion {
    pub id: String,
    pub prompt: Value,
    pub answer: Value,
    pub explanation: Option<Value>,
    pub mask_ids: Vec<String>,
    pub mode: OcclusionMode,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DisplayOrder {
    pub item_ids: Vec<String>,
    pub target_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LearnerAnswer {
    pub schema_version: u32,
    pub instance_id: String,
    pub response: QuestionResponse,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum QuestionResponse {
    SingleChoice { selected_option_id: Option<String> },
    SelectAll { selected_option_ids: Vec<String> },
    TrueFalse { selected: Option<bool> },
    Matching { assignments: Vec<Assignment> },
    Grouping { assignments: Vec<Assignment> },
    Ordering { ordered_item_ids: Vec<String> },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Assignment {
    pub item_id: String,
    pub target_id: String,
}

/// Pre-answer output has no answer key, correction, rationale, or authored pair.
/// Occlusion is intentionally absent until a managed masked derivative exists.
#[derive(Clone, Serialize, PartialEq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum PublicQuestion {
    SingleChoice {
        prompt: Value,
        options: Vec<RichItem>,
    },
    SelectAll {
        prompt: Value,
        options: Vec<RichItem>,
    },
    TrueFalse {
        statement: Value,
    },
    Matching {
        prompt: Value,
        items: Vec<RichItem>,
        targets: Vec<RichItem>,
    },
    Grouping {
        prompt: Value,
        items: Vec<RichItem>,
        categories: Vec<RichItem>,
    },
    Ordering {
        prompt: Value,
        items: Vec<RichItem>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Correct,
    Incorrect,
    Incomplete,
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Evaluation {
    pub grading_policy_version: u32,
    pub verdict: Verdict,
    /// Diagnostic counts are not a grade, percentage, or number of reviews.
    /// An empty select-all key can be correct with both counts equal to zero.
    pub matched_count: usize,
    pub required_count: usize,
    pub components: Vec<ComponentFeedback>,
    pub explanation: Option<Value>,
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ComponentFeedback {
    Choice {
        item_id: String,
        selected: bool,
        expected: bool,
        rationale: Option<Value>,
    },
    Boolean {
        selected: Option<bool>,
        expected: bool,
        corrected_statement: Option<Value>,
    },
    Placement {
        item_id: String,
        selected_target_id: Option<String>,
        expected_target_id: String,
        rationale: Option<Value>,
    },
    Position {
        item_id: String,
        selected_position: Option<usize>,
        expected_position: usize,
    },
}
