#!/usr/bin/env python3
"""Apply exact Stage 3.3 recovery and committed-success edits once."""

from __future__ import annotations

import re
from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    file_path = Path(path)
    text = file_path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match in {path}, found {count}")
    file_path.write_text(text.replace(old, new, 1), encoding="utf-8")


def regex_once(path: str, pattern: str, replacement: str, label: str) -> None:
    file_path = Path(path)
    text = file_path.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.DOTALL)
    if count != 1:
        raise SystemExit(f"{label}: expected one match in {path}, found {count}")
    file_path.write_text(updated, encoding="utf-8")


# A multi-image save must undo ordinary partial renames immediately. The special
# post-rename injected crash remains uncompensated so startup repair is tested.
regex_once(
    "src-tauri/src/asset_lifecycle.rs",
    r"fn finalize_assets_with_faults\(.*?\n\}\n\n(?=fn retire_unused_draft_assets)",
    r'''fn finalize_assets_with_faults(
    transaction: &Transaction<'_>,
    paths: &AppPaths,
    asset_ids: &[String],
    draft_id: Option<&str>,
    owner_id: Option<&str>,
    faults: PromotionFaults,
) -> Result<DraftFinalization, CommandError> {
    let normalized = normalize_asset_ids(asset_ids.to_vec())?;
    let now = unix_time_millis()?;
    if let Some(draft_id) = draft_id {
        validate_entity_id(draft_id, "asset draft")?;
        ensure_active_draft(transaction, draft_id, now)?;
    }
    ensure_assets_available(transaction, paths, &normalized, draft_id)?;

    let mut promoted = Vec::<(PathBuf, PathBuf)>::new();
    let result = (|| {
        for asset_id in &normalized {
            let asset = load_asset(transaction, asset_id)?;
            if asset.lifecycle_state != "staged" {
                continue;
            }
            let staged_relative = asset.staged_relative_path.as_deref().ok_or_else(|| {
                CommandError::new("asset_stage_missing", "A staged image has no managed path.")
            })?;
            let staged_path = paths.resolve_relative(staged_relative);
            let final_path = paths.resolve_relative(&asset.relative_path);
            create_parent(&final_path)?;
            if final_path.exists() {
                return Err(CommandError::new(
                    "asset_path_conflict",
                    "A managed image path is already occupied.",
                ));
            }
            fs::rename(&staged_path, &final_path).map_err(CommandError::io)?;
            promoted.push((staged_path.clone(), final_path.clone()));
            sync_directory(staged_path.parent())?;
            sync_directory(final_path.parent())?;
            if faults.fail_after_rename {
                return Err(CommandError::new(
                    "asset_test_post_rename_crash",
                    "Injected failure after the managed rename.",
                ));
            }
            transaction
                .execute(
                    "UPDATE assets
                     SET lifecycle_state = 'ready', draft_id = NULL,
                         staged_relative_path = NULL, quarantine_relative_path = NULL,
                         ready_at = COALESCE(ready_at, ?1), verified_at = ?1,
                         updated_at = ?1, lifecycle_reason = '', deleted_at = NULL
                     WHERE id = ?2 AND lifecycle_state = 'staged'",
                    params![now, asset_id],
                )
                .map_err(CommandError::database)?;
            record_event(
                transaction,
                Some(asset_id),
                "promoted",
                Some(staged_relative),
                Some(&asset.relative_path),
                "asset promoted with durable owner",
                now,
            )?;
        }

        let cleanup_paths = if let Some(draft_id) = draft_id {
            let retained = normalized.iter().cloned().collect::<BTreeSet<_>>();
            let cleanup = retire_unused_draft_assets(
                transaction,
                paths,
                draft_id,
                &retained,
                now,
                "unused when owner committed",
            )?;
            transaction
                .execute(
                    "UPDATE asset_drafts
                     SET owner_id = COALESCE(?1, owner_id), updated_at = ?2, committed_at = ?2
                     WHERE id = ?3 AND committed_at IS NULL AND canceled_at IS NULL",
                    params![owner_id, now, draft_id],
                )
                .map_err(CommandError::database)?;
            cleanup
        } else {
            Vec::new()
        };

        Ok(DraftFinalization { cleanup_paths })
    })();

    if let Err(error) = &result {
        if error.code != "asset_test_post_rename_crash" {
            rollback_promoted_files(&promoted);
        }
    }
    result
}

fn rollback_promoted_files(promoted: &[(PathBuf, PathBuf)]) {
    for (staged_path, final_path) in promoted.iter().rev() {
        if !final_path.exists() {
            continue;
        }
        if staged_path.exists() {
            crate::logging::error(
                "asset_promotion_rollback_conflict",
                format!(
                    "staged={} final={}",
                    staged_path.display(),
                    final_path.display()
                ),
            );
            continue;
        }
        if let Err(error) = fs::rename(final_path, staged_path) {
            crate::logging::error(
                "asset_promotion_rollback_failed",
                format!(
                    "staged={} final={} error={error}",
                    staged_path.display(),
                    final_path.display()
                ),
            );
            continue;
        }
        if let Err(error) = sync_directory(final_path.parent()) {
            crate::logging::error("asset_promotion_rollback_sync_failed", error.message);
        }
        if let Err(error) = sync_directory(staged_path.parent()) {
            crate::logging::error("asset_promotion_rollback_sync_failed", error.message);
        }
    }
}

''',
    "make asset promotion compensating",
)

