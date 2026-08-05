use super::*;
use crate::scheduler::insert_new_schedule;
use rusqlite::TransactionBehavior;

struct StoredImportOperation {
    request_fingerprint: String,
    status: String,
    result_deck_id: Option<String>,
    result_card_count: Option<i64>,
    result_asset_count: Option<i64>,
}

pub(super) fn import_package(
    connection: &mut Connection,
    paths: &AppPaths,
    operation_id: &str,
    fingerprint: &str,
    package: PackageData,
) -> Result<ImportResultDto, CommandError> {
    if let Some(result) = prepare_operation(
        connection,
        paths,
        operation_id,
        fingerprint,
        package.version,
    )? {
        return Ok(result);
    }

    let draft_id = create_import_draft(connection, operation_id)?;
    let stage_result = stage_package_assets(connection, paths, &draft_id, &package.assets);
    let asset_map = match stage_result {
        Ok(asset_map) => asset_map,
        Err(error) => {
            fail_operation(connection, paths, operation_id, &draft_id, &error.code)?;
            return Err(error);
        }
    };

    let result = apply_package_transaction(
        connection,
        paths,
        operation_id,
        fingerprint,
        &draft_id,
        package,
        asset_map,
    );
    match result {
        Ok(result) => Ok(result),
        Err(error) => {
            fail_operation(connection, paths, operation_id, &draft_id, &error.code)?;
            Err(error)
        }
    }
}

fn prepare_operation(
    connection: &mut Connection,
    paths: &AppPaths,
    operation_id: &str,
    fingerprint: &str,
    package_version: i64,
) -> Result<Option<ImportResultDto>, CommandError> {
    let existing: Option<StoredImportOperation> = connection
        .query_row(
            "SELECT request_fingerprint, status, result_deck_id,
                    result_card_count, result_asset_count
             FROM deck_import_operations WHERE id = ?1",
            [operation_id],
            |row| {
                Ok(StoredImportOperation {
                    request_fingerprint: row.get(0)?,
                    status: row.get(1)?,
                    result_deck_id: row.get(2)?,
                    result_card_count: row.get(3)?,
                    result_asset_count: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    if let Some(existing) = existing {
        if existing.request_fingerprint != fingerprint {
            return Err(CommandError::new(
                "idempotency_conflict",
                "This deck import operation identifier was already used for another file.",
            ));
        }
        if existing.status == "completed" {
            let deck_id = existing.result_deck_id.ok_or_else(|| {
                CommandError::new(
                    "invalid_import_operation",
                    "The saved import result is incomplete.",
                )
            })?;
            return Ok(Some(ImportResultDto {
                deck: get_deck_from_connection(connection, &deck_id)?,
                card_count: existing.result_card_count.unwrap_or_default(),
                asset_count: existing.result_asset_count.unwrap_or_default(),
            }));
        }
        cleanup_operation_drafts(connection, paths, operation_id)?;
        let now = unix_time_millis()?;
        connection
            .execute(
                "UPDATE deck_import_operations
                 SET package_format = ?1, package_version = ?2,
                     status = 'pending', result_deck_id = NULL,
                     result_card_count = NULL, result_asset_count = NULL,
                     error_code = NULL, updated_at = ?3, completed_at = NULL
                 WHERE id = ?4",
                params![PACKAGE_FORMAT, package_version, now, operation_id],
            )
            .map_err(CommandError::database)?;
        return Ok(None);
    }

    let now = unix_time_millis()?;
    connection
        .execute(
            "INSERT INTO deck_import_operations (
                 id, request_fingerprint, package_format, package_version,
                 status, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 'pending', ?5, ?5)",
            params![
                operation_id,
                fingerprint,
                PACKAGE_FORMAT,
                package_version,
                now
            ],
        )
        .map_err(CommandError::database)?;
    Ok(None)
}

fn create_import_draft(
    connection: &Connection,
    operation_id: &str,
) -> Result<String, CommandError> {
    let now = unix_time_millis()?;
    let draft_id = generate_id(connection)?;
    connection
        .execute(
            "INSERT INTO asset_drafts (
                 id, purpose, owner_id, created_at, updated_at, expires_at
             ) VALUES (?1, 'import', ?2, ?3, ?3, ?4)",
            params![
                draft_id,
                operation_id,
                now,
                now.saturating_add(24 * 60 * 60 * 1_000),
            ],
        )
        .map_err(CommandError::database)?;
    Ok(draft_id)
}

fn stage_package_assets(
    connection: &Connection,
    paths: &AppPaths,
    draft_id: &str,
    assets: &[PackageAsset],
) -> Result<HashMap<String, String>, CommandError> {
    let now = unix_time_millis()?;
    let mut mapping = HashMap::new();
    for asset in assets {
        let (mime, extension) = detect_image_bytes(&asset.data)?;
        if mime != asset.mime_type {
            return Err(CommandError::invalid_package(
                "An imported image has an invalid file type.",
            ));
        }
        let mut new_id = choose_id(connection, "assets", &asset.source_id)?;
        loop {
            let relative_path = AppPaths::image_relative_path(&new_id, extension);
            let final_path = paths.resolve_relative(&relative_path);
            if !final_path.exists() {
                let staged_relative =
                    AppPaths::staged_asset_relative_path(draft_id, &new_id, extension);
                let staged_path = paths.resolve_relative(&staged_relative);
                let _ = fs::remove_file(&staged_path);
                write_new_file(&staged_path, &asset.data)?;
                sync_directory(staged_path.parent())?;
                if let Err(error) = connection.execute(
                    "INSERT INTO assets (
                         id, original_filename, mime_type, byte_size,
                         relative_path, created_at, lifecycle_state,
                         draft_id, staged_relative_path, updated_at
                     ) VALUES (
                         ?1, ?2, ?3, ?4, ?5, ?6,
                         'staged', ?7, ?8, ?6
                     )",
                    params![
                        new_id,
                        asset.original_filename,
                        asset.mime_type,
                        asset.byte_size,
                        relative_path,
                        now,
                        draft_id,
                        staged_relative,
                    ],
                ) {
                    let _ = fs::remove_file(staged_path);
                    return Err(CommandError::database(error));
                }
                connection
                    .execute(
                        "INSERT INTO asset_reconciliation_events (
                             id, asset_id, action, from_path, to_path,
                             detail, occurred_at
                         ) VALUES (?1, ?2, 'staged', NULL, ?3, ?4, ?5)",
                        params![
                            generate_id(connection)?,
                            new_id,
                            staged_relative,
                            "studydeck import asset staged",
                            now,
                        ],
                    )
                    .map_err(CommandError::database)?;
                mapping.insert(asset.source_id.clone(), new_id);
                break;
            }
            new_id = generate_id(connection)?;
        }
    }
    Ok(mapping)
}

