/// Public `import_deck` compatibility entry point.
///
/// The Stage 3 transfer layer first imports the core package and restores any
/// optional reverse/cloze metadata that newer packages contain. This wrapper
/// then runs the canonical Stage 3 normalization so packages created before the
/// single-cloze-card invariant are immediately represented by one active cloze
/// identity. The package's original count can include rows retired by that
/// normalization, so the result is recounted from active local cards before it
/// is returned to the frontend.
#[tauri::command(rename = "import_deck")]
pub fn import_deck_with_single_cloze_normalization(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    source_path: String,
) -> Result<crate::transfer::ImportResultDto, CommandError> {
    let mut result = crate::stage3_transfer::import_deck_with_card_types(
        database.clone(),
        paths,
        source_path,
    )?;
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