# Referenced corruption must never prevent the library from opening or cause the
# referenced file to be quarantined/deleted.
replace_once(
    "src-tauri/src/asset_lifecycle.rs",
    """    if references > 0 {
        if final_exists {
            verify_asset_file(&asset, &final_path)?;
            transition_asset(connection, &asset, \"ready\", None, None, \"\", now, \"restored\")?;
            return Ok(());
        }""",
    """    if references > 0 {
        if final_exists {
            if !verify_referenced_asset(connection, &asset, &final_path, now)? {
                return Ok(());
            }
            transition_asset(connection, &asset, \"ready\", None, None, \"\", now, \"restored\")?;
            return Ok(());
        }""",
    "handle corrupt referenced final asset",
)
replace_once(
    "src-tauri/src/asset_lifecycle.rs",
    """            sync_directory(staged.parent())?;
            sync_directory(final_path.parent())?;
            verify_asset_file(&asset, &final_path)?;
            transition_asset(
                connection,
                &asset,""",
    """            sync_directory(staged.parent())?;
            sync_directory(final_path.parent())?;
            if !verify_referenced_asset(connection, &asset, &final_path, now)? {
                return Ok(());
            }
            transition_asset(
                connection,
                &asset,""",
    "handle corrupt referenced staged recovery",
)
replace_once(
    "src-tauri/src/asset_lifecycle.rs",
    """            sync_directory(quarantined.parent())?;
            sync_directory(final_path.parent())?;
            verify_asset_file(&asset, &final_path)?;
            transition_asset(
                connection,
                &asset,""",
    """            sync_directory(quarantined.parent())?;
            sync_directory(final_path.parent())?;
            if !verify_referenced_asset(connection, &asset, &final_path, now)? {
                return Ok(());
            }
            transition_asset(
                connection,
                &asset,""",
    "handle corrupt referenced quarantine recovery",
)
replace_once(
    "src-tauri/src/asset_lifecycle.rs",
    """fn reconcile_asset(
    connection: &mut Connection,""",
    """fn verify_referenced_asset(
    connection: &Connection,
    asset: &AssetRecord,
    path: &Path,
    now: i64,
) -> Result<bool, CommandError> {
    if let Err(error) = verify_asset_file(asset, path) {
        let reason = format!(\"referenced file verification failed: {}\", error.message);
        crate::logging::error(
            \"referenced_asset_verification_failed\",
            format!(\"asset_id={} path={} error={}\", asset.id, path.display(), error.message),
        );
        transition_asset(
            connection,
            asset,
            \"missing\",
            None,
            None,
            &reason,
            now,
            \"missing\",
        )?;
        return Ok(false);
    }
    Ok(true)
}

fn reconcile_asset(
    connection: &mut Connection,""",
    "add referenced verification boundary",
)

# Append failure-path tests immediately before the lifecycle test module closes.
asset_path = Path("src-tauri/src/asset_lifecycle.rs")
asset_text = asset_path.read_text(encoding="utf-8")
if "ordinary_multi_asset_failure_returns_prior_files_to_staging" in asset_text:
    raise SystemExit("asset recovery tests already exist")
last_close = asset_text.rfind("\n}")
if last_close < 0:
    raise SystemExit("asset lifecycle test module close is missing")
