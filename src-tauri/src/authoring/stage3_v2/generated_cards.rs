struct GeneratedNoteDraft {
    card_type: String,
    reverse_enabled: bool,
    cloze_source: String,
    cloze_mode: String,
    front_doc: Value,
    back_doc: Value,
    front_text: String,
    back_text: String,
}

fn create_generated_card_in_database(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: CreateGeneratedCardInput,
) -> Result<OrganizedCardDto, CommandError> {
    let CreateGeneratedCardInput {
        deck_id,
        card_type,
        reverse_enabled,
        cloze_source,
        cloze_mode,
        front_doc,
        back_doc,
        front_text,
        back_text,
        tags,
        allow_duplicate,
        asset_draft_id,
    } = input;
    validate_entity_id(&deck_id, "deck")?;
    if let Some(draft_id) = asset_draft_id.as_deref() {
        validate_entity_id(draft_id, "asset draft")?;
    }
    let prepared = prepare_generated_note(GeneratedNoteDraft {
        card_type,
        reverse_enabled,
        cloze_source,
        cloze_mode,
        front_doc,
        back_doc,
        front_text,
        back_text,
    })?;
    let asset_ids = content_asset_ids(&prepared.content);
    let tags = validate_tags(tags)?;
    let normalized = normalized_pair(&prepared.content.front_text, &prepared.content.back_text);
    let hash = duplicate_hash(&normalized.0, &normalized.1);

    let note_id = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &deck_id)?;
        ensure_assets_available(
            &transaction,
            &paths,
            &asset_ids,
            asset_draft_id.as_deref(),
        )?;
        if !allow_duplicate
            && duplicate_checkable(&prepared.content.front_text, &prepared.content.back_text)
            && has_exact_duplicate(
                &transaction,
                &deck_id,
                &hash,
                &normalized.0,
                &normalized.1,
                None,
            )?
        {
            return Err(duplicate_error());
        }

        let now = unix_time_millis()?;
        let note_id = generate_id(&transaction)?;
        transaction
            .execute(
                "INSERT INTO notes (
                     id, deck_id, note_type, content_format, content_schema_version,
                     front_doc, back_doc, front_text, back_text, duplicate_hash,
                     reverse_enabled, cloze_source, cloze_mode,
                     created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, ?3, 'tiptap-json', 1,
                     ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, 1
                 )",
                params![
                    note_id,
                    deck_id,
                    prepared.note_type,
                    prepared.content.front_doc_json,
                    prepared.content.back_doc_json,
                    prepared.content.front_text,
                    prepared.content.back_text,
                    hash,
                    crate::util::bool_to_integer(prepared.reverse_enabled),
                    prepared.cloze_source,
                    prepared.cloze_mode,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        sync_generated_cards(
            &transaction,
            &note_id,
            &deck_id,
            &prepared.cards,
            now,
        )?;
        replace_note_assets(&transaction, &note_id, &prepared.content)?;
        replace_note_tags(&transaction, &note_id, &tags, now)?;
        let finalization = finalize_assets_for_owner(
            &transaction,
            &paths,
            &asset_ids,
            asset_draft_id.as_deref(),
            Some(&note_id),
        )?;
        transaction.commit().map_err(CommandError::database)?;
        finalization.cleanup();
        Ok(note_id)
    })?;

    database.with_connection(|connection| hydrate_organized_card(connection, &paths, &note_id))
}

