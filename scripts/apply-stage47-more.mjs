#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const path = fileURLToPath(new URL("../src-tauri/src/document_more.rs", import.meta.url));
let source = await readFile(path, "utf8");
const original = source;

function replaceOnce(label, search, replacement) {
  if (source.includes(replacement)) return;
  if (!source.includes(search)) throw new Error(`Could not find ${label}`);
  source = source.replace(search, replacement);
}

function replaceRegex(label, pattern, replacement, marker) {
  if (marker && source.includes(marker)) return;
  if (!pattern.test(source)) throw new Error(`Could not find ${label}`);
  source = source.replace(pattern, replacement);
}

replaceOnce(
  "additional generation imports",
  `    database::Database,
    document_semantics::{build_semantic_windows, SemanticChunk, SemanticWindow},
    error::CommandError,`,
  `    database::Database,
    document_scale::DocumentScalePlan,
    document_semantics::{build_partitioned_semantic_windows, SemanticChunk, SemanticWindow},
    error::CommandError,`,
);
replaceRegex(
  "legacy additional window constants",
  /\/\/ Additional passes use[\s\S]*?const MAX_IMAGES_PER_GENERATION_WINDOW: usize = 6;\n/u,
  "",
  "build_partitioned_semantic_windows",
);

replaceOnce(
  "partitioned additional windows",
  `    let windows = build_semantic_windows(
        &chunks,
        GENERATION_WINDOW_MAX_TOKENS,
        GENERATION_WINDOW_MAX_PAGES,
        GENERATION_WINDOW_OVERLAP_TOKENS,
    );`,
  `    let page_count = usize::try_from(context.page_count).unwrap_or(usize::MAX);
    let plan = DocumentScalePlan::for_page_count(page_count);
    let windows = build_partitioned_semantic_windows(
        &chunks,
        plan,
        plan.generation_window_max_tokens,
        plan.generation_window_max_pages,
        plan.generation_window_overlap_tokens,
    );`,
);
replaceOnce(
  "additional progress initialization",
  `    let task = document_intelligence::load_document_task_context(database.inner())?;`,
  `    initialize_additional_progress(
        database.inner(),
        &input.document_id,
        plan,
        windows.len(),
    )?;
    let task = document_intelligence::load_document_task_context(database.inner())?;`,
);
replaceOnce(
  "selective additional images",
  `            load_window_images(paths.inner(), &pages, window)?`,
  `            load_window_images(
                paths.inner(),
                &pages,
                window,
                plan.max_images_per_window,
            )?`,
);
replaceOnce(
  "budgeted additional generation call",
  `        let cards = document_intelligence::generate_window_flashcards(
            runtime.inner(),
            &task,
            &context.title,
            &second_pass_outline,
            window,
            &images,
        )
        .await?;`,
  `        let cards = generate_window_with_recovery(
            runtime.inner(),
            &task,
            &context.title,
            &second_pass_outline,
            window,
            &images,
            plan.max_cards_per_window,
        )
        .await?;`,
);
replaceOnce(
  "additional generation progress update",
  `        mark_window_complete(database.inner(), &job_id, window, cards.len())?;
        generated.extend(cards);`,
  `        mark_window_complete(database.inner(), &job_id, window, cards.len())?;
        update_additional_progress(
            database.inner(),
            &input.document_id,
            i64::try_from(window.window_index + 1).unwrap_or(i64::MAX),
        )?;
        generated.extend(cards);`,
);
replaceOnce(
  "additional commit progress completion",
  `                         generated_card_count = generated_card_count + ?2,
                         safe_error_code = '', safe_error_message = '',`,
  `                         generated_card_count = generated_card_count + ?2,
                         generation_completed_window_count = generation_window_count,
                         safe_error_code = '', safe_error_message = '',`,
);

