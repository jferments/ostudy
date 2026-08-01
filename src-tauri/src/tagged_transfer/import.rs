fn read_payload(path: &Path) -> Result<TagPayload, CommandError> {
    let package =
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|error| {
            CommandError::invalid_package(format!("Could not open the deck tags: {error}"))
        })?;
    package
        .execute_batch("PRAGMA trusted_schema = OFF; PRAGMA query_only = ON;")
        .map_err(|error| {
            CommandError::invalid_package(format!("Could not secure the tag reader: {error}"))
        })?;

    if !table_exists(&package, "tags")? || !table_exists(&package, "note_tags")? {
        return Ok(TagPayload::default());
    }

    let tags = {
        let mut statement = package
            .prepare(
                "SELECT source_id, name, normalized_name
                 FROM tags ORDER BY source_id",
            )
            .map_err(CommandError::database)?;
        let values = statement
            .query_map([], |row| {
                Ok(ExportTag {
                    source_id: row.get(0)?,
                    name: row.get(1)?,
                    normalized_name: row.get(2)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        if values.len() > MAX_PACKAGE_TAGS {
            return Err(CommandError::invalid_package(
                "The deck contains too many tags.",
            ));
        }
        values
    };

    let link_count: i64 = package
        .query_row("SELECT COUNT(*) FROM note_tags", [], |row| row.get(0))
        .map_err(CommandError::database)?;
    if !(0..=MAX_PACKAGE_TAG_LINKS).contains(&link_count) {
        return Err(CommandError::invalid_package(
            "The deck contains too many card-tag relationships.",
        ));
    }

    let links = {
        let mut statement = package
            .prepare(
                "SELECT source_note_id, source_tag_id
                 FROM note_tags ORDER BY source_note_id, source_tag_id",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };

    let notes = {
        let mut statement = package
            .prepare(
                "SELECT source_id, front_text, back_text, created_at, updated_at, revision
                 FROM notes ORDER BY created_at, source_id",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([], |row| {
                Ok(SourceNoteSignature {
                    source_id: row.get(0)?,
                    front_text: row.get(1)?,
                    back_text: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    revision: row.get(5)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };

    validate_payload(&tags, &links, &notes)?;
    Ok(TagPayload { tags, links, notes })
}

fn table_exists(connection: &Connection, name: &str) -> Result<bool, CommandError> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1
             )",
            [name],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    Ok(exists == 1)
}

fn validate_payload(
    tags: &[ExportTag],
    links: &[(String, String)],
    notes: &[SourceNoteSignature],
) -> Result<(), CommandError> {
    let note_ids = notes
        .iter()
        .map(|note| note.source_id.as_str())
        .collect::<BTreeSet<_>>();
    let tag_ids = tags
        .iter()
        .map(|tag| tag.source_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut per_note = HashMap::<&str, usize>::new();

    for note in notes {
        validate_entity_id(&note.source_id, "exported note")?;
    }
    for tag in tags {
        validate_entity_id(&tag.source_id, "exported tag")?;
        let normalized = validate_tag_name(&tag.name)?;
        if normalized != tag.normalized_name {
            return Err(CommandError::invalid_package(
                "A deck tag has an invalid normalized name.",
            ));
        }
    }
    for (note_id, tag_id) in links {
        if !note_ids.contains(note_id.as_str()) || !tag_ids.contains(tag_id.as_str()) {
            return Err(CommandError::invalid_package(
                "A deck tag refers to a missing card or tag.",
            ));
        }
        let count = per_note.entry(note_id).or_default();
        *count += 1;
        if *count > MAX_TAGS_PER_NOTE {
            return Err(CommandError::invalid_package(format!(
                "A card contains more than {MAX_TAGS_PER_NOTE} tags."
            )));
        }
    }
    Ok(())
}

fn validate_tag_name(value: &str) -> Result<String, CommandError> {
    let name = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty()
        || name.chars().count() > MAX_TAG_CHARS
        || name.chars().any(char::is_control)
    {
        return Err(CommandError::invalid_package(format!(
            "Deck tags must contain 1 to {MAX_TAG_CHARS} visible characters."
        )));
    }
    Ok(name.to_lowercase())
}

fn apply_imported_tags(
    connection: &mut Connection,
    deck_id: &str,
    payload: TagPayload,
) -> Result<(), CommandError> {
    let source_to_local = match_imported_notes(connection, deck_id, &payload.notes)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let now = unix_time_millis()?;
    let mut tag_map = HashMap::new();

    for tag in payload.tags {
        let existing: Option<String> = transaction
            .query_row(
                "SELECT id FROM tags WHERE normalized_name = ?1",
                [tag.normalized_name.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let local_tag_id = match existing {
            Some(id) => id,
            None => {
                let id = generate_id(&transaction)?;
                transaction
                    .execute(
                        "INSERT INTO tags (
                             id, name, normalized_name, created_at, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?4)",
                        params![id, tag.name, tag.normalized_name, now],
                    )
                    .map_err(CommandError::database)?;
                id
            }
        };
        tag_map.insert(tag.source_id, local_tag_id);
    }

    for (source_note_id, source_tag_id) in payload.links {
        let local_note_id = source_to_local.get(&source_note_id).ok_or_else(|| {
            CommandError::invalid_package("An imported tag could not be matched to its card.")
        })?;
        let local_tag_id = tag_map.get(&source_tag_id).ok_or_else(|| {
            CommandError::invalid_package("An imported card refers to a missing tag.")
        })?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                params![local_note_id, local_tag_id],
            )
            .map_err(CommandError::database)?;
    }
    transaction.commit().map_err(CommandError::database)
}

fn match_imported_notes(
    connection: &Connection,
    deck_id: &str,
    source_notes: &[SourceNoteSignature],
) -> Result<HashMap<String, String>, CommandError> {
    let local_notes = {
        let mut statement = connection
            .prepare(
                "SELECT id, front_text, back_text, created_at, updated_at, revision
                 FROM notes
                 WHERE deck_id = ?1 AND deleted_at IS NULL
                 ORDER BY rowid",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([deck_id], |row| {
                Ok(SourceNoteSignature {
                    source_id: row.get(0)?,
                    front_text: row.get(1)?,
                    back_text: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    revision: row.get(5)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };

    let local_ids = local_notes
        .iter()
        .map(|note| note.source_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut result = HashMap::new();
    let mut used_local = BTreeSet::new();

    // The normal import path preserves source IDs. Resolve those exact matches
    // first, then use stable content signatures only for IDs remapped because a
    // deck was imported more than once.
    for source in source_notes {
        if local_ids.contains(source.source_id.as_str()) {
            result.insert(source.source_id.clone(), source.source_id.clone());
            used_local.insert(source.source_id.clone());
        }
    }

    let mut source_groups = BTreeMap::<NoteSignature, VecDeque<String>>::new();
    for source in source_notes {
        if result.contains_key(&source.source_id) {
            continue;
        }
        source_groups
            .entry(signature(source))
            .or_default()
            .push_back(source.source_id.clone());
    }
    let mut local_groups = BTreeMap::<NoteSignature, VecDeque<String>>::new();
    for local in local_notes {
        if used_local.contains(&local.source_id) {
            continue;
        }
        local_groups
            .entry(signature(&local))
            .or_default()
            .push_back(local.source_id);
    }

    for (signature, mut source_ids) in source_groups {
        let mut local_ids = local_groups.remove(&signature).ok_or_else(|| {
            CommandError::invalid_package("Imported tags could not be matched to the imported cards.")
        })?;
        if source_ids.len() != local_ids.len() {
            return Err(CommandError::invalid_package(
                "Imported tags could not be matched to duplicate cards.",
            ));
        }
        while let (Some(source_id), Some(local_id)) =
            (source_ids.pop_front(), local_ids.pop_front())
        {
            result.insert(source_id, local_id);
        }
    }

    if result.len() != source_notes.len() {
        return Err(CommandError::invalid_package(
            "Imported tags could not be matched to every card.",
        ));
    }
    Ok(result)
}

fn signature(note: &SourceNoteSignature) -> NoteSignature {
    NoteSignature {
        front_text: note.front_text.clone(),
        back_text: note.back_text.clone(),
        created_at: note.created_at,
        updated_at: note.updated_at,
        revision: note.revision,
    }
}