fn update_generated_card_in_database(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: UpdateGeneratedCardInput,
) -> Result<OrganizedCardDto, CommandError> {
    let UpdateGeneratedCardInput {
        note_id,
        deck_id,
        revision,
        card_type,
        reverse_enabled,
        cloze_source,
        cloze_mode,
        front_doc,
        back_doc,
        front_text,
        back_text,
        tags,
        allow_duplicate,
        asset_draft_id,
    } = input;
    validate_entity_id(&note_id, "note")?;
    validate_entity_id(&deck_id, "deck")?;
    if let Some(draft_id) = asset_draft_id.as_deref() {
        validate_entity_id(draft_id, "asset draft")?;
    }
    if revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The card revision is invalid.",
        ));
    }
    let prepared = prepare_generated_note(GeneratedNoteDraft {
        card_type,
        reverse_enabled,
        cloze_source,
        cloze_mode,
        front_doc,
        back_doc,
        front_text,
        back_text,
    })?;
    let asset_ids = content_asset_ids(&prepared.content);
    let tags = validate_tags(tags)?;
    let normalized = normalized_pair(&prepared.content.front_text, &prepared.content.back_text);
    let hash = duplicate_hash(&normalized.0, &normalized.1);

    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &deck_id)?;
        ensure_assets_available(
            &transaction,
            &paths,
            &asset_ids,
            asset_draft_id.as_deref(),
        )?;
        let existing_type: Option<String> = transaction
            .query_row(
                "SELECT note_type FROM notes WHERE id = ?1 AND deleted_at IS NULL",
                [note_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some(existing_type) = existing_type else {
            return Err(CommandError::new(
                "card_not_found",
                "That flashcard no longer exists.",
            ));
        };
        if display_card_type(&existing_type) != display_card_type(prepared.note_type) {
            return Err(CommandError::new(
                "card_type_locked",
                "A saved card cannot be converted between Basic and Fill in the blank. Create a new card instead.",
            ));
        }
        if !allow_duplicate
            && duplicate_checkable(&prepared.content.front_text, &prepared.content.back_text)
            && has_exact_duplicate(
                &transaction,
                &deck_id,
                &hash,
                &normalized.0,
                &normalized.1,
                Some(&note_id),
            )?
        {
            return Err(duplicate_error());
        }

        let now = unix_time_millis()?;
        let changed = transaction
            .execute(
                "UPDATE notes
                 SET deck_id = ?1, note_type = ?2,
                     front_doc = ?3, back_doc = ?4,
                     front_text = ?5, back_text = ?6, duplicate_hash = ?7,
                     reverse_enabled = ?8, cloze_source = ?9, cloze_mode = ?10,
                     updated_at = ?11, revision = revision + 1
                 WHERE id = ?12 AND revision = ?13 AND deleted_at IS NULL",
                params![
                    deck_id,
                    prepared.note_type,
                    prepared.content.front_doc_json,
                    prepared.content.back_doc_json,
                    prepared.content.front_text,
                    prepared.content.back_text,
                    hash,
                    crate::util::bool_to_integer(prepared.reverse_enabled),
                    prepared.cloze_source,
                    prepared.cloze_mode,
                    now,
                    note_id,
                    revision,
                ],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "card_changed",
                "This card changed in another window. Reload it and try again.",
            ));
        }
        sync_generated_cards(
            &transaction,
            &note_id,
            &deck_id,
            &prepared.cards,
            now,
        )?;
        replace_note_assets(&transaction, &note_id, &prepared.content)?;
        replace_note_tags(&transaction, &note_id, &tags, now)?;
        let finalization = finalize_assets_for_owner(
            &transaction,
            &paths,
            &asset_ids,
            asset_draft_id.as_deref(),
            Some(&note_id),
        )?;
        transaction.commit().map_err(CommandError::database)?;
        finalization.cleanup();
        Ok(())
    })?;

    database.with_connection(|connection| hydrate_organized_card(connection, &paths, &note_id))
}

