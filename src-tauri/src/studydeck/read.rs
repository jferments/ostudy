use super::*;

pub(super) fn read_package(source: &Path) -> Result<PackageData, CommandError> {
    let package =
        Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|error| {
            CommandError::invalid_package(format!("Could not open the deck: {error}"))
        })?;
    package
        .execute_batch(
            "PRAGMA trusted_schema = OFF;
             PRAGMA foreign_keys = ON;
             PRAGMA query_only = ON;",
        )
        .map_err(|error| {
            CommandError::invalid_package(format!("Could not secure the deck reader: {error}"))
        })?;
    let integrity: String = package
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| {
            CommandError::invalid_package(format!("Integrity check failed: {error}"))
        })?;
    if integrity != "ok" {
        return Err(CommandError::invalid_package(
            "The selected deck file is damaged.",
        ));
    }

    let format = metadata_value(&package, "format")?;
    let version = metadata_value(&package, "formatVersion")?
        .parse::<i64>()
        .map_err(|_| CommandError::invalid_package("The deck has an invalid package version."))?;
    if format != PACKAGE_FORMAT
        || !matches!(version, LEGACY_PACKAGE_VERSION | CURRENT_PACKAGE_VERSION)
    {
        return Err(CommandError::invalid_package(
            "This file is not a supported ostudy deck package.",
        ));
    }

    let deck = read_deck(&package, version)?;
    let notes = read_notes(&package, version, &deck)?;
    let cards = read_cards(&package)?;
    let tags = read_tags(&package, version, &deck)?;
    let note_tags = read_note_tags(&package)?;
    let assets = read_assets(&package)?;
    let note_assets = read_note_assets(&package)?;
    let mut data = PackageData {
        version,
        deck,
        notes,
        cards,
        tags,
        note_tags,
        assets,
        note_assets,
    };
    validate_package_data(&data, version == CURRENT_PACKAGE_VERSION)?;
    canonicalize_legacy_presentations(&mut data)?;
    validate_package_data(&data, true)?;
    Ok(data)
}

