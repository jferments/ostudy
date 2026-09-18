use std::collections::BTreeMap;

use super::{invalid, EvidenceStance, GeneratedQuestion};
use crate::error::CommandError;

/// Evidence that is mandatory because the learner's objective result depends on
/// this exact answer component. Prompt/category/item context may also be
/// grounded, but it is not independently graded and therefore cannot make an
/// otherwise fully grounded question invalid merely because the provider omits a
/// redundant binding.
pub(super) fn requirements(
    question: &GeneratedQuestion,
    maximum: usize,
) -> Result<BTreeMap<String, EvidenceStance>, CommandError> {
    use EvidenceStance::{Contradicts, Supports};
    let mut required = BTreeMap::new();
    match question {
        GeneratedQuestion::SingleChoice {
            options,
            correct_option_id,
            ..
        } => {
            bound(options.len(), maximum)?;
            for option in options {
                let correct = &option.id == correct_option_id;
                if !correct {
                    rationale(&option.rationale)?;
                }
                required.insert(
                    format!("option:{}", option.id),
                    if correct { Supports } else { Contradicts },
                );
            }
        }
        GeneratedQuestion::SelectAll {
            options,
            correct_option_ids,
            ..
        } => {
            bound(options.len(), maximum)?;
            for option in options {
                let correct = correct_option_ids.contains(&option.id);
                if !correct {
                    rationale(&option.rationale)?;
                }
                required.insert(
                    format!("option:{}", option.id),
                    if correct { Supports } else { Contradicts },
                );
            }
        }
        GeneratedQuestion::TrueFalse {
            correct,
            explanation,
            corrected_statement,
            ..
        } => {
            rationale(explanation)?;
            required.insert(
                "statement".to_string(),
                if *correct { Supports } else { Contradicts },
            );
            if !correct {
                rationale(corrected_statement)?;
                required.insert("correction".to_string(), Supports);
            }
        }
        GeneratedQuestion::Matching { pairs, .. } => {
            bound(pairs.len(), maximum)?;
            for pair in pairs {
                required.insert(format!("pair:{}", pair.id), Supports);
            }
        }
        GeneratedQuestion::Grouping {
            categories, items, ..
        } => {
            bound(categories.len(), maximum)?;
            bound(items.len(), maximum)?;
            for item in items {
                // The graded proposition is membership in the selected category;
                // the category label itself is context, not a separate answer.
                required.insert(format!("item:{}", item.id), Supports);
            }
        }
        GeneratedQuestion::Ordering { items, .. } => {
            bound(items.len(), maximum)?;
            // Evidence for isolated steps does not establish their order. Every
            // adjacent precedence edge is the graded relation and must be
            // grounded under the same criterion.
            for pair in items.windows(2) {
                required.insert(format!("before:{}:{}", pair[0].id, pair[1].id), Supports);
            }
        }
    }
    Ok(required)
}

/// Superset accepted for compatibility with earlier Stage-7 checkpoints and for
/// providers that choose to return useful contextual grounding in addition to
/// the mandatory graded components.
pub(super) fn allowed_requirements(
    question: &GeneratedQuestion,
    maximum: usize,
) -> Result<BTreeMap<String, EvidenceStance>, CommandError> {
    use EvidenceStance::Supports;
    let mut allowed = requirements(question, maximum)?;
    allowed.insert("prompt".to_string(), Supports);
    match question {
        GeneratedQuestion::Grouping { categories, .. } => {
            for category in categories {
                allowed.insert(format!("category:{}", category.id), Supports);
            }
        }
        GeneratedQuestion::Ordering { items, .. } => {
            for item in items {
                allowed.insert(format!("item:{}", item.id), Supports);
            }
        }
        _ => {}
    }
    Ok(allowed)
}

fn bound(count: usize, maximum: usize) -> Result<(), CommandError> {
    if count > maximum {
        return Err(invalid(
            "The generated board exceeds the chosen component limit.",
        ));
    }
    Ok(())
}

fn rationale(value: &Option<String>) -> Result<(), CommandError> {
    if value.as_deref().is_none_or(|text| text.trim().is_empty()) {
        return Err(invalid("Generated distractors and false statements require a source-supported correction or rationale."));
    }
    Ok(())
}
