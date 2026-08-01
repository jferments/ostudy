fn load_export_payload(
    connection: &Connection,
    deck_id: &str,
) -> Result<TagPayload, CommandError> {
    let tags = {
        let mut statement = connection
            .prepare(
                "SELECT DISTINCT t.id, t.name, t.normalized_name
                 FROM tags t
                 JOIN note_tags nt ON nt.tag_id = t.id
                 JOIN notes n ON n.id = nt.note_id
                 WHERE n.deck_id = ?1 AND n.deleted_at IS NULL
                 ORDER BY t.id",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([deck_id], |row| {
                Ok(ExportTag {
                    source_id: row.get(0)?,
                    name: row.get(1)?,
                    normalized_name: row.get(2)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };

    let links = {
        let mut statement = connection
            .prepare(
                "SELECT nt.note_id, nt.tag_id
                 FROM note_tags nt
                 JOIN notes n ON n.id = nt.note_id
                 WHERE n.deck_id = ?1 AND n.deleted_at IS NULL
                 ORDER BY nt.note_id, nt.tag_id",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([deck_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };

    let notes = {
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

    Ok(TagPayload { tags, links, notes })
}

fn append_payload(path: &Path, payload: &TagPayload) -> Result<(), CommandError> {
    let mut package = Connection::open(path).map_err(CommandError::database)?;
    package
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = DELETE;
             PRAGMA synchronous = FULL;
             CREATE TABLE IF NOT EXISTS tags (
                 source_id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 normalized_name TEXT NOT NULL UNIQUE
             ) STRICT;
             CREATE TABLE IF NOT EXISTS note_tags (
                 source_note_id TEXT NOT NULL,
                 source_tag_id TEXT NOT NULL,
                 PRIMARY KEY(source_note_id, source_tag_id)
             ) STRICT;",
        )
        .map_err(CommandError::database)?;

    let transaction = package
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    for tag in &payload.tags {
        transaction
            .execute(
                "INSERT INTO tags (source_id, name, normalized_name)
                 VALUES (?1, ?2, ?3)",
                params![tag.source_id, tag.name, tag.normalized_name],
            )
            .map_err(CommandError::database)?;
    }
    for (note_id, tag_id) in &payload.links {
        transaction
            .execute(
                "INSERT INTO note_tags (source_note_id, source_tag_id)
                 VALUES (?1, ?2)",
                params![note_id, tag_id],
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
            "The exported deck did not pass its final tag integrity check.",
        ));
    }
    Ok(())
}