pub(super) fn validate_package_data(
    data: &PackageData,
    strict_current: bool,
) -> Result<(), CommandError> {
    validate_entity_id(&data.deck.source_id, "exported deck")?;
    validate_name(data.deck.name.clone())
        .map_err(|error| CommandError::invalid_package(error.message))?;
    validate_description(data.deck.description.clone())
        .map_err(|error| CommandError::invalid_package(error.message))?;
    validate_timestamp_pair(data.deck.created_at, data.deck.updated_at, "deck")?;
    if data.deck.revision < 1 || data.deck.sort_order < 0 {
        return Err(CommandError::invalid_package(
            "The deck has invalid ordering or revision metadata.",
        ));
    }
    if data.notes.is_empty() || data.notes.len() > MAX_PACKAGE_NOTES {
        return Err(CommandError::invalid_package(
            "The deck contains an invalid number of source notes.",
        ));
    }
    if data.cards.is_empty() || data.cards.len() > MAX_PACKAGE_CARDS {
        return Err(CommandError::invalid_package(
            "The deck contains an invalid number of generated cards.",
        ));
    }
    if data.tags.len() > MAX_PACKAGE_TAGS {
        return Err(CommandError::invalid_package(
            "The deck contains too many tags.",
        ));
    }

    let note_ids = data
        .notes
        .iter()
        .map(|note| note.source_id.as_str())
        .collect::<BTreeSet<_>>();
    if note_ids.len() != data.notes.len() {
        return Err(CommandError::invalid_package(
            "The deck contains duplicate source-note identifiers.",
        ));
    }
    let asset_ids = data
        .assets
        .iter()
        .map(|asset| asset.source_id.as_str())
        .collect::<BTreeSet<_>>();
    if asset_ids.len() != data.assets.len() {
        return Err(CommandError::invalid_package(
            "The deck contains duplicate image identifiers.",
        ));
    }
    let tag_ids = data
        .tags
        .iter()
        .map(|tag| tag.source_id.as_str())
        .collect::<BTreeSet<_>>();
    if tag_ids.len() != data.tags.len() {
        return Err(CommandError::invalid_package(
            "The deck contains duplicate tag identifiers.",
        ));
    }

    let mut cards_by_note = BTreeMap::<&str, Vec<&PackageCard>>::new();
    let mut card_ids = BTreeSet::new();
    for card in &data.cards {
        validate_entity_id(&card.source_id, "exported card")?;
        validate_entity_id(&card.source_note_id, "exported note")?;
        validate_timestamp_pair(card.created_at, card.updated_at, "generated card")?;
        if !card_ids.insert(card.source_id.as_str())
            || !note_ids.contains(card.source_note_id.as_str())
            || card.template_ordinal < 0
        {
            return Err(CommandError::invalid_package(
                "The deck contains an invalid generated-card record.",
            ));
        }
        cards_by_note
            .entry(card.source_note_id.as_str())
            .or_default()
            .push(card);
    }

    for note in &data.notes {
        validate_entity_id(&note.source_id, "exported note")?;
        if note.source_deck_id != data.deck.source_id
            || note.content_format != "tiptap-json"
            || note.content_schema_version != 1
            || !matches!(note.note_type.as_str(), "basic" | "cloze")
            || note.revision < 1
            || note.cloze_source.chars().count() > MAX_CLOZE_SOURCE_CHARS
            || !matches!(note.cloze_mode.as_str(), "one_at_a_time" | "all_together")
        {
            return Err(CommandError::invalid_package(
                "The deck contains unsupported note metadata.",
            ));
        }
        validate_timestamp_pair(note.created_at, note.updated_at, "source note")?;
        let cards = cards_by_note.get(note.source_id.as_str()).ok_or_else(|| {
            CommandError::invalid_package("A source note has no generated cards.")
        })?;
        let keys = cards
            .iter()
            .map(|card| (card.direction.as_str(), card.template_ordinal))
            .collect::<BTreeSet<_>>();
        if keys.len() != cards.len() {
            return Err(CommandError::invalid_package(
                "A source note contains duplicate generated-card identities.",
            ));
        }
        match note.note_type.as_str() {
            "basic" => {
                let forward = cards
                    .iter()
                    .filter(|card| card.direction == "forward" && card.template_ordinal == 0)
                    .count();
                let reverse = cards
                    .iter()
                    .filter(|card| card.direction == "reverse" && card.template_ordinal == 0)
                    .count();
                if forward != 1
                    || reverse > 1
                    || cards.iter().any(|card| {
                        !matches!(card.direction.as_str(), "forward" | "reverse")
                            || card.template_ordinal != 0
                    })
                    || !note.cloze_source.is_empty()
                    || (strict_current && note.reverse_enabled != (reverse == 1))
                {
                    return Err(CommandError::invalid_package(
                        "A Basic note contains inconsistent generated-card metadata.",
                    ));
                }
            }
            "cloze" => {
                let canonical = cards.len() == 1
                    && cards[0].direction == "forward"
                    && cards[0].template_ordinal == 0;
                let legacy = !strict_current
                    && cards
                        .iter()
                        .all(|card| matches!(card.direction.as_str(), "cloze" | "forward"));
                if note.reverse_enabled
                    || note.cloze_source.trim().is_empty()
                    || (!canonical && !legacy)
                {
                    return Err(CommandError::invalid_package(
                        "A fill-in-the-blank note contains inconsistent metadata.",
                    ));
                }
            }
            _ => unreachable!(),
        }
    }

    let expected_assets = expected_asset_links(&data.notes)?;
    let actual_assets = data.note_assets.iter().cloned().collect::<BTreeSet<_>>();
    if expected_assets != actual_assets {
        return Err(CommandError::invalid_package(
            "Image references do not match the packaged rich documents.",
        ));
    }
    for (note_id, asset_id, side) in &data.note_assets {
        if !note_ids.contains(note_id.as_str())
            || !asset_ids.contains(asset_id.as_str())
            || !matches!(side.as_str(), "front" | "back")
        {
            return Err(CommandError::invalid_package(
                "The deck contains an invalid image relationship.",
            ));
        }
    }

    let mut links_per_note = HashMap::<&str, usize>::new();
    let mut seen_tag_links = BTreeSet::new();
    for (note_id, tag_id) in &data.note_tags {
        if !note_ids.contains(note_id.as_str())
            || !tag_ids.contains(tag_id.as_str())
            || !seen_tag_links.insert((note_id.as_str(), tag_id.as_str()))
        {
            return Err(CommandError::invalid_package(
                "The deck contains an invalid tag relationship.",
            ));
        }
        let count = links_per_note.entry(note_id).or_default();
        *count += 1;
        if *count > MAX_TAGS_PER_NOTE {
            return Err(CommandError::invalid_package(
                "A card in this deck contains more than 32 tags.",
            ));
        }
    }

    let mut normalized_names = BTreeSet::new();
    for tag in &data.tags {
        validate_entity_id(&tag.source_id, "exported tag")?;
        validate_timestamp_pair(tag.created_at, tag.updated_at, "tag")?;
        let canonical = normalize_tag_name(&tag.name);
        if canonical.is_empty()
            || canonical.chars().count() > 64
            || canonical != tag.normalized_name
            || !normalized_names.insert(canonical)
        {
            return Err(CommandError::invalid_package(
                "The deck contains an invalid or duplicate tag.",
            ));
        }
    }

    let mut total_asset_bytes = 0_usize;
    for asset in &data.assets {
        validate_entity_id(&asset.source_id, "exported asset")?;
        validate_original_filename(&asset.original_filename)
            .map_err(|error| CommandError::invalid_package(error.message))?;
        if asset.data.is_empty() || asset.data.len() > MAX_PACKAGE_SINGLE_ASSET_BYTES {
            return Err(CommandError::invalid_package(
                "An image is empty or exceeds the 25 MB per-image limit.",
            ));
        }
        total_asset_bytes = total_asset_bytes
            .checked_add(asset.data.len())
            .ok_or_else(|| CommandError::invalid_package("The deck's images are too large."))?;
        if total_asset_bytes > MAX_PACKAGE_ASSET_BYTES
            || i64::try_from(asset.data.len()).unwrap_or(i64::MAX) != asset.byte_size
        {
            return Err(CommandError::invalid_package(
                "The deck contains invalid or excessive image data.",
            ));
        }
        let (mime, _) = detect_image_bytes(&asset.data)?;
        if mime != asset.mime_type {
            return Err(CommandError::invalid_package(
                "An image does not match its declared file type.",
            ));
        }
    }
    Ok(())
}

