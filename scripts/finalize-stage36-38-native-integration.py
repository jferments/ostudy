#!/usr/bin/env python3
"""Finish native integration after the cross-module applicator runs."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content)


def replace_once(content: str, old: str, new: str, label: str) -> str:
    count = content.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return content.replace(old, new, 1)


# Preserve existing deck API consumers while keeping grouped projections.
path = "src-tauri/src/decks.rs"
content = read(path)
content = content.replace(
    "let description = validate_description(input.description)?;",
    "let description = validate_description(input.description.unwrap_or_default())?;",
)
content = content.replace(
    "fn validate_name(value: String) -> Result<String, CommandError>",
    "pub(crate) fn validate_name(value: String) -> Result<String, CommandError>",
)
content = content.replace(
    "fn validate_description(value: Option<String>) -> Result<String, CommandError> {\n"
    "    let value = value.unwrap_or_default().trim().to_string();",
    "pub(crate) fn validate_description(value: String) -> Result<String, CommandError> {\n"
    "    let value = value.trim().to_string();",
)
anchor = "pub(crate) fn create_deck_in_connection(\n"
helper = '''pub(crate) fn get_deck_from_connection(
    connection: &Connection,
    deck_id: &str,
) -> Result<DeckDto, CommandError> {
    load_deck_summary(
        connection,
        deck_id,
        false,
        &StudyTimeContext::capture()?,
    )
}

'''
if "pub(crate) fn get_deck_from_connection(" not in content:
    content = replace_once(content, anchor, helper + anchor, path)
write(path, content)

# Existing schedule controls now delegate local-day boundaries to the shared calendar.
path = "src-tauri/src/workload.rs"
content = read(path)
calendar_helper = '''pub(crate) fn next_local_day_start_millis(now: i64) -> Result<i64, CommandError> {
    Ok(StudyTimeContext::at(now)?.tomorrow_start_utc_millis)
}

'''
if "pub(crate) fn next_local_day_start_millis(" not in content:
    content = replace_once(
        content,
        "pub fn workload_overview_scoped(\n",
        calendar_helper + "pub fn workload_overview_scoped(\n",
        path,
    )
write(path, content)

# Replace the first-pass pure renderer with the repository's actual cloze helpers.
path = "src-tauri/src/authoring/stage3_v2/generated_cards.rs"
content = read(path)
marker = "/// Returns the immutable presentation variant captured for a projected card."
start = content.find(marker)
if start == -1:
    raise SystemExit(f"{path}: projected renderer marker is missing")
correct = r'''/// Returns the immutable presentation variant captured for a projected card.
pub(crate) fn presentation_variant_for_projected(
    note_type: &str,
    cloze_source: &str,
    cloze_mode: &str,
    reps: i64,
) -> Result<String, CommandError> {
    if note_type != "cloze" {
        return Ok(String::new());
    }
    validate_cloze_mode(cloze_mode)?;
    if cloze_mode == "all_together" {
        return Ok("cloze:all".to_string());
    }
    let parsed = parse_cloze(cloze_source)?;
    let index = usize::try_from(reps.max(0)).unwrap_or(0) % parsed.groups.len();
    Ok(format!("cloze:{}", parsed.groups[index]))
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
        if input.direction != "forward" || input.template_ordinal != 0 {
            return Err(CommandError::new(
                "invalid_generated_card",
                "The Fill in the blank note contains an unsupported active card identity.",
            ));
        }
        validate_cloze_mode(&input.cloze_mode)?;
        let parsed = parse_cloze(&input.cloze_source)?;
        let target = cloze_target_from_variant(
            &parsed,
            &input.cloze_mode,
            input.presentation_variant.as_deref(),
            input.reps,
        )?;
        let question = render_cloze_question(&parsed, target);
        let answer = render_cloze_answer(&parsed, target);
        let label = if input.cloze_mode == "all_together" {
            "Fill in the blank · all blanks".to_string()
        } else {
            format!(
                "Fill in the blank · {} alternating {}",
                parsed.groups.len(),
                if parsed.groups.len() == 1 { "blank" } else { "blanks" }
            )
        };
        return Ok(RenderedCardPresentation {
            card_type: "cloze".to_string(),
            card_label: label,
            front_doc: plain_text_document(&question),
            back_doc: plain_text_document(&answer),
            front_text: question,
            back_text: answer,
        });
    }
    if input.note_type != "basic" {
        return Err(CommandError::new(
            "invalid_generated_card",
            "The stored card type is unsupported.",
        ));
    }
    let front_doc: Value =
        serde_json::from_str(&input.front_doc_json).map_err(CommandError::serialization)?;
    let back_doc: Value =
        serde_json::from_str(&input.back_doc_json).map_err(CommandError::serialization)?;
    match input.direction.as_str() {
        "forward" => Ok(RenderedCardPresentation {
            card_type: "basic".to_string(),
            card_label: if input.template_ordinal == 0 {
                "Basic".to_string()
            } else {
                format!("Card {}", input.template_ordinal.saturating_add(1))
            },
            front_doc,
            back_doc,
            front_text: input.front_text,
            back_text: input.back_text,
        }),
        "reverse" => Ok(RenderedCardPresentation {
            card_type: "basic".to_string(),
            card_label: "Reverse".to_string(),
            front_doc: back_doc,
            back_doc: front_doc,
            front_text: input.back_text,
            back_text: input.front_text,
        }),
        _ => unreachable!(),
    }
}
'''
content = content[:start] + correct + "\n"
write(path, content)

# Materialize rusqlite mapped rows before statement locals are dropped.
path = "src-tauri/src/authoring/search_and_tags.rs"
content = read(path)
old = '''    statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok(TagDto {
                id: row.get(0)?,
                name: row.get(1)?,
                source_note_count: row.get(2)?,
                generated_card_count: row.get(3)?,
                studyable_due_count: row.get(4)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}'''
new = '''    let tags = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok(TagDto {
                id: row.get(0)?,
                name: row.get(1)?,
                source_note_count: row.get(2)?,
                generated_card_count: row.get(3)?,
                studyable_due_count: row.get(4)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(tags)
}'''
content = replace_once(content, old, new, path)
write(path, content)

path = "src-tauri/src/template_projections.rs"
content = read(path)
old = '''    statement
        .query_map([], |row| {
            Ok(CardTemplateSummaryDto {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                field_count: row.get(3)?,
                rule_count: row.get(4)?,
                asset_count: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                revision: row.get(8)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}'''
new = '''    let summaries = statement
        .query_map([], |row| {
            Ok(CardTemplateSummaryDto {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                field_count: row.get(3)?,
                rule_count: row.get(4)?,
                asset_count: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                revision: row.get(8)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(summaries)
}'''
content = replace_once(content, old, new, path)
write(path, content)

path = "src-tauri/src/historical_attribution.rs"
content = read(path)
old = '''    statement
        .query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |row| {
            Ok(LegacyReviewRow {
                review_event_id: row.get(0)?,
                reviewed_at: row.get(1)?,
                card_id: row.get(2)?,
                note_id: row.get(3)?,
                note_revision: row.get(4)?,
                deck_id: row.get(5)?,
                deck_name: row.get(6)?,
                note_type: row.get(7)?,
                direction: row.get(8)?,
                template_ordinal: row.get(9)?,
                front_doc_json: row.get(10)?,
                back_doc_json: row.get(11)?,
                front_text: row.get(12)?,
                back_text: row.get(13)?,
                cloze_source: row.get(14)?,
                cloze_mode: row.get(15)?,
                reps: row.get(16)?,
                presentation_variant: row.get(17)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}'''
new = '''    let rows = statement
        .query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |row| {
            Ok(LegacyReviewRow {
                review_event_id: row.get(0)?,
                reviewed_at: row.get(1)?,
                card_id: row.get(2)?,
                note_id: row.get(3)?,
                note_revision: row.get(4)?,
                deck_id: row.get(5)?,
                deck_name: row.get(6)?,
                note_type: row.get(7)?,
                direction: row.get(8)?,
                template_ordinal: row.get(9)?,
                front_doc_json: row.get(10)?,
                back_doc_json: row.get(11)?,
                front_text: row.get(12)?,
                back_text: row.get(13)?,
                cloze_source: row.get(14)?,
                cloze_mode: row.get(15)?,
                reps: row.get(16)?,
                presentation_variant: row.get(17)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)
}'''
content = replace_once(content, old, new, path)
old = '''        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };'''
new = '''        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };'''
content = replace_once(content, old, new, path)
write(path, content)

path = "src-tauri/src/analytics.rs"
content = read(path)
for old, new in [
    (
'''    statement
        .query_map(params_from_iter(values.iter()), |row| {
            let tags: String = row.get(6)?;
            Ok(RecentReviewDto {''',
'''    let reviews = statement
        .query_map(params_from_iter(values.iter()), |row| {
            let tags: String = row.get(6)?;
            Ok(RecentReviewDto {'''
    ),
    (
'''        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn recent_sessions(''',
'''        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(reviews)
}

fn recent_sessions('''
    ),
    (
'''    statement
        .query_map(params_from_iter(values.iter()), |row| {
            let session_id: String = row.get(0)?;''',
'''    let sessions = statement
        .query_map(params_from_iter(values.iter()), |row| {
            let session_id: String = row.get(0)?;'''
    ),
    (
'''        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn dimensions(''',
'''        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(sessions)
}

fn dimensions('''
    ),
]:
    content = replace_once(content, old, new, path)
write(path, content)

print("Finished native integration follow-up.")
