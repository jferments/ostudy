//! Native guide admission and provider schema. Editorial targets never truncate claims.
use super::*;

pub(super) fn parse_section_with_images(
    value: Value,
    knowledge: &KnowledgeCatalog,
    images_attached: bool,
) -> Result<GroundedStudyGuideSection, CommandError> {
    let mut value = value;
    if !images_attached {
        if let Some(content) = value.get_mut("content").and_then(Value::as_object_mut) {
            let discarded = content.get("visualExplanations").is_some_and(|items| {
                !items.is_null() && items.as_array().is_none_or(|items| !items.is_empty())
            });
            if discarded {
                crate::logging::debug_target("ai.document.study_guide", "study_guide_unavailable_visuals_omitted",
                    "Omitted an unsupported optional visual category; the remaining text and evidence still require validation.", json!({"imagesAttached":false}));
            }
            // Never publish invented visual claims. This optional category has
            // no authority to invalidate independent supported text sections.
            content.insert("visualExplanations".into(), json!([]));
        }
    }
    parse_grounded_section_with_images(value, knowledge, images_attached)
}

#[cfg(test)]
pub(super) fn parse_grounded_section(
    value: Value,
    knowledge: &KnowledgeCatalog,
) -> Result<GroundedStudyGuideSection, CommandError> {
    parse_grounded_section_with_images(value, knowledge, true)
}

fn parse_grounded_section_with_images(
    value: Value,
    knowledge: &KnowledgeCatalog,
    images_attached: bool,
) -> Result<GroundedStudyGuideSection, CommandError> {
    let schema = protocol::schema(knowledge, images_attached);
    crate::ai::structured_output::log_shape(&value, &schema, "study_guide_envelope");
    let content = value
        .get("content")
        .filter(|content| content.is_object())
        .ok_or_else(|| {
            output_error(format!(
                "The guide response requires a content object. {}",
                crate::ai::structured_output::shape_detail(&value, &schema)
            ))
        })?;
    let ids: Vec<String> = value
        .get("knowledgeUnitIds")
        .and_then(|ids| serde_json::from_value(ids.clone()).ok())
        .ok_or_else(|| {
            output_error(format!(
                "The guide response requires a knowledgeUnitIds array. {}",
                crate::ai::structured_output::shape_detail(&value, &schema)
            ))
        })?;
    let ids = protocol::resolve(&ids, knowledge)?;
    Ok(GroundedStudyGuideSection {
        content: parse_study_guide_section(content.clone())?,
        knowledge_support: knowledge.select(&ids, 64)?,
        catalog_fingerprint: knowledge.fingerprint()?,
    })
}

pub(super) fn parse_study_guide_section(
    value: Value,
) -> Result<StudyGuideSectionContent, CommandError> {
    let value = protocol::normalize_content(value);
    let section: StudyGuideSectionContent =
        serde_json::from_value(value.clone()).map_err(|_| {
            output_error(format!(
                "The guide content does not match its contract. {}",
                crate::ai::structured_output::shape_detail(&value, &study_guide_section_schema())
            ))
        })?;
    // Validate BEFORE the shared authored-content normalizer can truncate an
    // oversized field. Never cut a qualification to meet an editorial target.
    validate_generated_safety(&section)?;
    if let Err(error) = validate_generated_outline_style(section.clone()) {
        crate::logging::debug_target(
            "ai.document.study_guide",
            "study_guide_style_advisory",
            "Accepted intact supported content exceeding a soft outline-style target.",
            json!({"characters":generated_section_char_count(&section),"code":error.code}),
        );
    }
    validate_study_guide_section_content(section).map_err(|_| {
        output_error("The generated study-guide content failed native content validation.")
    })
}

