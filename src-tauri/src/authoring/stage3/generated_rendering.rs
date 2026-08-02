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
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
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
        if deleted_at.is_none() && !desired_keys.contains(&(direction.clone(), ordinal)) {
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

pub(crate) fn render_stored_card_presentation(
    connection: &Connection,
    card_id: &str,
) -> Result<RenderedCardPresentation, CommandError> {
    let stored = connection
        .query_row(
            "SELECT c.direction, c.template_ordinal,
                    n.note_type, n.front_doc, n.back_doc, n.front_text, n.back_text,
                    n.cloze_source, n.cloze_mode
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
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
    if note_type != "cloze" || !matches!(direction.as_str(), "forward" | "cloze") {
        return Err(CommandError::new(
            "invalid_generated_card",
            "The note contains an unsupported generated-card presentation.",
        ));
    }
    validate_cloze_mode(&cloze_mode)?;
    let parsed = parse_cloze(&cloze_source)?;
    let target = if ordinal == 0 {
        None
    } else {
        let group = u32::try_from(ordinal).map_err(|_| invalid_cloze_marker())?;
        if !parsed.groups.contains(&group) {
            return Err(CommandError::new(
                "invalid_generated_card",
                "A fill-in-the-blank card refers to a blank that no longer exists.",
            ));
        }
        Some(group)
    };
    let question = render_cloze_question(&parsed, target);
    let answer = render_cloze_answer(&parsed, target);
    let label = match target {
        Some(group) => format!("Blank {group}"),
        None => "All blanks".to_string(),
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
    let rendered = render_stored_card_presentation(connection, card_id)?;
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
