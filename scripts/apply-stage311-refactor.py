#!/usr/bin/env python3
"""Apply the Stage 3.11 source refactor against the exact post-PR-33 tree.

This transport helper is temporary: it performs asserted, deterministic edits
that are awkward through the repository contents API, and is deleted before the
branch is qualified for merge.
"""

from pathlib import Path
import re
import shutil

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text, encoding="utf-8")


def replace_exact(path: str, old: str, new: str, count: int = 1) -> None:
    text = read(path)
    actual = text.count(old)
    if actual != count:
        raise SystemExit(f"{path}: expected {count} exact replacement target(s), found {actual}")
    write(path, text.replace(old, new, count))


def replace_between(path: str, start: str, end: str, replacement: str) -> None:
    text = read(path)
    start_pos = text.find(start)
    if start_pos < 0:
        raise SystemExit(f"{path}: start marker not found: {start[:80]!r}")
    end_pos = text.find(end, start_pos)
    if end_pos < 0:
        raise SystemExit(f"{path}: end marker not found: {end[:80]!r}")
    write(path, text[:start_pos] + replacement + text[end_pos:])


# ---------------------------------------------------------------------------
# Strict Basic/Cloze authoring contracts and one canonical note-preparation
# model. Rust derives authoritative text from validated documents.
# ---------------------------------------------------------------------------
card_types = "src-tauri/src/authoring/card_types_and_templates.rs"
text = read(card_types)
contract_start = text.index("#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\", deny_unknown_fields)]\npub struct CreateGeneratedCardInput")
contract_end = text.index("#[derive(Clone, Debug, Deserialize, Serialize)]\n#[serde(rename_all = \"camelCase\", deny_unknown_fields)]\npub struct CardTemplateFieldInput", contract_start)
new_contracts = r'''#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ClozeMode {
    #[default]
    OneAtATime,
    AllTogether,
}

impl ClozeMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::OneAtATime => "one_at_a_time",
            Self::AllTogether => "all_together",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "cardType",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CreateGeneratedCardInput {
    Basic {
        deck_id: String,
        #[serde(default)]
        reverse_enabled: bool,
        front_doc: Value,
        back_doc: Value,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        allow_duplicate: bool,
        #[serde(default)]
        asset_draft_id: Option<String>,
    },
    Cloze {
        deck_id: String,
        cloze_source: String,
        #[serde(default)]
        cloze_mode: ClozeMode,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        allow_duplicate: bool,
        #[serde(default)]
        asset_draft_id: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "cardType",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum UpdateGeneratedCardInput {
    Basic {
        note_id: String,
        deck_id: String,
        revision: i64,
        #[serde(default)]
        reverse_enabled: bool,
        front_doc: Value,
        back_doc: Value,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        allow_duplicate: bool,
        #[serde(default)]
        asset_draft_id: Option<String>,
    },
    Cloze {
        note_id: String,
        deck_id: String,
        revision: i64,
        cloze_source: String,
        #[serde(default)]
        cloze_mode: ClozeMode,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        allow_duplicate: bool,
        #[serde(default)]
        asset_draft_id: Option<String>,
    },
}

'''
write(card_types, text[:contract_start] + new_contracts + text[contract_end:])

