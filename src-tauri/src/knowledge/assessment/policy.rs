//! Soft format preferences never outrank source coverage or native admission.

use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{invalid, GeneratedQuestion};
use crate::{
    error::CommandError,
    knowledge::{
        authoring::{card_support, KnowledgeCatalog},
        contracts::hash,
    },
};

pub(crate) const MAX_COMPOUND_ITEMS: usize = 16;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum QuestionFamily {
    Recall,
    SingleChoice,
    SelectAll,
    TrueFalse,
    Matching,
    Grouping,
    Ordering,
}

impl QuestionFamily {
    pub(crate) fn from_kind(kind: &str) -> Result<Self, CommandError> {
        match kind {
            "basic" | "cloze" | "recall" => Ok(Self::Recall),
            "single_choice" => Ok(Self::SingleChoice),
            "select_all" => Ok(Self::SelectAll),
            "true_false" => Ok(Self::TrueFalse),
            "matching" => Ok(Self::Matching),
            "grouping" => Ok(Self::Grouping),
            "ordering" => Ok(Self::Ordering),
            _ => Err(invalid(
                "That question type is not eligible for automatic generation.",
            )),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct GenerationPolicy {
    pub allowed_types: Vec<QuestionFamily>,
    pub weights: BTreeMap<QuestionFamily, u16>,
    pub max_compound_items: usize,
}

impl Default for GenerationPolicy {
    fn default() -> Self {
        use QuestionFamily::*;
        Self {
            allowed_types: vec![
                Recall,
                SingleChoice,
                SelectAll,
                TrueFalse,
                Matching,
                Grouping,
                Ordering,
            ],
            weights: BTreeMap::from([
                (Recall, 50),
                (SingleChoice, 10),
                (Matching, 15),
                (Grouping, 10),
                (SelectAll, 10),
                (Ordering, 3),
                (TrueFalse, 2),
            ]),
            max_compound_items: 8,
        }
    }
}

impl GenerationPolicy {
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        if self.allowed_types.is_empty()
            || self.allowed_types.len() > 7
            || self.allowed_types.iter().collect::<BTreeSet<_>>().len() != self.allowed_types.len()
            || !(2..=MAX_COMPOUND_ITEMS).contains(&self.max_compound_items)
            || self.weights.values().any(|weight| *weight > 1000)
            || self
                .allowed_types
                .iter()
                .map(|kind| u32::from(*self.weights.get(kind).unwrap_or(&0)))
                .sum::<u32>()
                == 0
        {
            return Err(invalid(
                "Choose distinct question types, a positive mix, and 2–16 components per board.",
            ));
        }
        Ok(())
    }

    pub(crate) fn fingerprint(&self) -> Result<String, CommandError> {
        self.validate()?;
        let mut canonical = self.clone();
        canonical.allowed_types.sort();
        Ok(hash(&format!(
            "assessment-policy-v1|{}",
            serde_json::to_string(&canonical).map_err(CommandError::serialization)?
        )))
    }

    pub(crate) fn prompt(&self) -> Result<String, CommandError> {
        self.validate()?;
        Ok(format!(
            "Allowed question families and relative soft mix: {}. Board component limit: {}. Recall includes existing Basic/Cloze. Only use a type supported by the material. Coverage and unambiguous evidence outrank these weights; they are not quotas. Never invent a relationship, false distractor, category, or order to fill the mix. No automatic image occlusion. A board is one scheduled presentation; a bidirectional definition counts as two. Return fewer useful presentations rather than filler.",
            serde_json::to_string(&self).map_err(CommandError::serialization)?, self.max_compound_items,
        ))
    }
}

struct Candidate {
    card: Value,
    family: QuestionFamily,
    cost: usize,
    units: Vec<String>,
    importance: u8,
    original_index: usize,
}

pub(crate) fn select_cards(
    cards: Vec<Value>,
    maximum: usize,
    knowledge: &KnowledgeCatalog,
    policy: &GenerationPolicy,
) -> Result<Vec<Value>, CommandError> {
    policy.validate()?;
    let mut candidates = Vec::new();
    let mut identities = BTreeSet::new();
    for (original_index, card) in cards.into_iter().enumerate() {
        let (family, identity) = if card["type"] == "interactive" {
            let question: GeneratedQuestion = serde_json::from_value(card["question"].clone())
                .map_err(|_| invalid("An admitted question lost its typed content."))?;
            let admitted = question.admit()?;
            (
                QuestionFamily::from_kind(admitted.kind().as_str())?,
                admitted.duplicate_identity()?,
            )
        } else {
            (
                QuestionFamily::from_kind(card["type"].as_str().unwrap_or(""))?,
                serde_json::to_string(&(
                    card["type"].clone(),
                    card["front"].clone(),
                    card["back"].clone(),
                    card["text"].clone(),
                    card["reverseEnabled"].clone(),
                    card["mode"].clone(),
                ))
                .map_err(CommandError::serialization)?,
            )
        };
        if !policy.allowed_types.contains(&family) {
            return Err(invalid("The model returned a disabled question family."));
        }
        let support = card_support(&card)?;
        if !identities.insert(identity) {
            continue;
        }
        let importance = support
            .iter()
            .filter_map(|item| knowledge.units.iter().find(|unit| unit.id == item.unit_id))
            .map(|unit| {
                if knowledge.document_core_units.contains(&unit.id) {
                    3
                } else {
                    match unit.content.grounding.as_ref().map(|g| g.importance) {
                        Some(crate::knowledge::evidence::Importance::Core) => 2,
                        Some(crate::knowledge::evidence::Importance::Detail) => 0,
                        _ => 1,
                    }
                }
            })
            .max()
            .unwrap_or(0);
        candidates.push(Candidate {
            cost: if card["type"] == "basic" && card["reverseEnabled"] == true {
                2
            } else {
                1
            },
            units: support.into_iter().map(|item| item.unit_id).collect(),
            card,
            family,
            importance,
            original_index,
        });
    }
    let total_weight = policy
        .allowed_types
        .iter()
        .map(|family| i64::from(*policy.weights.get(family).unwrap_or(&0)))
        .sum::<i64>();
    let mut chosen = Vec::new();
    let mut counts = BTreeMap::<QuestionFamily, usize>::new();
    let mut covered = BTreeSet::new();
    let mut spent = 0;
    while spent < maximum {
        let best = candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.cost <= maximum - spent)
            .max_by_key(|(_, candidate)| {
                let fresh = candidate.units.iter().any(|id| !covered.contains(id));
                let deficit = i64::from(*policy.weights.get(&candidate.family).unwrap_or(&0))
                    * (spent + candidate.cost) as i64
                    - *counts.get(&candidate.family).unwrap_or(&0) as i64 * total_weight;
                (
                    fresh,
                    candidate.importance,
                    deficit,
                    Reverse(candidate.original_index),
                )
            })
            .map(|(index, _)| index);
        let Some(index) = best else {
            break;
        };
        let candidate = candidates.remove(index);
        spent += candidate.cost;
        *counts.entry(candidate.family).or_default() += candidate.cost;
        covered.extend(candidate.units);
        // The stored transport remains byte-for-byte in content: no shortened
        // questions, changed keys, invented fillers, or rewritten source links.
        chosen.push(candidate.card);
    }
    Ok(chosen)
}

/// Generation settings affect candidate reuse, not canonical source extraction.
pub(crate) fn checkpoint_contract(
    contract: Option<String>,
    policy: &GenerationPolicy,
) -> Result<Option<String>, CommandError> {
    let fingerprint = policy.fingerprint()?;
    Ok(contract.map(|contract| hash(&format!("assessment-candidates-v1|{contract}|{fingerprint}"))))
}
