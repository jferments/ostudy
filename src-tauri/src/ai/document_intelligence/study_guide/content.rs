//! Native admission and provider schema share defensive capacities. Writing
//! guidance is request-local and may never reject or truncate valid content.
use super::*;

/// Most knowledge units one section's support metadata keeps.
const MAX_SUPPORT_UNITS: usize = 64;

pub(super) fn parse_section_with_images(
    value: Value,
    knowledge: &KnowledgeCatalog,
    images_attached: bool,
) -> Result<GroundedStudyGuideSection, CommandError> {
    parse_section_with_images_mode(value, knowledge, images_attached, true)
}

pub(super) fn parse_section_with_images_style_advisory(
    value: Value,
    knowledge: &KnowledgeCatalog,
    images_attached: bool,
) -> Result<GroundedStudyGuideSection, CommandError> {
    parse_section_with_images_mode(value, knowledge, images_attached, false)
}

fn parse_section_with_images_mode(
    mut value: Value,
    knowledge: &KnowledgeCatalog,
    images_attached: bool,
    require_writing_style: bool,
) -> Result<GroundedStudyGuideSection, CommandError> {
    let bytes = value.to_string().len();
    if bytes > MAX_CANDIDATE_BYTES {
        return Err(resource_error("response bytes", bytes, MAX_CANDIDATE_BYTES));
    }
    if !images_attached {
        if let Some(content) = value.get_mut("content").and_then(Value::as_object_mut) {
            let discarded = content.get("visualExplanations").is_some_and(|items| {
                !items.is_null() && items.as_array().is_none_or(|items| !items.is_empty())
            });
            if discarded {
                crate::logging::debug_target("ai.document.study_guide", "study_guide_unavailable_visuals_omitted",
                    "Omitted an unsupported optional visual category; the remaining text and evidence still require validation.", json!({"imagesAttached":false}));
            }
            // No authority to invent visual claims when no image was supplied.
            content.insert("visualExplanations".into(), json!([]));
        }
    }
    parse_grounded_section_with_images(value, knowledge, images_attached, require_writing_style)
}

#[cfg(test)]
pub(super) fn parse_grounded_section(
    value: Value,
    knowledge: &KnowledgeCatalog,
) -> Result<GroundedStudyGuideSection, CommandError> {
    parse_section_with_images_mode(value, knowledge, true, true)
}

fn parse_grounded_section_with_images(
    value: Value,
    knowledge: &KnowledgeCatalog,
    images_attached: bool,
    require_writing_style: bool,
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
    let mut ids = protocol::resolve(&ids, knowledge)?;
    // Support metadata keeps at most 64 units. A model that names more has
    // selected most of the catalog; failing the section for it cost whole
    // topics of an otherwise valid guide.
    if ids.is_empty() {
        // The section was written from this catalog; an empty selection
        // records it as the section's support instead of losing the topic.
        ids = knowledge.units.iter().map(|unit| unit.id.clone()).collect();
        crate::logging::debug_target(
            "ai.document.study_guide",
            "study_guide_empty_support_filled",
            "Recorded the topic's catalog as support for a section that selected no units.",
            json!({"units": knowledge.units.len()}),
        );
    }
    ids.truncate(MAX_SUPPORT_UNITS);
    let content = parse_study_guide_section_mode(content.clone(), require_writing_style)?;
    let budget = WritingBudget::for_knowledge(knowledge);
    if budget.exceeded_by(&content) {
        crate::logging::debug_target(
            "ai.document.study_guide",
            "study_guide_size_advisory",
            "Retained intact content beyond a flexible writing target; no size-only repair was requested.",
            json!({"characters":generated_section_char_count(&content),
                "overviewCharacters":content.overview.chars().count(),
                "targetCharacters":budget.target_chars,"knowledgeUnits":knowledge.units.len()}),
        );
    }
    Ok(GroundedStudyGuideSection {
        content,
        knowledge_support: knowledge.select(&ids, MAX_SUPPORT_UNITS)?,
        catalog_fingerprint: knowledge.fingerprint()?,
    })
}

#[cfg(test)]
pub(super) fn parse_study_guide_section(
    value: Value,
) -> Result<StudyGuideSectionContent, CommandError> {
    parse_study_guide_section_mode(value, true)
}