replace_between(
    card_types,
    "struct GeneratedNoteDraft {",
    "fn sync_generated_cards(",
    r'''enum CardNoteDraft {
    Basic {
        reverse_enabled: bool,
        front_doc: Value,
        back_doc: Value,
    },
    Cloze {
        source: String,
        mode: ClozeMode,
    },
}

struct GeneratedCardWrite {
    note_id: Option<String>,
    deck_id: String,
    revision: Option<i64>,
    draft: CardNoteDraft,
    tags: Vec<String>,
    allow_duplicate: bool,
    asset_draft_id: Option<String>,
}

fn create_generated_card_in_database(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: CreateGeneratedCardInput,
) -> Result<OrganizedCardDto, CommandError> {
    let write = match input {
        CreateGeneratedCardInput::Basic {
            deck_id,
            reverse_enabled,
            front_doc,
            back_doc,
            tags,
            allow_duplicate,
            asset_draft_id,
        } => GeneratedCardWrite {
            note_id: None,
            deck_id,
            revision: None,
            draft: CardNoteDraft::Basic {
                reverse_enabled,
                front_doc,
                back_doc,
            },
            tags,
            allow_duplicate,
            asset_draft_id,
        },
        CreateGeneratedCardInput::Cloze {
            deck_id,
            cloze_source,
            cloze_mode,
            tags,
            allow_duplicate,
            asset_draft_id,
        } => GeneratedCardWrite {
            note_id: None,
            deck_id,
            revision: None,
            draft: CardNoteDraft::Cloze {
                source: cloze_source,
                mode: cloze_mode,
            },
            tags,
            allow_duplicate,
            asset_draft_id,
        },
    };
    create_prepared_card_note(database, paths, write)
}

fn create_prepared_card_note(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    write: GeneratedCardWrite,
) -> Result<OrganizedCardDto, CommandError> {
    let GeneratedCardWrite {
        note_id: _,
        deck_id,
        revision: _,
        draft,
        tags,
        allow_duplicate,
        asset_draft_id,
    } = write;
    validate_entity_id(&deck_id, "deck")?;
    if let Some(draft_id) = asset_draft_id.as_deref() {
        validate_entity_id(draft_id, "asset draft")?;
    }
    let prepared = prepare_card_note(draft)?;
    let asset_ids = content_asset_ids(&prepared.content);
    let tags = validate_tags(tags)?;
    let normalized = normalized_pair(&prepared.content.front_text, &prepared.content.back_text);
    let hash = duplicate_hash(&normalized.0, &normalized.1);

    let note_id = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &deck_id)?;
        ensure_assets_available(&transaction, &paths, &asset_ids, asset_draft_id.as_deref())?;
        if !allow_duplicate
            && duplicate_checkable(&prepared.content.front_text, &prepared.content.back_text)
            && has_exact_duplicate(
                &transaction,
                &deck_id,
                &hash,
                &normalized.0,
                &normalized.1,
                None,
            )?
        {
            return Err(duplicate_error());
        }

        let now = unix_time_millis()?;
        let note_id = generate_id(&transaction)?;
        transaction
            .execute(
                "INSERT INTO notes (
                     id, deck_id, note_type, content_format, content_schema_version,
                     front_doc, back_doc, front_text, back_text, duplicate_hash,
                     reverse_enabled, cloze_source, cloze_mode,
                     created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, ?3, 'tiptap-json', 1,
                     ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, 1
                 )",
                params![
                    note_id,
                    deck_id,
                    prepared.note_type,
                    prepared.content.front_doc_json,
                    prepared.content.back_doc_json,
                    prepared.content.front_text,
                    prepared.content.back_text,
                    hash,
                    crate::util::bool_to_integer(prepared.reverse_enabled),
                    prepared.cloze_source,
                    prepared.cloze_mode,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        sync_generated_cards(&transaction, &note_id, &deck_id, &prepared.cards, now)?;
        replace_note_assets(&transaction, &note_id, &prepared.content)?;
        replace_note_tags(&transaction, &note_id, &tags, now)?;
        let finalization = finalize_assets_for_owner(
            &transaction,
            &paths,
            &asset_ids,
            asset_draft_id.as_deref(),
            Some(&note_id),
        )?;
        transaction.commit().map_err(CommandError::database)?;
        finalization.cleanup();
        Ok(note_id)
    })?;

    database.with_connection(|connection| hydrate_organized_card(connection, &paths, &note_id))
}

fn update_generated_card_in_database(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: UpdateGeneratedCardInput,
) -> Result<OrganizedCardDto, CommandError> {
    let write = match input {
        UpdateGeneratedCardInput::Basic {
            note_id,
            deck_id,
            revision,
            reverse_enabled,
            front_doc,
            back_doc,
            tags,
            allow_duplicate,
            asset_draft_id,
        } => GeneratedCardWrite {
            note_id: Some(note_id),
            deck_id,
            revision: Some(revision),
            draft: CardNoteDraft::Basic {
                reverse_enabled,
                front_doc,
                back_doc,
            },
            tags,
            allow_duplicate,
            asset_draft_id,
        },
        UpdateGeneratedCardInput::Cloze {
            note_id,
            deck_id,
            revision,
            cloze_source,
            cloze_mode,
            tags,
            allow_duplicate,
            asset_draft_id,
        } => GeneratedCardWrite {
            note_id: Some(note_id),
            deck_id,
            revision: Some(revision),
            draft: CardNoteDraft::Cloze {
                source: cloze_source,
                mode: cloze_mode,
            },
            tags,
            allow_duplicate,
            asset_draft_id,
        },
    };
    update_prepared_card_note(database, paths, write)
}

fn update_prepared_card_note(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    write: GeneratedCardWrite,
) -> Result<OrganizedCardDto, CommandError> {
    let GeneratedCardWrite {
        note_id,
        deck_id,
        revision,
        draft,
        tags,
        allow_duplicate,
        asset_draft_id,
    } = write;
    let note_id = note_id.ok_or_else(|| {
        CommandError::new("card_not_found", "That flashcard no longer exists.")
    })?;
    let revision = revision.unwrap_or_default();
    validate_entity_id(&note_id, "note")?;
    validate_entity_id(&deck_id, "deck")?;
    if let Some(draft_id) = asset_draft_id.as_deref() {
        validate_entity_id(draft_id, "asset draft")?;
    }
    if revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The card revision is invalid.",
        ));
    }
    let prepared = prepare_card_note(draft)?;
    let asset_ids = content_asset_ids(&prepared.content);
    let tags = validate_tags(tags)?;
    let normalized = normalized_pair(&prepared.content.front_text, &prepared.content.back_text);
    let hash = duplicate_hash(&normalized.0, &normalized.1);

    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_deck(&transaction, &deck_id)?;
        ensure_assets_available(&transaction, &paths, &asset_ids, asset_draft_id.as_deref())?;
        let existing_type: Option<String> = transaction
            .query_row(
                "SELECT note_type FROM notes WHERE id = ?1 AND deleted_at IS NULL",
                [note_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some(existing_type) = existing_type else {
            return Err(CommandError::new(
                "card_not_found",
                "That flashcard no longer exists.",
            ));
        };
        if display_card_type(&existing_type) != prepared.note_type {
            return Err(CommandError::new(
                "card_type_locked",
                "A saved card cannot be converted between Basic and Fill in the blank. Create a new card instead.",
            ));
        }
        if !allow_duplicate
            && duplicate_checkable(&prepared.content.front_text, &prepared.content.back_text)
            && has_exact_duplicate(
                &transaction,
                &deck_id,
                &hash,
                &normalized.0,
                &normalized.1,
                Some(&note_id),
            )?
        {
            return Err(duplicate_error());
        }

        let now = unix_time_millis()?;
        let changed = transaction
            .execute(
                "UPDATE notes
                 SET deck_id = ?1, note_type = ?2,
                     front_doc = ?3, back_doc = ?4,
                     front_text = ?5, back_text = ?6, duplicate_hash = ?7,
                     reverse_enabled = ?8, cloze_source = ?9, cloze_mode = ?10,
                     updated_at = ?11, revision = revision + 1
                 WHERE id = ?12 AND revision = ?13 AND deleted_at IS NULL",
                params![
                    deck_id,
                    prepared.note_type,
                    prepared.content.front_doc_json,
                    prepared.content.back_doc_json,
                    prepared.content.front_text,
                    prepared.content.back_text,
                    hash,
                    crate::util::bool_to_integer(prepared.reverse_enabled),
                    prepared.cloze_source,
                    prepared.cloze_mode,
                    now,
                    note_id,
                    revision,
                ],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "card_changed",
                "This card changed in another window. Reload it and try again.",
            ));
        }
        sync_generated_cards(&transaction, &note_id, &deck_id, &prepared.cards, now)?;
        replace_note_assets(&transaction, &note_id, &prepared.content)?;
        replace_note_tags(&transaction, &note_id, &tags, now)?;
        let finalization = finalize_assets_for_owner(
            &transaction,
            &paths,
            &asset_ids,
            asset_draft_id.as_deref(),
            Some(&note_id),
        )?;
        transaction.commit().map_err(CommandError::database)?;
        finalization.cleanup();
        Ok(())
    })?;

    database.with_connection(|connection| hydrate_organized_card(connection, &paths, &note_id))
}

fn prepare_basic_card_content(front_doc: Value, back_doc: Value) -> Result<ValidatedContent, CommandError> {
    validate_content(front_doc, back_doc, String::new(), String::new())
}

fn prepare_card_note(input: CardNoteDraft) -> Result<PreparedGeneratedNote, CommandError> {
    match input {
        CardNoteDraft::Basic {
            reverse_enabled,
            front_doc,
            back_doc,
        } => {
            let content = prepare_basic_card_content(front_doc, back_doc)?;
            let mut cards = vec![GeneratedCardSpec {
                direction: "forward",
                template_ordinal: 0,
            }];
            if reverse_enabled {
                cards.push(GeneratedCardSpec {
                    direction: "reverse",
                    template_ordinal: 0,
                });
            }
            Ok(PreparedGeneratedNote {
                note_type: "basic",
                reverse_enabled,
                cloze_source: String::new(),
                cloze_mode: ClozeMode::OneAtATime.as_str().to_string(),
                content,
                cards,
            })
        }
        CardNoteDraft::Cloze { source, mode } => {
            let parsed = parse_cloze(&source)?;
            let target = if mode == ClozeMode::AllTogether {
                None
            } else {
                parsed.groups.first().copied()
            };
            let question = render_cloze_question(&parsed, target);
            let answer = render_cloze_answer(&parsed, target);
            let content = validate_content(
                plain_text_document(&question),
                plain_text_document(&answer),
                String::new(),
                String::new(),
            )?;
            Ok(PreparedGeneratedNote {
                note_type: "cloze",
                reverse_enabled: false,
                cloze_source: source,
                cloze_mode: mode.as_str().to_string(),
                content,
                cards: vec![GeneratedCardSpec {
                    direction: "forward",
                    template_ordinal: 0,
                }],
            })
        }
    }
}

''',
)