extra_tests = r'''

    #[test]
    fn ordinary_multi_asset_failure_returns_prior_files_to_staging() {
        let database = Database::open_in_memory().expect("database should open");
        let paths = temp_paths("promotion-compensation");
        let source = source_image(&paths, "source.png");
        database
            .with_connection(|connection| {
                let draft = begin_draft_in_connection(connection, "card", None)?;
                let first = stage_source_in_connection(
                    connection,
                    &paths,
                    &source,
                    &draft.id,
                    Default::default(),
                )?;
                let second = stage_source_in_connection(
                    connection,
                    &paths,
                    &source,
                    &draft.id,
                    Default::default(),
                )?;
                let mut ids = vec![first.id.clone(), second.id.clone()];
                ids.sort();
                let first_record = load_asset(connection, &ids[0])?;
                let second_record = load_asset(connection, &ids[1])?;
                let first_staged = paths.resolve_relative(
                    first_record
                        .staged_relative_path
                        .as_deref()
                        .expect("first staged path"),
                );
                let first_final = paths.resolve_relative(&first_record.relative_path);
                let second_final = paths.resolve_relative(&second_record.relative_path);
                fs::write(&second_final, png_bytes()).map_err(CommandError::io)?;

                let transaction = connection.transaction().map_err(CommandError::database)?;
                let error = finalize_assets_with_faults(
                    &transaction,
                    &paths,
                    &ids,
                    Some(&draft.id),
                    None,
                    Default::default(),
                )
                .expect_err("second final path should conflict");
                assert_eq!(error.code, "asset_path_conflict");
                drop(transaction);

                assert!(first_staged.exists());
                assert!(!first_final.exists());
                assert!(second_final.exists());
                Ok(())
            })
            .expect("ordinary failure should compensate prior renames");
        let _ = fs::remove_dir_all(paths.app_data_dir());
    }

    #[test]
    fn corrupt_referenced_file_is_marked_missing_without_blocking_startup() {
        let database = Database::open_in_memory().expect("database should open");
        let paths = temp_paths("corrupt-reference");
        database
            .with_connection(|connection| {
                let now = unix_time_millis()?;
                let asset_id = generate_id(connection)?;
                let relative = AppPaths::image_relative_path(&asset_id, "png");
                let corrupt = vec![b'x'; png_bytes().len()];
                fs::write(paths.resolve_relative(&relative), &corrupt)
                    .map_err(CommandError::io)?;
                connection.execute(
                    "INSERT INTO assets (
                         id, original_filename, mime_type, byte_size, relative_path,
                         created_at, lifecycle_state, ready_at, verified_at, updated_at
                     ) VALUES (?1, 'corrupt.png', 'image/png', ?2, ?3, ?4,
                               'ready', ?4, ?4, ?4)",
                    params![asset_id, corrupt.len() as i64, relative, now],
                ).map_err(CommandError::database)?;
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput { name: "Corrupt".to_string(), description: None },
                )?;
                let note_id = generate_id(connection)?;
                connection.execute(
                    "INSERT INTO notes (
                         id, deck_id, note_type, content_format, content_schema_version,
                         front_doc, back_doc, front_text, back_text, duplicate_hash,
                         created_at, updated_at, revision
                     ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1,
                               '{}', '{}', 'Q', 'A', '', ?3, ?3, 1)",
                    params![note_id, deck.id, now],
                ).map_err(CommandError::database)?;
                connection.execute(
                    "INSERT INTO note_assets (note_id, asset_id, side)
                     VALUES (?1, ?2, 'front')",
                    params![note_id, asset_id],
                ).map_err(CommandError::database)?;

                let first = reconcile_in_connection(connection, &paths, now + 1)?;
                assert!(first.is_empty());
                let state: String = connection.query_row(
                    "SELECT lifecycle_state FROM assets WHERE id = ?1",
                    [asset_id.as_str()],
                    |row| row.get(0),
                ).map_err(CommandError::database)?;
                assert_eq!(state, "missing");
                assert!(paths.resolve_relative(&relative).exists());
                let event_count: i64 = connection.query_row(
                    "SELECT COUNT(*) FROM asset_reconciliation_events
                     WHERE asset_id = ?1 AND action = 'missing'",
                    [asset_id.as_str()],
                    |row| row.get(0),
                ).map_err(CommandError::database)?;
                assert_eq!(event_count, 1);

                let second = reconcile_in_connection(connection, &paths, now + 2)?;
                assert!(second.is_empty());
                let repeated_count: i64 = connection.query_row(
                    "SELECT COUNT(*) FROM asset_reconciliation_events
                     WHERE asset_id = ?1 AND action = 'missing'",
                    [asset_id.as_str()],
                    |row| row.get(0),
                ).map_err(CommandError::database)?;
                assert_eq!(repeated_count, event_count);
                assert!(paths.resolve_relative(&relative).exists());
                Ok(())
            })
            .expect("corrupt reference should not abort reconciliation");
        let _ = fs::remove_dir_all(paths.app_data_dir());
    }
'''
asset_path.write_text(asset_text[:last_close] + extra_tests + asset_text[last_close:], encoding="utf-8")