fn parse_study_guide_section_mode(
    value: Value,
    require_writing_style: bool,
) -> Result<StudyGuideSectionContent, CommandError> {
    let value = protocol::normalize_content(value);
    let mut section: StudyGuideSectionContent =
        serde_json::from_value(value.clone()).map_err(|_| {
            output_error(format!(
                "The guide content does not match its contract. {}",
                crate::ai::structured_output::shape_detail(&value, &study_guide_section_schema())
            ))
        })?;
    repair_table_shapes(&mut section);
    let restated = super::dedupe::drop_restated_items(&mut section);
    if restated > 0 {
        crate::logging::debug_target(
            "ai.document.study_guide",
            "study_guide_restated_items_dropped",
            "Dropped study-aid items that restated an earlier sentence; content is not logged.",
            json!({"droppedItems": restated}),
        );
    }
    // All resource checks precede the legacy normalizer. Each capacity is below
    // that normalizer's truncation boundary, including table column labels.
    validate_generated_safety(&section)?;
    if let Err(error) = validate_generated_writing_style(&section) {
        if require_writing_style {
            return Err(error);
        }
        crate::logging::warn_target(
            "ai.document.study_guide",
            "study_guide_style_repair_fallback",
            "Retained structurally valid content with admitted support after bounded style repair; content is not logged.",
            json!({"code":error.code}),
        );
    }
    validate_study_guide_section_content(section).map_err(|_| {
        output_error("The generated study-guide content failed native content validation.")
    })
}

/// Mark a cell the model left blank or short. Readers see a gap, not a shift.
const MISSING_TABLE_CELL: &str = "—";

/// A ragged table is a common, harmless model slip; failing the whole section
/// for it threw away good prose. Short rows are padded with a visible gap,
/// surplus cells join the last column so no text is lost, blank cells are
/// marked, and a table with no usable grid (under two columns or no rows) is
/// left out. Every other table limit is still enforced afterwards.
fn repair_table_shapes(section: &mut StudyGuideSectionContent) {
    let mut repaired = 0usize;
    let before = section.tables.len();
    section
        .tables
        .retain(|table| table.columns.len() >= 2 && !table.rows.is_empty());
    for table in &mut section.tables {
        let width = table.columns.len();
        for column in &mut table.columns {
            if column.trim().is_empty() {
                *column = MISSING_TABLE_CELL.to_string();
                repaired += 1;
            }
        }
        for row in &mut table.rows {
            if row.len() > width {
                let surplus = row
                    .split_off(width)
                    .into_iter()
                    .filter(|cell| !cell.trim().is_empty())
                    .collect::<Vec<_>>();
                if !surplus.is_empty() {
                    let last = &mut row[width - 1];
                    let joined = std::iter::once(last.trim())
                        .filter(|cell| !cell.is_empty())
                        .chain(surplus.iter().map(|cell| cell.trim()))
                        .collect::<Vec<_>>()
                        .join("; ");
                    *last = joined;
                }
                repaired += 1;
            }
            if row.len() < width {
                row.resize(width, MISSING_TABLE_CELL.to_string());
                repaired += 1;
            }
            for cell in row.iter_mut() {
                if cell.trim().is_empty() {
                    *cell = MISSING_TABLE_CELL.to_string();
                    repaired += 1;
                }
            }
        }
    }
    let dropped = before - section.tables.len();
    if repaired > 0 || dropped > 0 {
        crate::logging::debug_target(
            "ai.document.study_guide",
            "study_guide_table_shape_repaired",
            "Repaired ragged guide tables natively; content is not logged.",
            json!({"repairedCells": repaired, "droppedTables": dropped}),
        );
    }
}

