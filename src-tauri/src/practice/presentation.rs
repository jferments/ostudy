//! A Practice presentation shares identity, not an invented front/back document.
//! Recall retains its existing wire shape. Interactive data is a key-free view
//! of the captured attempt and owns no duplicate rich-document representation.

use serde::Serialize;
use serde_json::Value;

use crate::assets::AssetDto;
use super::question_attempts::PracticeQuestionDto;

#[derive(Clone, Serialize)]
#[serde(untagged)]
pub enum PracticeContentDto {
    Recall {
        #[serde(rename = "frontDoc")]
        front_doc: Value,
        #[serde(rename = "backDoc")]
        back_doc: Value,
        #[serde(rename = "backText")]
        back_text: String,
        assets: Vec<AssetDto>,
    },
    Interactive {
        question: Box<PracticeQuestionDto>,
    },
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticePresentationDto {
    #[serde(rename = "cardId")]
    pub id: String,
    pub note_id: String,
    pub deck_id: String,
    pub direction: String,
    pub front_text: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(rename = "noteRevision")]
    pub revision: i64,
    #[serde(flatten)]
    pub content: PracticeContentDto,
}

impl std::fmt::Debug for PracticePresentationDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("PracticePresentationDto")
            .field("card_id", &self.id)
            .field("note_id", &self.note_id)
            .field("note_revision", &self.revision)
            .finish_non_exhaustive()
    }
}