fn apply_package_transaction(
    connection: &mut Connection,
    paths: &AppPaths,
    operation_id: &str,
    fingerprint: &str,
    draft_id: &str,
    package: PackageData,
    asset_map: HashMap<String, String>,
) -> Result<ImportResultDto, CommandError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let deck_id = choose_id(&transaction, "decks", &package.deck.source_id)?;
    let sort_order: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1
             FROM decks WHERE deleted_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO decks (
                 id, name, description, sort_order,
                 created_at, updated_at, revision
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                deck_id,
                package.deck.name,
                package.deck.description,
                sort_order,
                package.deck.created_at,
                package.deck.updated_at,
                package.deck.revision.max(1),
            ],
        )
        .map_err(CommandError::database)?;

    let mut tag_map = HashMap::new();
    for tag in &package.tags {
        let existing: Option<String> = transaction
            .query_row(
                "SELECT id FROM tags WHERE normalized_name = ?1",
                [tag.normalized_name.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let tag_id = if let Some(existing) = existing {
            existing
        } else {
            let tag_id = choose_id(&transaction, "tags", &tag.source_id)?;
            transaction
                .execute(
                    "INSERT INTO tags (
                         id, name, normalized_name, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        tag_id,
                        tag.name,
                        tag.normalized_name,
                        tag.created_at,
                        tag.updated_at,
                    ],
                )
                .map_err(CommandError::database)?;
            tag_id
        };
        tag_map.insert(tag.source_id.clone(), tag_id);
    }

    let mut note_map = HashMap::new();
    for note in &package.notes {
        let note_id = choose_id(&transaction, "notes", &note.source_id)?;
        let mut front_doc = note.front_doc.clone();
        let mut back_doc = note.back_doc.clone();
        rewrite_asset_ids(&mut front_doc, &asset_map)?;
        rewrite_asset_ids(&mut back_doc, &asset_map)?;
        let validated = validate_content(
            front_doc,
            back_doc,
            note.front_text.clone(),
            note.back_text.clone(),
        )?;
        let duplicate_hash = duplicate_fingerprint(&validated.front_text, &validated.back_text);
        transaction
            .execute(
                "INSERT INTO notes (
                     id, deck_id, note_type,
                     content_format, content_schema_version,
                     front_doc, back_doc, front_text, back_text, duplicate_hash,
                     reverse_enabled, cloze_source, cloze_mode,
                     created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, ?3, 'tiptap-json', 1,
                     ?4, ?5, ?6, ?7, ?8,
                     ?9, ?10, ?11, ?12, ?13, ?14
                 )",
                params![
                    note_id,
                    deck_id,
                    note.note_type,
                    validated.front_doc_json,
                    validated.back_doc_json,
                    validated.front_text,
                    validated.back_text,
                    duplicate_hash,
                    i64::from(note.reverse_enabled),
                    note.cloze_source,
                    note.cloze_mode,
                    note.created_at,
                    note.updated_at,
                    note.revision.max(1),
                ],
            )
            .map_err(CommandError::database)?;
        note_map.insert(note.source_id.clone(), note_id);
    }

    for (source_note_id, source_tag_id) in &package.note_tags {
        let note_id = note_map.get(source_note_id).ok_or_else(|| {
            CommandError::invalid_package("A tag relationship refers to a missing note.")
        })?;
        let tag_id = tag_map.get(source_tag_id).ok_or_else(|| {
            CommandError::invalid_package("A tag relationship refers to a missing tag.")
        })?;
        transaction
            .execute(
                "INSERT INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                params![note_id, tag_id],
            )
            .map_err(CommandError::database)?;
    }

    for (source_note_id, source_asset_id, side) in &package.note_assets {
        let note_id = note_map.get(source_note_id).ok_or_else(|| {
            CommandError::invalid_package("An image relationship refers to a missing note.")
        })?;
        let asset_id = asset_map.get(source_asset_id).ok_or_else(|| {
            CommandError::invalid_package("An image relationship refers to a missing image.")
        })?;
        transaction
            .execute(
                "INSERT INTO note_assets (note_id, asset_id, side)
                 VALUES (?1, ?2, ?3)",
                params![note_id, asset_id, side],
            )
            .map_err(CommandError::database)?;
    }

    let imported_at = unix_time_millis()?;
    for card in &package.cards {
        let note_id = note_map.get(&card.source_note_id).ok_or_else(|| {
            CommandError::invalid_package("A generated card refers to a missing note.")
        })?;
        let card_id = choose_id(&transaction, "cards", &card.source_id)?;
        transaction
            .execute(
                "INSERT INTO cards (
                     id, note_id, deck_id, direction, template_ordinal,
                     created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    card_id,
                    note_id,
                    deck_id,
                    card.direction,
                    card.template_ordinal,
                    card.created_at,
                    card.updated_at,
                ],
            )
            .map_err(CommandError::database)?;
        insert_new_schedule(&transaction, &card_id, imported_at)?;
    }

    // Build the complete response before the commit. Once SQLite commits, no
    // hydration or serialization failure may reclassify the durable import as a
    // failed user action and encourage a duplicate retry.
    let result_card_count = i64::try_from(package.cards.len()).unwrap_or(i64::MAX);
    let result_asset_count = i64::try_from(package.assets.len()).unwrap_or(i64::MAX);
    let result_deck = get_deck_from_connection(&transaction, &deck_id)?;

    let mut promoted = Vec::new();
    let promotion_result = (|| -> Result<(), CommandError> {
        for new_id in asset_map.values() {
            let row: (String, String) = transaction
                .query_row(
                    "SELECT relative_path, staged_relative_path
                 FROM assets WHERE id = ?1 AND lifecycle_state = 'staged' AND draft_id = ?2",
                    params![new_id, draft_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(CommandError::database)?;
            let staged = paths.resolve_relative(&row.1);
            let final_path = paths.resolve_relative(&row.0);
            if let Some(parent) = final_path.parent() {
                fs::create_dir_all(parent).map_err(CommandError::io)?;
            }
            if let Err(error) = fs::rename(&staged, &final_path) {
                compensate_promotions(&promoted);
                return Err(CommandError::io(error));
            }
            if let Err(error) =
                sync_directory(staged.parent()).and_then(|_| sync_directory(final_path.parent()))
            {
                let _ = fs::rename(&final_path, &staged);
                compensate_promotions(&promoted);
                return Err(error);
            }
            promoted.push((final_path, staged));
            transaction
                .execute(
                    "UPDATE assets
                 SET lifecycle_state = 'ready', draft_id = NULL,
                     staged_relative_path = NULL, ready_at = COALESCE(ready_at, ?1),
                     verified_at = ?1, updated_at = ?1,
                     lifecycle_reason = '', deleted_at = NULL
                 WHERE id = ?2",
                    params![imported_at, new_id],
                )
                .map_err(CommandError::database)?;
        }

        transaction
            .execute(
                "UPDATE asset_drafts
             SET owner_id = ?1, updated_at = ?2, committed_at = ?2
             WHERE id = ?3 AND committed_at IS NULL AND canceled_at IS NULL",
                params![deck_id, imported_at, draft_id],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE deck_import_operations
             SET status = 'completed', result_deck_id = ?1,
                 result_card_count = ?2, result_asset_count = ?3,
                 error_code = NULL, updated_at = ?4, completed_at = ?4
             WHERE id = ?5 AND request_fingerprint = ?6 AND status = 'pending'",
                params![
                    deck_id,
                    result_card_count,
                    result_asset_count,
                    imported_at,
                    operation_id,
                    fingerprint,
                ],
            )
            .map_err(CommandError::database)?;

        Ok(())
    })();
    if let Err(error) = promotion_result {
        compensate_promotions(&promoted);
        return Err(error);
    }

    if let Err(error) = transaction.commit() {
        compensate_promotions(&promoted);
        return Err(CommandError::database(error));
    }
    Ok(ImportResultDto {
        deck: result_deck,
        card_count: result_card_count,
        asset_count: result_asset_count,
    })
}

fn compensate_promotions(promoted: &[(PathBuf, PathBuf)]) {
    for (final_path, staged_path) in promoted.iter().rev() {
        let _ = fs::rename(final_path, staged_path);
    }
}

fn fail_operation(
    connection: &mut Connection,
    paths: &AppPaths,
    operation_id: &str,
    draft_id: &str,
    error_code: &str,
) -> Result<(), CommandError> {
    cleanup_draft(connection, paths, draft_id)?;
    let now = unix_time_millis()?;
    connection
        .execute(
            "UPDATE deck_import_operations
             SET status = 'failed', error_code = ?1,
                 result_deck_id = NULL, result_card_count = NULL,
                 result_asset_count = NULL, updated_at = ?2, completed_at = NULL
             WHERE id = ?3 AND status <> 'completed'",
            params![error_code, now, operation_id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn cleanup_operation_drafts(
    connection: &mut Connection,
    paths: &AppPaths,
    operation_id: &str,
) -> Result<(), CommandError> {
    let draft_ids = {
        let mut statement = connection
            .prepare(
                "SELECT id FROM asset_drafts
                 WHERE purpose = 'import' AND owner_id = ?1 AND committed_at IS NULL",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([operation_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };
    for draft_id in draft_ids {
        cleanup_draft(connection, paths, &draft_id)?;
    }
    Ok(())
}

fn cleanup_draft(
    connection: &mut Connection,
    paths: &AppPaths,
    draft_id: &str,
) -> Result<(), CommandError> {
    let assets = {
        let mut statement = connection
            .prepare(
                "SELECT id, relative_path, staged_relative_path
                 FROM assets WHERE draft_id = ?1",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([draft_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    for (asset_id, _, _) in &assets {
        let references: i64 = transaction
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM note_assets WHERE asset_id = ?1) +
                     (SELECT COUNT(*) FROM card_template_assets WHERE asset_id = ?1)",
                [asset_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if references == 0 {
            transaction
                .execute("DELETE FROM assets WHERE id = ?1", [asset_id])
                .map_err(CommandError::database)?;
        }
    }
    let now = unix_time_millis()?;
    transaction
        .execute(
            "UPDATE asset_drafts
             SET canceled_at = COALESCE(canceled_at, ?1), updated_at = ?1
             WHERE id = ?2 AND committed_at IS NULL",
            params![now, draft_id],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;
    for (_, relative, staged) in assets {
        if let Some(staged) = staged {
            let _ = fs::remove_file(paths.resolve_relative(&staged));
        }
        let final_path = paths.resolve_relative(&relative);
        if final_path.exists() {
            let _ = fs::remove_file(final_path);
        }
    }
    Ok(())
}