pub(super) fn validate_generated_safety(
    section: &StudyGuideSectionContent,
) -> Result<(), CommandError> {
    if section.overview.trim().is_empty() {
        return Err(output_error(
            "A guide requires a source-supported narrative overview.",
        ));
    }
    require_count_limit(
        "section characters",
        generated_section_char_count(section),
        MAX_GENERATED_SECTION_CHARS,
    )?;
    require_char_limit("overview", &section.overview, MAX_OVERVIEW_CHARS)?;
    for (field, values, count, chars) in [
        (
            "keyPoints",
            &section.key_points,
            MAX_KEY_POINTS,
            MAX_KEY_POINT_CHARS,
        ),
        (
            "processSteps",
            &section.process_steps,
            MAX_PROCESS_STEPS,
            MAX_PROCESS_STEP_CHARS,
        ),
        (
            "commonMisconceptions",
            &section.common_misconceptions,
            MAX_MISCONCEPTIONS,
            MAX_MISCONCEPTION_CHARS,
        ),
    ] {
        require_list_limits(field, values, count, chars)?;
    }
    require_count_limit("explanations", section.explanations.len(), MAX_EXPLANATIONS)?;
    for item in &section.explanations {
        require_char_limit(
            "explanations.heading",
            &item.heading,
            MAX_EXPLANATION_HEADING_CHARS,
        )?;
        require_char_limit("explanations.body", &item.body, MAX_EXPLANATION_BODY_CHARS)?;
        require_nonempty(&[&item.heading, &item.body])?;
    }
    require_count_limit(
        "importantTerms",
        section.important_terms.len(),
        MAX_IMPORTANT_TERMS,
    )?;
    for item in &section.important_terms {
        require_char_limit("importantTerms.term", &item.term, MAX_TERM_CHARS)?;
        require_char_limit(
            "importantTerms.explanation",
            &item.explanation,
            MAX_TERM_EXPLANATION_CHARS,
        )?;
        require_nonempty(&[&item.term, &item.explanation])?;
    }
    require_count_limit("comparisons", section.comparisons.len(), MAX_COMPARISONS)?;
    for item in &section.comparisons {
        require_char_limit(
            "comparisons.subjectA",
            &item.subject_a,
            MAX_COMPARISON_SUBJECT_CHARS,
        )?;
        require_char_limit(
            "comparisons.subjectB",
            &item.subject_b,
            MAX_COMPARISON_SUBJECT_CHARS,
        )?;
        require_char_limit(
            "comparisons.relationship",
            &item.relationship,
            MAX_COMPARISON_RELATIONSHIP_CHARS,
        )?;
        require_nonempty(&[&item.subject_a, &item.subject_b, &item.relationship])?;
    }
    require_count_limit("callouts", section.callouts.len(), MAX_CALLOUTS)?;
    for item in &section.callouts {
        require_char_limit("callouts.title", &item.title, MAX_CALLOUT_TITLE_CHARS)?;
        require_char_limit("callouts.body", &item.body, MAX_CALLOUT_BODY_CHARS)?;
        require_nonempty(&[&item.title, &item.body])?;
    }
    require_count_limit("tables", section.tables.len(), MAX_TABLES)?;
    for table in &section.tables {
        if table.columns.len() < 2
            || table.rows.is_empty()
            || table
                .rows
                .iter()
                .any(|row| row.len() != table.columns.len())
        {
            return Err(output_error(
                "A guide table has an invalid rectangular shape.",
            ));
        }
        require_count_limit("tables.columns", table.columns.len(), MAX_TABLE_COLUMNS)?;
        require_count_limit("tables.rows", table.rows.len(), MAX_TABLE_ROWS)?;
        require_char_limit("tables.title", &table.title, MAX_CALLOUT_TITLE_CHARS)?;
        require_char_limit(
            "tables.caption",
            &table.caption,
            MAX_COMPARISON_RELATIONSHIP_CHARS,
        )?;
        require_nonempty(&[&table.title])?;
        for cell in table.columns.iter().chain(table.rows.iter().flatten()) {
            require_char_limit("tables.cell", cell, MAX_TABLE_CELL_CHARS)?;
            require_nonempty(&[cell])?;
        }
    }
    require_count_limit(
        "workedExamples",
        section.worked_examples.len(),
        MAX_WORKED_EXAMPLES,
    )?;
    for item in &section.worked_examples {
        require_char_limit("workedExamples.title", &item.title, MAX_CALLOUT_TITLE_CHARS)?;
        require_char_limit(
            "workedExamples.prompt",
            &item.prompt,
            MAX_EXPLANATION_BODY_CHARS,
        )?;
        require_list_limits(
            "workedExamples.steps",
            &item.steps,
            MAX_WORKED_EXAMPLE_STEPS,
            MAX_PROCESS_STEP_CHARS,
        )?;
        require_char_limit(
            "workedExamples.answer",
            &item.answer,
            MAX_EXPLANATION_BODY_CHARS,
        )?;
        require_char_limit(
            "workedExamples.takeaway",
            &item.takeaway,
            MAX_EXPLANATION_BODY_CHARS,
        )?;
        require_nonempty(&[&item.title, &item.prompt, &item.answer])?;
        if item.steps.is_empty() {
            return Err(output_error("A worked example is incomplete."));
        }
    }
    require_count_limit(
        "retrievalChecks",
        section.retrieval_checks.len(),
        MAX_RETRIEVAL_CHECKS,
    )?;
    for item in &section.retrieval_checks {
        require_char_limit(
            "retrievalChecks.prompt",
            &item.prompt,
            MAX_MISCONCEPTION_CHARS,
        )?;
        require_char_limit(
            "retrievalChecks.answer",
            &item.answer,
            MAX_EXPLANATION_BODY_CHARS,
        )?;
        require_nonempty(&[&item.prompt, &item.answer])?;
    }
    require_count_limit(
        "visualExplanations",
        section.visual_explanations.len(),
        MAX_VISUAL_EXPLANATIONS,
    )?;
    for item in &section.visual_explanations {
        require_char_limit(
            "visualExplanations.title",
            &item.title,
            MAX_VISUAL_TITLE_CHARS,
        )?;
        require_char_limit(
            "visualExplanations.explanation",
            &item.explanation,
            MAX_VISUAL_EXPLANATION_CHARS,
        )?;
        require_nonempty(&[&item.title, &item.explanation])?;
    }
    Ok(())
}

