#!/usr/bin/env python3
"""Complete studydeck response and regression module boundaries."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


path = "src-tauri/src/studydeck.rs"
text = read(path)
text = replace_once(
    text,
    '''#[cfg(test)]
mod tests;
''',
    '''#[cfg(test)]
mod tests;
#[cfg(test)]
mod extended_tests;
''',
    "studydeck extended tests",
)
write(path, text)

path = "src-tauri/src/studydeck/import.rs"
text = read(path)
text = replace_once(
    text,
    '''        insert_new_schedule(&transaction, &card_id, imported_at)?;
    }

    let mut promoted = Vec::new();''',
    '''        insert_new_schedule(&transaction, &card_id, imported_at)?;
    }

    // Build the complete response before the commit. Once SQLite commits, no
    // hydration or serialization failure may reclassify the durable import as a
    // failed user action and encourage a duplicate retry.
    let result_card_count = i64::try_from(package.cards.len()).unwrap_or(i64::MAX);
    let result_asset_count = i64::try_from(package.assets.len()).unwrap_or(i64::MAX);
    let result_deck = get_deck_from_connection(&transaction, &deck_id)?;

    let mut promoted = Vec::new();''',
    "pre-commit import response",
)
text = replace_once(
    text,
    '''                    i64::try_from(package.cards.len()).unwrap_or(i64::MAX),
                    i64::try_from(package.assets.len()).unwrap_or(i64::MAX),''',
    '''                    result_card_count,
                    result_asset_count,''',
    "stored import result counts",
)
text = replace_once(
    text,
    '''    Ok(ImportResultDto {
        deck: get_deck_from_connection(connection, &deck_id)?,
        card_count: i64::try_from(package.cards.len()).unwrap_or(i64::MAX),
        asset_count: i64::try_from(package.assets.len()).unwrap_or(i64::MAX),
    })''',
    '''    Ok(ImportResultDto {
        deck: result_deck,
        card_count: result_card_count,
        asset_count: result_asset_count,
    })''',
    "post-commit import response",
)
write(path, text)

print("Applied studydeck response and regression module boundaries.")
