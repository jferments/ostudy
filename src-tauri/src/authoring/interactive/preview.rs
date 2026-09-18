use serde::{Deserialize, Serialize};

use crate::{
    cards::questions::{
        Evaluation, PublicQuestion, QuestionBody, QuestionResponse, ValidatedQuestion,
    },
    error::CommandError,
};

const AUTHOR_PREVIEW_SHUFFLE_SEED: &str = "ostudy-author-preview-v1";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewQuestionDraftInput {
    pub definition_json: String,
    #[serde(default)]
    pub response: Option<QuestionResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionDraftPreviewDto {
    pub question: PublicQuestion,
    pub evaluation: Option<Evaluation>,
}

/// Pure authoring projection: validate with the canonical Rust question domain,
/// use the same deterministic display-order algorithm as Practice, and optionally
/// grade one transient response. No database, schedule, attempt, or review exists.
#[tauri::command]
pub fn preview_question_draft(
    input: PreviewQuestionDraftInput,
) -> Result<QuestionDraftPreviewDto, CommandError> {
    let question = ValidatedQuestion::parse(&input.definition_json)?;
    if matches!(
        &question.definition().question,
        QuestionBody::ImageOcclusion { .. }
    ) {
        return Err(CommandError::new(
            "question_preview_uses_occlusion_renderer",
            "Image occlusion preview uses the managed masked-image renderer.",
        ));
    }
    let order = question.display_order(AUTHOR_PREVIEW_SHUFFLE_SEED);
    let public = question.project(&order)?;
    let evaluation = input
        .response
        .as_ref()
        .map(|response| question.evaluate(response))
        .transpose()?;
    Ok(QuestionDraftPreviewDto { question: public, evaluation })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use super::*;
    use crate::cards::questions::Verdict;

    fn id(value: char) -> String { std::iter::repeat_n(value, 32).collect() }
    fn doc(text: &str) -> serde_json::Value {
        json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
    }
    fn definition() -> String {
        json!({"schemaVersion":1,"question":{
            "kind":"single_choice","prompt":doc("Choose one"),
            "options":[
                {"id":id('a'),"content":doc("Wrong"),"rationale":doc("Not this one")},
                {"id":id('b'),"content":doc("Right"),"rationale":doc("Supported answer")}
            ],
            "correctOptionId":id('b'),"explanation":doc("Overall explanation")
        }}).to_string()
    }

    #[test]
    fn preview_validates_projects_and_evaluates_without_storage() {
        let prepared = preview_question_draft(PreviewQuestionDraftInput {
            definition_json: definition(), response: None,
        }).expect("preview should validate");
        let PublicQuestion::SingleChoice { options, .. } = prepared.question else {
            panic!("expected choice projection");
        };
        assert_eq!(options.len(), 2);
        assert!(prepared.evaluation.is_none());

        let checked = preview_question_draft(PreviewQuestionDraftInput {
            definition_json: definition(),
            response: Some(QuestionResponse::SingleChoice {
                selected_option_id: Some(id('b')),
            }),
        }).expect("native preview check should succeed");
        let evaluation = checked.evaluation.expect("evaluation");
        assert_eq!(evaluation.verdict, Verdict::Correct);
        assert_eq!(evaluation.matched_count, 1);
        assert_eq!(evaluation.required_count, 1);
    }

    #[test]
    fn preview_rejects_occlusion_before_public_source_image_projection() {
        let definition = json!({"schemaVersion":1,"question":{
            "kind":"image_occlusion","imageAssetId":id('f'),
            "masks":[{"id":id('a'),"rect":{"x":0.1,"y":0.1,"width":0.2,"height":0.2}}],
            "questions":[{"id":id('b'),"prompt":doc("Identify"),"answer":doc("Answer"),
                "explanation":null,"maskIds":[id('a')],"mode":"hide_target"}]
        }}).to_string();
        let error = preview_question_draft(PreviewQuestionDraftInput {
            definition_json: definition, response: None,
        }).err().expect("occlusion uses dedicated renderer");
        assert_eq!(error.code, "question_preview_uses_occlusion_renderer");
    }
}