fn require_nonempty(values: &[&str]) -> Result<(), CommandError> {
    if values.iter().any(|value| value.trim().is_empty()) {
        return Err(output_error(
            "A guide contains an empty required field or item.",
        ));
    }
    Ok(())
}

pub(super) fn inventory_like_prose(value: &str) -> bool {
    let clean = value.trim();
    let has_sentence_ending = clean.contains('.') || clean.contains('!') || clean.contains('?');
    let nonempty_lines = clean
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    (nonempty_lines.len() > 1
        && nonempty_lines
            .iter()
            .all(|line| line.starts_with("- ") || line.starts_with("* ") || line.starts_with("• ")))
        || (!has_sentence_ending
            && clean.split_whitespace().count() < 32
            && (clean.contains(" / ")
                || clean.contains(" | ")
                || clean.matches("; ").count() >= 3
                || clean.matches(',').count() >= 4))
}

pub(super) fn validate_generated_writing_style(
    section: &StudyGuideSectionContent,
) -> Result<(), CommandError> {
    if section.overview.split_whitespace().count() < 8 || inventory_like_prose(&section.overview) {
        return Err(CommandError::new(
            "ai_document_output_style",
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
        return Err(CommandError::new(
            "ai_document_output_style",
            "Guide key points must be factual propositions; bare vocabulary belongs in importantTerms.",
        ));
    }
    Ok(())
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
        require_nonempty(&[value])?;
    }
    Ok(())
}

fn require_count_limit(field: &str, count: usize, maximum: usize) -> Result<(), CommandError> {
    if count > maximum {
        return Err(resource_error(field, count, maximum));
    }
    Ok(())
}

fn require_char_limit(field: &str, value: &str, maximum: usize) -> Result<(), CommandError> {
    require_count_limit(field, value.chars().count(), maximum)
}

fn resource_error(field: &str, count: usize, maximum: usize) -> CommandError {
    CommandError::new(
        "ai_document_output_resource",
        format!(
            "The generated study-guide {field} exceeds a defensive resource capacity ({count}; capacity {maximum}). No text was truncated. Preserve complete qualifications in any repair."
        ),
    )
}

pub(super) fn generated_section_char_count(section: &StudyGuideSectionContent) -> usize {
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
                    .chain(value.rows.iter().flatten())
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

/// Bare provider-neutral JSON Schema; capacities mirror native safety checks.
pub(super) fn study_guide_section_schema() -> Value {
    json!({
        "type": "object", "additionalProperties": false,
        "required": ["overview", "keyPoints", "explanations", "importantTerms", "processSteps", "comparisons", "commonMisconceptions", "callouts", "tables", "workedExamples", "retrievalChecks", "visualExplanations"],
        "properties": {
            "overview": {"type": "string", "minLength": 1, "maxLength": MAX_OVERVIEW_CHARS},
            "keyPoints": string_array_schema(MAX_KEY_POINTS, MAX_KEY_POINT_CHARS),
            "explanations": {"type": "array", "maxItems": MAX_EXPLANATIONS, "items": object_schema(&["heading", "body"], json!({
                "heading": {"type": "string", "minLength": 1, "maxLength": MAX_EXPLANATION_HEADING_CHARS},
                "body": {"type": "string", "minLength": 1, "maxLength": MAX_EXPLANATION_BODY_CHARS}
            }))},
            "importantTerms": {"type": "array", "maxItems": MAX_IMPORTANT_TERMS, "items": object_schema(&["term", "explanation"], json!({
                "term": {"type": "string", "minLength": 1, "maxLength": MAX_TERM_CHARS},
                "explanation": {"type": "string", "minLength": 1, "maxLength": MAX_TERM_EXPLANATION_CHARS}
            }))},
            "processSteps": string_array_schema(MAX_PROCESS_STEPS, MAX_PROCESS_STEP_CHARS),
            "comparisons": {"type": "array", "maxItems": MAX_COMPARISONS, "items": object_schema(&["subjectA", "subjectB", "relationship"], json!({
                "subjectA": {"type": "string", "minLength": 1, "maxLength": MAX_COMPARISON_SUBJECT_CHARS},
                "subjectB": {"type": "string", "minLength": 1, "maxLength": MAX_COMPARISON_SUBJECT_CHARS},
                "relationship": {"type": "string", "minLength": 1, "maxLength": MAX_COMPARISON_RELATIONSHIP_CHARS}
            }))},
            "commonMisconceptions": string_array_schema(MAX_MISCONCEPTIONS, MAX_MISCONCEPTION_CHARS),
            "callouts": {"type": "array", "maxItems": MAX_CALLOUTS, "items": object_schema(&["kind", "title", "body"], json!({
                "kind": {"type": "string", "enum": ["key_idea", "warning", "tip", "note"]},
                "title": {"type": "string", "minLength": 1, "maxLength": MAX_CALLOUT_TITLE_CHARS},
                "body": {"type": "string", "minLength": 1, "maxLength": MAX_CALLOUT_BODY_CHARS}
            }))},
            "tables": {"type": "array", "maxItems": MAX_TABLES, "items": object_schema(&["title", "columns", "rows", "caption"], json!({
                "title": {"type": "string", "minLength": 1, "maxLength": MAX_CALLOUT_TITLE_CHARS},
                "columns": {"type": "array", "minItems": 2, "maxItems": MAX_TABLE_COLUMNS, "items": {"type": "string", "minLength": 1, "maxLength": MAX_TABLE_CELL_CHARS}},
                "rows": {"type": "array", "minItems": 1, "maxItems": MAX_TABLE_ROWS, "items": {"type": "array", "minItems": 2, "maxItems": MAX_TABLE_COLUMNS, "items": {"type": "string", "minLength": 1, "maxLength": MAX_TABLE_CELL_CHARS}}},
                "caption": {"type": "string", "maxLength": MAX_COMPARISON_RELATIONSHIP_CHARS}
            }))},
            "workedExamples": {"type": "array", "maxItems": MAX_WORKED_EXAMPLES, "items": object_schema(&["title", "prompt", "steps", "answer", "takeaway"], json!({
                "title": {"type": "string", "minLength": 1, "maxLength": MAX_CALLOUT_TITLE_CHARS},
                "prompt": {"type": "string", "minLength": 1, "maxLength": MAX_EXPLANATION_BODY_CHARS},
                "steps": {"minItems": 1, "type": "array", "maxItems": MAX_WORKED_EXAMPLE_STEPS, "items": {"type": "string", "minLength": 1, "maxLength": MAX_PROCESS_STEP_CHARS}},
                "answer": {"type": "string", "minLength": 1, "maxLength": MAX_EXPLANATION_BODY_CHARS},
                "takeaway": {"type": "string", "maxLength": MAX_EXPLANATION_BODY_CHARS}
            }))},
            "retrievalChecks": {"type": "array", "maxItems": MAX_RETRIEVAL_CHECKS, "items": object_schema(&["prompt", "answer"], json!({
                "prompt": {"type": "string", "minLength": 1, "maxLength": MAX_MISCONCEPTION_CHARS},
                "answer": {"type": "string", "minLength": 1, "maxLength": MAX_EXPLANATION_BODY_CHARS}
            }))},
            "visualExplanations": {"type": "array", "maxItems": MAX_VISUAL_EXPLANATIONS, "items": object_schema(&["title", "explanation"], json!({
                "title": {"type": "string", "minLength": 1, "maxLength": MAX_VISUAL_TITLE_CHARS},
                "explanation": {"type": "string", "minLength": 1, "maxLength": MAX_VISUAL_EXPLANATION_CHARS}
            }))}
        }
    })
}

fn string_array_schema(maximum_items: usize, maximum_length: usize) -> Value {
    json!({"type": "array", "maxItems": maximum_items,
        "items": {"type": "string", "minLength": 1, "maxLength": maximum_length}})
}

fn object_schema(required: &[&str], properties: Value) -> Value {
    json!({"type": "object", "additionalProperties": false, "required": required, "properties": properties})
}
