#!/usr/bin/env python3
# Apply cross-module Stage 3.6-3.8 integration on the clean branch.
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]

def read(path):
    return (ROOT / path).read_text()

def write(path, content):
    (ROOT / path).write_text(content)

def replace_once(content, old, new, label):
    count = content.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return content.replace(old, new, 1)

# Exact time-zone dependencies.
path = "src-tauri/Cargo.toml"
content = read(path)
if 'chrono-tz = "=0.10.4"' not in content:
    content = replace_once(
        content,
        'chrono = { version = "=0.4.45", features = ["serde"] }\n',
        'chrono = { version = "=0.4.45", features = ["serde"] }\n'
        'chrono-tz = "=0.10.4"\n'
        'iana-time-zone = "=0.1.65"\n',
        path,
    )
write(path, content)

# Schema/repair ordering.
path = "src-tauri/src/database.rs"
content = read(path)
content = replace_once(
    content,
    "    data_repairs::{apply_required_repairs, verify_repair_ledger},\n",
    "    data_repairs::{\n"
    "        apply_required_repairs_after, apply_required_repairs_through,\n"
    "        verify_repair_ledger,\n"
    "    },\n",
    path,
)
migration14 = '''    Migration {
        version: 14,
        name: "review_attribution_and_projection_indexes",
        checksum: "0d9630b33ca42fd7813172a6fba780555fb8246c375dfadfb918836655bc21bd",
        sql: include_str!("../migrations/0014_review_attribution_and_projection_indexes.sql"),
    },
'''
old_tail = '''    Migration {
        version: 13,
        name: "canonical_data_model",
        checksum: "2e56fa0b68049f8a4d2e937b1dbdb18ec45d9c9041b06f5994b3c847f1d2f049",
        sql: include_str!("../migrations/0013_canonical_data_model.sql"),
    },
];

pub(crate) const LATEST_SCHEMA_VERSION: i64 = 13;'''
new_tail = old_tail.replace("];", migration14 + "];").replace(
    "LATEST_SCHEMA_VERSION: i64 = 13", "LATEST_SCHEMA_VERSION: i64 = 14"
)
content = replace_once(content, old_tail, new_tail, path)
content = replace_once(
    content,
    "    apply_migrations_through(connection, 12)?;\n"
    "    apply_required_repairs(connection)?;\n"
    "    apply_migrations_after(connection, 12)?;\n"
    "    verify_repair_ledger(connection)?;",
    "    apply_migrations_through(connection, 12)?;\n"
    "    apply_required_repairs_through(connection, 2)?;\n"
    "    apply_migrations_after(connection, 12)?;\n"
    "    apply_required_repairs_after(connection, 2)?;\n"
    "    verify_repair_ledger(connection)?;",
    path,
)
write(path, content)

# Composition root and bounded commands.
path = "src-tauri/src/lib.rs"
content = read(path)
for anchor, module in [
    ("mod cards;\n", "mod card_projections;\n"),
    ("mod decks;\n", "mod historical_attribution;\n"),
    ("mod practice_selection;\n", "mod projections;\n"),
    ("mod settings;\n", "mod study_calendar;\nmod studyability;\n"),
    ("mod studydeck;\n", "mod template_projections;\n"),
]:
    if module not in content:
        content = replace_once(content, anchor, anchor + module, path)
content = replace_once(
    content,
    "            decks::list_decks,\n"
    "            decks::get_deck,\n"
    "            decks::create_deck,\n"
    "            decks::update_deck,\n",
    "            decks::list_decks,\n"
    "            decks::list_archived_decks,\n"
    "            decks::get_deck,\n"
    "            decks::create_deck,\n"
    "            decks::update_deck,\n"
    "            decks::archive_deck,\n"
    "            decks::unarchive_deck,\n",
    path,
)
content = content.replace("            cards::list_cards,\n", "")
content = replace_once(
    content,
    "            deletion_commands::delete_card_with_undo,\n",
    "            card_projections::list_deck_cards,\n"
    "            deletion_commands::delete_card_with_undo,\n",
    path,
)
content = content.replace("            authoring::list_card_templates,\n", "")
content = replace_once(
    content,
    "            authoring::get_card_template,\n",
    "            template_projections::list_card_template_summaries,\n"
    "            authoring::get_card_template,\n",
    path,
)
content = replace_once(
    content,
    "            practice_selection::start_card_practice,\n",
    "            practice_selection::start_card_practice,\n"
    "            practice_selection::get_practice_card_window,\n",
    path,
)
write(path, content)

