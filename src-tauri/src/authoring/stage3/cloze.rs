fn validate_cloze_mode(value: &str) -> Result<(), CommandError> {
    if matches!(value, "one_at_a_time" | "all_together") {
        Ok(())
    } else {
        Err(CommandError::new(
            "invalid_cloze_mode",
            "Choose whether to hide one blank at a time or all blanks together.",
        ))
    }
}

fn parse_cloze(value: &str) -> Result<ParsedCloze, CommandError> {
    if value.trim().is_empty() || value.chars().count() > MAX_CLOZE_SOURCE_CHARS {
        return Err(CommandError::new(
            "invalid_cloze_text",
            format!(
                "Fill-in-the-blank text must contain 1 to {MAX_CLOZE_SOURCE_CHARS} characters."
            ),
        ));
    }
    let mut segments = Vec::new();
    let mut groups = BTreeSet::new();
    let mut answers_by_group = BTreeMap::<u32, Vec<String>>::new();
    let mut full_text = String::new();
    let mut remainder = value;

    while let Some(open) = remainder.find("{{c") {
        let before = &remainder[..open];
        if before.contains("}}") {
            return Err(invalid_cloze_marker());
        }
        if !before.is_empty() {
            segments.push(ClozeSegment::Text(before.to_string()));
            full_text.push_str(before);
        }
        let marker = &remainder[open + 3..];
        let separator = marker.find("::").ok_or_else(invalid_cloze_marker)?;
        let group_text = &marker[..separator];
        if group_text.is_empty()
            || !group_text
                .chars()
                .all(|character| character.is_ascii_digit())
        {
            return Err(invalid_cloze_marker());
        }
        let group: u32 = group_text.parse().map_err(|_| invalid_cloze_marker())?;
        if group == 0 || group > MAX_CLOZE_GROUP {
            return Err(invalid_cloze_marker());
        }
        let after_separator = &marker[separator + 2..];
        let close = after_separator.find("}}").ok_or_else(invalid_cloze_marker)?;
        let raw_blank = &after_separator[..close];
        let blank = raw_blank.trim();
        if blank.is_empty() || raw_blank.contains("{{") || raw_blank.contains("}}") {
            return Err(invalid_cloze_marker());
        }
        groups.insert(group);
        answers_by_group
            .entry(group)
            .or_default()
            .push(blank.to_string());
        segments.push(ClozeSegment::Blank {
            group,
            text: blank.to_string(),
        });
        full_text.push_str(blank);
        remainder = &after_separator[close + 2..];
    }

    if remainder.contains("{{") || remainder.contains("}}") {
        return Err(invalid_cloze_marker());
    }
    if !remainder.is_empty() {
        segments.push(ClozeSegment::Text(remainder.to_string()));
        full_text.push_str(remainder);
    }
    if groups.is_empty() {
        return Err(CommandError::new(
            "missing_cloze_blank",
            "Mark at least one word or phrase as a blank.",
        ));
    }
    Ok(ParsedCloze {
        segments,
        groups: groups.into_iter().collect(),
        answers_by_group,
        full_text,
    })
}

fn invalid_cloze_marker() -> CommandError {
    CommandError::new(
        "invalid_cloze_marker",
        "A fill-in-the-blank marker is incomplete or nested. Select text and use Mark as blank instead of typing marker braces manually.",
    )
}

fn render_cloze_question(parsed: &ParsedCloze, target: Option<u32>) -> String {
    parsed
        .segments
        .iter()
        .map(|segment| match segment {
            ClozeSegment::Text(text) => text.clone(),
            ClozeSegment::Blank { group, text } => {
                if target.is_none() || target == Some(*group) {
                    "[…]".to_string()
                } else {
                    text.clone()
                }
            }
        })
        .collect()
}

fn render_cloze_answer(parsed: &ParsedCloze, target: Option<u32>) -> String {
    let answers = match target {
        Some(group) => parsed
            .answers_by_group
            .get(&group)
            .cloned()
            .unwrap_or_default(),
        None => parsed
            .groups
            .iter()
            .flat_map(|group| {
                parsed
                    .answers_by_group
                    .get(group)
                    .cloned()
                    .unwrap_or_default()
            })
            .collect(),
    };
    format!(
        "{}\n\nComplete statement: {}",
        answers.join(" · "),
        parsed.full_text
    )
}
