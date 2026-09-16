//! A Practice presentation shares identity, not an invented front/back document.
//! Recall retains its existing wire shape. Interactive data is a key-free view
//! of the captured attempt and owns no duplicate rich-document representation.

use serde::{Serialize, Serializer};
use serde_json::Value;

use super::question_attempts::PracticeQuestionDto;
use crate::{assets::AssetDto, cards::CardPresentationDto};

#[derive(Clone)]
pub enum PracticeContentDto {
    Recall {
        #[allow(dead_code)]
        front_doc: Value,
        #[allow(dead_code)]
        back_doc: Value,
        #[allow(dead_code)]
        back_text: String,
        #[allow(dead_code)]
        assets: Vec<AssetDto>,
    },
    Interactive {
        question: Box<PracticeQuestionDto>,
    },
}

#[derive(Clone)]
pub struct PracticePresentationDto {
    pub id: String,
    pub note_id: String,
    pub deck_id: String,
    pub direction: String,
    pub front_text: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
    pub content: PracticeContentDto,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InteractivePresentationWire<'a> {
    #[serde(rename = "cardId")]
    id: &'a str,
    note_id: &'a str,
    deck_id: &'a str,
    direction: &'a str,
    front_text: &'a str,
    created_at: i64,
    updated_at: i64,
    #[serde(rename = "noteRevision")]
    revision: i64,
    question: &'a PracticeQuestionDto,
}

impl Serialize for PracticePresentationDto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.content {
            PracticeContentDto::Recall {
                front_doc,
                back_doc,
                back_text,
                assets,
            } => CardPresentationDto {
                id: self.id.clone(),
                note_id: self.note_id.clone(),
                deck_id: self.deck_id.clone(),
                direction: self.direction.clone(),
                front_doc: front_doc.clone(),
                back_doc: back_doc.clone(),
                front_text: self.front_text.clone(),
                back_text: back_text.clone(),
                assets: assets.clone(),
                created_at: self.created_at,
                updated_at: self.updated_at,
                revision: self.revision,
            }
            .serialize(serializer),
            PracticeContentDto::Interactive { question } => InteractivePresentationWire {
                id: &self.id,
                note_id: &self.note_id,
                deck_id: &self.deck_id,
                direction: &self.direction,
                front_text: &self.front_text,
                created_at: self.created_at,
                updated_at: self.updated_at,
                revision: self.revision,
                question: question.as_ref(),
            }
            .serialize(serializer),
        }
    }
}

impl std::fmt::Debug for PracticePresentationDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PracticePresentationDto")
            .field("card_id", &self.id)
            .field("note_id", &self.note_id)
            .field("note_revision", &self.revision)
            .finish_non_exhaustive()
    }
}
