#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const path = fileURLToPath(new URL("../src-tauri/src/documents.rs", import.meta.url));
let source = await readFile(path, "utf8");
const original = source;

function replaceRequired(label, search, replacement, marker = replacement) {
  if (marker && source.includes(marker)) return;
  const index = source.indexOf(search);
  if (index < 0) throw new Error(`Could not find ${label} in documents.rs`);
  source = `${source.slice(0, index)}${replacement}${source.slice(index + search.length)}`;
}

function replaceRegex(label, pattern, replacement, marker) {
  if (marker && source.includes(marker)) return;
  if (!pattern.test(source)) throw new Error(`Could not find ${label} in documents.rs`);
  source = source.replace(pattern, replacement);
}

replaceRequired(
  "generated import dependency",
  `authoring::card_types_and_templates::generated_import::{
        import_generated_flashcards, FlashcardImportResultDto,
    },`,
  "authoring::card_types_and_templates::generated_import::import_generated_flashcards_bytes_with_commit,",
  "import_generated_flashcards_bytes_with_commit,",
);

replaceRegex(
  "retry extraction reuse",
  /(pub async fn retry_pdf_flashcard_generation[\s\S]*?&input\.document_id,\s*)true(,\s*\)\s*\.await)/u,
  "$1false$2",
  null,
);

replaceRegex(
  "restore lifecycle active deck lookup",
  /let generated_deck_id: Option<String> = connection[\s\S]*?\.flatten\(\);\s*if generated_deck_id\.is_some\(\) \{/u,
  `let active_generated_deck = connection
                .query_row(
                    "SELECT EXISTS(
                         SELECT 1
                         FROM documents document
                         JOIN decks deck ON deck.id = document.generated_deck_id
                         WHERE document.id = ?1
                           AND document.revision = ?2
                           AND deck.deleted_at IS NULL
                     )",
                    params![input.document_id, input.revision],
                    |row| row.get::<_, i64>(0),
                )
                .map(|value| value != 0)
                .map_err(CommandError::database)?;
            if active_generated_deck {`,
  "let active_generated_deck = connection",
);

replaceRegex(
  "direct atomic generated import",
  /mark_job_committing\(database\.inner\(\), &job_id, all_cards\.len\(\)\)\?;[\s\S]*?finalize_generation\(database\.inner\(\), document_id, &job_id, &import_result\)\?;/u,
  `mark_job_committing(database.inner(), &job_id, all_cards.len())?;
    let generated_json = build_generated_import_json(&stored.dto, all_cards)?;
    let import_result = import_generated_flashcards_bytes_with_commit(
        database.inner(),
        generated_json.as_bytes(),
        |connection, result, now| {
            connection
                .execute(
                    "INSERT INTO document_decks (document_id, deck_id, job_id, created_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![document_id, result.deck_id.as_str(), job_id, now],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE documents
                     SET lifecycle_state = 'ready',
                         generated_note_count = ?1,
                         generated_card_count = ?2,
                         generated_deck_id = ?3,
                         safe_error_code = '',
                         safe_error_message = '',
                         updated_at = ?4,
                         completed_at = ?4,
                         revision = revision + 1
                     WHERE id = ?5",
                    params![
                        result.created_note_count,
                        result.created_card_count,
                        result.deck_id.as_str(),
                        now,
                        document_id,
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE document_generation_jobs
                     SET state = 'succeeded',
                         generated_deck_id = ?1,
                         safe_error_code = '',
                         safe_error_message = '',
                         updated_at = ?2,
                         completed_at = ?2
                     WHERE id = ?3",
                    params![result.deck_id.as_str(), now, job_id],
                )
                .map_err(CommandError::database)?;
            Ok(())
        },
    )?;`,
  "import_generated_flashcards_bytes_with_commit(\n        database.inner()",
);

replaceRegex(
  "job unit generated content retention",
  /\s*let cards_json = serde_json::to_string\(cards\)\.map_err\(CommandError::serialization\)\?;\s*if cards_json\.len\(\) > 1_048_576 \{[\s\S]*?\n    \}\s*let now = unix_time_millis\(\)\?;/u,
  "\n    let now = unix_time_millis()?;",
  null,
);

replaceRegex(
  "job unit update payload",
  /"UPDATE document_generation_units\s+SET state = \?1,\s+cards_json = \?2,\s+card_count = \?3,\s+safe_error_code = '',\s+safe_error_message = '',\s+updated_at = \?4\s+WHERE job_id = \?5 AND page_number = \?6",\s*params!\[[\s\S]*?page_number,\s*\],/u,
  `"UPDATE document_generation_units
                 SET state = ?1,
                     cards_json = '[]',
                     card_count = ?2,
                     safe_error_code = '',
                     safe_error_message = '',
                     updated_at = ?3
                 WHERE job_id = ?4 AND page_number = ?5",
                params![
                    if skipped { "skipped" } else { "succeeded" },
                    card_count,
                    now,
                    job_id,
                    page_number,
                ],`,
  "cards_json = '[]',",
);

if (source.includes("\nfn finalize_generation(")) {
  const start = source.indexOf("\nfn finalize_generation(");
  const end = source.indexOf("\nfn mark_document_processing", start);
  if (end < 0) throw new Error("Could not find the end of finalize_generation in documents.rs");
  source = `${source.slice(0, start)}${source.slice(end)}`;
}

if (!source.includes("AS generated_deck_id")) {
  const needle = "generated_note_count, generated_card_count, generated_deck_id,";
  const replacement = `generated_note_count, generated_card_count,
                    CASE WHEN EXISTS (
                        SELECT 1
                        FROM decks active_generated_deck
                        WHERE active_generated_deck.id = documents.generated_deck_id
                          AND active_generated_deck.deleted_at IS NULL
                    ) THEN documents.generated_deck_id ELSE NULL END AS generated_deck_id,`;
  const parts = source.split(needle);
  if (parts.length !== 3) {
    throw new Error(`Active generated deck projection: expected 2 replacements, found ${parts.length - 1}`);
  }
  source = parts.join(replacement);
}

if (source !== original) await writeFile(path, source, "utf8");
