#!/usr/bin/env python3
"""Remove per-dimension Analytics lookups and correct distinct-card counts."""
from pathlib import Path

path = Path(__file__).resolve().parents[1] / "src-tauri/src/analytics.rs"
content = path.read_text()

old = '''    let ids = inventory
        .keys()
        .chain(historical.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut result = Vec::new();
    for deck_id in ids {
        let Some((name, archived)) = connection
            .query_row(
                "SELECT name, archived_at IS NOT NULL
                 FROM decks WHERE id = ?1 AND deleted_at IS NULL",
                [deck_id.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0)),
            )
            .optional()
            .map_err(CommandError::database)?
        else {
            continue;
        };
'''
new = '''    let ids = inventory
        .keys()
        .chain(historical.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let names = retained_deck_names(connection)?;
    let mut result = Vec::new();
    for deck_id in ids {
        let Some((name, archived)) = names.get(&deck_id).cloned() else {
            continue;
        };
'''
if old not in content:
    raise SystemExit("deck_stats lookup marker missing")
content = content.replace(old, new, 1)

anchor = "fn deck_inventory(\n"
helper = '''fn retained_deck_names(
    connection: &Connection,
) -> Result<BTreeMap<String, (String, bool)>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, archived_at IS NOT NULL
             FROM decks
             WHERE deleted_at IS NULL",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                (row.get::<_, String>(1)?, row.get::<_, i64>(2)? != 0),
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)
}

'''
if "fn retained_deck_names(" not in content:
    content = content.replace(anchor, helper + anchor, 1)

old = '''    let ids = inventory
        .keys()
        .chain(historical.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut result = Vec::new();
    for tag_id in ids {
        let current_name = connection
            .query_row(
                "SELECT name FROM tags WHERE id = ?1",
                [tag_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let captured_name = connection
            .query_row(
                "SELECT captured_name
                 FROM review_event_tag_attributions
                 WHERE tag_id = ?1
                 ORDER BY review_event_id DESC LIMIT 1",
                [tag_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some(name) = current_name.or(captured_name) else {
            continue;
        };
'''
new = '''    let ids = inventory
        .keys()
        .chain(historical.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let current_names = retained_tag_names(connection)?;
    let captured_names = captured_tag_names(connection)?;
    let mut result = Vec::new();
    for tag_id in ids {
        let Some(name) = current_names
            .get(&tag_id)
            .or_else(|| captured_names.get(&tag_id))
            .cloned()
        else {
            continue;
        };
'''
if old not in content:
    raise SystemExit("tag_stats lookup marker missing")
content = content.replace(old, new, 1)

anchor = "fn tag_inventory(\n"
helper = '''fn retained_tag_names(
    connection: &Connection,
) -> Result<BTreeMap<String, String>, CommandError> {
    let mut statement = connection
        .prepare("SELECT id, name FROM tags")
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(CommandError::database)?
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)
}

fn captured_tag_names(
    connection: &Connection,
) -> Result<BTreeMap<String, String>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT tag_id, MAX(captured_name)
             FROM review_event_tag_attributions
             GROUP BY tag_id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(CommandError::database)?
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)
}

'''
if "fn retained_tag_names(" not in content:
    content = content.replace(anchor, helper + anchor, 1)

content = content.replace(
    '''                COALESCE(SUM(DISTINCT CASE
                    WHEN c.suspended_at IS NULL
                     AND (c.buried_until IS NULL OR c.buried_until <= ?1)
                     AND COALESCE(ls.needs_more_practice, 0) = 1
                    THEN 1 ELSE 0 END), 0)''',
    '''                COUNT(DISTINCT CASE
                    WHEN c.suspended_at IS NULL
                     AND (c.buried_until IS NULL OR c.buried_until <= ?1)
                     AND COALESCE(ls.needs_more_practice, 0) = 1
                    THEN c.id END)''',
    1,
)

path.write_text(content)
print("Bounded Analytics dimension lookup queries.")
