use super::*;

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "cardType",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum CardNoteDto {
    Basic {
        note_id: String,
        deck_id: String,
        front_doc: Value,
        back_doc: Value,
        front_text: String,
        back_text: String,
        reverse_enabled: bool,
        assets: Vec<AssetDto>,
        tags: Vec<String>,
        generated_card_count: i64,
        created_at: i64,
        updated_at: i64,
        revision: i64,
    },
    Cloze {
        note_id: String,
        deck_id: String,
        cloze_source: String,
        cloze_mode: String,
        tags: Vec<String>,
        generated_card_count: i64,
        created_at: i64,
        updated_at: i64,
        revision: i64,
    },
}

pub(super) fn hydrate_card_note(
    connection: &Connection,
    paths: &AppPaths,
    note_id: &str,
) -> Result<CardNoteDto, CommandError> {
    let stored = connection
        .query_row(
            "SELECT n.deck_id, n.note_type, n.front_doc, n.back_doc,
                    n.front_text, n.back_text, n.reverse_enabled,
                    n.cloze_source, COALESCE(n.cloze_mode, 'one_at_a_time'),
                    n.created_at, n.updated_at, n.revision,
                    (SELECT COUNT(*) FROM cards c
                     WHERE c.note_id = n.id AND c.deleted_at IS NULL)
             FROM notes n
             JOIN decks d ON d.id = n.deck_id
             WHERE n.id = ?1
               AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL
               AND d.archived_at IS NULL",
            [note_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)? != 0,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, i64>(11)?,
                    row.get::<_, i64>(12)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| CommandError::new("card_not_found", "That flashcard no longer exists."))?;

    let tags = load_tag_names(connection, note_id)?;
    match stored.1.as_str() {
        "cloze" => Ok(CardNoteDto::Cloze {
            note_id: note_id.to_string(),
            deck_id: stored.0,
            cloze_source: stored.7,
            cloze_mode: stored.8,
            tags,
            generated_card_count: stored.12,
            created_at: stored.9,
            updated_at: stored.10,
            revision: stored.11,
        }),
        "basic" => {
            let front_doc = serde_json::from_str(&stored.2).map_err(CommandError::serialization)?;
            let back_doc = serde_json::from_str(&stored.3).map_err(CommandError::serialization)?;
            let assets = crate::assets::get_assets_for_note(connection, paths, note_id)?;
            Ok(CardNoteDto::Basic {
                note_id: note_id.to_string(),
                deck_id: stored.0,
                front_doc,
                back_doc,
                front_text: stored.4,
                back_text: stored.5,
                reverse_enabled: stored.6,
                assets,
                tags,
                generated_card_count: stored.12,
                created_at: stored.9,
                updated_at: stored.10,
                revision: stored.11,
            })
        }
        _ => Err(CommandError::new(
            "invalid_card_type",
            "The saved flashcard has an unsupported card type.",
        )),
    }
}