# Explicit count units and availability/local-date filters.
path = "src-tauri/src/authoring/types.rs"
content = read(path)
content = replace_once(
    content,
    "pub struct TagDto {\n"
    "    pub id: String,\n"
    "    pub name: String,\n"
    "    pub card_count: i64,\n"
    "}",
    "pub struct TagDto {\n"
    "    pub id: String,\n"
    "    pub name: String,\n"
    "    pub source_note_count: i64,\n"
    "    pub generated_card_count: i64,\n"
    "    pub studyable_due_count: i64,\n"
    "}",
    path,
)
content = replace_once(
    content,
    '    #[serde(default = "default_due_filter")]\n'
    "    pub due_filter: String,\n",
    '    #[serde(default = "default_availability_filter")]\n'
    "    pub availability_filter: String,\n",
    path,
)
write(path, content)

path = "src-tauri/src/authoring/normalization.rs"
content = read(path)
content = replace_once(
    content,
    'fn default_due_filter() -> String {\n    "all".to_string()\n}',
    'fn default_availability_filter() -> String {\n    "all".to_string()\n}',
    path,
)
write(path, content)

# Pure presentation projection shared by bounded readers.
path = "src-tauri/src/authoring/stage3_v2/types_commands.rs"
content = read(path)
marker = '''pub(crate) struct RenderedCardPresentation {
    pub(crate) card_type: String,
    pub(crate) card_label: String,
    pub(crate) front_doc: Value,
    pub(crate) back_doc: Value,
    pub(crate) front_text: String,
    pub(crate) back_text: String,
}
'''
projected_types = '''#[derive(Clone, Debug)]
pub(crate) struct ProjectedPresentationInput {
    pub(crate) direction: String,
    pub(crate) template_ordinal: i64,
    pub(crate) note_type: String,
    pub(crate) front_doc_json: String,
    pub(crate) back_doc_json: String,
    pub(crate) front_text: String,
    pub(crate) back_text: String,
    pub(crate) cloze_source: String,
    pub(crate) cloze_mode: String,
    pub(crate) reps: i64,
    pub(crate) presentation_variant: Option<String>,
}'''
if "pub(crate) struct ProjectedPresentationInput" not in content:
    content = replace_once(content, marker, marker + "\n" + projected_types + "\n", path)
start = content.find("#[tauri::command]\npub fn list_card_templates(")
if start != -1:
    end = content.find("\n#[tauri::command]\npub fn get_card_template(", start)
    if end == -1:
        raise SystemExit(f"{path}: list command end not found")
    content = content[:start] + content[end + 1:]
write(path, content)

path = "src-tauri/src/authoring/stage3_v2/generated_cards.rs"
content = read(path)
projected_functions = r'''/// Returns the immutable presentation variant captured for a projected card.
pub(crate) fn presentation_variant_for_projected(
    note_type: &str,
    cloze_source: &str,
    cloze_mode: &str,
    reps: i64,
) -> Result<String, CommandError> {
    if note_type != "cloze" {
        return Ok(String::new());
    }
    let parsed = parse_cloze(cloze_source, cloze_mode)?;
    Ok(select_cloze_group(&parsed, cloze_mode, reps)
        .map(|group| group.to_string())
        .unwrap_or_default())
}

/// Pure, query-free card presentation used by bounded list and snapshot readers.
pub(crate) fn render_projected_presentation(
    input: ProjectedPresentationInput,
) -> Result<RenderedCardPresentation, CommandError> {
    if input.direction != "forward" && input.direction != "reverse" {
        return Err(CommandError::new(
            "invalid_card_direction",
            "The stored card direction is invalid.",
        ));
    }
    if input.note_type == "cloze" {
        let parsed = parse_cloze(&input.cloze_source, &input.cloze_mode)?;
        let selected_group = match input.presentation_variant.as_deref() {
            Some(value) if !value.is_empty() => value.parse::<u32>().ok(),
            _ => select_cloze_group(&parsed, &input.cloze_mode, input.reps),
        };
        let (question, answer) = if let Some(group) = selected_group {
            let answers = parsed.answers_by_group.get(&group).cloned().unwrap_or_default();
            (render_cloze_prompt(&parsed, group), answers.join(" / "))
        } else {
            (parsed.full_text.clone(), parsed.full_text)
        };
        return Ok(RenderedCardPresentation {
            card_type: "cloze".to_string(),
            card_label: "Fill in the blank".to_string(),
            front_doc: paragraph_doc(&question),
            back_doc: paragraph_doc(&answer),
            front_text: question,
            back_text: answer,
        });
    }
    let front_doc: Value =
        serde_json::from_str(&input.front_doc_json).map_err(CommandError::serialization)?;
    let back_doc: Value =
        serde_json::from_str(&input.back_doc_json).map_err(CommandError::serialization)?;
    if input.direction == "reverse" {
        Ok(RenderedCardPresentation {
            card_type: "basic".to_string(),
            card_label: "Reverse".to_string(),
            front_doc: back_doc,
            back_doc: front_doc,
            front_text: input.back_text,
            back_text: input.front_text,
        })
    } else {
        let label = if input.template_ordinal > 0 {
            format!("Card {}", input.template_ordinal.saturating_add(1))
        } else {
            "Basic".to_string()
        };
        Ok(RenderedCardPresentation {
            card_type: "basic".to_string(),
            card_label: label,
            front_doc,
            back_doc,
            front_text: input.front_text,
            back_text: input.back_text,
        })
    }
}'''
if "pub(crate) fn render_projected_presentation(" not in content:
    content = content.rstrip() + "\n\n" + projected_functions + "\n"