# Remove the now-obsolete string-only cloze write validator/default.
text = read(card_types)
text = re.sub(
    r'fn default_stage3_cloze_mode\(\) -> String \{\n    "one_at_a_time"\.to_string\(\)\n\}\n\n',
    '',
    text,
    count=1,
)
text = re.sub(
    r'fn validate_cloze_mode\(value: &str\) -> Result<\(\), CommandError> \{.*?\n\}\n\n(?=fn parse_cloze)',
    '',
    text,
    count=1,
    flags=re.S,
)
write(card_types, text)

# One template update path only: input IDs are validated directly rather than
# persisted as fake pending children.
replace_exact(
    card_types,
    '''        let has_new_children = template_update_contains_new_children(connection, &input)?;\n        let updated = if has_new_children {\n            update_card_template_with_stable_children_in_connection(connection, &paths, input)?\n        } else {\n            update_card_template_in_connection(connection, &paths, input)?\n        };''',
    '''        let updated = update_card_template_in_connection(connection, &paths, input)?;''',
)
replace_exact(
    card_types,
    '''    prepare_automatic_create_template_input, prepare_automatic_update_template_input,\n    render_automatic_template_previews, template_update_contains_new_children,\n    update_card_template_in_connection, update_card_template_with_stable_children_in_connection,\n};''',
    '''    prepare_automatic_create_template_input, prepare_automatic_update_template_input,\n    render_automatic_template_previews, update_card_template_in_connection,\n};''',
)
replace_exact(
    card_types,
    '''#[path = "generated_import_module.rs"]\npub(crate) mod generated_import;''',
    '''pub(crate) mod generated_import;''',
)