# Template helpers already commit their content. Asset finalization must therefore
# never make a durable template/card generation look failed to the caller.
replace_once(
    "src-tauri/src/authoring/stage3_v2/types_commands.rs",
    """        let created = create_card_template_in_connection(connection, &paths, input)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let finalization = finalize_assets_for_owner(
            &transaction,
            &paths,
            &asset_ids,
            draft_id.as_deref(),
            Some(&created.id),
        )?;
        transaction.commit().map_err(CommandError::database)?;
        finalization.cleanup();
        let template = load_stored_template(connection, &paths, &created.id, true)?.dto;
        normalize_card_template_dto_for_editing(template)""",
    """        let created = create_card_template_in_connection(connection, &paths, input)?;
        finalize_assets_after_committed_write(
            connection,
            &paths,
            &asset_ids,
            draft_id.as_deref(),
            &created.id,
            \"create_card_template\",
        );
        let template = match load_stored_template(connection, &paths, &created.id, true) {
            Ok(stored) => stored.dto,
            Err(error) => {
                crate::logging::error(
                    \"committed_template_refresh_failed\",
                    format!(\"template_id={} error={}\", created.id, error.message),
                );
                created
            }
        };
        normalize_card_template_dto_for_editing(template)""",
    "preserve committed template creation",
)
replace_once(
    "src-tauri/src/authoring/stage3_v2/types_commands.rs",
    """        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let finalization = finalize_assets_for_owner(
            &transaction,
            &paths,
            &asset_ids,
            draft_id.as_deref(),
            Some(&updated.id),
        )?;
        transaction.commit().map_err(CommandError::database)?;
        finalization.cleanup();
        let template = load_stored_template(connection, &paths, &updated.id, true)?.dto;
        normalize_card_template_dto_for_editing(template)""",
    """        finalize_assets_after_committed_write(
            connection,
            &paths,
            &asset_ids,
            draft_id.as_deref(),
            &updated.id,
            \"update_card_template\",
        );
        let template = match load_stored_template(connection, &paths, &updated.id, true) {
            Ok(stored) => stored.dto,
            Err(error) => {
                crate::logging::error(
                    \"committed_template_refresh_failed\",
                    format!(\"template_id={} error={}\", updated.id, error.message),
                );
                updated
            }
        };
        normalize_card_template_dto_for_editing(template)""",
    "preserve committed template update",
)
replace_once(
    "src-tauri/src/authoring/stage3_v2/types_commands.rs",
    """        let result = generate_automatic_template_cards_in_connection(connection, &paths, input)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let finalization = finalize_assets_for_owner(
            &transaction,
            &paths,
            &asset_ids,
            draft_id.as_deref(),
            Some(&owner_id),
        )?;
        transaction.commit().map_err(CommandError::database)?;
        finalization.cleanup();
        Ok(result)""",
    """        let result = generate_automatic_template_cards_in_connection(connection, &paths, input)?;
        finalize_assets_after_committed_write(
            connection,
            &paths,
            &asset_ids,
            draft_id.as_deref(),
            &owner_id,
            \"generate_cards_from_template\",
        );
        Ok(result)""",
    "preserve committed template generation",
)
replace_once(
    "src-tauri/src/authoring/stage3_v2/types_commands.rs",
    """fn collect_template_input_asset_ids(
    rules: &[CardTemplateRuleInput],""",
    """fn finalize_assets_after_committed_write(
    connection: &mut Connection,
    paths: &AppPaths,
    asset_ids: &[String],
    draft_id: Option<&str>,
    owner_id: &str,
    operation: &str,
) {
    let transaction = match connection.transaction_with_behavior(TransactionBehavior::Immediate) {
        Ok(transaction) => transaction,
        Err(error) => {
            crate::logging::error(
                \"committed_asset_finalization_start_failed\",
                format!(\"operation={operation} owner_id={owner_id} error={error}\"),
            );
            return;
        }
    };
    let finalization = match finalize_assets_for_owner(
        &transaction,
        paths,
        asset_ids,
        draft_id,
        Some(owner_id),
    ) {
        Ok(finalization) => finalization,
        Err(error) => {
            crate::logging::error(
                \"committed_asset_finalization_failed\",
                format!(
                    \"operation={operation} owner_id={owner_id} error={}\",
                    error.message
                ),
            );
            return;
        }
    };
    if let Err(error) = transaction.commit() {
        crate::logging::error(
            \"committed_asset_finalization_commit_failed\",
            format!(\"operation={operation} owner_id={owner_id} error={error}\"),
        );
        return;
    }
    finalization.cleanup();
}

fn collect_template_input_asset_ids(
    rules: &[CardTemplateRuleInput],""",
    "add post-commit asset finalization boundary",
)