/// Separate hard resource bounds from concise-writing targets. These are below
/// the storage normalizer's limits; no provider claim is truncated on admission.
pub(super) fn validate_generated_safety(
    section: &StudyGuideSectionContent,
) -> Result<(), CommandError> {
    if section.overview.trim().is_empty() || section.key_points.is_empty() {
        return Err(output_error(
            "A guide requires an overview and supported key points.",
        ));
    }
    if section.overview.split_whitespace().count() < 8 || inventory_like_prose(&section.overview) {
        return Err(output_error(
            "A guide overview must be narrative prose rather than a topic inventory.",
        ));
    }
    if section.key_points.iter().any(|value| {
        let words = value.split_whitespace().count();
        words <= 2
            && !value.contains(':')
            && !value.contains('=')
            && !value.chars().any(|character| character.is_ascii_digit())
    }) {
        return Err(output_error(
            "Guide key points must be factual propositions; bare vocabulary belongs in importantTerms.",
        ));
    }
    if generated_section_char_count(section) > 24_000 {
        return Err(output_error(
            "The guide section exceeds its hard 24000-character limit.",
        ));
    }
    require_char_limit("overview", &section.overview, 4_000)?;
    for (field, values) in [
        ("keyPoints", &section.key_points),
        ("processSteps", &section.process_steps),
        ("commonMisconceptions", &section.common_misconceptions),
    ] {
        require_list_limits(field, values, 32, 2_000)?;
        if values.iter().any(|value| value.trim().is_empty()) {
            return Err(output_error("A guide contains an empty item."));
        }
    }
    require_count_limit("explanations", section.explanations.len(), 16)?;
    for item in &section.explanations {
        require_char_limit("explanations.heading", &item.heading, 500)?;
        require_char_limit("explanations.body", &item.body, 4_000)?;
        if item.heading.trim().is_empty() || item.body.trim().is_empty() {
            return Err(output_error("A guide explanation is empty."));
        }
    }
    require_count_limit("importantTerms", section.important_terms.len(), 32)?;
    for item in &section.important_terms {
        require_char_limit("importantTerms.term", &item.term, 500)?;
        require_char_limit("importantTerms.explanation", &item.explanation, 2_000)?;
        if item.term.trim().is_empty() || item.explanation.trim().is_empty() {
            return Err(output_error("A guide term is empty."));
        }
    }
    require_count_limit("comparisons", section.comparisons.len(), 16)?;
    for item in &section.comparisons {
        require_char_limit("comparisons.subjectA", &item.subject_a, 500)?;
        require_char_limit("comparisons.subjectB", &item.subject_b, 500)?;
        require_char_limit("comparisons.relationship", &item.relationship, 2_000)?;
        if item.subject_a.trim().is_empty()
            || item.subject_b.trim().is_empty()
            || item.relationship.trim().is_empty()
        {
            return Err(output_error("A guide comparison is empty."));
        }
    }
    require_count_limit("callouts", section.callouts.len(), 16)?;
    for item in &section.callouts {
        require_char_limit("callouts.title", &item.title, 500)?;
        require_char_limit("callouts.body", &item.body, 4_000)?;
        if item.title.trim().is_empty() || item.body.trim().is_empty() {
            return Err(output_error("A guide callout is empty."));
        }
    }
    require_count_limit("tables", section.tables.len(), 8)?;
    for table in &section.tables {
        if table.title.trim().is_empty()
            || table.columns.len() < 2
            || table.columns.len() > 8
            || table.rows.is_empty()
            || table.rows.len() > 32
            || table
                .rows
                .iter()
                .any(|row| row.len() != table.columns.len())
        {
            return Err(output_error(
                "A guide table has an invalid rectangular shape.",
            ));
        }
        require_char_limit("tables.title", &table.title, 500)?;
        require_char_limit("tables.caption", &table.caption, 2_000)?;
        for cell in table.columns.iter().chain(table.rows.iter().flatten()) {
            require_char_limit("tables.cell", cell, 2_000)?;
            if cell.trim().is_empty() {
                return Err(output_error("A guide table contains an empty cell."));
            }
        }
    }
    require_count_limit("workedExamples", section.worked_examples.len(), 8)?;
    for example in &section.worked_examples {
        require_char_limit("workedExamples.title", &example.title, 500)?;
        require_char_limit("workedExamples.prompt", &example.prompt, 4_000)?;
        require_list_limits("workedExamples.steps", &example.steps, 16, 2_000)?;
        require_char_limit("workedExamples.answer", &example.answer, 4_000)?;
        require_char_limit("workedExamples.takeaway", &example.takeaway, 2_000)?;
        if example.title.trim().is_empty()
            || example.prompt.trim().is_empty()
            || example.steps.is_empty()
            || example.answer.trim().is_empty()
        {
            return Err(output_error("A worked example is incomplete."));
        }
    }
    require_count_limit("retrievalChecks", section.retrieval_checks.len(), 16)?;
    for check in &section.retrieval_checks {
        require_char_limit("retrievalChecks.prompt", &check.prompt, 2_000)?;
        require_char_limit("retrievalChecks.answer", &check.answer, 3_000)?;
        if check.prompt.trim().is_empty() || check.answer.trim().is_empty() {
            return Err(output_error("A retrieval check is incomplete."));
        }
    }
    require_count_limit("visualExplanations", section.visual_explanations.len(), 16)?;
    for item in &section.visual_explanations {
        require_char_limit("visualExplanations.title", &item.title, 500)?;
        require_char_limit("visualExplanations.explanation", &item.explanation, 4_000)?;
        if item.title.trim().is_empty() || item.explanation.trim().is_empty() {
            return Err(output_error("A guide visual explanation is empty."));
        }
    }
    Ok(())
}

