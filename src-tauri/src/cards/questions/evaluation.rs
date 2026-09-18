use std::collections::BTreeSet;

use super::{
    bounded_size, invalid,
    model::*,
    validation::{member, subset},
    QuestionError, ValidatedQuestion, GRADING_POLICY_VERSION, MAX_RESPONSE_BYTES,
};

impl ValidatedQuestion {
    pub(super) fn evaluate(
        &self,
        response: &QuestionResponse,
    ) -> Result<Evaluation, QuestionError> {
        bounded_size(response, MAX_RESPONSE_BYTES)?;
        let (matched, required, complete, exact, components, explanation) =
            match (&self.definition.question, response) {
                (
                    QuestionBody::SingleChoice {
                        options,
                        correct_option_id,
                        explanation,
                        ..
                    },
                    QuestionResponse::SingleChoice { selected_option_id },
                ) => {
                    if let Some(id) = selected_option_id {
                        member(id, options.iter().map(|option| option.id.as_str()))?;
                    }
                    let exact = selected_option_id.as_ref() == Some(correct_option_id);
                    let components = options
                        .iter()
                        .map(|option| ComponentFeedback::Choice {
                            item_id: option.id.clone(),
                            selected: selected_option_id.as_ref() == Some(&option.id),
                            expected: &option.id == correct_option_id,
                            rationale: option.rationale.clone(),
                        })
                        .collect();
                    (
                        usize::from(exact),
                        1,
                        selected_option_id.is_some(),
                        exact,
                        components,
                        explanation.clone(),
                    )
                }
                (
                    QuestionBody::SelectAll {
                        options,
                        correct_option_ids,
                        explanation,
                        ..
                    },
                    QuestionResponse::SelectAll {
                        selected_option_ids,
                    },
                ) => {
                    subset(
                        selected_option_ids,
                        options.iter().map(|option| option.id.as_str()),
                    )?;
                    let matched = selected_option_ids
                        .iter()
                        .filter(|id| correct_option_ids.contains(id))
                        .count();
                    let exact =
                        matched == correct_option_ids.len() && matched == selected_option_ids.len();
                    let components = options
                        .iter()
                        .map(|option| ComponentFeedback::Choice {
                            item_id: option.id.clone(),
                            selected: selected_option_ids.contains(&option.id),
                            expected: correct_option_ids.contains(&option.id),
                            rationale: option.rationale.clone(),
                        })
                        .collect();
                    // Explicit submission of [] is complete, not missing input.
                    (
                        matched,
                        correct_option_ids.len(),
                        true,
                        exact,
                        components,
                        explanation.clone(),
                    )
                }
                (
                    QuestionBody::TrueFalse {
                        correct,
                        explanation,
                        corrected_statement,
                        ..
                    },
                    QuestionResponse::TrueFalse { selected },
                ) => {
                    let exact = selected.as_ref() == Some(correct);
                    (
                        usize::from(exact),
                        1,
                        selected.is_some(),
                        exact,
                        vec![ComponentFeedback::Boolean {
                            selected: *selected,
                            expected: *correct,
                            corrected_statement: corrected_statement.clone(),
                        }],
                        explanation.clone(),
                    )
                }
                (
                    QuestionBody::Matching {
                        pairs, explanation, ..
                    },
                    QuestionResponse::Matching { assignments },
                ) => {
                    validate_assignments(
                        assignments,
                        pairs.iter().map(|pair| pair.left.id.as_str()),
                        pairs.iter().map(|pair| pair.right.id.as_str()),
                        true,
                    )?;
                    let mut matched = 0;
                    let components = pairs
                        .iter()
                        .map(|pair| {
                            let selected = assignments
                                .iter()
                                .find(|entry| entry.item_id == pair.left.id)
                                .map(|entry| entry.target_id.clone());
                            matched += usize::from(selected.as_ref() == Some(&pair.right.id));
                            ComponentFeedback::Placement {
                                item_id: pair.left.id.clone(),
                                selected_target_id: selected,
                                expected_target_id: pair.right.id.clone(),
                                rationale: pair.rationale.clone(),
                            }
                        })
                        .collect();
                    (
                        matched,
                        pairs.len(),
                        assignments.len() == pairs.len(),
                        matched == pairs.len(),
                        components,
                        explanation.clone(),
                    )
                }
                (
                    QuestionBody::Grouping {
                        categories,
                        items,
                        explanation,
                        ..
                    },
                    QuestionResponse::Grouping { assignments },
                ) => {
                    validate_assignments(
                        assignments,
                        items.iter().map(|item| item.item.id.as_str()),
                        categories.iter().map(|category| category.id.as_str()),
                        false,
                    )?;
                    let mut matched = 0;
                    let components = items
                        .iter()
                        .map(|grouped| {
                            let selected = assignments
                                .iter()
                                .find(|entry| entry.item_id == grouped.item.id)
                                .map(|entry| entry.target_id.clone());
                            matched += usize::from(selected.as_ref() == Some(&grouped.category_id));
                            ComponentFeedback::Placement {
                                item_id: grouped.item.id.clone(),
                                selected_target_id: selected,
                                expected_target_id: grouped.category_id.clone(),
                                rationale: grouped.rationale.clone(),
                            }
                        })
                        .collect();
                    (
                        matched,
                        items.len(),
                        assignments.len() == items.len(),
                        matched == items.len(),
                        components,
                        explanation.clone(),
                    )
                }
                (
                    QuestionBody::Ordering {
                        items, explanation, ..
                    },
                    QuestionResponse::Ordering { ordered_item_ids },
                ) => {
                    subset(ordered_item_ids, items.iter().map(|item| item.id.as_str()))?;
                    let mut matched = 0;
                    let components = items
                        .iter()
                        .enumerate()
                        .map(|(expected, item)| {
                            let selected = ordered_item_ids.iter().position(|id| id == &item.id);
                            matched += usize::from(selected == Some(expected));
                            ComponentFeedback::Position {
                                item_id: item.id.clone(),
                                selected_position: selected,
                                expected_position: expected,
                            }
                        })
                        .collect();
                    (
                        matched,
                        items.len(),
                        ordered_item_ids.len() == items.len(),
                        matched == items.len(),
                        components,
                        explanation.clone(),
                    )
                }
                (QuestionBody::ImageOcclusion { .. }, _) => {
                    return Err(QuestionError::new(
                        "question_requires_self_rating",
                        concat!(
                            "Reveal image-occlusion answers and rate recall; ",
                            "automatic grading is not supported.",
                        ),
                    ));
                }
                _ => {
                    return Err(invalid(
                        "The response kind does not match the captured question.",
                    ))
                }
            };
        Ok(Evaluation {
            grading_policy_version: GRADING_POLICY_VERSION,
            verdict: if !complete {
                Verdict::Incomplete
            } else if exact {
                Verdict::Correct
            } else {
                Verdict::Incorrect
            },
            matched_count: matched,
            required_count: required,
            components,
            explanation,
        })
    }
}

fn validate_assignments<'a>(
    assignments: &[Assignment],
    items: impl Iterator<Item = &'a str>,
    targets: impl Iterator<Item = &'a str>,
    one_to_one: bool,
) -> Result<(), QuestionError> {
    let items: BTreeSet<_> = items.collect();
    let targets: BTreeSet<_> = targets.collect();
    if assignments.len() > items.len() {
        return Err(invalid("A response has more assignments than items."));
    }
    let mut used_items = BTreeSet::new();
    let mut used_targets = BTreeSet::new();
    for assignment in assignments {
        if !items.contains(assignment.item_id.as_str())
            || !targets.contains(assignment.target_id.as_str())
            || !used_items.insert(assignment.item_id.as_str())
            || (one_to_one && !used_targets.insert(assignment.target_id.as_str()))
        {
            return Err(invalid(
                "A response contains an invalid or duplicate assignment.",
            ));
        }
    }
    Ok(())
}
