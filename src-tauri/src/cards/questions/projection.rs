use super::{invalid, model::*, validation::permutation, QuestionError, ValidatedQuestion};
use crate::cards::collect_asset_ids;

/// Native-only rendering input, deliberately not Serialize. The original image
/// identity must not enter a pre-answer WebView/Tutor projection.
pub struct OcclusionMaskPlan {
    pub image_asset_id: String,
    pub target_id: String,
    pub hidden_masks: Vec<ImageMask>,
    pub target_mask_ids: Vec<String>,
}

impl ValidatedQuestion {
    pub(super) fn validate_order(&self, order: &DisplayOrder) -> Result<(), QuestionError> {
        match &self.definition.question {
            QuestionBody::SingleChoice { options, .. }
            | QuestionBody::SelectAll { options, .. } => {
                permutation(
                    &order.item_ids,
                    options.iter().map(|option| option.id.as_str()),
                )?;
                permutation(&order.target_ids, std::iter::empty())
            }
            QuestionBody::Matching { pairs, .. } => {
                permutation(
                    &order.item_ids,
                    pairs.iter().map(|pair| pair.left.id.as_str()),
                )?;
                permutation(
                    &order.target_ids,
                    pairs.iter().map(|pair| pair.right.id.as_str()),
                )
            }
            QuestionBody::Grouping {
                categories, items, ..
            } => {
                permutation(
                    &order.item_ids,
                    items.iter().map(|item| item.item.id.as_str()),
                )?;
                permutation(
                    &order.target_ids,
                    categories.iter().map(|category| category.id.as_str()),
                )
            }
            QuestionBody::Ordering { items, .. } => {
                permutation(&order.item_ids, items.iter().map(|item| item.id.as_str()))?;
                permutation(&order.target_ids, std::iter::empty())
            }
            QuestionBody::TrueFalse { .. } | QuestionBody::ImageOcclusion { .. } => {
                permutation(&order.item_ids, std::iter::empty())?;
                permutation(&order.target_ids, std::iter::empty())
            }
        }
    }

    pub(super) fn project(&self, order: &DisplayOrder) -> Result<PublicQuestion, QuestionError> {
        self.validate_order(order)?;
        let public = match &self.definition.question {
            QuestionBody::SingleChoice {
                prompt, options, ..
            } => PublicQuestion::SingleChoice {
                prompt: prompt.clone(),
                options: ordered_choices(options, &order.item_ids)?,
            },
            QuestionBody::SelectAll {
                prompt, options, ..
            } => PublicQuestion::SelectAll {
                prompt: prompt.clone(),
                options: ordered_choices(options, &order.item_ids)?,
            },
            QuestionBody::TrueFalse { statement, .. } => PublicQuestion::TrueFalse {
                statement: statement.clone(),
            },
            QuestionBody::Matching { prompt, pairs, .. } => PublicQuestion::Matching {
                prompt: prompt.clone(),
                items: ordered_items(pairs.iter().map(|pair| &pair.left), &order.item_ids)?,
                targets: ordered_items(pairs.iter().map(|pair| &pair.right), &order.target_ids)?,
            },
            QuestionBody::Grouping {
                prompt,
                categories,
                items,
                ..
            } => PublicQuestion::Grouping {
                prompt: prompt.clone(),
                items: ordered_items(items.iter().map(|item| &item.item), &order.item_ids)?,
                categories: ordered_items(categories.iter(), &order.target_ids)?,
            },
            QuestionBody::Ordering { prompt, items, .. } => PublicQuestion::Ordering {
                prompt: prompt.clone(),
                items: ordered_items(items.iter(), &order.item_ids)?,
            },
            QuestionBody::ImageOcclusion { .. } => {
                return Err(QuestionError::new(
                    "question_image_not_prepared",
                    "Prepare a managed masked image before displaying this question.",
                ));
            }
        };
        Ok(public)
    }

    pub fn occlusion_mask_plan(
        &self,
        presentation_key: &str,
    ) -> Result<OcclusionMaskPlan, QuestionError> {
        let QuestionBody::ImageOcclusion {
            image_asset_id,
            masks,
            questions,
        } = &self.definition.question
        else {
            return Err(invalid("Only image-occlusion questions have a mask plan."));
        };
        let target = questions
            .iter()
            .find(|target| target.id == presentation_key)
            .ok_or_else(|| invalid("The image-occlusion target no longer exists."))?;
        Ok(OcclusionMaskPlan {
            image_asset_id: image_asset_id.clone(),
            target_id: target.id.clone(),
            hidden_masks: masks
                .iter()
                .filter(|mask| {
                    target.mode == OcclusionMode::HideAll || target.mask_ids.contains(&mask.id)
                })
                .cloned()
                .collect(),
            target_mask_ids: target.mask_ids.clone(),
        })
    }
}

impl PublicQuestion {
    /// Only visible field assets; no option rationales or hidden answer media.
    pub fn asset_ids(&self) -> Result<Vec<String>, QuestionError> {
        let mut documents = Vec::new();
        match self {
            Self::SingleChoice { prompt, options } | Self::SelectAll { prompt, options } => {
                documents.push(prompt);
                documents.extend(options.iter().map(|option| &option.content));
            }
            Self::TrueFalse { statement } => documents.push(statement),
            Self::Matching {
                prompt,
                items,
                targets,
            } => {
                documents.push(prompt);
                documents.extend(items.iter().chain(targets).map(|item| &item.content));
            }
            Self::Grouping {
                prompt,
                items,
                categories,
            } => {
                documents.push(prompt);
                documents.extend(items.iter().chain(categories).map(|item| &item.content));
            }
            Self::Ordering { prompt, items } => {
                documents.push(prompt);
                documents.extend(items.iter().map(|item| &item.content));
            }
        }
        let mut ids = std::collections::BTreeSet::new();
        for document in documents {
            ids.extend(collect_asset_ids(document)?);
        }
        Ok(ids.into_iter().collect())
    }
}

fn ordered_choices(
    options: &[ChoiceOption],
    order: &[String],
) -> Result<Vec<RichItem>, QuestionError> {
    order
        .iter()
        .map(|id| {
            let option = options
                .iter()
                .find(|option| &option.id == id)
                .ok_or_else(|| invalid("A displayed option does not exist."))?;
            Ok(RichItem {
                id: option.id.clone(),
                content: option.content.clone(),
            })
        })
        .collect()
}

fn ordered_items<'a>(
    items: impl Iterator<Item = &'a RichItem>,
    order: &[String],
) -> Result<Vec<RichItem>, QuestionError> {
    let items: Vec<_> = items.collect();
    order
        .iter()
        .map(|id| {
            items
                .iter()
                .find(|item| &item.id == id)
                .map(|item| (*item).clone())
                .ok_or_else(|| invalid("A displayed item does not exist."))
        })
        .collect()
}