fn prepare_generated_note(
    input: GeneratedNoteDraft,
) -> Result<PreparedGeneratedNote, CommandError> {
    let GeneratedNoteDraft {
        card_type,
        reverse_enabled,
        cloze_source,
        cloze_mode,
        front_doc,
        back_doc,
        front_text,
        back_text,
    } = input;
    match card_type.as_str() {
        "basic" => {
            let content = validate_content(front_doc, back_doc, front_text, back_text)?;
            let mut cards = vec![GeneratedCardSpec {
                direction: "forward",
                template_ordinal: 0,
            }];
            if reverse_enabled {
                cards.push(GeneratedCardSpec {
                    direction: "reverse",
                    template_ordinal: 0,
                });
            }
            Ok(PreparedGeneratedNote {
                note_type: "basic",
                reverse_enabled,
                cloze_source: String::new(),
                cloze_mode: "one_at_a_time".to_string(),
                content,
                cards,
            })
        }
        "cloze" => {
            validate_cloze_mode(&cloze_mode)?;
            let parsed = parse_cloze(&cloze_source)?;
            let target = if cloze_mode == "all_together" {
                None
            } else {
                parsed.groups.first().copied()
            };
            let question = render_cloze_question(&parsed, target);
            let answer = render_cloze_answer(&parsed, target);
            let content = validate_content(
                plain_text_document(&question),
                plain_text_document(&answer),
                question,
                answer,
            )?;
            Ok(PreparedGeneratedNote {
                note_type: "cloze",
                reverse_enabled: false,
                cloze_source,
                cloze_mode,
                content,
                cards: vec![GeneratedCardSpec {
                    direction: "forward",
                    template_ordinal: 0,
                }],
            })
        }
        _ => Err(CommandError::new(
            "invalid_card_type",
            "Choose Basic or Fill in the blank as the card type.",
        )),
    }
}

