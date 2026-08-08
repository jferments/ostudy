#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const path = fileURLToPath(new URL("../src-tauri/src/documents.rs", import.meta.url));
let source = await readFile(path, "utf8");
const original = source;

function replaceRegex(label, pattern, replacement, alreadyAppliedMarker) {
  if (alreadyAppliedMarker && source.includes(alreadyAppliedMarker)) return;
  if (!pattern.test(source)) {
    throw new Error(`Could not find ${label} in documents.rs`);
  }
  source = source.replace(pattern, replacement);
}

replaceRegex(
  "generated import dependency",
  /authoring::card_types_and_templates::generated_import::\{\s*import_generated_flashcards,\s*FlashcardImportResultDto,\s*\},/u,
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

replaceRegex(
  "standalone finalize generation transaction",
  /\nfn finalize_generation\([\s\S]*?\n}\n\nfn mark_document_processing/u,
  "\nfn mark_document_processing",
  null,
);

if (!source.includes("AS generated_deck_id")) {
  let projectionCount = 0;
  source = source.replace(
    /(\s*)generated_note_count, generated_card_count, generated_deck_id,/gu,
    (_match, indentation) => {
      projectionCount += 1;
      return `${indentation}generated_note_count, generated_card_count,
${indentation}CASE WHEN EXISTS (
${indentation}    SELECT 1
${indentation}    FROM decks active_generated_deck
${indentation}    WHERE active_generated_deck.id = documents.generated_deck_id
${indentation}      AND active_generated_deck.deleted_at IS NULL
${indentation}) THEN documents.generated_deck_id ELSE NULL END AS generated_deck_id,`;
    },
  );
  if (projectionCount !== 2) {
    throw new Error(`Active generated deck projection: expected 2 replacements, found ${projectionCount}`);
  }
}

if (!source.includes("completed_generation_units_store_counts_without_duplicate_card_text")) {
  const tests = `

    #[test]
    fn completed_generation_units_store_counts_without_duplicate_card_text() {
        let database = Database::open_in_memory().expect("database should open");
        let document_id = "d".repeat(32);
        let job_id = "e".repeat(32);
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO documents (
                             id, title, original_filename, mime_type, byte_size, sha256,
                             source_relative_path, lifecycle_state, created_at, updated_at
                         ) VALUES (?1, 'Fixture', 'fixture.pdf', 'application/pdf', 1, ?2,
                                   ?3, 'processing', 1, 1)",
                        params![
                            document_id,
                            "0".repeat(64),
                            format!("documents/{document_id}/source.pdf"),
                        ],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO document_generation_jobs (
                             id, document_id, provider_kind, model_id,
                             profile_revision, credential_epoch, network_scope,
                             state, page_count, created_at, updated_at
                         ) VALUES (?1, ?2, 'llama_cpp', 'model', 1, 0, 'local',
                                   'generating', 1, 1, 1)",
                        params![job_id, document_id],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO document_generation_units (
                             job_id, page_number, state, cards_json, card_count, updated_at
                         ) VALUES (?1, 1, 'pending', '[]', 0, 1)",
                        [job_id.as_str()],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("generation fixture should insert");

        mark_generation_unit_complete(
            &database,
            &job_id,
            1,
            &[serde_json::json!({
                "type": "basic",
                "front": "Sensitive generated question",
                "back": "Sensitive generated answer"
            })],
            false,
        )
        .expect("generation unit should complete");

        database
            .with_connection(|connection| {
                let (cards_json, card_count): (String, i64) = connection
                    .query_row(
                        "SELECT cards_json, card_count
                         FROM document_generation_units
                         WHERE job_id = ?1 AND page_number = 1",
                        [job_id.as_str()],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(cards_json, "[]");
                assert_eq!(card_count, 1);
                Ok(())
            })
            .expect("completed unit should be readable");
    }

    #[test]
    fn deleted_generated_decks_are_not_reported_as_active() {
        let database = Database::open_in_memory().expect("database should open");
        let document_id = "f".repeat(32);
        let deck_id = "a".repeat(32);
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO decks (
                             id, name, description, sort_order, created_at, updated_at, revision
                         ) VALUES (?1, 'Generated deck', '', 0, 1, 1, 1)",
                        [deck_id.as_str()],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO documents (
                             id, title, original_filename, mime_type, byte_size, sha256,
                             source_relative_path, lifecycle_state, generated_deck_id,
                             created_at, updated_at
                         ) VALUES (?1, 'Fixture', 'fixture.pdf', 'application/pdf', 1, ?2,
                                   ?3, 'ready', ?4, 1, 1)",
                        params![
                            document_id,
                            "1".repeat(64),
                            format!("documents/{document_id}/source.pdf"),
                            deck_id,
                        ],
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(
                    load_document(connection, &document_id)?.dto.generated_deck_id,
                    Some(deck_id.clone())
                );
                connection
                    .execute(
                        "UPDATE decks SET deleted_at = 2 WHERE id = ?1",
                        [deck_id.as_str()],
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(load_document(connection, &document_id)?.dto.generated_deck_id, None);
                Ok(())
            })
            .expect("active deck projection should track soft deletion");
    }
`;
  const finalModuleClose = source.lastIndexOf("\n}");
  if (finalModuleClose < 0) throw new Error("Could not find documents test module close");
  source = `${source.slice(0, finalModuleClose)}${tests}${source.slice(finalModuleClose)}`;
}

if (source !== original) {
  await writeFile(path, source, "utf8");
}