pub(super) fn inventory_like_prose(value: &str) -> bool {
    let clean = value.trim();
    clean.contains(" / ")
        || clean.contains(" | ")
        || clean.matches("; ").count() >= 2
        || clean.lines().count() > 2
        || (clean.matches(',').count() >= 4
            && !clean.contains('.')
            && !clean.contains('!')
            && !clean.contains('?'))
}

pub(super) fn validate_generated_outline_style(
    section: StudyGuideSectionContent,
) -> Result<StudyGuideSectionContent, CommandError> {
    require_char_limit("overview", &section.overview, MAX_OVERVIEW_CHARS)?;
    if section.key_points.is_empty() {
        return Err(output_error(
            "The generated study-guide section did not include any high-yield key points.",
        ));
    }
    require_list_limits(
        "keyPoints",
        &section.key_points,
        MAX_KEY_POINTS,
        MAX_KEY_POINT_CHARS,
    )?;
    require_count_limit("explanations", section.explanations.len(), MAX_EXPLANATIONS)?;
    for explanation in &section.explanations {
        require_char_limit(
            "explanations.heading",
            &explanation.heading,
            MAX_EXPLANATION_HEADING_CHARS,
        )?;
        require_char_limit(
            "explanations.body",
            &explanation.body,
            MAX_EXPLANATION_BODY_CHARS,
        )?;
    }
    require_count_limit(
        "importantTerms",
        section.important_terms.len(),
        MAX_IMPORTANT_TERMS,
    )?;
    for term in &section.important_terms {
        require_char_limit("importantTerms.term", &term.term, MAX_TERM_CHARS)?;
        require_char_limit(
            "importantTerms.explanation",
            &term.explanation,
            MAX_TERM_EXPLANATION_CHARS,
        )?;
    }
    require_list_limits(
        "processSteps",
        &section.process_steps,
        MAX_PROCESS_STEPS,
        MAX_PROCESS_STEP_CHARS,
    )?;
    require_count_limit("comparisons", section.comparisons.len(), MAX_COMPARISONS)?;
    for comparison in &section.comparisons {
        require_char_limit(
            "comparisons.subjectA",
            &comparison.subject_a,
            MAX_COMPARISON_SUBJECT_CHARS,
        )?;
        require_char_limit(
            "comparisons.subjectB",
            &comparison.subject_b,
            MAX_COMPARISON_SUBJECT_CHARS,
        )?;
        require_char_limit(
            "comparisons.relationship",
            &comparison.relationship,
            MAX_COMPARISON_RELATIONSHIP_CHARS,
        )?;
    }
    require_list_limits(
        "commonMisconceptions",
        &section.common_misconceptions,
        MAX_MISCONCEPTIONS,
        MAX_MISCONCEPTION_CHARS,
    )?;
    require_count_limit("callouts", section.callouts.len(), MAX_CALLOUTS)?;
    for callout in &section.callouts {
        require_char_limit("callouts.title", &callout.title, MAX_CALLOUT_TITLE_CHARS)?;
        require_char_limit("callouts.body", &callout.body, MAX_CALLOUT_BODY_CHARS)?;
    }
    require_count_limit("tables", section.tables.len(), MAX_TABLES)?;
    for table in &section.tables {
        if table.columns.len() > MAX_TABLE_COLUMNS || table.rows.len() > MAX_TABLE_ROWS {
            return Err(output_error(
                "A guide table exceeded the concise table target.",
            ));
        }
        for cell in table.columns.iter().chain(table.rows.iter().flatten()) {
            require_char_limit("tables.cell", cell, MAX_TABLE_CELL_CHARS)?;
        }
    }
    require_count_limit(
        "workedExamples",
        section.worked_examples.len(),
        MAX_WORKED_EXAMPLES,
    )?;
    for example in &section.worked_examples {
        require_count_limit(
            "workedExamples.steps",
            example.steps.len(),
            MAX_WORKED_EXAMPLE_STEPS,
        )?;
    }
    require_count_limit(
        "retrievalChecks",
        section.retrieval_checks.len(),
        MAX_RETRIEVAL_CHECKS,
    )?;
    require_count_limit(
        "visualExplanations",
        section.visual_explanations.len(),
        MAX_VISUAL_EXPLANATIONS,
    )?;
    for visual in &section.visual_explanations {
        require_char_limit(
            "visualExplanations.title",
            &visual.title,
            MAX_VISUAL_TITLE_CHARS,
        )?;
        require_char_limit(
            "visualExplanations.explanation",
            &visual.explanation,
            MAX_VISUAL_EXPLANATION_CHARS,
        )?;
    }
    let total = generated_section_char_count(&section);
    if total > MAX_GENERATED_SECTION_CHARS {
        return Err(output_error(format!(
            "The generated study-guide section was too verbose ({total} characters; maximum {MAX_GENERATED_SECTION_CHARS}). Condense it to prioritized outline points and omit secondary detail."
        )));
    }
    Ok(section)
}