fn sync_generated_cards(
    connection: &Connection,
    note_id: &str,
    deck_id: &str,
    desired: &[GeneratedCardSpec],
    now: i64,
) -> Result<(), CommandError> {
    if desired.is_empty() {
        return Err(CommandError::new(
            "missing_generated_card",
            "A note must generate at least one study card.",
        ));
    }
    let existing = {
        let mut statement = connection
            .prepare(
                "SELECT id, direction, template_ordinal, deleted_at
                 FROM cards WHERE note_id = ?1
                 ORDER BY direction, template_ordinal, created_at",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([note_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            })
            .map_err(CommandError::database)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    let mut by_key = BTreeMap::new();
    for (card_id, direction, ordinal, deleted_at) in &existing {
        by_key.insert(
            (direction.clone(), *ordinal),
            (card_id.clone(), *deleted_at),
        );
    }
    let desired_keys = desired
        .iter()
        .map(|spec| (spec.direction.to_string(), spec.template_ordinal))
        .collect::<BTreeSet<_>>();

    for spec in desired {
        if let Some((card_id, _)) =
            by_key.get(&(spec.direction.to_string(), spec.template_ordinal))
        {
            connection
                .execute(
                    "UPDATE cards
                     SET deck_id = ?1, deleted_at = NULL, updated_at = ?2
                     WHERE id = ?3",
                    params![deck_id, now, card_id],
                )
                .map_err(CommandError::database)?;
            let schedule_exists: i64 = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM card_schedules WHERE card_id = ?1)",
                    [card_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            if schedule_exists == 0 {
                insert_new_schedule(connection, card_id, now)?;
            }
        } else {
            let card_id = generate_id(connection)?;
            connection
                .execute(
                    "INSERT INTO cards (
                         id, note_id, deck_id, direction, template_ordinal,
                         created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                    params![
                        card_id,
                        note_id,
                        deck_id,
                        spec.direction,
                        spec.template_ordinal,
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
            insert_new_schedule(connection, &card_id, now)?;
        }
    }

    for (card_id, direction, ordinal, deleted_at) in existing {
        if deleted_at.is_none() && !desired_keys.contains(&(direction, ordinal)) {
            connection
                .execute(
                    "UPDATE cards SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
                    params![now, card_id],
                )
                .map_err(CommandError::database)?;
        }
    }
    Ok(())
}

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

pub(crate) fn presentation_variant_for_card(
    connection: &Connection,
    card_id: &str,
) -> Result<String, CommandError> {
    let stored = connection
        .query_row(
            "SELECT n.note_type, n.cloze_source, n.cloze_mode, s.reps
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN card_schedules s ON s.card_id = c.id
             WHERE c.id = ?1 AND c.deleted_at IS NULL AND n.deleted_at IS NULL",
            [card_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "practice_card_not_found",
                "That generated card is no longer available.",
            )
        })?;
    if stored.0 != "cloze" {
        return Ok(String::new());
    }
    validate_cloze_mode(&stored.2)?;
    if stored.2 == "all_together" {
        return Ok("cloze:all".to_string());
    }
    let parsed = parse_cloze(&stored.1)?;
    let index = usize::try_from(stored.3.max(0)).unwrap_or(0) % parsed.groups.len();
    Ok(format!("cloze:{}", parsed.groups[index]))
}

fn cloze_target_from_variant(
    parsed: &ParsedCloze,
    mode: &str,
    variant: Option<&str>,
    reps: i64,
) -> Result<Option<u32>, CommandError> {
    if mode == "all_together" {
        return Ok(None);
    }
    if let Some(variant) = variant {
        if variant == "cloze:all" {
            return Err(CommandError::new(
                "invalid_cloze_variant",
                "This Fill in the blank card is not configured to hide every blank together.",
            ));
        }
        let group = variant
            .strip_prefix("cloze:")
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|group| parsed.groups.contains(group))
            .ok_or_else(|| {
                CommandError::new(
                    "invalid_cloze_variant",
                    "The selected Fill in the blank presentation no longer exists.",
                )
            })?;
        return Ok(Some(group));
    }
    let index = usize::try_from(reps.max(0)).unwrap_or(0) % parsed.groups.len();
    Ok(Some(parsed.groups[index]))
}

pub(crate) fn render_projected_presentation(
    input: ProjectedPresentationInput<'_>,
) -> Result<RenderedCardPresentation, CommandError> {
    let ProjectedPresentationInput {
        direction,
        template_ordinal,
        note_type,
        front_doc_json,
        back_doc_json,
        front_text,
        back_text,
        cloze_source,
        cloze_mode,
        reps,
        presentation_variant,
    } = input;
    if note_type == "basic" {
        let front_doc =
            serde_json::from_str(&front_doc_json).map_err(CommandError::serialization)?;
        let back_doc =
            serde_json::from_str(&back_doc_json).map_err(CommandError::serialization)?;
        return match direction.as_str() {
            "forward" => Ok(RenderedCardPresentation {
                card_type: "basic".to_string(),
                card_label: "Basic".to_string(),
                front_doc,
                back_doc,
                front_text,
                back_text,
            }),
            "reverse" => Ok(RenderedCardPresentation {
                card_type: "basic".to_string(),
                card_label: "Reverse".to_string(),
                front_doc: back_doc,
                back_doc: front_doc,
                front_text: back_text,
                back_text: front_text,
            }),
            _ => Err(CommandError::new(
                "invalid_generated_card",
                "A Basic note contains an unsupported generated-card direction.",
            )),
        };
    }
    if note_type != "cloze" || direction != "forward" || template_ordinal != 0 {
        return Err(CommandError::new(
            "invalid_generated_card",
            "The Fill in the blank note contains an unsupported active card identity.",
        ));
    }
    validate_cloze_mode(&cloze_mode)?;
    let parsed = parse_cloze(&cloze_source)?;
    let target = cloze_target_from_variant(&parsed, &cloze_mode, presentation_variant, reps)?;
    let question = render_cloze_question(&parsed, target);
    let answer = render_cloze_answer(&parsed, target);
    let label = if cloze_mode == "all_together" {
        "Fill in the blank · all blanks".to_string()
    } else {
        format!(
            "Fill in the blank · {} alternating {}",
            parsed.groups.len(),
            if parsed.groups.len() == 1 { "blank" } else { "blanks" }
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

pub(crate) fn render_stored_card_presentation(
    connection: &Connection,
    card_id: &str,
) -> Result<RenderedCardPresentation, CommandError> {
    render_stored_card_presentation_for_variant(connection, card_id, None)
}

pub(crate) fn render_stored_card_presentation_for_variant(
    connection: &Connection,
    card_id: &str,
    presentation_variant: Option<&str>,
) -> Result<RenderedCardPresentation, CommandError> {
    let stored = connection
        .query_row(
            "SELECT c.direction, c.template_ordinal,
                    n.note_type, n.front_doc, n.back_doc, n.front_text, n.back_text,
                    n.cloze_source, n.cloze_mode, s.reps
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             WHERE c.id = ?1
               AND c.deleted_at IS NULL
               AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL
               AND d.archived_at IS NULL",
            [card_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "practice_card_not_found",
                "That generated card is no longer available.",
            )
        })?;

    let (
        direction,
        ordinal,
        note_type,
        front_json,
        back_json,
        front_text,
        back_text,
        cloze_source,
        cloze_mode,
        reps,
    ) = stored;
    if note_type == "basic" {
        let front_doc =
            serde_json::from_str(&front_json).map_err(CommandError::serialization)?;
        let back_doc = serde_json::from_str(&back_json).map_err(CommandError::serialization)?;
        return match direction.as_str() {
            "forward" => Ok(RenderedCardPresentation {
                card_type: "basic".to_string(),
                card_label: "Basic".to_string(),
                front_doc,
                back_doc,
                front_text,
                back_text,
            }),
            "reverse" => Ok(RenderedCardPresentation {
                card_type: "basic".to_string(),
                card_label: "Reverse".to_string(),
                front_doc: back_doc,
                back_doc: front_doc,
                front_text: back_text,
                back_text: front_text,
            }),
            _ => Err(CommandError::new(
                "invalid_generated_card",
                "A Basic note contains an unsupported generated-card direction.",
            )),
        };
    }
    if note_type != "cloze" || direction != "forward" || ordinal != 0 {
        return Err(CommandError::new(
            "invalid_generated_card",
            "The Fill in the blank note contains an unsupported active card identity.",
        ));
    }
    validate_cloze_mode(&cloze_mode)?;
    let parsed = parse_cloze(&cloze_source)?;
    let target = cloze_target_from_variant(&parsed, &cloze_mode, presentation_variant, reps)?;
    let question = render_cloze_question(&parsed, target);
    let answer = render_cloze_answer(&parsed, target);
    let label = if cloze_mode == "all_together" {
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

pub(crate) fn get_generated_card_for_practice(
    connection: &Connection,
    paths: &AppPaths,
    card_id: &str,
    presentation_variant: &str,
) -> Result<CardDto, CommandError> {
    let raw = connection
        .query_row(
            "SELECT c.id, c.note_id, c.deck_id, c.direction,
                    n.created_at, n.updated_at, n.revision
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             WHERE c.id = ?1
               AND c.deleted_at IS NULL
               AND c.suspended_at IS NULL
               AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL
               AND d.archived_at IS NULL",
            [card_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "practice_card_not_found",
                "That generated card is no longer available.",
            )
        })?;
    let variant = (!presentation_variant.is_empty()).then_some(presentation_variant);
    let rendered = render_stored_card_presentation_for_variant(connection, card_id, variant)?;
    let assets = crate::assets::get_assets_for_note(connection, paths, &raw.1)?;
    Ok(CardDto {
        id: raw.0,
        note_id: raw.1,
        deck_id: raw.2,
        direction: raw.3,
        front_doc: rendered.front_doc,
        back_doc: rendered.back_doc,
        front_text: rendered.front_text,
        back_text: rendered.back_text,
        assets,
        created_at: raw.4,
        updated_at: raw.5,
        revision: raw.6,
    })
}
