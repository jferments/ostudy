#!/usr/bin/env python3
"""Restore shared authoring helpers and finish the first native compile pass."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

search = ROOT / "src-tauri/src/authoring/search_and_tags.rs"
content = search.read_text()
if "fn hydrate_organized_card(" not in content:
    content += r'''

fn hydrate_organized_card(
    connection: &Connection,
    paths: &AppPaths,
    note_id: &str,
) -> Result<OrganizedCardDto, CommandError> {
    let card = get_card_from_connection(connection, paths, note_id)?;
    let tags = load_tag_names(connection, note_id)?;
    let due_at = connection
        .query_row(
            "SELECT s.due_at
             FROM card_schedules s
             JOIN cards c ON c.id = s.card_id
             WHERE c.note_id = ?1
               AND c.deleted_at IS NULL
               AND c.direction = 'forward'",
            [note_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    Ok(organized_from_card(card, tags, due_at))
}

fn organized_from_card(card: CardDto, tags: Vec<String>, due_at: i64) -> OrganizedCardDto {
    OrganizedCardDto {
        id: card.id,
        note_id: card.note_id,
        deck_id: card.deck_id,
        direction: card.direction,
        front_doc: card.front_doc,
        back_doc: card.back_doc,
        front_text: card.front_text,
        back_text: card.back_text,
        assets: card.assets,
        tags,
        due_at,
        created_at: card.created_at,
        updated_at: card.updated_at,
        revision: card.revision,
    }
}

fn load_tag_names(connection: &Connection, note_id: &str) -> Result<Vec<String>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT t.name
             FROM tags t
             JOIN note_tags nt ON nt.tag_id = t.id
             WHERE nt.note_id = ?1
             ORDER BY t.name COLLATE NOCASE ASC",
        )
        .map_err(CommandError::database)?;
    let tags = statement
        .query_map([note_id], |row| row.get(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(tags)
}

fn validate_tags(values: Vec<String>) -> Result<Vec<ValidatedTag>, CommandError> {
    if values.len() > MAX_TAGS_PER_NOTE {
        return Err(CommandError::new(
            "too_many_tags",
            format!("A card can have at most {MAX_TAGS_PER_NOTE} tags."),
        ));
    }
    let mut seen = BTreeSet::new();
    let mut tags = Vec::new();
    for value in values {
        let name = collapse_whitespace(&value);
        if name.is_empty()
            || name.chars().count() > MAX_TAG_CHARS
            || name.chars().any(char::is_control)
        {
            return Err(CommandError::new(
                "invalid_tag",
                format!("Each tag must contain 1 to {MAX_TAG_CHARS} visible characters."),
            ));
        }
        let normalized = name.to_lowercase();
        if seen.insert(normalized.clone()) {
            tags.push(ValidatedTag { name, normalized });
        }
    }
    Ok(tags)
}

fn replace_note_tags(
    transaction: &Transaction<'_>,
    note_id: &str,
    tags: &[ValidatedTag],
    now: i64,
) -> Result<(), CommandError> {
    transaction
        .execute("DELETE FROM note_tags WHERE note_id = ?1", [note_id])
        .map_err(CommandError::database)?;
    for tag in tags {
        let existing: Option<String> = transaction
            .query_row(
                "SELECT id FROM tags WHERE normalized_name = ?1",
                [tag.normalized.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let tag_id = match existing {
            Some(id) => id,
            None => {
                let id = generate_id(transaction)?;
                transaction
                    .execute(
                        "INSERT INTO tags (
                             id, name, normalized_name, created_at, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?4)",
                        params![id, tag.name, tag.normalized, now],
                    )
                    .map_err(CommandError::database)?;
                id
            }
        };
        transaction
            .execute(
                "INSERT INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                params![note_id, tag_id],
            )
            .map_err(CommandError::database)?;
    }
    cleanup_unused_tags(transaction)
}

fn cleanup_unused_tags(connection: &Connection) -> Result<(), CommandError> {
    connection
        .execute(
            "DELETE FROM tags
             WHERE NOT EXISTS (
                 SELECT 1 FROM note_tags nt WHERE nt.tag_id = tags.id
             )",
            [],
        )
        .map_err(CommandError::database)?;
    Ok(())
}
'''
search.write_text(content)

schedule = ROOT / "src-tauri/src/schedule_controls.rs"
content = schedule.read_text()
old = "Some(next_local_day_start_millis()?)"
if old in content:
    content = content.replace(old, "Some(next_local_day_start_millis(now)?)", 1)
schedule.write_text(content)

print("Restored shared authoring helpers and schedule calendar call.")
