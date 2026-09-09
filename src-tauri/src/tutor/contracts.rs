//! Public conversation contracts and private assessment data. A tutor answer is
//! never a scheduler review; no rating, mastery, or curriculum field exists.

use serde::{Deserialize, Serialize};

use crate::{
    error::CommandError,
    knowledge::contracts::{bounded, invalid, SourceCitation, StudyScope},
    util::validate_entity_id,
};

pub(crate) const TUTOR_PROMPT_VERSION: i64 = 1;
pub(crate) const MAX_HISTORY_TURNS: usize = 50;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TutorMode {
    Explain,
    Hint,
    Quiz,
    Answer,
}

impl TutorMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Explain => "explain",
            Self::Hint => "hint",
            Self::Quiz => "quiz",
            Self::Answer => "answer",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TutorFocus {
    Card {
        session_id: Option<String>,
        card_id: String,
        note_revision: i64,
    },
    Guide {
        document_id: String,
        guide_id: String,
        topic_id: Option<String>,
        content_hash: Option<String>,
    },
}

impl TutorFocus {
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        match self {
            Self::Card {
                session_id,
                card_id,
                note_revision,
            } => {
                if let Some(id) = session_id {
                    validate_entity_id(id, "practice session")?;
                }
                validate_entity_id(card_id, "card")?;
                if *note_revision < 1 {
                    return Err(invalid("The focused card revision is invalid."));
                }
            }
            Self::Guide {
                document_id,
                guide_id,
                topic_id,
                content_hash,
            } => {
                validate_entity_id(document_id, "guide document")?;
                validate_entity_id(guide_id, "study guide")?;
                if let Some(id) = topic_id {
                    validate_entity_id(id, "guide topic")?;
                    if !content_hash
                        .as_deref()
                        .is_some_and(crate::knowledge::contracts::is_hash)
                    {
                        return Err(invalid(
                            "The focused guide section fingerprint is required.",
                        ));
                    }
                } else if content_hash.is_some() {
                    return Err(invalid(
                        "An overview focus must not include a section fingerprint.",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateConversationInput {
    pub operation_id: String,
    pub scope: StudyScope,
    pub title: String,
}

impl CreateConversationInput {
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        validate_entity_id(&self.operation_id, "conversation operation")?;
        self.scope.validate()?;
        bounded(&self.title, 200, "conversation title", false)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorTurnInput {
    pub operation_id: String,
    pub conversation_id: String,
    pub expected_revision: i64,
    pub mode: TutorMode,
    pub text: String,
    pub focus: Option<TutorFocus>,
    pub profile_id: Option<String>,
    #[serde(default)]
    pub include_conversations: bool,
    pub question_id: Option<String>,
}

impl TutorTurnInput {
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        validate_entity_id(&self.operation_id, "tutor operation")?;
        validate_entity_id(&self.conversation_id, "conversation")?;
        if self.expected_revision < 1 {
            return Err(invalid("The conversation revision is invalid."));
        }
        bounded(
            &self.text,
            8_000,
            "tutor message",
            self.mode == TutorMode::Quiz,
        )?;
        if let Some(id) = &self.profile_id {
            validate_entity_id(id, "tutor profile")?;
        }
        if let Some(focus) = &self.focus {
            focus.validate()?;
        }
        match (&self.question_id, self.mode) {
            (Some(id), TutorMode::Answer) => validate_entity_id(id, "quiz question")?,
            (None, TutorMode::Answer) => {
                return Err(invalid("Select the quiz question being answered."))
            }
            (Some(_), _) => return Err(invalid("Only a quiz answer may reference a question.")),
            (None, _) => {}
        }
        if self.mode == TutorMode::Hint
            && !matches!(
                self.focus,
                Some(TutorFocus::Card {
                    session_id: Some(_),
                    ..
                })
            )
        {
            return Err(invalid(
                "Flashcard hints require the current practice card.",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PrivateQuiz {
    pub question: String,
    pub expected_answer: String,
    pub criteria: Vec<String>,
    pub explanation: String,
    pub source_ids: Vec<String>,
    pub knowledge_unit_ids: Vec<String>,
}

impl PrivateQuiz {
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        bounded(&self.question, 3_000, "quiz question", false)?;
        bounded(&self.expected_answer, 4_000, "expected quiz answer", false)?;
        bounded(&self.explanation, 4_000, "quiz explanation", false)?;
        if self.criteria.is_empty()
            || self.criteria.len() > 8
            || self.source_ids.len() > 12
            || self.knowledge_unit_ids.len() > 12
        {
            return Err(invalid(
                "The quiz criteria or supporting-reference count is invalid.",
            ));
        }
        for criterion in &self.criteria {
            bounded(criterion, 1_000, "quiz criterion", false)?;
        }
        for id in self.source_ids.iter().chain(self.knowledge_unit_ids.iter()) {
            validate_entity_id(id, "quiz reference")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuizOutcome {
    Correct,
    Partial,
    Incorrect,
    Unclear,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuizFeedback {
    pub outcome: QuizOutcome,
    pub explanation: String,
    pub criteria_feedback: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub scope: StudyScope,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorTurn {
    pub id: String,
    pub operation_id: String,
    pub sequence: i64,
    pub mode: TutorMode,
    pub user_text: String,
    pub assistant_text: String,
    pub state: String,
    pub citations: Vec<SourceCitation>,
    pub evidence_current: bool,
    pub pending_question: bool,
    pub feedback: Option<QuizFeedback>,
    pub provider: String,
    pub model: String,
    pub error_code: String,
    pub error_message: String,
    pub total_tokens: Option<i64>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationDetail {
    pub conversation: ConversationSummary,
    pub turns: Vec<TutorTurn>,
    pub has_earlier_turns: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationPageInput {
    pub conversation_id: String,
    pub before_sequence: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteConversationInput {
    pub conversation_id: String,
    pub expected_revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderAnswer {
    pub text: String,
    pub source_ids: Vec<String>,
    pub quiz: Option<PrivateQuiz>,
    pub feedback: Option<QuizFeedback>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::contracts::ScopeKind;

    #[test]
    fn conversations_do_not_require_a_learning_profile() {
        let input = CreateConversationInput {
            operation_id: "a".repeat(32),
            scope: StudyScope {
                kind: ScopeKind::General,
                id: None,
            },
            title: "Explain this concept".to_string(),
        };
        assert!(input.validate().is_ok());
    }

    #[test]
    fn hints_cannot_be_requested_without_a_native_practice_snapshot() {
        let input = TutorTurnInput {
            operation_id: "a".repeat(32),
            conversation_id: "b".repeat(32),
            expected_revision: 1,
            mode: TutorMode::Hint,
            text: "A hint".to_string(),
            focus: None,
            profile_id: None,
            include_conversations: false,
            question_id: None,
        };
        assert!(input.validate().is_err());
        let serialized = serde_json::to_value(input).unwrap();
        assert!(serialized.get("rating").is_none());
        assert!(serialized.get("mastery").is_none());
    }
}
