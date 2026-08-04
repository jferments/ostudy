/// Public `import_deck` compatibility entry point.
///
/// The Stage 3 transfer layer first imports the core package and restores any
/// optional reverse/cloze metadata that newer packages contain. This wrapper
/// journals the filesystem operation before import, preserves the committed
/// result even when post-commit journal reconciliation fails, and then runs the
/// canonical Stage 3 normalization so historical multi-card cloze packages are
/// immediately represented by one active cloze identity.
#[tauri::command(rename = "import_deck")]
pub fn import_deck_with_single_cloze_normalization(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    source_path: String,
) -> Result<crate::transfer::ImportResultDto, CommandError> {
    let operation_id = crate::transfer_lifecycle::begin_import(database.inner())?;
    let imported = crate::stage3_transfer::import_deck_with_card_types(
        database.clone(),
        paths.clone(),
        source_path,
    );
    let mut result = match imported {
        Ok(result) => result,
        Err(error) => {
            if let Err(recovery_error) = crate::transfer_lifecycle::abort_import(
                database.inner(),
                paths.inner(),
                &operation_id,
                &error.code,
            ) {
                crate::logging::error(
                    "asset_import_abort_failed",
                    format!(
                        "operation_id={operation_id} import_error={} recovery_error={}",
                        error.code, recovery_error.message
                    ),
                );
            }
            return Err(error);
        }
    };

    // The package transaction is committed at this boundary. Journal and
    // reconciliation failures are logged rather than reclassifying the durable
    // import as a failed operation in the UI.
    if let Err(error) = crate::transfer_lifecycle::finish_import(
        database.inner(),
        paths.inner(),
        &operation_id,
        &result.deck.id,
    ) {
        crate::logging::error(
            "asset_import_finalize_failed",
            format!(
                "operation_id={operation_id} deck_id={} error={}",
                result.deck.id, error.message
            ),
        );
    }

    result.card_count = database.with_connection(|connection| {
        normalize_stage3_schema(connection)?;
        connection
            .query_row(
                "SELECT COUNT(*)
                 FROM cards c
                 JOIN notes n ON n.id = c.note_id
                 WHERE c.deck_id = ?1
                   AND c.deleted_at IS NULL
                   AND n.deleted_at IS NULL",
                [result.deck.id.as_str()],
                |row| row.get(0),
            )
            .map_err(CommandError::database)
    })?;
    Ok(result)
}