fn require_list_limits(
    field: &str,
    values: &[String],
    maximum_items: usize,
    maximum_chars: usize,
) -> Result<(), CommandError> {
    require_count_limit(field, values.len(), maximum_items)?;
    for value in values {
        require_char_limit(field, value, maximum_chars)?;
    }
    Ok(())
}

fn require_count_limit(field: &str, count: usize, maximum: usize) -> Result<(), CommandError> {
    if count > maximum {
        return Err(output_error(format!(
            "The generated study-guide field {field} contained {count} items; maximum {maximum}. Keep only the highest-yield items."
        )));
    }
    Ok(())
}

fn require_char_limit(field: &str, value: &str, maximum: usize) -> Result<(), CommandError> {
    let count = value.chars().count();
    if count > maximum {
        return Err(output_error(format!(
            "The generated study-guide field {field} was too wordy ({count} characters; maximum {maximum}). Rewrite it as concise outline material."
        )));
    }
    Ok(())
}

fn generated_section_char_count(section: &StudyGuideSectionContent) -> usize {
    let mut total = section.overview.chars().count();
    total += section
        .key_points
        .iter()
        .chain(section.process_steps.iter())
        .chain(section.common_misconceptions.iter())
        .map(|value| value.chars().count())
        .sum::<usize>();
    total += section
        .explanations
        .iter()
        .map(|value| value.heading.chars().count() + value.body.chars().count())
        .sum::<usize>();
    total += section
        .important_terms
        .iter()
        .map(|value| value.term.chars().count() + value.explanation.chars().count())
        .sum::<usize>();
    total += section
        .comparisons
        .iter()
        .map(|value| {
            value.subject_a.chars().count()
                + value.subject_b.chars().count()
                + value.relationship.chars().count()
        })
        .sum::<usize>();
    total += section
        .callouts
        .iter()
        .map(|value| value.title.chars().count() + value.body.chars().count())
        .sum::<usize>();
    total += section
        .tables
        .iter()
        .map(|value| {
            value.title.chars().count()
                + value.caption.chars().count()
                + value
                    .columns
                    .iter()
                    .map(|cell| cell.chars().count())
                    .sum::<usize>()
                + value
                    .rows
                    .iter()
                    .flatten()
                    .map(|cell| cell.chars().count())
                    .sum::<usize>()
        })
        .sum::<usize>();
    total += section
        .worked_examples
        .iter()
        .map(|value| {
            value.title.chars().count()
                + value.prompt.chars().count()
                + value
                    .steps
                    .iter()
                    .map(|step| step.chars().count())
                    .sum::<usize>()
                + value.answer.chars().count()
                + value.takeaway.chars().count()
        })
        .sum::<usize>();
    total += section
        .retrieval_checks
        .iter()
        .map(|value| value.prompt.chars().count() + value.answer.chars().count())
        .sum::<usize>();
    total += section
        .visual_explanations
        .iter()
        .map(|value| value.title.chars().count() + value.explanation.chars().count())
        .sum::<usize>();
    total
}