replaceRegex(
  "additional selective image loader",
  /fn load_window_images\([\s\S]*?\n}\n\nfn create_additional_job/u,
  `fn load_window_images(
    paths: &AppPaths,
    pages: &[PageRenderRecord],
    window: &SemanticWindow,
    maximum: usize,
) -> Result<Vec<document_intelligence::SourcePageImage>, CommandError> {
    if window.visual_page_numbers.is_empty() || maximum == 0 {
        return Ok(Vec::new());
    }
    let selected = select_image_pages(&window.visual_page_numbers, maximum);
    let mut images = Vec::with_capacity(selected.len());
    for page_number in selected {
        let page = pages
            .iter()
            .find(|page| page.page_number == page_number)
            .ok_or_else(|| {
                CommandError::new(
                    "document_render_missing",
                    "A rendered page required by the additional pass is missing.",
                )
            })?;
        let bytes = fs::read(paths.resolve_relative(&page.render_relative_path))
            .map_err(CommandError::io)?;
        if bytes.is_empty()
            || bytes.len() > MAX_RENDER_BYTES
            || i64::try_from(bytes.len()).unwrap_or(i64::MAX) != page.render_byte_size
            || sha256_hex(&bytes) != page.render_sha256
        {
            return Err(CommandError::new(
                "document_render_invalid",
                "A retained rendered PDF page failed its integrity check. Rebuild the document before generating more cards.",
            ));
        }
        images.push(document_intelligence::SourcePageImage { page_number, bytes });
    }
    Ok(images)
}

async fn generate_window_with_recovery(
    runtime: &AiRuntimeState,
    task: &document_intelligence::DocumentTaskContext,
    document_title: &str,
    outline: &document_intelligence::DocumentOutline,
    window: &SemanticWindow,
    images: &[document_intelligence::SourcePageImage],
    maximum_cards: usize,
) -> Result<Vec<Value>, CommandError> {
    let mut card_budget = maximum_cards.max(1);
    loop {
        let result = document_intelligence::generate_window_flashcards(
            runtime,
            task,
            document_title,
            outline,
            window,
            images,
            card_budget,
        )
        .await;
        match result {
            Err(error)
                if card_budget > 2
                    && (error.code == "ai_document_output_truncated"
                        || (error.code == "ai_document_output_invalid"
                            && error.message.contains("more than"))) =>
            {
                let next_budget = (card_budget / 2).max(2);
                crate::logging::info(
                    "document_additional_window_budget_reduced",
                    format!(
                        "window={} partition={} pages={}-{} old_budget={} new_budget={} code={}",
                        window.window_index + 1,
                        window.partition_index + 1,
                        window.page_start,
                        window.page_end,
                        card_budget,
                        next_budget,
                        error.code,
                    ),
                );
                card_budget = next_budget;
            }
            other => return other,
        }
    }
}

fn initialize_additional_progress(
    database: &Database,
    document_id: &str,
    plan: DocumentScalePlan,
    window_count: usize,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE documents
                 SET scale_mode = ?1, generation_window_count = ?2,
                     generation_completed_window_count = 0, updated_at = ?3
                 WHERE id = ?4 AND lifecycle_state = 'processing'",
                params![
                    plan.label(),
                    i64::try_from(window_count).unwrap_or(i64::MAX),
                    now,
                    document_id,
                ],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn update_additional_progress(
    database: &Database,
    document_id: &str,
    completed: i64,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE documents
                 SET generation_completed_window_count = MIN(generation_window_count, MAX(0, ?1)),
                     updated_at = ?2
                 WHERE id = ?3 AND lifecycle_state = 'processing'",
                params![completed, now, document_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn create_additional_job`,
  "document_additional_window_budget_reduced",
);

replaceOnce(
  "additional image test reduced budget",
  `        let selected = select_image_pages(&pages, 6);
        assert_eq!(selected.len(), 6);`,
  `        let selected = select_image_pages(&pages, 3);
        assert_eq!(selected.len(), 3);`,
);
replaceOnce(
  "additional image test unique size",
  `            6
        );`,
  `            3
        );`,
);

if (source !== original) await writeFile(path, source, "utf8");
