pub(crate) struct ProjectedPresentationInput<'a> {
    pub direction: &'a str,
    pub template_ordinal: i64,
    pub note_type: &'a str,
    pub front_doc_json: &'a str,
    pub back_doc_json: &'a str,
    pub front_text: &'a str,
    pub back_text: &'a str,
    pub cloze_source: &'a str,
    pub cloze_mode: &'a str,
    pub reps: i64,
    pub presentation_variant: Option<&'a str>,
}

pub(crate) fn projected_presentation_variant(
    note_type: &str,
    cloze_source: &str,
    cloze_mode: &str,
    reps: i64,
) -> Result<String, CommandError> {
    if note_type != "cloze" {
        return Ok(String::new());
    }
    validate_cloze_mode(cloze_mode)?;
    if cloze_mode == "all_together" {
        return Ok("cloze:all".to_string());
    }
    let parsed = parse_cloze(cloze_source)?;
    let index = usize::try_from(reps.max(0)).unwrap_or(0) % parsed.groups.len();
    Ok(format!("cloze:{}", parsed.groups[index]))
}

/// Render a generated-card presentation entirely from an already loaded SQL
/// row. Bounded list and practice-window projections use this function so card
/// count affects CPU work, not SQL statement count.
pub(crate) fn render_projected_presentation(
    input: ProjectedPresentationInput<'_>,
) -> Result<RenderedCardPresentation, CommandError> {
    if input.note_type == "basic" {
        let front_doc = serde_json::from_str(input.front_doc_json)
            .map_err(CommandError::serialization)?;
        let back_doc = serde_json::from_str(input.back_doc_json)
            .map_err(CommandError::serialization)?;
        return match input.direction {
            "forward" => Ok(RenderedCardPresentation {
                card_type: "basic".to_string(),
                card_label: "Basic".to_string(),
                front_doc,
                back_doc,
                front_text: input.front_text.to_string(),
                back_text: input.back_text.to_string(),
            }),
            "reverse" => Ok(RenderedCardPresentation {
                card_type: "basic".to_string(),
                card_label: "Reverse".to_string(),
                front_doc: back_doc,
                back_doc: front_doc,
                front_text: input.back_text.to_string(),
                back_text: input.front_text.to_string(),
            }),
            _ => Err(CommandError::new(
                "invalid_generated_card",
                "A Basic note contains an unsupported generated-card direction.",
            )),
        };
    }

    if input.note_type != "cloze"
        || input.direction != "forward"
        || input.template_ordinal != 0
    {
        return Err(CommandError::new(
            "invalid_generated_card",
            "The Fill in the blank note contains an unsupported active card identity.",
        ));
    }
    validate_cloze_mode(input.cloze_mode)?;
    let parsed = parse_cloze(input.cloze_source)?;
    let target = cloze_target_from_variant(
        &parsed,
        input.cloze_mode,
        input.presentation_variant,
        input.reps,
    )?;
    let question = render_cloze_question(&parsed, target);
    let answer = render_cloze_answer(&parsed, target);
    let label = if input.cloze_mode == "all_together" {
        "Fill in the blank · all blanks".to_string()
    } else {
        format!(
            "Fill in the blank · {} alternating {}",
            parsed.groups.len(),
            if parsed.groups.len() == 1 {
                "blank"
            } else {
                "blanks"
            }
        )
    };
    Ok(RenderedCardPresentation {
        card_type: "cloze".to_string(),
        card_label: label,
        front_doc: plain_text_document(&question),
        back_doc: plain_text_document(&answer),
        front_text: question,
        back_text: answer,
    })
}