# Strict contract tests live next to the Rust transport definitions.
with_tests = read(card_types) + r'''

#[cfg(test)]
mod authoring_contract_tests {
    use super::*;

    fn paragraph(text: &str) -> Value {
        json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [{"type": "text", "text": text}]}]
        })
    }

    #[test]
    fn generated_card_contracts_reject_type_inappropriate_fields() {
        let basic = serde_json::json!({
            "cardType": "basic",
            "deckId": "11111111111111111111111111111111",
            "frontDoc": paragraph("Question"),
            "backDoc": paragraph("Answer"),
            "clozeSource": "should not be accepted"
        });
        assert!(serde_json::from_value::<CreateGeneratedCardInput>(basic).is_err());

        let cloze = serde_json::json!({
            "cardType": "cloze",
            "deckId": "11111111111111111111111111111111",
            "clozeSource": "The {{c1::heart}} pumps blood.",
            "frontDoc": paragraph("should not be accepted")
        });
        assert!(serde_json::from_value::<CreateGeneratedCardInput>(cloze).is_err());
    }

    #[test]
    fn basic_contract_does_not_require_browser_plain_text_snapshots() {
        let input = serde_json::json!({
            "cardType": "basic",
            "deckId": "11111111111111111111111111111111",
            "frontDoc": paragraph("Question"),
            "backDoc": paragraph("Answer")
        });
        assert!(serde_json::from_value::<CreateGeneratedCardInput>(input).is_ok());
    }
}
'''
write(card_types, with_tests)

