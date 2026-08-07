#!/usr/bin/env python3
"""Apply the one-time Stage 3.10 module-boundary compiler fix.

This script is intentionally temporary. It makes deterministic, asserted text
replacements against the exact draft-PR source tree, then the workflow runs
rustfmt before committing the result. The script is removed once the corrected
source has been committed and qualified.
"""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_exact(path: str, old: str, new: str, count: int = 1) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    actual = text.count(old)
    if actual != count:
        raise SystemExit(
            f"{path}: expected {count} occurrence(s) of replacement target, found {actual}"
        )
    target.write_text(text.replace(old, new, count), encoding="utf-8")


replace_exact(
    "src-tauri/src/authoring/mod.rs",
    """mod card_commands;\nmod card_types_and_templates;\nmod transfer;\n\n#[cfg(test)]\nmod history_tests;\n#[cfg(test)]\nmod tests;\n\npub(crate) use card_commands::{\n    analyze_bulk_cards, backfill_duplicate_hashes_in_connection, bulk_create_cards, bulk_tag_cards,\n    create_organized_card, find_duplicate_cards, get_organized_card, list_matching_cards,\n    list_tags, search_cards, update_organized_card,\n};\npub(crate) use card_types_and_templates::{\n    apply_stage3_data_repairs, create_card_template, create_generated_card, delete_card_template,\n    generate_cards_from_template, get_card_template, get_card_type_info,\n    import_generated_flashcards, list_card_template_summaries, preview_template_cards,\n    projected_presentation_variant, render_projected_presentation, update_card_template,\n    update_generated_card, ProjectedPresentationInput, RenderedCardPresentation,\n};\npub(crate) use transfer::transfer_cards;""",
    """pub(crate) mod card_commands;\npub(crate) mod card_types_and_templates;\npub(crate) mod transfer;\n\n#[cfg(test)]\nmod history_tests;\n#[cfg(test)]\nmod tests;\n\npub(crate) use card_commands::backfill_duplicate_hashes_in_connection;\npub(crate) use card_types_and_templates::{\n    apply_stage3_data_repairs, projected_presentation_variant, render_projected_presentation,\n    ProjectedPresentationInput,\n};""",
)

replace_exact(
    "src-tauri/src/authoring/card_types_and_templates.rs",
    """mod templates;\npub(super) use templates::{\n    __cmd__list_card_template_summaries, apply_stage3_data_repairs, list_card_template_summaries,\n    CardTemplateSummaryDto,\n};\nuse templates::{\n    create_card_template_in_connection, delete_card_template_in_connection,\n    ensure_template_revision, generate_automatic_template_cards_in_connection,\n    load_stored_template, normalize_card_template_dto_for_editing,\n    prepare_automatic_create_template_input, prepare_automatic_update_template_input,\n    render_automatic_template_previews, template_update_contains_new_children,\n    update_card_template_in_connection, update_card_template_with_stable_children_in_connection,\n};\n\n#[path = \"generated_import_module.rs\"]\nmod generated_import;\npub(super) use generated_import::{\n    __cmd__import_generated_flashcards, import_generated_flashcards, FlashcardImportResultDto,\n};""",
    """pub(crate) mod templates;\npub(crate) use templates::apply_stage3_data_repairs;\nuse templates::{\n    create_card_template_in_connection, delete_card_template_in_connection,\n    ensure_template_revision, generate_automatic_template_cards_in_connection,\n    load_stored_template, normalize_card_template_dto_for_editing,\n    prepare_automatic_create_template_input, prepare_automatic_update_template_input,\n    render_automatic_template_previews, template_update_contains_new_children,\n    update_card_template_in_connection, update_card_template_with_stable_children_in_connection,\n};\n\n#[path = \"generated_import_module.rs\"]\npub(crate) mod generated_import;""",
)

replace_exact(
    "src-tauri/src/authoring/card_types_and_templates/templates.rs",
    "pub(super) use repair::apply_stage3_data_repairs;",
    "pub(crate) use repair::apply_stage3_data_repairs;",
)

replace_exact(
    "src-tauri/src/authoring/generated_import_module.rs",
    """use super::*;\n\n#[rustfmt::skip]\n#[path = \"generated_import.rs\"]\nmod implementation;\n\npub(super) use implementation::*;\n""",
    """use super::*;\n\n#[rustfmt::skip]\n#[path = \"generated_import.rs\"]\npub(crate) mod implementation;\n""",
)

lib_path = ROOT / "src-tauri/src/lib.rs"
lib_text = lib_path.read_text(encoding="utf-8")
command_paths = {
    "authoring::list_tags": "authoring::card_commands::list_tags",
    "authoring::search_cards": "authoring::card_commands::search_cards",
    "authoring::list_matching_cards": "authoring::card_commands::list_matching_cards",
    "authoring::get_organized_card": "authoring::card_commands::get_organized_card",
    "authoring::create_organized_card": "authoring::card_commands::create_organized_card",
    "authoring::update_organized_card": "authoring::card_commands::update_organized_card",
    "authoring::find_duplicate_cards": "authoring::card_commands::find_duplicate_cards",
    "authoring::analyze_bulk_cards": "authoring::card_commands::analyze_bulk_cards",
    "authoring::bulk_create_cards": "authoring::card_commands::bulk_create_cards",
    "authoring::transfer_cards": "authoring::transfer::transfer_cards",
    "authoring::bulk_tag_cards": "authoring::card_commands::bulk_tag_cards",
    "authoring::get_card_type_info": "authoring::card_types_and_templates::get_card_type_info",
    "authoring::create_generated_card": "authoring::card_types_and_templates::create_generated_card",
    "authoring::update_generated_card": "authoring::card_types_and_templates::update_generated_card",
    "authoring::list_card_template_summaries": "authoring::card_types_and_templates::templates::list_card_template_summaries",
    "authoring::get_card_template": "authoring::card_types_and_templates::get_card_template",
    "authoring::create_card_template": "authoring::card_types_and_templates::create_card_template",
    "authoring::update_card_template": "authoring::card_types_and_templates::update_card_template",
    "authoring::delete_card_template": "authoring::card_types_and_templates::delete_card_template",
    "authoring::preview_template_cards": "authoring::card_types_and_templates::preview_template_cards",
    "authoring::generate_cards_from_template": "authoring::card_types_and_templates::generate_cards_from_template",
    "authoring::import_generated_flashcards": "authoring::card_types_and_templates::generated_import::implementation::import_generated_flashcards",
}
for old, new in command_paths.items():
    actual = lib_text.count(old)
    if actual != 1:
        raise SystemExit(f"src-tauri/src/lib.rs: expected one {old}, found {actual}")
    lib_text = lib_text.replace(old, new, 1)
lib_path.write_text(lib_text, encoding="utf-8")

replace_exact(
    "src-tauri/src/authoring/generated_import.rs",
    """        let error = prepare_flashcard_import_bytes(invalid)\n            .expect_err(\"invalid cards should be reported together\");""",
    """        let error = match prepare_flashcard_import_bytes(invalid) {\n            Ok(_) => panic!(\"invalid cards should be reported together\"),\n            Err(error) => error,\n        };""",
)
replace_exact(
    "src-tauri/src/authoring/generated_import.rs",
    """        let error = prepare_flashcard_import_bytes(invalid.as_bytes())\n            .expect_err(\"invalid cards should be summarized\");""",
    """        let error = match prepare_flashcard_import_bytes(invalid.as_bytes()) {\n            Ok(_) => panic!(\"invalid cards should be summarized\"),\n            Err(error) => error,\n        };""",
)

print("Applied Stage 3.10 module-boundary fixes.")
