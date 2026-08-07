#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
path = ROOT / "src-tauri/src/cards.rs"
text = path.read_text(encoding="utf-8")

text = text.replace(
    "//! Shared card hydration, deletion, and rich-text validation.\n",
    "//! Shared card-presentation transport and rich-text validation.\n",
    1,
)
text = text.replace(
    "use rusqlite::{Connection, OptionalExtension};\n",
    "use rusqlite::Connection;\n",
    1,
)
text = text.replace(
    "    assets::{get_assets_for_note, AssetDto},\n",
    "    assets::AssetDto,\n",
    1,
)
text = text.replace("    paths::AppPaths,\n", "", 1)

start = text.index("struct RawCard {\n")
end = text.index("pub(crate) fn validate_content(\n", start)
removed = text[start:end]
for expected in ["struct RawCard", "get_card_from_connection", "hydrate_card", "raw_card_from_row"]:
    if expected not in removed:
        raise SystemExit(f"expected stale hydrator symbol missing from removal block: {expected}")
text = text[:start] + text[end:]

# The old generic hydrator was used only by this deletion regression after the
# CardNote/CardPresentation split. Preserve deletion coverage by asserting the
# authoritative note/card tombstones directly rather than retaining a fake API.
test_start = text.index("    #[test]\n    fn hydrates_and_deletes_a_card() {\n")
test_end = text.index("    #[test]\n    fn rejects_javascript_links() {\n", test_start)
replacement = '''    #[test]
    fn deletes_a_card_note_and_its_presentation() {
        let database = Database::open_in_memory().expect("database should open");

        let deck = database
            .with_connection(|connection| {
                crate::decks::create_deck_in_connection(
                    connection,
                    crate::decks::CreateDeckInput {
                        name: "Biology".to_string(),
                        description: None,
                    },
                )
            })
            .expect("deck should be created");

        let (note_id, card_id) = database
            .with_connection(|connection| {
                let content = validate_content(
                    paragraph("What is ATP?"),
                    paragraph("The cell's immediately usable energy currency."),
                    "What is ATP?".to_string(),
                    "The cell's immediately usable energy currency.".to_string(),
                )?;
                let now = unix_time_millis()?;
                let note_id = crate::util::generate_id(connection)?;
                let card_id = crate::util::generate_id(connection)?;
                connection
                    .execute(
                        "INSERT INTO notes (
                             id, deck_id, note_type, content_format, content_schema_version,
                             front_doc, back_doc, front_text, back_text,
                             created_at, updated_at, revision
                         ) VALUES (
                             ?1, ?2, 'basic', 'tiptap-json', 1,
                             ?3, ?4, ?5, ?6, ?7, ?7, 1
                         )",
                        params![
                            note_id,
                            deck.id,
                            content.front_doc_json,
                            content.back_doc_json,
                            content.front_text,
                            content.back_text,
                            now
                        ],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO cards (
                             id, note_id, deck_id, direction, template_ordinal,
                             created_at, updated_at
                         ) VALUES (?1, ?2, ?3, 'forward', 0, ?4, ?4)",
                        params![card_id, note_id, deck.id, now],
                    )
                    .map_err(CommandError::database)?;
                crate::scheduler::insert_new_schedule(connection, &card_id, now)?;
                Ok((note_id, card_id))
            })
            .expect("card fixture should be created");

        let deletion = database
            .with_connection(|connection| {
                crate::deletion::delete_cards(
                    connection,
                    vec![note_id.clone()],
                    "Flashcard".to_string(),
                )
            })
            .expect("card should delete");
        assert_eq!(deletion.affected_count, 1);

        database
            .with_connection(|connection| {
                let active_note: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM notes WHERE id = ?1 AND deleted_at IS NULL",
                        [note_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                let active_card: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM cards WHERE id = ?1 AND deleted_at IS NULL",
                        [card_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(active_note, 0);
                assert_eq!(active_card, 0);
                Ok(())
            })
            .expect("deletion tombstones should be readable");
    }

'''
text = text[:test_start] + replacement + text[test_end:]

for obsolete in [
    "RawCard",
    "get_card_from_connection",
    "hydrate_card",
    "raw_card_from_row",
    "get_assets_for_note",
    "AppPaths",
    "OptionalExtension",
]:
    if obsolete in text:
        raise SystemExit(f"stale card hydrator residue remains: {obsolete}")

path.write_text(text, encoding="utf-8")
print("Removed obsolete forward-card hydrator while preserving deletion coverage.")