# ---------------------------------------------------------------------------
# Template graph update cleanup.
# ---------------------------------------------------------------------------
templates = "src-tauri/src/authoring/card_types_and_templates/templates.rs"
replace_exact(templates, "        (true, None) => generate_id(connection),", "        (true, None) => Ok(id),")
replace_exact(templates, "mod stable_update;\n", "")
replace_exact(
    templates,
    '''    prepare_automatic_create_template_input, prepare_automatic_update_template_input,\n    render_automatic_template_previews, template_update_contains_new_children,\n};''',
    '''    prepare_automatic_create_template_input, prepare_automatic_update_template_input,\n    render_automatic_template_previews,\n};''',
)
replace_exact(templates, "pub(super) use stable_update::update_card_template_with_stable_children_in_connection;\n", "")

# Prove that an unused client-generated update ID is accepted without any row
# being inserted during validation.
replace_exact(
    templates,
    '''        assert!(summaries.is_empty());\n    }\n}\n\nmod automatic_layout;''',
    '''        assert!(summaries.is_empty());\n    }\n\n    #[test]\n    fn unused_update_child_id_is_preserved_without_pending_rows() {\n        let database = Database::open_in_memory().expect("database should open");\n        database\n            .with_connection(|connection| {\n                let child_id = "22222222222222222222222222222222".to_string();\n                let resolved = resolve_template_child_id(\n                    connection,\n                    "card_template_fields",\n                    "11111111111111111111111111111111",\n                    Some(child_id.clone()),\n                    true,\n                    "template field",\n                )?;\n                assert_eq!(resolved, child_id);\n                let count: i64 = connection\n                    .query_row(\n                        "SELECT COUNT(*) FROM card_template_fields WHERE id = ?1",\n                        [resolved],\n                        |row| row.get(0),\n                    )\n                    .map_err(CommandError::database)?;\n                assert_eq!(count, 0);\n                Ok(())\n            })\n            .expect("unused IDs should validate without persistence");\n    }\n}\n\nmod automatic_layout;''',
)

# Remove the update-probing helper that only selected the pending-row wrapper.
auto = "src-tauri/src/authoring/card_types_and_templates/templates/automatic_layout.rs"
text = read(auto)
start = text.find("pub(in super::super) fn template_update_contains_new_children(")
if start < 0:
    raise SystemExit("automatic_layout.rs: template update probe not found")
end = text.find("fn template_value_placement(", start)
if end < 0:
    raise SystemExit("automatic_layout.rs: template value marker not found")
write(auto, text[:start] + text[end:])

stable = ROOT / "src-tauri/src/authoring/card_types_and_templates/templates/stable_update.rs"
if not stable.exists():
    raise SystemExit("stable_update.rs missing")
stable.unlink()

# ---------------------------------------------------------------------------
# Generated import becomes an ordinary module with schema/Markdown/prepare/apply
# responsibilities. The v1 external envelope and one-step UX remain unchanged.
# ---------------------------------------------------------------------------
old_import = ROOT / "src-tauri/src/authoring/generated_import.rs"
wrapper = ROOT / "src-tauri/src/authoring/generated_import_module.rs"
new_dir = ROOT / "src-tauri/src/authoring/card_types_and_templates/generated_import"
new_dir.mkdir(parents=True, exist_ok=True)
new_mod = new_dir / "mod.rs"
if new_mod.exists():
    raise SystemExit("generated import module already moved")