fn read_deck(connection: &Connection, version: i64) -> Result<PackageDeck, CommandError> {
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM deck", [], |row| row.get(0))
        .map_err(CommandError::database)?;
    if count != 1 {
        return Err(CommandError::invalid_package(
            "A deck package must contain exactly one deck.",
        ));
    }
    let has_sort_order =
        version >= CURRENT_PACKAGE_VERSION && column_exists(connection, "deck", "sort_order")?;
    let sql = if has_sort_order {
        "SELECT source_id, name, description, sort_order,
                created_at, updated_at, revision FROM deck"
    } else {
        "SELECT source_id, name, description, 0,
                created_at, updated_at, revision FROM deck"
    };
    connection
        .query_row(sql, [], |row| {
            Ok(PackageDeck {
                source_id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                sort_order: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                revision: row.get(6)?,
            })
        })
        .map_err(CommandError::database)
}

fn read_notes(
    connection: &Connection,
    version: i64,
    deck: &PackageDeck,
) -> Result<Vec<PackageNote>, CommandError> {
    let metadata =
        if version == LEGACY_PACKAGE_VERSION && table_exists(connection, "card_type_metadata")? {
            let mut statement = connection
                .prepare(
                    "SELECT source_note_id, reverse_enabled, cloze_source, cloze_mode
                 FROM card_type_metadata",
                )
                .map_err(CommandError::database)?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        (
                            row.get::<_, i64>(1)? == 1,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ),
                    ))
                })
                .map_err(CommandError::database)?
                .collect::<Result<HashMap<_, _>, _>>()
                .map_err(CommandError::database)?;
            rows
        } else {
            HashMap::new()
        };

    let sql = if version == CURRENT_PACKAGE_VERSION {
        "SELECT source_id, source_deck_id, note_type,
                content_format, content_schema_version,
                front_doc, back_doc, front_text, back_text, duplicate_hash,
                reverse_enabled, cloze_source, cloze_mode,
                created_at, updated_at, revision
         FROM notes ORDER BY created_at, source_id"
    } else {
        "SELECT source_id, source_deck_id, note_type,
                content_format, content_schema_version,
                front_doc, back_doc, front_text, back_text, '',
                0, '', 'one_at_a_time',
                created_at, updated_at, revision
         FROM notes ORDER BY created_at, source_id"
    };
    let mut statement = connection.prepare(sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, String>(12)?,
                row.get::<_, i64>(13)?,
                row.get::<_, i64>(14)?,
                row.get::<_, i64>(15)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if rows.len() > MAX_PACKAGE_NOTES {
        return Err(CommandError::invalid_package(
            "This deck contains more than 100,000 source notes.",
        ));
    }
    rows.into_iter()
        .map(|row| {
            let front_doc: Value = serde_json::from_str(&row.5).map_err(|error| {
                CommandError::invalid_package(format!("Invalid question document: {error}"))
            })?;
            let back_doc: Value = serde_json::from_str(&row.6).map_err(|error| {
                CommandError::invalid_package(format!("Invalid answer document: {error}"))
            })?;
            let validated = validate_content(front_doc, back_doc, row.7, row.8)
                .map_err(|error| CommandError::invalid_package(error.message))?;
            let legacy_metadata = metadata.get(&row.0);
            let reverse_enabled = legacy_metadata.map_or(row.10 == 1, |value| value.0);
            let cloze_source = legacy_metadata.map_or(row.11, |value| value.1.clone());
            let cloze_mode = legacy_metadata.map_or(row.12, |value| value.2.clone());
            let duplicate_hash = if row.9.is_empty() {
                duplicate_fingerprint(&validated.front_text, &validated.back_text)
            } else {
                row.9
            };
            Ok(PackageNote {
                source_id: row.0,
                source_deck_id: if row.1.is_empty() {
                    deck.source_id.clone()
                } else {
                    row.1
                },
                note_type: row.2,
                content_format: row.3,
                content_schema_version: row.4,
                front_doc: serde_json::from_str(&validated.front_doc_json)
                    .map_err(CommandError::serialization)?,
                back_doc: serde_json::from_str(&validated.back_doc_json)
                    .map_err(CommandError::serialization)?,
                front_text: validated.front_text,
                back_text: validated.back_text,
                duplicate_hash,
                reverse_enabled,
                cloze_source,
                cloze_mode,
                created_at: row.13,
                updated_at: row.14,
                revision: row.15.max(1),
            })
        })
        .collect()
}

