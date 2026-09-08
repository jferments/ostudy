use super::*;
use rusqlite::TransactionBehavior;

pub(super) fn export_deck_from_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    deck_id: &str,
    destination: &Path,
) -> Result<ExportResultDto, CommandError> {
    let parent = destination
        .parent()
        .ok_or_else(|| CommandError::new("invalid_export_path", "Choose a valid export folder."))?;
    if !parent.is_dir() {
        return Err(CommandError::new(
            "invalid_export_path",
            "The selected export folder no longer exists.",
        ));
    }

    let temporary = temporary_export_path(destination);
    let _ = fs::remove_file(&temporary);
    let guard = TemporaryFileGuard::new(temporary.clone());

    // The read transaction is held until every relational row and immutable
    // ready-asset file has been copied. Tags and generated-card metadata cannot
    // therefore drift away from the core note/card snapshot.
    let source = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(CommandError::database)?;
    let package_data = load_snapshot(&source, paths, deck_id)?;
    read::validate_package_data(&package_data, true)?;

    let mut package = Connection::open(&temporary).map_err(CommandError::database)?;
    package
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = DELETE;
             PRAGMA synchronous = FULL;
             PRAGMA trusted_schema = OFF;",
        )
        .map_err(CommandError::database)?;
    package
        .execute_batch(PACKAGE_SCHEMA)
        .map_err(CommandError::database)?;
    let transaction = package.transaction().map_err(CommandError::database)?;
    let exported_at = unix_time_millis()?;
    for (key, value) in [
        ("format", PACKAGE_FORMAT.to_string()),
        ("formatVersion", CURRENT_PACKAGE_VERSION.to_string()),
        ("exportedAt", exported_at.to_string()),
        ("applicationVersion", env!("CARGO_PKG_VERSION").to_string()),
        ("historyPolicy", "fresh-local-schedules".to_string()),
    ] {
        transaction
            .execute(
                "INSERT INTO package_metadata (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .map_err(CommandError::database)?;
    }

    let deck = &package_data.deck;
    transaction
        .execute(
            "INSERT INTO deck (
                 source_id, name, description, sort_order,
                 created_at, updated_at, revision
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                deck.source_id,
                deck.name,
                deck.description,
                deck.sort_order,
                deck.created_at,
                deck.updated_at,
                deck.revision,
            ],
        )
        .map_err(CommandError::database)?;

    for note in &package_data.notes {
        transaction
            .execute(
                "INSERT INTO notes (
                     source_id, source_deck_id, note_type,
                     content_format, content_schema_version,
                     front_doc, back_doc, front_text, back_text, duplicate_hash,
                     reverse_enabled, cloze_source, cloze_mode,
                     created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, ?3, ?4, ?5,
                     ?6, ?7, ?8, ?9, ?10,
                     ?11, ?12, ?13, ?14, ?15, ?16
                 )",
                params![
                    note.source_id,
                    note.source_deck_id,
                    note.note_type,
                    note.content_format,
                    note.content_schema_version,
                    serde_json::to_string(&note.front_doc).map_err(CommandError::serialization)?,
                    serde_json::to_string(&note.back_doc).map_err(CommandError::serialization)?,
                    note.front_text,
                    note.back_text,
                    note.duplicate_hash,
                    i64::from(note.reverse_enabled),
                    note.cloze_source,
                    note.cloze_mode,
                    note.created_at,
                    note.updated_at,
                    note.revision,
                ],
            )
            .map_err(CommandError::database)?;
    }

    for card in &package_data.cards {
        transaction
            .execute(
                "INSERT INTO cards (
                     source_id, source_note_id, direction, template_ordinal,
                     created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    card.source_id,
                    card.source_note_id,
                    card.direction,
                    card.template_ordinal,
                    card.created_at,
                    card.updated_at,
                ],
            )
            .map_err(CommandError::database)?;
    }

    for tag in &package_data.tags {
        transaction
            .execute(
                "INSERT INTO tags (
                     source_id, name, normalized_name, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    tag.source_id,
                    tag.name,
                    tag.normalized_name,
                    tag.created_at,
                    tag.updated_at,
                ],
            )
            .map_err(CommandError::database)?;
    }
    for (note_id, tag_id) in &package_data.note_tags {
        transaction
            .execute(
                "INSERT INTO note_tags (source_note_id, source_tag_id)
                 VALUES (?1, ?2)",
                params![note_id, tag_id],
            )
            .map_err(CommandError::database)?;
    }

    for asset in &package_data.assets {
        transaction
            .execute(
                "INSERT INTO assets (
                     source_id, original_filename, mime_type, byte_size, data
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    asset.source_id,
                    asset.original_filename,
                    asset.mime_type,
                    asset.byte_size,
                    asset.data,
                ],
            )
            .map_err(CommandError::database)?;
    }
    for (note_id, asset_id, side) in &package_data.note_assets {
        transaction
            .execute(
                "INSERT INTO note_assets (
                     source_note_id, source_asset_id, side
                 ) VALUES (?1, ?2, ?3)",
                params![note_id, asset_id, side],
            )
            .map_err(CommandError::database)?;
    }

    transaction.commit().map_err(CommandError::database)?;
    let integrity: String = package
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(CommandError::database)?;
    if integrity != "ok" {
        return Err(CommandError::new(
            "export_validation_failed",
            "The exported deck did not pass its integrity check.",
        ));
    }
    drop(package);
    source.commit().map_err(CommandError::database)?;

    OpenOptions::new()
        .read(true)
        .open(&temporary)
        .and_then(|file| file.sync_all())
        .map_err(CommandError::io)?;
    replace_export_file(&temporary, destination)?;
    guard.keep();

    Ok(ExportResultDto {
        destination_path: destination.to_string_lossy().into_owned(),
        card_count: i64::try_from(package_data.cards.len()).unwrap_or(i64::MAX),
        asset_count: i64::try_from(package_data.assets.len()).unwrap_or(i64::MAX),
    })
}