shutil.move(str(old_import), str(new_mod))
if not wrapper.exists():
    raise SystemExit("generated import wrapper missing")
wrapper.unlink()

mod_text = new_mod.read_text(encoding="utf-8")

# External schema types.
schema_start = mod_text.index("#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]")
schema_end = mod_text.index("#[derive(Clone, Debug, Serialize)]\n#[serde(rename_all = \"camelCase\")]\npub struct FlashcardImportResultDto", schema_start)
schema_text = r'''use serde::Deserialize;

use super::super::ClozeMode;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum GeneratedImportDeckMode {
    Create,
    Append,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct GeneratedImportFile {
    pub(super) format: String,
    pub(super) format_version: i64,
    pub(super) deck: GeneratedImportDeck,
    #[serde(default)]
    pub(super) default_tags: Vec<String>,
    pub(super) cards: Vec<GeneratedImportCard>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GeneratedImportDeck {
    pub(super) mode: GeneratedImportDeckMode,
    pub(super) name: String,
    #[serde(default)]
    pub(super) description: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum GeneratedImportCard {
    Basic {
        front: String,
        back: String,
        #[serde(default, rename = "reverseEnabled")]
        reverse_enabled: bool,
        #[serde(default)]
        tags: Vec<String>,
    },
    Cloze {
        text: String,
        #[serde(default)]
        mode: ClozeMode,
        #[serde(default)]
        tags: Vec<String>,
    },
}
'''
(new_dir / "schema.rs").write_text(schema_text, encoding="utf-8")
mod_text = mod_text[:schema_start] + "mod schema;\nuse schema::{GeneratedImportCard, GeneratedImportDeckMode, GeneratedImportFile};\n\n" + mod_text[schema_end:]

# Restricted Markdown parser.
markdown_start = mod_text.index("fn markdown_document(")
markdown_end = mod_text.index("#[cfg(test)]\nmod generated_import_tests", markdown_start)
markdown_body = mod_text[markdown_start:markdown_end]
markdown_body = markdown_body.replace("fn markdown_document(", "pub(super) fn markdown_document(", 1)
old_html = '''    if source.contains('<') || source.contains('>') && source.contains("</") {\n        return Err(unsupported_markdown(path, "raw HTML is not supported"));\n    }'''
if old_html not in markdown_body:
    raise SystemExit("generated import Markdown HTML heuristic not found")
markdown_body = markdown_body.replace(
    old_html,
    '''    if contains_raw_html(source) {\n        return Err(unsupported_markdown(path, "raw HTML is not supported"));\n    }''',
    1,
)
markdown_body += r'''
fn contains_raw_html(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut index = 0_usize;
    while index < bytes.len() {
        if bytes[index] != b'<' {
            index += 1;
            continue;
        }
        let Some(next) = bytes.get(index + 1).copied() else {
            return false;
        };
        if (next.is_ascii_alphabetic() || matches!(next, b'/' | b'!' | b'?'))
            && source[index + 1..].contains('>')
        {
            return true;
        }
        index += 1;
    }
    false
}
'''
(new_dir / "markdown.rs").write_text(
    "use serde_json::{json, Value};\n\nuse crate::error::CommandError;\n\n" + markdown_body,
    encoding="utf-8",
)
mod_text = mod_text[:markdown_start] + "mod markdown;\nuse markdown::markdown_document;\n\n" + mod_text[markdown_end:]