/// Provider-neutral study-guide schema.
///
/// This must remain a bare JSON Schema object. Provider-specific `name`,
/// `strict`, `response_format`, and function-tool wrappers belong exclusively to
/// transport. Double-wrapping this schema weakens OpenRouter enforcement and is
/// rejected by DeepSeek strict function calling.
pub(super) fn study_guide_section_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "overview",
            "keyPoints",
            "explanations",
            "importantTerms",
            "processSteps",
            "comparisons",
            "commonMisconceptions",
            "callouts",
            "tables",
            "workedExamples",
            "retrievalChecks",
            "visualExplanations"
        ],
        "properties": {
            "overview": {
                "type": "string",
                "minLength": 1,
                "maxLength": MAX_OVERVIEW_CHARS
            },
            "keyPoints": {
                "type": "array", "minItems": 1, "maxItems": MAX_KEY_POINTS,
                "items": {"type": "string", "minLength": 1, "maxLength": MAX_KEY_POINT_CHARS}
            },
            "explanations": {
                "type": "array",
                "maxItems": MAX_EXPLANATIONS,
                "items": object_schema(
                    &["heading", "body"],
                    json!({
                        "heading": {"type": "string", "minLength": 1, "maxLength": MAX_EXPLANATION_HEADING_CHARS},
                        "body": {"type": "string", "minLength": 1, "maxLength": MAX_EXPLANATION_BODY_CHARS}
                    })
                )
            },
            "importantTerms": {
                "type": "array",
                "maxItems": MAX_IMPORTANT_TERMS,
                "items": object_schema(
                    &["term", "explanation"],
                    json!({
                        "term": {"type": "string", "minLength": 1, "maxLength": MAX_TERM_CHARS},
                        "explanation": {"type": "string", "minLength": 1, "maxLength": MAX_TERM_EXPLANATION_CHARS}
                    })
                )
            },
            "processSteps": string_array_schema(MAX_PROCESS_STEPS, MAX_PROCESS_STEP_CHARS),
            "comparisons": {
                "type": "array",
                "maxItems": MAX_COMPARISONS,
                "items": object_schema(
                    &["subjectA", "subjectB", "relationship"],
                    json!({
                        "subjectA": {"type": "string", "minLength": 1, "maxLength": MAX_COMPARISON_SUBJECT_CHARS},
                        "subjectB": {"type": "string", "minLength": 1, "maxLength": MAX_COMPARISON_SUBJECT_CHARS},
                        "relationship": {"type": "string", "minLength": 1, "maxLength": MAX_COMPARISON_RELATIONSHIP_CHARS}
                    })
                )
            },
            "commonMisconceptions": string_array_schema(MAX_MISCONCEPTIONS, MAX_MISCONCEPTION_CHARS),
            "callouts": {
                "type": "array",
                "maxItems": MAX_CALLOUTS,
                "items": object_schema(
                    &["kind", "title", "body"],
                    json!({
                        "kind": {"type": "string", "enum": ["key_idea", "warning", "tip", "note"]},
                        "title": {"type": "string", "minLength": 1, "maxLength": MAX_CALLOUT_TITLE_CHARS},
                        "body": {"type": "string", "minLength": 1, "maxLength": MAX_CALLOUT_BODY_CHARS}
                    })
                )
            },
            "tables": {
                "type": "array",
                "maxItems": MAX_TABLES,
                "items": object_schema(
                    &["title", "columns", "rows", "caption"],
                    json!({
                        "title": {"type": "string", "minLength": 1, "maxLength": MAX_CALLOUT_TITLE_CHARS},
                        "columns": {"type": "array", "minItems": 2, "maxItems": MAX_TABLE_COLUMNS, "items": {"type": "string", "minLength": 1, "maxLength": MAX_TABLE_CELL_CHARS}},
                        "rows": {"type": "array", "minItems": 1, "maxItems": MAX_TABLE_ROWS, "items": {"type": "array", "minItems": 2, "maxItems": MAX_TABLE_COLUMNS, "items": {"type": "string", "minLength": 1, "maxLength": MAX_TABLE_CELL_CHARS}}},
                        "caption": {"type": "string", "maxLength": MAX_COMPARISON_RELATIONSHIP_CHARS}
                    })
                )
            },
            "workedExamples": {
                "type": "array",
                "maxItems": MAX_WORKED_EXAMPLES,
                "items": object_schema(
                    &["title", "prompt", "steps", "answer", "takeaway"],
                    json!({
                        "title": {"type": "string", "minLength": 1, "maxLength": MAX_CALLOUT_TITLE_CHARS},
                        "prompt": {"type": "string", "minLength": 1, "maxLength": MAX_EXPLANATION_BODY_CHARS},
                        "steps": string_array_schema(MAX_WORKED_EXAMPLE_STEPS, MAX_PROCESS_STEP_CHARS),
                        "answer": {"type": "string", "minLength": 1, "maxLength": MAX_EXPLANATION_BODY_CHARS},
                        "takeaway": {"type": "string", "maxLength": MAX_EXPLANATION_BODY_CHARS}
                    })
                )
            },
            "retrievalChecks": {
                "type": "array",
                "maxItems": MAX_RETRIEVAL_CHECKS,
                "items": object_schema(
                    &["prompt", "answer"],
                    json!({
                        "prompt": {"type": "string", "minLength": 1, "maxLength": MAX_MISCONCEPTION_CHARS},
                        "answer": {"type": "string", "minLength": 1, "maxLength": MAX_EXPLANATION_BODY_CHARS}
                    })
                )
            },
            "visualExplanations": {
                "type": "array",
                "maxItems": MAX_VISUAL_EXPLANATIONS,
                "items": object_schema(
                    &["title", "explanation"],
                    json!({
                        "title": {"type": "string", "minLength": 1, "maxLength": MAX_VISUAL_TITLE_CHARS},
                        "explanation": {"type": "string", "minLength": 1, "maxLength": MAX_VISUAL_EXPLANATION_CHARS}
                    })
                )
            }
        }
    })
}

fn string_array_schema(maximum_items: usize, maximum_length: usize) -> Value {
    json!({
        "type": "array",
        "maxItems": maximum_items,
        "items": {
            "type": "string",
            "minLength": 1,
            "maxLength": maximum_length
        }
    })
}

fn object_schema(required: &[&str], properties: Value) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties
    })
}