fn load_snapshot(
    connection: &Transaction<'_>,
    paths: &AppPaths,
    deck_id: &str,
) -> Result<PackageData, CommandError> {
    let deck = connection
        .query_row(
            "SELECT id, name, description, sort_order,
                    created_at, updated_at, revision
             FROM decks
             WHERE id = ?1 AND deleted_at IS NULL AND archived_at IS NULL",
            [deck_id],
            |row| {
                Ok(PackageDeck {
                    source_id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    sort_order: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    revision: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| CommandError::new("deck_not_found", "That deck no longer exists."))?;

    let notes = {
        let mut statement = connection
            .prepare(
                "SELECT id, deck_id, note_type, content_format,
                        content_schema_version, front_doc, back_doc,
                        front_text, back_text, duplicate_hash,
                        reverse_enabled, cloze_source, cloze_mode,
                        created_at, updated_at, revision
                 FROM notes
                 WHERE deck_id = ?1 AND deleted_at IS NULL
                 ORDER BY created_at, id",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([deck_id], |row| {
                let front_json: String = row.get(5)?;
                let back_json: String = row.get(6)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    front_json,
                    back_json,
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
            .map_err(CommandError::database)?
            .into_iter()
            .map(|row| {
                Ok(PackageNote {
                    source_id: row.0,
                    source_deck_id: row.1,
                    note_type: row.2,
                    content_format: row.3,
                    content_schema_version: row.4,
                    front_doc: serde_json::from_str(&row.5).map_err(CommandError::serialization)?,
                    back_doc: serde_json::from_str(&row.6).map_err(CommandError::serialization)?,
                    front_text: row.7,
                    back_text: row.8,
                    duplicate_hash: row.9,
                    reverse_enabled: row.10 == 1,
                    cloze_source: row.11,
                    cloze_mode: row.12,
                    created_at: row.13,
                    updated_at: row.14,
                    revision: row.15,
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        rows
    };

    let cards = {
        let mut statement = connection
            .prepare(
                "SELECT c.id, c.note_id, c.direction, c.template_ordinal,
                        c.created_at, c.updated_at
                 FROM cards c
                 JOIN notes n ON n.id = c.note_id
                 WHERE c.deck_id = ?1
                   AND c.deleted_at IS NULL
                   AND n.deleted_at IS NULL
                 ORDER BY c.created_at, c.id",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([deck_id], |row| {
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
        rows
    };

    let tags = {
        let mut statement = connection
            .prepare(
                "SELECT DISTINCT t.id, t.name, t.normalized_name,
                                 t.created_at, t.updated_at
                 FROM tags t
                 JOIN note_tags nt ON nt.tag_id = t.id
                 JOIN notes n ON n.id = nt.note_id
                 WHERE n.deck_id = ?1 AND n.deleted_at IS NULL
                 ORDER BY t.normalized_name, t.id",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([deck_id], |row| {
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

    let note_tags = {
        let mut statement = connection
            .prepare(
                "SELECT nt.note_id, nt.tag_id
                 FROM note_tags nt
                 JOIN notes n ON n.id = nt.note_id
                 WHERE n.deck_id = ?1 AND n.deleted_at IS NULL
                 ORDER BY nt.note_id, nt.tag_id",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([deck_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };

    let note_assets = {
        let mut statement = connection
            .prepare(
                "SELECT na.note_id, na.asset_id, na.side
                 FROM note_assets na
                 JOIN notes n ON n.id = na.note_id
                 WHERE n.deck_id = ?1 AND n.deleted_at IS NULL
                 ORDER BY na.note_id, na.asset_id, na.side",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([deck_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };

    let assets = {
        let mut statement = connection
            .prepare(
                "SELECT DISTINCT a.id, a.original_filename, a.mime_type,
                                 a.byte_size, a.relative_path
                 FROM assets a
                 JOIN note_assets na ON na.asset_id = a.id
                 JOIN notes n ON n.id = na.note_id
                 WHERE n.deck_id = ?1
                   AND n.deleted_at IS NULL
                   AND a.deleted_at IS NULL
                   AND a.lifecycle_state = 'ready'
                 ORDER BY a.id",
            )
            .map_err(CommandError::database)?;
        let metadata = statement
            .query_map([deck_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        let mut total_bytes = 0_usize;
        let mut assets = Vec::with_capacity(metadata.len());
        for row in metadata {
            let path = paths.resolve_relative(&row.4);
            let data = fs::read(&path).map_err(|_| {
                CommandError::new(
                    "missing_asset",
                    format!("The asset '{}' is missing from the local library.", row.1),
                )
            })?;
            if data.is_empty() || data.len() > MAX_PACKAGE_SINGLE_ASSET_BYTES {
                return Err(CommandError::new(
                    "asset_too_large",
                    format!("The asset '{}' is empty or exceeds 25 MB.", row.1),
                ));
            }
            total_bytes = total_bytes.checked_add(data.len()).ok_or_else(|| {
                CommandError::new("deck_too_large", "The deck contains too much asset data.")
            })?;
            if total_bytes > MAX_PACKAGE_ASSET_BYTES {
                return Err(CommandError::new(
                    "deck_too_large",
                    "This deck contains more than 512 MB of managed assets.",
                ));
            }
            if i64::try_from(data.len()).unwrap_or(i64::MAX) != row.3 {
                return Err(CommandError::new(
                    "asset_size_mismatch",
                    format!("The asset '{}' does not match its size record.", row.1),
                ));
            }
            let detected = crate::assets::detect_managed_asset_bytes(&data, &row.1)?;
            if detected.mime_type != row.2 {
                return Err(CommandError::new(
                    "asset_type_mismatch",
                    format!("The asset '{}' does not match its file type.", row.1),
                ));
            }
            assets.push(PackageAsset {
                source_id: row.0,
                original_filename: row.1,
                mime_type: row.2,
                byte_size: row.3,
                data,
            });
        }
        assets
    };

    Ok(PackageData {
        version: CURRENT_PACKAGE_VERSION,
        deck,
        notes,
        cards,
        tags,
        note_tags,
        assets,
        note_assets,
    })
}