# Preparation: read/validate the entire file, build canonical note plans, and
# produce bounded semantic diagnostics before any database transaction.
prepare_start = mod_text.index("fn prepare_flashcard_import(")
apply_start = mod_text.index("fn import_prepared_flashcards(", prepare_start)
prepare_head = mod_text[prepare_start:apply_start]
prepare_card_start = mod_text.index("fn prepare_import_card(", apply_start)
insert_deck_start = mod_text.index("fn insert_generated_import_deck(", prepare_card_start)
prepare_tail = mod_text[prepare_card_start:insert_deck_start]
prepare_source = "use super::*;\nuse super::markdown::markdown_document;\n\n" + prepare_head + prepare_tail
prepare_source = prepare_source.replace("fn prepare_flashcard_import(", "pub(super) fn prepare_flashcard_import(", 1)
prepare_source = prepare_source.replace("fn prepare_flashcard_import_bytes(", "pub(super) fn prepare_flashcard_import_bytes(", 1)
(new_dir / "prepare.rs").write_text(prepare_source, encoding="utf-8")

# Application: destination resolution, duplicate decisions, and one write tx.
apply_end = prepare_card_start
apply_head = mod_text[apply_start:apply_end]
insert_deck_end = mod_text.index("mod markdown;", insert_deck_start)
apply_tail = mod_text[insert_deck_start:insert_deck_end]
apply_source = "use super::*;\n\n" + apply_head + apply_tail
apply_source = apply_source.replace("fn import_prepared_flashcards(", "pub(super) fn import_prepared_flashcards(", 1)
(new_dir / "apply.rs").write_text(apply_source, encoding="utf-8")

# Remove moved regions from mod.rs, keeping command/result/shared plan/test code.
segments = [
    (prepare_start, apply_start),
    (apply_start, apply_end),
    (prepare_card_start, insert_deck_start),
    (insert_deck_start, insert_deck_end),
]
for start, end in sorted(segments, reverse=True):
    mod_text = mod_text[:start] + mod_text[end:]

command_marker = "#[tauri::command]\npub fn import_generated_flashcards("
cmd_pos = mod_text.index(command_marker)
insert = '''mod apply;\nmod prepare;\n\nuse apply::import_prepared_flashcards;\nuse prepare::prepare_flashcard_import;\n#[cfg(test)]\nuse prepare::prepare_flashcard_import_bytes;\n\n'''
mod_text = mod_text[:cmd_pos] + insert + mod_text[cmd_pos:]

# Map external JSON variants through the same canonical card-note preparation
# boundary as manual editor writes.
mod_text = mod_text.replace(
    '''                GeneratedNoteDraft {\n                    card_type: "basic".to_string(),\n                    reverse_enabled,\n                    cloze_source: String::new(),\n                    cloze_mode: "one_at_a_time".to_string(),\n                    front_doc,\n                    back_doc,\n                    front_text: String::new(),\n                    back_text: String::new(),\n                },''',
    '''                CardNoteDraft::Basic {\n                    reverse_enabled,\n                    front_doc,\n                    back_doc,\n                },''',
)
mod_text = mod_text.replace(
    '''        GeneratedImportCard::Cloze { text, mode, tags } => (\n            GeneratedNoteDraft {\n                card_type: "cloze".to_string(),\n                reverse_enabled: false,\n                cloze_source: text,\n                cloze_mode: mode,\n                front_doc: json!({"type": "doc", "content": []}),\n                back_doc: json!({"type": "doc", "content": []}),\n                front_text: String::new(),\n                back_text: String::new(),\n            },\n            tags,\n        ),''',
    '''        GeneratedImportCard::Cloze { text, mode, tags } => (\n            CardNoteDraft::Cloze { source: text, mode },\n            tags,\n        ),''',
)
mod_text = mod_text.replace("prepare_generated_note(draft)", "prepare_card_note(draft)")

# Parser regression: comparison characters are ordinary text, actual HTML still fails.
test_marker = '''    #[test]\n    fn markdown_rejects_html_images_and_tables() {'''
comparison_test = r'''    #[test]
    fn markdown_allows_literal_comparison_angle_brackets() {
        let document = markdown_document("2 < 3 and 5 > 4", "front")
            .expect("comparison characters are ordinary text");
        crate::cards::validate_document(&document).expect("document should validate");
    }

'''
if test_marker not in mod_text:
    raise SystemExit("generated import test marker not found")
mod_text = mod_text.replace(test_marker, comparison_test + test_marker, 1)
new_mod.write_text(mod_text, encoding="utf-8")

