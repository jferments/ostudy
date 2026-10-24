//! A hand-written lesson: how the loop of Henle concentrates urine.

use serde_json::{json, Value};

use super::content::{
    CalloutKind, CheckTiming, LessonBlock, LessonContent, LessonStep, LESSON_CONTENT_VERSION,
};
use crate::cards::questions::{
    ChoiceOption, QuestionBody, QuestionDefinition, QUESTION_SCHEMA_VERSION,
};

pub(crate) const CORRECT_OPTION: &str = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
const WRONG_OPTION: &str = "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b";

fn rich(text: &str) -> Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
}

fn items(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

/// Three steps, the second with a check. `units` are the knowledge units the
/// first step explains.
pub(crate) fn loop_of_henle(units: &[&str]) -> LessonContent {
    LessonContent {
        schema_version: LESSON_CONTENT_VERSION,
        title: "Loop of Henle".to_string(),
        summary: "How the two limbs of the loop build the gradient that lets the kidney concentrate urine."
            .to_string(),
        objectives: items(&[
            "Explain why the descending limb loses water but not salt.",
            "Describe how countercurrent multiplication builds the medullary gradient.",
        ]),
        steps: vec![
            LessonStep {
                key: "two-limbs".to_string(),
                title: "Two limbs, two jobs".to_string(),
                blocks: vec![
                    LessonBlock::Explanation {
                        heading: None,
                        body: "The descending limb is permeable to water but not to salt, so water leaves it into the salty medulla. The thick ascending limb pumps salt out but stays watertight."
                            .to_string(),
                    },
                    LessonBlock::Comparison {
                        subject_a: "Descending limb".to_string(),
                        subject_b: "Thick ascending limb".to_string(),
                        relationship: "One lets water out, the other moves salt out; neither does both."
                            .to_string(),
                    },
                ],
                narration: "Picture the loop as two tubes side by side. Going down, water slips out into the salty tissue around it. Coming back up, the wall is watertight, but pumps push salt out."
                    .to_string(),
                slide_points: items(&[
                    "Descending limb: water leaves, salt stays",
                    "Ascending limb: salt pumped out, water stays",
                ]),
                check: None,
                check_timing: CheckTiming::After,
                knowledge_unit_ids: units.iter().map(|unit| unit.to_string()).collect(),
            },
            LessonStep {
                key: "multiplier".to_string(),
                title: "The countercurrent multiplier".to_string(),
                blocks: vec![
                    LessonBlock::Process {
                        title: "How the gradient builds".to_string(),
                        steps: items(&[
                            "The ascending limb pumps salt into the interstitium.",
                            "Water leaves the descending limb toward that salt.",
                            "Fluid moves on and the step repeats deeper in the medulla.",
                        ]),
                    },
                    LessonBlock::Callout {
                        callout: CalloutKind::KeyIdea,
                        title: "Small steps add up".to_string(),
                        body: "Each pass makes only a small difference, but flow in opposite directions multiplies it along the loop."
                            .to_string(),
                    },
                ],
                narration: "Each pump only makes a small difference across the wall. Because fluid flows down one side and up the other, those small differences stack along the length of the loop."
                    .to_string(),
                slide_points: items(&["Pump, then water follows", "Opposite flows multiply the effect"]),
                check: Some(QuestionDefinition {
                    schema_version: QUESTION_SCHEMA_VERSION,
                    question: QuestionBody::SingleChoice {
                        prompt: rich("Which part of the loop pumps salt out but keeps water in?"),
                        options: vec![
                            ChoiceOption {
                                id: CORRECT_OPTION.to_string(),
                                content: rich("The thick ascending limb"),
                                rationale: None,
                            },
                            ChoiceOption {
                                id: WRONG_OPTION.to_string(),
                                content: rich("The descending limb"),
                                rationale: None,
                            },
                        ],
                        correct_option_id: CORRECT_OPTION.to_string(),
                        explanation: Some(rich(
                            "The thick ascending limb is watertight while its pumps move salt out.",
                        )),
                    },
                }),
                check_timing: CheckTiming::After,
                knowledge_unit_ids: Vec::new(),
            },
            LessonStep {
                key: "why-it-matters".to_string(),
                title: "Why it matters".to_string(),
                blocks: vec![LessonBlock::Misconception {
                    misconception: "The loop of Henle makes urine concentrated by itself.".to_string(),
                    correction: "The loop builds the gradient; the collecting duct, under ADH, uses it to pull water out of urine."
                        .to_string(),
                }],
                narration: "The loop does not finish the job. It sets up the salty gradient, and the collecting duct, when ADH opens its water channels, uses that gradient to concentrate urine."
                    .to_string(),
                slide_points: items(&["The loop builds the gradient", "The collecting duct uses it, under ADH"]),
                check: None,
                check_timing: CheckTiming::After,
                knowledge_unit_ids: Vec::new(),
            },
        ],
    }
}
