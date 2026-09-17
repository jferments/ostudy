use std::collections::BTreeMap;

use super::{invalid, EvidenceStance, GeneratedQuestion};
use crate::error::CommandError;

pub(super) fn requirements(
    question: &GeneratedQuestion,
    maximum: usize,
) -> Result<BTreeMap<String, EvidenceStance>, CommandError> {
    use EvidenceStance::{Contradicts, Supports};
    let mut required = BTreeMap::from([("prompt".to_string(), Supports)]);
    match question {
        GeneratedQuestion::SingleChoice { options, correct_option_id, .. } => {
            bound(options.len(), maximum)?;
            for option in options {
                let correct = &option.id == correct_option_id;
                if !correct { rationale(&option.rationale)?; }
                required.insert(format!("option:{}", option.id), if correct { Supports } else { Contradicts });
            }
        }
        GeneratedQuestion::SelectAll { options, correct_option_ids, .. } => {
            bound(options.len(), maximum)?;
            for option in options {
                let correct = correct_option_ids.contains(&option.id);
                if !correct { rationale(&option.rationale)?; }
                required.insert(format!("option:{}", option.id), if correct { Supports } else { Contradicts });
            }
        }
        GeneratedQuestion::TrueFalse { correct, explanation, corrected_statement, .. } => {
            rationale(explanation)?;
            required.insert("statement".to_string(), if *correct { Supports } else { Contradicts });
            if !correct {
                rationale(corrected_statement)?;
                required.insert("correction".to_string(), Supports);
            }
        }
        GeneratedQuestion::Matching { pairs, .. } => {
            bound(pairs.len(), maximum)?;
            for pair in pairs { required.insert(format!("pair:{}", pair.id), Supports); }
        }
        GeneratedQuestion::Grouping { categories, items, .. } => {
            bound(categories.len(), maximum)?;
            bound(items.len(), maximum)?;
            for category in categories { required.insert(format!("category:{}", category.id), Supports); }
            for item in items { required.insert(format!("item:{}", item.id), Supports); }
        }
        GeneratedQuestion::Ordering { items, .. } => {
            bound(items.len(), maximum)?;
            for item in items { required.insert(format!("item:{}", item.id), Supports); }
            // Evidence for isolated steps does not establish their order. Every
            // adjacent precedence edge must be grounded under the same criterion.
            for pair in items.windows(2) {
                required.insert(format!("before:{}:{}", pair[0].id, pair[1].id), Supports);
            }
        }
    }
    Ok(required)
}

fn bound(count: usize, maximum: usize) -> Result<(), CommandError> {
    if count > maximum { return Err(invalid("The generated board exceeds the chosen component limit.")); }
    Ok(())
}

fn rationale(value: &Option<String>) -> Result<(), CommandError> {
    if value.as_deref().is_none_or(|text| text.trim().is_empty()) {
        return Err(invalid("Generated distractors and false statements require a source-supported correction or rationale."));
    }
    Ok(())
}