fn read_cards(connection: &Connection) -> Result<Vec<PackageCard>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT source_id, source_note_id, direction, template_ordinal,
                    created_at, updated_at
             FROM cards ORDER BY created_at, source_id",
        )
        .map_err(CommandError::database)?;
    let cards = statement
        .query_map([], |row| {
            Ok(PackageCard {
                source_id: row.get(0)?,
                source_note_id: row.get(1)?,
                direction: row.get(2)?,
                template_ordinal: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if cards.len() > MAX_PACKAGE_CARDS {
        return Err(CommandError::invalid_package(
            "This deck contains more than 200,000 generated cards.",
        ));
    }
    Ok(cards)
}

fn read_tags(
    connection: &Connection,
    version: i64,
    deck: &PackageDeck,
) -> Result<Vec<PackageTag>, CommandError> {
    if !table_exists(connection, "tags")? {
        return Ok(Vec::new());
    }
    let has_timestamps =
        version == CURRENT_PACKAGE_VERSION && column_exists(connection, "tags", "created_at")?;
    let sql = if has_timestamps {
        "SELECT source_id, name, normalized_name, created_at, updated_at
         FROM tags ORDER BY normalized_name, source_id"
    } else {
        "SELECT source_id, name, normalized_name, ?1, ?1
         FROM tags ORDER BY normalized_name, source_id"
    };
    let mut statement = connection.prepare(sql).map_err(CommandError::database)?;
    let values = if has_timestamps {
        let rows = statement
            .query_map([], |row| {
                Ok(PackageTag {
                    source_id: row.get(0)?,
                    name: row.get(1)?,
                    normalized_name: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    } else {
        let rows = statement
            .query_map([deck.created_at], |row| {
                Ok(PackageTag {
                    source_id: row.get(0)?,
                    name: row.get(1)?,
                    normalized_name: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };
    Ok(values)
}

fn read_note_tags(connection: &Connection) -> Result<Vec<(String, String)>, CommandError> {
    if !table_exists(connection, "note_tags")? {
        return Ok(Vec::new());
    }
    let mut statement = connection
        .prepare(
            "SELECT source_note_id, source_tag_id
             FROM note_tags ORDER BY source_note_id, source_tag_id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database);
    rows
}

fn read_assets(connection: &Connection) -> Result<Vec<PackageAsset>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT source_id, original_filename, mime_type, byte_size, data
             FROM assets ORDER BY source_id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| {
            Ok(PackageAsset {
                source_id: row.get(0)?,
                original_filename: row.get(1)?,
                mime_type: row.get(2)?,
                byte_size: row.get(3)?,
                data: row.get(4)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database);
    rows
}

fn read_note_assets(
    connection: &Connection,
) -> Result<Vec<(String, String, String)>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT source_note_id, source_asset_id, side
             FROM note_assets ORDER BY source_note_id, source_asset_id, side",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database);
    rows
}

fn canonicalize_legacy_presentations(data: &mut PackageData) -> Result<(), CommandError> {
    if data.version != LEGACY_PACKAGE_VERSION {
        return Ok(());
    }
    let mut cards_by_note = BTreeMap::<String, Vec<PackageCard>>::new();
    for card in data.cards.drain(..) {
        cards_by_note
            .entry(card.source_note_id.clone())
            .or_default()
            .push(card);
    }
    let mut canonical = Vec::new();
    for note in &mut data.notes {
        let mut cards = cards_by_note.remove(&note.source_id).ok_or_else(|| {
            CommandError::invalid_package("A source note has no generated cards.")
        })?;
        cards.sort_by(|left, right| {
            let left_rank = (
                left.direction != "forward",
                left.template_ordinal,
                &left.source_id,
            );
            let right_rank = (
                right.direction != "forward",
                right.template_ordinal,
                &right.source_id,
            );
            left_rank.cmp(&right_rank)
        });
        if note.note_type == "cloze" {
            let mut primary = cards.remove(0);
            primary.direction = "forward".to_string();
            primary.template_ordinal = 0;
            canonical.push(primary);
            note.reverse_enabled = false;
        } else {
            let forward = cards
                .iter()
                .position(|card| card.direction == "forward")
                .ok_or_else(|| {
                    CommandError::invalid_package("A Basic note has no forward card.")
                })?;
            let mut primary = cards.remove(forward);
            primary.direction = "forward".to_string();
            primary.template_ordinal = 0;
            canonical.push(primary);
            if note.reverse_enabled || cards.iter().any(|card| card.direction == "reverse") {
                let mut reverse = cards
                    .into_iter()
                    .find(|card| card.direction == "reverse")
                    .ok_or_else(|| {
                        CommandError::invalid_package(
                            "Reverse metadata exists without a reverse card identity.",
                        )
                    })?;
                reverse.direction = "reverse".to_string();
                reverse.template_ordinal = 0;
                canonical.push(reverse);
                note.reverse_enabled = true;
            }
        }
    }
    if !cards_by_note.is_empty() {
        return Err(CommandError::invalid_package(
            "The package contains generated cards for missing notes.",
        ));
    }
    data.cards = canonical;
    Ok(())
}

fn expected_asset_links(
    notes: &[PackageNote],
) -> Result<BTreeSet<(String, String, String)>, CommandError> {
    let mut links = BTreeSet::new();
    for note in notes {
        for asset_id in collect_asset_ids(&note.front_doc)? {
            links.insert((note.source_id.clone(), asset_id, "front".to_string()));
        }
        for asset_id in collect_asset_ids(&note.back_doc)? {
            links.insert((note.source_id.clone(), asset_id, "back".to_string()));
        }
    }
    Ok(links)
}
