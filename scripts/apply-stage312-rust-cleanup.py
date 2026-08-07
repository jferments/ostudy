#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace(path: str, old: str, new: str, expected: int = 1) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{path}: expected {expected} occurrence(s), found {count}: {old[:100]!r}")
    target.write_text(text.replace(old, new), encoding="utf-8")


replace(
    "src-tauri/src/projections/practice.rs",
    "            card: CardPresentationDto {",
    "            presentation: CardPresentationDto {",
    expected=2,
)

replace(
    "src-tauri/src/practice_selection/operation_tests.rs",
    "replay.first_window.cards[0].card.id",
    "replay.first_window.cards[0].presentation.id",
)

replace(
    "src-tauri/src/authoring/tests.rs",
    '''    let organized = database
        .with_connection(|connection| hydrate_organized_card(connection, &paths, &note_id))
        .expect("card should hydrate");
    assert_eq!(organized.tags, vec!["Anticoagulants"]);''',
    '''    let note = database
        .with_connection(|connection| hydrate_card_note(connection, &paths, &note_id))
        .expect("card note should hydrate");
    let tags = match note {
        CardNoteDto::Basic { tags, .. } | CardNoteDto::Cloze { tags, .. } => tags,
    };
    assert_eq!(tags, vec!["Anticoagulants"]);''',
)

path = ROOT / "src-tauri/src/authoring/card_types_and_templates/generated_import/conformance_tests.rs"
text = path.read_text(encoding="utf-8")
replacements = [
    (
        '''    let utf8_error = prepare_flashcard_import(path_text(&invalid_utf8))
        .expect_err("invalid UTF-8 should be rejected");''',
        '''    let utf8_error = match prepare_flashcard_import(path_text(&invalid_utf8)) {
        Ok(_) => panic!("invalid UTF-8 should be rejected"),
        Err(error) => error,
    };''',
    ),
    (
        '''    let too_many = prepare_flashcard_import(path_text(&too_many_path))
        .expect_err("maximum plus one source note should fail");''',
        '''    let too_many = match prepare_flashcard_import(path_text(&too_many_path)) {
        Ok(_) => panic!("maximum plus one source note should fail"),
        Err(error) => error,
    };''',
    ),
    (
        '''    let too_large = prepare_flashcard_import(path_text(&over_size))
        .expect_err("maximum file bytes plus one should fail");''',
        '''    let too_large = match prepare_flashcard_import(path_text(&over_size)) {
        Ok(_) => panic!("maximum file bytes plus one should fail"),
        Err(error) => error,
    };''',
    ),
]
for old, new in replacements:
    if text.count(old) != 1:
        raise SystemExit(f"conformance_tests.rs: expected error assertion once: {old[:80]!r}")
    text = text.replace(old, new, 1)
path.write_text(text, encoding="utf-8")

print("Applied Stage 3.12 Rust migration cleanup.")
