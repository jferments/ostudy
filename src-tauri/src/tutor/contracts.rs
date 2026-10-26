//! Public conversation contracts and private assessment data. A tutor answer is
//! never a scheduler review; no rating, mastery, or curriculum field exists.

use serde::{Deserialize, Serialize};

use crate::{
    error::CommandError,
    knowledge::contracts::{bounded, invalid, SourceCitation, StudyScope},
    util::validate_entity_id,
};

pub(crate) const TUTOR_PROMPT_VERSION: i64 = 3;
pub(crate) const MAX_QUIZ_QUESTIONS: i64 = 20;
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
    /// The conversation's last turn this one replaces (Regenerate, or the
    /// question edited and sent again). Left out of the stored request when
    /// absent, so earlier operations keep their identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaces_turn_id: Option<String>,
    /// The question a quiz's Next question follows, so the new one takes the
    /// next place in its run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_question_id: Option<String>,
    /// The option picked for a choose-one question, which is graded natively.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<usize>,
    /// Pictures and text files sent with a question, in the order attached.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<TutorAttachmentInput>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorImageInputQuery {
    /// The chat's own model; `None` for the Tutor's assigned one.
    pub profile_id: Option<String>,
}

/// Most pictures one message carries; a provider request takes six images,
/// which leaves room for a page of the focused material.
pub(crate) const MAX_ATTACHED_IMAGES: usize = 4;
pub(crate) const MAX_ATTACHMENTS: usize = 8;
pub(crate) const MAX_ATTACHED_TEXT_CHARS: usize = 30_000;
pub(crate) const MAX_ATTACHED_TEXT_TOTAL: usize = 60_000;

/// One thing attached to a question. A picture is a managed image: a new one
/// is still staged in its own import draft, and one sent before (when the
/// question is asked again) is already kept. A text file is sent as its text.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TutorAttachmentInput {
    Image {
        asset_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        draft_id: Option<String>,
    },
    Text {
        name: String,
        text: String,
    },
}

/// An attachment as the chat shows it, and as Regenerate sends it again.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum TutorAttachment {
    Image { asset_id: String, name: String },
    Text { name: String, text: String },
}

fn validate_attachments(attachments: &[TutorAttachmentInput]) -> Result<(), CommandError> {
    if attachments.len() > MAX_ATTACHMENTS {
        return Err(invalid(format!(
            "A message can carry at most {MAX_ATTACHMENTS} attachments."
        )));
    }
    let mut images = std::collections::BTreeSet::new();
    let mut characters = 0_usize;
    for attachment in attachments {
        match attachment {
            TutorAttachmentInput::Image { asset_id, draft_id } => {
                validate_entity_id(asset_id, "attached picture")?;
                if let Some(id) = draft_id {
                    validate_entity_id(id, "attached picture draft")?;
                }
                if !images.insert(asset_id.as_str()) {
                    return Err(invalid("The same picture is attached twice."));
                }
            }
            TutorAttachmentInput::Text { name, text } => {
                bounded(name, 255, "attached file name", false)?;
                if name.chars().any(char::is_control) {
                    return Err(invalid("An attached file name is invalid."));
                }
                bounded(text, MAX_ATTACHED_TEXT_CHARS, "attached file", false)?;
                characters = characters.saturating_add(text.chars().count());
            }
        }
    }
    if images.len() > MAX_ATTACHED_IMAGES {
        return Err(invalid(format!(
            "A message can carry at most {MAX_ATTACHED_IMAGES} pictures."
        )));
    }
    if characters > MAX_ATTACHED_TEXT_TOTAL {
        return Err(invalid(
            "The attached files are too long to send together. Attach fewer or shorter files.",
        ));
    }
    Ok(())
}