# ---------------------------------------------------------------------------
# Remove obsolete Basic-only authoring write IPC; generated-card authoring owns
# Basic and Cloze writes now.
# ---------------------------------------------------------------------------
authoring_root = "src-tauri/src/authoring/mod.rs"
root_text = read(authoring_root)
start = root_text.index("#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\", deny_unknown_fields)]\npub struct CreateOrganizedCardInput")
end = root_text.index("#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\", deny_unknown_fields)]\npub struct DuplicateCheckInput", start)
write(authoring_root, root_text[:start] + root_text[end:])

commands = "src-tauri/src/authoring/card_commands.rs"
cmd_text = read(commands)
start = cmd_text.index("#[tauri::command]\npub fn create_organized_card(")
end = cmd_text.index("#[tauri::command]\npub fn find_duplicate_cards(", start)
write(commands, cmd_text[:start] + cmd_text[end:])

lib = "src-tauri/src/lib.rs"
replace_exact(lib, "            authoring::card_commands::create_organized_card,\n", "")
replace_exact(lib, "            authoring::card_commands::update_organized_card,\n", "")
replace_exact(
    lib,
    "            authoring::card_types_and_templates::generated_import::implementation::import_generated_flashcards,",
    "            authoring::card_types_and_templates::generated_import::import_generated_flashcards,",
)

ipc = "src/ipc.ts"
ipc_text = read(ipc)
start = ipc_text.index("export interface CreateOrganizedCardInput")
end = ipc_text.index("export interface CardTypeInfo", start)
ipc_text = ipc_text[:start] + ipc_text[end:]
start = ipc_text.index("export interface CreateGeneratedCardInput")
end = ipc_text.index("export interface CardTemplateField", start)
ipc_text = ipc_text[:start] + r'''interface GeneratedCardWriteCommon {
  deckId: string;
  tags: string[];
  allowDuplicate: boolean;
  assetDraftId?: string;
}

export type CreateGeneratedCardInput =
  | (GeneratedCardWriteCommon & {
      cardType: "basic";
      reverseEnabled: boolean;
      frontDoc: RichDocument;
      backDoc: RichDocument;
    })
  | (GeneratedCardWriteCommon & {
      cardType: "cloze";
      clozeSource: string;
      clozeMode: ClozeMode;
    });

export type UpdateGeneratedCardInput = CreateGeneratedCardInput & {
  noteId: string;
  revision: number;
};

''' + ipc_text[end:]
start = ipc_text.index("export async function createOrganizedCard(")
end = ipc_text.index("export async function getCardTypeInfo(", start)
ipc_text = ipc_text[:start] + ipc_text[end:]
write(ipc, ipc_text)

editor = "src/screens/CardEditorScreen.tsx"
editor_text = read(editor)
start = editor_text.index("      const common = {")
end = editor_text.index("\n\n      markAssetDraftCommitted();", start)
new_save = r'''      let saved: OrganizedCard;
      if (builtInCardType === "cloze") {
        const input = {
          deckId: selectedDeckId,
          cardType: "cloze" as const,
          clozeSource,
          clozeMode,
          tags,
          allowDuplicate,
          assetDraftId: assetDraftId ?? undefined,
        };
        saved =
          existing && card
            ? await updateGeneratedCard({
                ...input,
                noteId: card.noteId,
                revision: card.revision,
              })
            : await createGeneratedCard(input);
      } else {
        const input = {
          deckId: selectedDeckId,
          cardType: "basic" as const,
          reverseEnabled,
          frontDoc,
          backDoc,
          tags,
          allowDuplicate,
          assetDraftId: assetDraftId ?? undefined,
        };
        saved =
          existing && card
            ? await updateGeneratedCard({
                ...input,
                noteId: card.noteId,
                revision: card.revision,
              })
            : await createGeneratedCard(input);
      }'''
write(editor, editor_text[:start] + new_save + editor_text[end:])

# Mark superseded IPC names as intentionally absent.
verify = "scripts/verify-ipc-contract.mjs"
replace_exact(
    verify,
    '''  "update_card",\n  "start_practice",''',
    '''  "update_card",\n  "create_organized_card",\n  "update_organized_card",\n  "start_practice",''',
)

print("Applied Stage 3.11 template/import/authoring contract refactor.")