write(path, content)

# Accepted review attribution is part of the same transaction as FSRS.
path = "src-tauri/src/practice.rs"
content = read(path)
content = replace_once(
    content,
    "    error::CommandError,\n",
    "    error::CommandError,\n"
    "    historical_attribution::insert_review_attribution,\n",
    path,
)
content = replace_once(
    content,
    "pub struct PracticeSessionDto {\n"
    "    pub id: String,\n"
    "    pub cards: Vec<PracticeCardDto>,\n"
    "    pub shuffle: bool,\n",
    "pub struct PracticeSessionDto {\n"
    "    pub id: String,\n"
    "    pub cards: Vec<PracticeCardDto>,\n"
    "    pub card_count: i64,\n"
    "    pub has_more: bool,\n"
    "    pub shuffle: bool,\n",
    path,
)
content = replace_once(
    content,
    "    insert_review_event(\n"
    "        &transaction,\n"
    "        &input,\n"
    "        &request_fingerprint,\n"
    "        &transition,\n"
    "        &result,\n"
    "        reviewed_at,\n"
    "    )?;\n"
    "    transaction.commit().map_err(CommandError::database)?;",
    "    let review_event_id = insert_review_event(\n"
    "        &transaction,\n"
    "        &input,\n"
    "        &request_fingerprint,\n"
    "        &transition,\n"
    "        &result,\n"
    "        reviewed_at,\n"
    "    )?;\n"
    "    insert_review_attribution(\n"
    "        &transaction,\n"
    "        &review_event_id,\n"
    "        &input.session_id,\n"
    "        &input.card_id,\n"
    "        reviewed_at,\n"
    "    )?;\n"
    "    transaction.commit().map_err(CommandError::database)?;",
    path,
)
content = replace_once(
    content,
    ") -> Result<(), CommandError> {\n"
    "    let event_id = generate_id(connection)?;\n"
    "    connection\n"
    "        .execute(\n"
    '            "INSERT INTO review_events (',
    ") -> Result<String, CommandError> {\n"
    "    let event_id = generate_id(connection)?;\n"
    "    connection\n"
    "        .execute(\n"
    '            "INSERT INTO review_events (',
    path,
)
content = replace_once(
    content,
    "        .map_err(CommandError::database)?;\n"
    "    Ok(())\n"
    "}\n\n"
    "fn review_result_for_submission(",
    "        .map_err(CommandError::database)?;\n"
    "    Ok(event_id)\n"
    "}\n\n"
    "fn review_result_for_submission(",
    path,
)
write(path, content)

# Final deck deletion purges all historical sources; archived decks are deletable.
path = "src-tauri/src/deletion.rs"
content = read(path)
content = replace_once(
    content,
    "    error::CommandError,\n",
    "    error::CommandError,\n"
    "    historical_attribution::purge_deck_history,\n",
    path,
)
content = content.replace(
    "WHERE id = ?1 AND deleted_at IS NULL AND archived_at IS NULL",
    "WHERE id = ?1 AND deleted_at IS NULL",
)
needle = '''    for (operation_id, operation) in operations {
        for note_id in &operation.note_ids {
'''
replacement = '''    for (operation_id, operation) in operations {
        if operation.entity_kind == "deck" {
            if let Some(deck_id) = operation.deck_id.as_deref() {
                purge_deck_history(&transaction, deck_id)?;
            }
        }
        for note_id in &operation.note_ids {
'''
content = replace_once(content, needle, replacement, path)
write(path, content)

print("Applied Stage 3.6-3.8 cross-module integration.")