impl TutorTurnInput {
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        validate_entity_id(&self.operation_id, "tutor operation")?;
        validate_entity_id(&self.conversation_id, "conversation")?;
        if self.expected_revision < 1 {
            return Err(invalid("The conversation revision is invalid."));
        }
        // A picture or file can be the whole question.
        bounded(
            &self.text,
            8_000,
            "tutor message",
            self.mode == TutorMode::Quiz || !self.attachments.is_empty(),
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
        if let Some(id) = &self.replaces_turn_id {
            validate_entity_id(id, "replaced tutor message")?;
            if self.mode != TutorMode::Explain {
                return Err(invalid(
                    "Only an explanation can replace an earlier answer.",
                ));
            }
        }
        if let Some(id) = &self.after_question_id {
            validate_entity_id(id, "earlier quiz question")?;
            if self.mode != TutorMode::Quiz {
                return Err(invalid("Only a quiz question can follow an earlier one."));
            }
        }
        if self.choice.is_some() && self.mode != TutorMode::Answer {
            return Err(invalid("Only a quiz answer may pick an option."));
        }
        if !self.attachments.is_empty() && self.mode != TutorMode::Explain {
            return Err(invalid("Only a question can carry attachments."));
        }
        validate_attachments(&self.attachments)?;
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
    /// A choose-one question's options; empty for a short answer.
    #[serde(default)]
    pub choices: Vec<String>,
    /// The right option's index, for a choose-one question only.
    #[serde(default)]
    pub correct_choice: Option<usize>,
    /// How many questions the learner asked for, as the model read it.
    #[serde(default)]
    pub question_count: Option<i64>,
    /// Where the question stands in its run. Set natively, never by the model.
    #[serde(default)]
    pub series: Option<QuizSeries>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct QuizSeries {
    pub number: i64,
    pub total: Option<i64>,
}

impl QuizSeries {
    /// The first question of a run, or the one after `previous`. A run that
    /// asked for a number of questions ends there.
    pub(crate) fn next(
        previous: Option<&PrivateQuiz>,
        asked: Option<i64>,
    ) -> Result<Self, CommandError> {
        let Some(previous) = previous else {
            return Ok(Self {
                number: 1,
                total: asked,
            });
        };
        let series = previous.series.unwrap_or(Self {
            number: 1,
            total: previous.question_count,
        });
        if series.total.is_some_and(|total| series.number >= total) {
            return Err(invalid("That was the last question in this quiz."));
        }
        Ok(Self {
            number: series.number + 1,
            total: series.total,
        })
    }
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
        if !self.choices.is_empty() {
            if !(2..=6).contains(&self.choices.len())
                || !self
                    .correct_choice
                    .is_some_and(|index| index < self.choices.len())
            {
                return Err(invalid("The quiz options are invalid."));
            }
            let mut seen = std::collections::HashSet::new();
            for choice in &self.choices {
                bounded(choice, 300, "quiz option", false)?;
                if !seen.insert(choice.trim().to_lowercase()) {
                    return Err(invalid("The quiz repeats an option."));
                }
            }
        } else if self.correct_choice.is_some() {
            return Err(invalid("Only a choose-one question has a right option."));
        }
        if self
            .question_count
            .is_some_and(|count| !(1..=MAX_QUIZ_QUESTIONS).contains(&count))
        {
            return Err(invalid("The quiz length is invalid."));
        }
        if let Some(series) = self.series {
            if series.number < 1
                || series
                    .total
                    .is_some_and(|total| total < series.number || total > MAX_QUIZ_QUESTIONS)
            {
                return Err(invalid("The quiz question's place is invalid."));
            }
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
    /// Whether the answer met each criterion, in order. Empty for feedback
    /// saved before this was recorded, and for a choose-one question.
    #[serde(default)]
    pub criteria_met: Vec<bool>,
    /// The option a choose-one answer picked.
    #[serde(default)]
    pub chosen: Option<usize>,
}

/// A quiz question's public part. What it expected shows only once it is
/// answered.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TutorQuestion {
    pub number: i64,
    pub total: Option<i64>,
    pub choices: Vec<String>,
    pub answered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_answer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correct_choice: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub scope: StudyScope,
    pub project_id: Option<String>,
    pub revision: i64,
    pub organization_revision: i64,
    pub archived_at: Option<i64>,
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
    /// A quiz question's place in its run and its options.
    pub question: Option<TutorQuestion>,
    /// The question an answer answers.
    pub question_id: Option<String>,
    pub provider: String,
    pub model: String,
    pub error_code: String,
    pub error_message: String,
    pub total_tokens: Option<i64>,
    pub created_at: i64,
    /// The newer version that replaced this turn; the chat carries on from it.
    pub replaced_by_turn_id: Option<String>,
    /// What the learner attached to this message, in order.
    pub attachments: Vec<TutorAttachment>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationDetail {
    pub conversation: ConversationSummary,
    pub turns: Vec<TutorTurn>,
    pub has_earlier_turns: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateProjectConversationInput {
    pub operation_id: String,
    pub project_id: String,
    pub title: String,
    pub scope: Option<StudyScope>,
}

impl CreateProjectConversationInput {
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        validate_entity_id(&self.operation_id, "conversation operation")?;
        validate_entity_id(&self.project_id, "Tutor project")?;
        bounded(&self.title, 200, "conversation title", false)?;
        if let Some(scope) = &self.scope {
            scope.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenameConversationInput {
    pub operation_id: String,
    pub conversation_id: String,
    pub expected_organization_revision: i64,
    pub title: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MoveConversationInput {
    pub operation_id: String,
    pub conversation_id: String,
    pub expected_organization_revision: i64,
    pub project_id: Option<String>,
    pub apply_project_default: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetConversationArchivedInput {
    pub operation_id: String,
    pub conversation_id: String,
    pub expected_organization_revision: i64,
    pub archived: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationPageInput {
    pub conversation_id: String,
    pub before_sequence: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteConversationInput {
    pub operation_id: String,
    pub conversation_id: String,
    pub expected_revision: i64,
    pub expected_organization_revision: i64,
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
                members: Vec::new(),
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
            replaces_turn_id: None,
            after_question_id: None,
            choice: None,
            attachments: Vec::new(),
        };
        assert!(input.validate().is_err());
        let serialized = serde_json::to_value(input).unwrap();
        assert!(serialized.get("rating").is_none());
        assert!(serialized.get("mastery").is_none());
    }
}
