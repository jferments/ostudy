#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

async function patch(relativePath, transform) {
  const path = fileURLToPath(new URL(`../${relativePath}`, import.meta.url));
  const source = await readFile(path, "utf8");
  const updated = transform(source);
  if (updated !== source) await writeFile(path, updated, "utf8");
}

function replaceOnce(source, label, search, replacement) {
  if (source.includes(replacement)) return source;
  if (!source.includes(search)) throw new Error(`Could not find ${label}`);
  return source.replace(search, replacement);
}

function replaceRegex(source, label, pattern, replacement, marker) {
  if (marker && source.includes(marker)) return source;
  if (!pattern.test(source)) throw new Error(`Could not find ${label}`);
  return source.replace(pattern, replacement);
}

await patch("src-tauri/src/lib.rs", (source) =>
  replaceOnce(
    source,
    "document checkpoint module",
    "mod document_backup;\nmod document_delete_reconciliation;",
    "mod document_backup;\nmod document_checkpoints;\nmod document_delete_reconciliation;",
  ),
);

await patch("src-tauri/src/database.rs", (source) => {
  source = replaceOnce(
    source,
    "migration 19 registry",
    `    Migration {
        version: 18,
        name: "document_semantics_and_outline",
        checksum: "06b0023711038b8f9ea2c90c17c5ffb9dda8b286c01e34c4881a0596e96a1c7e",
        sql: include_str!("../migrations/0018_document_semantics_and_outline.sql"),
    },
];`,
    `    Migration {
        version: 18,
        name: "document_semantics_and_outline",
        checksum: "06b0023711038b8f9ea2c90c17c5ffb9dda8b286c01e34c4881a0596e96a1c7e",
        sql: include_str!("../migrations/0018_document_semantics_and_outline.sql"),
    },
    Migration {
        version: 19,
        name: "resumable_large_document_generation",
        checksum: "80dc940f7018b6a754d440cbf5c3b287150b2dfb4def096539c4ff1f14d5fc66",
        sql: include_str!("../migrations/0019_resumable_large_document_generation.sql"),
    },
];`,
  );
  return replaceOnce(
    source,
    "latest schema version",
    "pub(crate) const LATEST_SCHEMA_VERSION: i64 = 18;",
    "pub(crate) const LATEST_SCHEMA_VERSION: i64 = 19;",
  );
});

await patch("src-tauri/src/documents.rs", (source) => {
  source = replaceOnce(
    source,
    "document pipeline imports",
    `    database::Database,
    document_semantics::{build_semantic_windows, SemanticChunk, SemanticWindow},
    error::CommandError,`,
    `    database::Database,
    document_checkpoints,
    document_scale::{DocumentScalePlan, MAX_SUPPORTED_PDF_PAGES},
    document_semantics::{build_partitioned_semantic_windows, SemanticChunk, SemanticWindow},
    error::CommandError,`,
  );
  source = replaceRegex(
    source,
    "legacy document window constants",
    /const MAX_PDF_PAGES: usize = 2_000;[\s\S]*?const MAX_IMAGES_PER_GENERATION_WINDOW: usize = 6;\n/u,
    `const MAX_PAGE_TEXT_CHARS: usize = 200_000;
const MAX_TOTAL_TEXT_BYTES: usize = 96 * 1024 * 1024;
const MAX_SEMANTIC_TEXT_BYTES: usize = 128 * 1024 * 1024;
const MAX_RENDER_BYTES: usize = 32 * 1024 * 1024;
const MAX_GENERATED_CARDS: usize = 10_000;
const MAX_DOCUMENTS_RETURNED: i64 = 500;
`,
    "MAX_SUPPORTED_PDF_PAGES",
  );
  source = replaceOnce(
    source,
    "document AI progress fields",
    `    pub rendered_page_count: i64,
    pub generated_note_count: i64,`,
    `    pub rendered_page_count: i64,
    pub scale_mode: String,
    pub estimated_source_tokens: i64,
    pub outline_window_count: i64,
    pub outline_completed_window_count: i64,
    pub generation_window_count: i64,
    pub generation_completed_window_count: i64,
    pub generated_note_count: i64,`,
  );
  source = replaceOnce(
    source,
    "retry derivative reuse",
    `        &input.document_id,
        true,
    )`,
    `        &input.document_id,
        false,
    )`,
  );

  source = replaceRegex(
    source,
    "segmented resumable document pipeline",
    /async fn process_and_generate\([\s\S]*?\n}\n\nfn extract_and_render_pdf/u,
    `async fn process_and_generate(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, AiRuntimeState>,
    render_runtime: PdfRenderRuntime,
    document_id: &str,
    force_extract: bool,
) -> Result<DocumentGenerationResultDto, CommandError> {
    mark_document_processing(database.inner(), document_id)?;
    let stored = database.with_connection(|connection| load_document(connection, document_id))?;
    let source_path = paths.resolve_relative(&stored.source_relative_path);
    if !source_path.is_file() {
        return Err(CommandError::new(
            "document_source_missing",
            "The managed PDF is missing. Restore the library from backup or import the PDF again.",
        ));
    }

    let backend_changed = stored.extraction_backend != pdf_backend::active_backend_id()
        || stored.render_backend != pdf_render_backend::active_backend_id();
    let incomplete_derivatives = stored.dto.page_count == 0
        || stored.dto.extracted_page_count != stored.dto.page_count
        || stored.dto.rendered_page_count != stored.dto.page_count
        || stored.semantic_chunk_count == 0;
    if force_extract || backend_changed || incomplete_derivatives {
        let paths_owned = paths.inner().clone();
        let source_owned = source_path.clone();
        let id_owned = document_id.to_string();
        let (progress_sender, mut progress_receiver) =
            tokio::sync::mpsc::unbounded_channel::<(usize, usize)>();
        let processing_task = tauri::async_runtime::spawn_blocking(move || {
            extract_and_render_pdf(
                &paths_owned,
                &render_runtime,
                &id_owned,
                &source_owned,
                |completed, total| {
                    progress_sender.send((completed, total)).map_err(|_| {
                        CommandError::new(
                            "document_processing_stopped",
                            "The PDF processing progress channel closed unexpectedly.",
                        )
                    })
                },
            )
        });
        while let Some((completed, total)) = progress_receiver.recv().await {
            update_render_progress(database.inner(), document_id, completed, total)?;
        }
        let processed = processing_task.await.map_err(|error| {
            CommandError::new(
                "document_processing_failed",
                format!("The PDF processing task stopped unexpectedly: {error}"),
            )
        })??;
        persist_processed_document(database.inner(), document_id, &processed)?;
    }

    let pages = load_pages(database.inner(), paths.inner(), document_id)?;
    if pages.is_empty() {
        return Err(CommandError::new(
            "document_has_no_pages",
            "The PDF did not contain any pages that ostudy could process.",
        ));
    }
    let chunks = load_semantic_chunks(database.inner(), document_id)?;
    if chunks.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The PDF did not contain structured text that ostudy could analyze. Image-only PDFs require an image-capable generation profile.",
        ));
    }

    let plan = DocumentScalePlan::for_page_count(pages.len());
    let estimated_source_tokens = chunks.iter().fold(0_usize, |total, chunk| {
        total.saturating_add(
            chunk
                .token_estimate
                .max(chunk.full_text.chars().count().div_ceil(4)),
        )
    });
    let task = document_intelligence::load_document_task_context(database.inner())?;
    let outline_windows = build_partitioned_semantic_windows(
        &chunks,
        plan,
        plan.outline_window_max_tokens,
        plan.outline_window_max_pages,
        plan.outline_window_overlap_tokens,
    );
    let generation_windows = build_partitioned_semantic_windows(
        &chunks,
        plan,
        plan.generation_window_max_tokens,
        plan.generation_window_max_pages,
        plan.generation_window_overlap_tokens,
    );
    if outline_windows.is_empty() || generation_windows.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The PDF did not contain bounded semantic source windows for document analysis.",
        ));
    }

    let outline_contract = document_checkpoints::outline_contract_key(&task, plan);
    let cached_outline = document_checkpoints::load_outline_fragments(
        database.inner(),
        document_id,
        &outline_contract,
        &outline_windows,
    )?;
    let outline_completed = cached_outline.iter().filter(|value| value.is_some()).count();
    initialize_document_ai_plan(
        database.inner(),
        document_id,
        plan,
        estimated_source_tokens,
        outline_windows.len(),
        outline_completed,
        generation_windows.len(),
        0,
    )?;
    crate::logging::info(
        "document_scale_plan_selected",
        format!(
            "document={} scale={} pages={} source_tokens={} outline_windows={} generation_windows={} max_cards_per_window={} max_images_per_window={}",
            document_id,
            plan.label(),
            pages.len(),
            estimated_source_tokens,
            outline_windows.len(),
            generation_windows.len(),
            plan.max_cards_per_window,
            plan.max_images_per_window,
        ),
    );

    mark_outline_building(database.inner(), document_id)?;
    let outline = document_intelligence::build_document_outline(
        runtime.inner(),
        &task,
        &stored.dto.title,
        i64::try_from(pages.len()).unwrap_or(i64::MAX),
        &outline_windows,
        &cached_outline,
        |window, fragment| {
            document_checkpoints::persist_outline_fragment(
                database.inner(),
                document_id,
                &outline_contract,
                window,
                fragment,
            )?;
            let completed = document_checkpoints::count_outline_checkpoints(
                database.inner(),
                document_id,
                &outline_contract,
            )?;
            update_outline_progress(database.inner(), document_id, completed)?;
            Ok(())
        },
    )
    .await?;
    update_outline_progress(
        database.inner(),
        document_id,
        i64::try_from(outline_windows.len()).unwrap_or(i64::MAX),
    )?;
    persist_outline(database.inner(), document_id, &task, &outline)?;

    let generation_contract = document_checkpoints::generation_contract_key(
        &task,
        plan,
        &outline,
        plan.max_cards_per_window,
    )?;
    let cached_generation = generation_windows
        .iter()
        .map(|window| {
            document_checkpoints::load_generation_cards(
                database.inner(),
                document_id,
                &generation_contract,
                window,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let generation_completed = cached_generation
        .iter()
        .filter(|value| value.is_some())
        .count();
    update_generation_progress(
        database.inner(),
        document_id,
        i64::try_from(generation_completed).unwrap_or(i64::MAX),
    )?;

    let job_id = create_generation_job(
        database.inner(),
        document_id,
        &task,
        pages.len(),
        &generation_windows,
    )?;
    let mut generated_cards = Vec::new();
    for (window, cached_cards) in generation_windows.iter().zip(cached_generation.into_iter()) {
        let cards = if let Some(cards) = cached_cards {
            crate::logging::info(
                "document_generation_window_reused",
                format!(
                    "window={}/{} partition={} pages={}-{} cards={}",
                    window.window_index + 1,
                    generation_windows.len(),
                    window.partition_index + 1,
                    window.page_start,
                    window.page_end,
                    cards.len(),
                ),
            );
            cards
        } else {
            mark_window_running(database.inner(), &job_id, window.window_index)?;
            let images = if task.include_page_images {
                load_window_images(
                    paths.inner(),
                    &pages,
                    window,
                    plan.max_images_per_window,
                )?
            } else {
                Vec::new()
            };
            let cards = generate_window_with_recovery(
                runtime.inner(),
                &task,
                &stored.dto.title,
                &outline,
                window,
                &images,
                plan.max_cards_per_window,
            )
            .await?;
            document_checkpoints::persist_generation_cards(
                database.inner(),
                document_id,
                &generation_contract,
                window,
                &cards,
            )?;
            let completed = document_checkpoints::count_generation_checkpoints(
                database.inner(),
                document_id,
                &generation_contract,
            )?;
            update_generation_progress(database.inner(), document_id, completed)?;
            cards
        };
        if generated_cards.len().saturating_add(cards.len()) > MAX_GENERATED_CARDS {
            return Err(CommandError::new(
                "too_many_generated_cards",
                format!("The model generated more than {MAX_GENERATED_CARDS} cards from this PDF."),
            ));
        }
        mark_window_complete(database.inner(), &job_id, window, cards.len())?;
        generated_cards.extend(cards);
    }
    update_generation_progress(
        database.inner(),
        document_id,
        i64::try_from(generation_windows.len()).unwrap_or(i64::MAX),
    )?;

    if generated_cards.is_empty() {
        return Err(CommandError::new(
            "document_generated_no_cards",
            "The document analysis found no material important enough to generate flashcards.",
        ));
    }
    let generated_cards = document_intelligence::curate_document_cards(
        runtime.inner(),
        &task,
        &outline,
        generated_cards,
    )
    .await?;
    if generated_cards.is_empty() {
        return Err(CommandError::new(
            "document_generated_no_cards",
            "The global relevance pass removed all generated cards as low-value or redundant.",
        ));
    }

    mark_job_committing(database.inner(), &job_id, generated_cards.len())?;
    let payload = build_generated_import_json(&stored.dto, generated_cards)?;
    let result = import_generated_flashcards_bytes_with_commit(
        database.inner(),
        payload.as_bytes(),
        |connection, import_result, now| {
            connection
                .execute(
                    "INSERT INTO document_decks (document_id, deck_id, job_id, created_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![document_id, import_result.deck_id, job_id, now],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE documents
                     SET lifecycle_state = 'ready', generated_note_count = ?1,
                         generated_card_count = ?2, generated_deck_id = ?3,
                         outline_completed_window_count = outline_window_count,
                         generation_completed_window_count = generation_window_count,
                         safe_error_code = '', safe_error_message = '',
                         updated_at = ?4, completed_at = ?4, revision = revision + 1
                     WHERE id = ?5",
                    params![
                        import_result.created_note_count,
                        import_result.created_card_count,
                        import_result.deck_id,
                        now,
                        document_id,
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE document_generation_jobs
                     SET state = 'succeeded', generated_candidate_count = ?1,
                         generated_deck_id = ?2, safe_error_code = '',
                         safe_error_message = '', updated_at = ?3, completed_at = ?3
                     WHERE id = ?4",
                    params![
                        import_result.created_note_count,
                        import_result.deck_id,
                        now,
                        job_id,
                    ],
                )
                .map_err(CommandError::database)?;
            Ok(())
        },
    )?;
    // Checkpoints are required until the canonical deck transaction succeeds.
    // Their cleanup is rebuildable/best-effort and cannot roll back the deck.
    let _ = document_checkpoints::clear_generation_checkpoints(database.inner(), document_id);
    let document = database
        .with_connection(|connection| load_document(connection, document_id))?
        .dto;
    Ok(DocumentGenerationResultDto {
        document,
        deck_id: result.deck_id,
        created_note_count: result.created_note_count,
        created_card_count: result.created_card_count,
        skipped_duplicate_count: result.skipped_duplicate_count,
    })
}

async fn generate_window_with_recovery(
    runtime: &AiRuntimeState,
    task: &document_intelligence::DocumentTaskContext,
    document_title: &str,
    outline: &document_intelligence::DocumentOutline,
    window: &SemanticWindow,
    images: &[document_intelligence::SourcePageImage],
    maximum_cards: usize,
) -> Result<Vec<serde_json::Value>, CommandError> {
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
                    "document_generation_window_budget_reduced",
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

fn extract_and_render_pdf`,
    "document_scale_plan_selected",
  );

  source = source.replaceAll("semantic.page_count > MAX_PDF_PAGES", "semantic.page_count > MAX_SUPPORTED_PDF_PAGES");
  source = source.replaceAll("{MAX_PDF_PAGES} pages", "{MAX_SUPPORTED_PDF_PAGES} pages");

  source = replaceOnce(
    source,
    "checkpoint cleanup during re-extraction",
    `        for table in [
            "document_pages",
            "document_semantic_elements",
            "document_semantic_chunks",
            "document_outlines",
        ] {`,
    `        for table in [
            "document_pages",
            "document_semantic_elements",
            "document_semantic_chunks",
            "document_outlines",
            "document_outline_checkpoints",
            "document_generation_checkpoints",
        ] {`,
  );
  source = replaceOnce(
    source,
    "reset document scale fields after extraction",
    `                     semantic_element_count = ?4, semantic_chunk_count = ?5,
                     outline_state = 'pending', safe_error_code = '', safe_error_message = '',
                     updated_at = ?6, revision = revision + 1`,
    `                     semantic_element_count = ?4, semantic_chunk_count = ?5,
                     scale_mode = 'standard', estimated_source_tokens = 0,
                     outline_window_count = 0, outline_completed_window_count = 0,
                     generation_window_count = 0, generation_completed_window_count = 0,
                     outline_state = 'pending', safe_error_code = '', safe_error_message = '',
                     updated_at = ?6, revision = revision + 1`,
  );
  source = replaceOnce(
    source,
    "outline persistence prompt version",
    `                 ) VALUES (?1, 1, 2, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)`,
    `                 ) VALUES (?1, 1, 4, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)`,
  );

  source = replaceOnce(
    source,
    "AI progress helpers",
    `fn create_generation_job(
    database: &Database,`,
    `fn initialize_document_ai_plan(
    database: &Database,
    document_id: &str,
    plan: DocumentScalePlan,
    estimated_source_tokens: usize,
    outline_window_count: usize,
    outline_completed_window_count: usize,
    generation_window_count: usize,
    generation_completed_window_count: usize,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE documents
                 SET scale_mode = ?1, estimated_source_tokens = ?2,
                     outline_window_count = ?3, outline_completed_window_count = ?4,
                     generation_window_count = ?5, generation_completed_window_count = ?6,
                     updated_at = ?7
                 WHERE id = ?8 AND lifecycle_state = 'processing'",
                params![
                    plan.label(),
                    i64::try_from(estimated_source_tokens).unwrap_or(i64::MAX),
                    i64::try_from(outline_window_count).unwrap_or(i64::MAX),
                    i64::try_from(outline_completed_window_count).unwrap_or(i64::MAX),
                    i64::try_from(generation_window_count).unwrap_or(i64::MAX),
                    i64::try_from(generation_completed_window_count).unwrap_or(i64::MAX),
                    now,
                    document_id,
                ],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn update_outline_progress(
    database: &Database,
    document_id: &str,
    completed: i64,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE documents
                 SET outline_completed_window_count = MIN(outline_window_count, MAX(0, ?1)),
                     updated_at = ?2
                 WHERE id = ?3 AND lifecycle_state = 'processing'",
                params![completed, now, document_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn update_generation_progress(
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

fn create_generation_job(
    database: &Database,`,
  );

  source = replaceRegex(
    source,
    "selective visual grounding loader",
    /fn load_window_images\([\s\S]*?\n}\n\nfn select_image_pages/u,
    `fn load_window_images(
    paths: &AppPaths,
    pages: &[PageRecord],
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
                    "A selected PDF page render is missing.",
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
                "A rendered PDF page failed its size or integrity check. Retry generation to rebuild it.",
            ));
        }
        images.push(document_intelligence::SourcePageImage { page_number, bytes });
    }
    Ok(images)
}

fn select_image_pages`,
    "window.visual_page_numbers.is_empty()",
  );

  source = replaceRegex(
    source,
    "document list query with progress",
    /fn list_document_rows\([\s\S]*?\n}\n\nfn load_document/u,
    `fn list_document_rows(
    connection: &Connection,
    include_archived: bool,
) -> Result<Vec<DocumentDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, title, original_filename, byte_size, sha256, lifecycle_state,
                    page_count, extracted_page_count, rendered_page_count,
                    scale_mode, estimated_source_tokens,
                    outline_window_count, outline_completed_window_count,
                    generation_window_count, generation_completed_window_count,
                    generated_note_count, generated_card_count,
                    CASE WHEN EXISTS (
                        SELECT 1 FROM decks deck
                        WHERE deck.id = documents.generated_deck_id AND deck.deleted_at IS NULL
                    ) THEN documents.generated_deck_id ELSE NULL END,
                    safe_error_code, safe_error_message, created_at, updated_at,
                    completed_at, archived_at, revision
             FROM documents
             WHERE (?1 = 1 OR archived_at IS NULL)
             ORDER BY updated_at DESC, id
             LIMIT ?2",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map(
            params![
                if include_archived { 1_i64 } else { 0_i64 },
                MAX_DOCUMENTS_RETURNED,
            ],
            document_row,
        )
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)
}

fn load_document`,
    "scale_mode, estimated_source_tokens",
  );

  source = replaceRegex(
    source,
    "stored document query with progress",
    /fn load_document\([\s\S]*?\n}\n\nfn document_row/u,
    `fn load_document(
    connection: &Connection,
    document_id: &str,
) -> Result<StoredDocument, CommandError> {
    connection
        .query_row(
            "SELECT id, title, original_filename, byte_size, sha256, lifecycle_state,
                    page_count, extracted_page_count, rendered_page_count,
                    scale_mode, estimated_source_tokens,
                    outline_window_count, outline_completed_window_count,
                    generation_window_count, generation_completed_window_count,
                    generated_note_count, generated_card_count,
                    CASE WHEN EXISTS (
                        SELECT 1 FROM decks deck
                        WHERE deck.id = documents.generated_deck_id AND deck.deleted_at IS NULL
                    ) THEN documents.generated_deck_id ELSE NULL END,
                    safe_error_code, safe_error_message, created_at, updated_at,
                    completed_at, archived_at, revision, source_relative_path,
                    extraction_backend, render_backend, semantic_chunk_count
             FROM documents WHERE id = ?1",
            [document_id],
            |row| {
                Ok(StoredDocument {
                    dto: document_row(row)?,
                    source_relative_path: row.get(25)?,
                    extraction_backend: row.get(26)?,
                    render_backend: row.get(27)?,
                    semantic_chunk_count: row.get(28)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "document_not_found",
                "That PDF is no longer in the library.",
            )
        })
}

fn document_row`,
    "source_relative_path: row.get(25)?",
  );

  source = replaceRegex(
    source,
    "document row decoder with progress",
    /fn document_row\(row: &rusqlite::Row<'_>\) -> rusqlite::Result<DocumentDto> \{[\s\S]*?\n}\n\nfn active_generated_deck_exists/u,
    `fn document_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentDto> {
    Ok(DocumentDto {
        id: row.get(0)?,
        title: row.get(1)?,
        original_filename: row.get(2)?,
        byte_size: row.get(3)?,
        sha256: row.get(4)?,
        lifecycle_state: row.get(5)?,
        page_count: row.get(6)?,
        extracted_page_count: row.get(7)?,
        rendered_page_count: row.get(8)?,
        scale_mode: row.get(9)?,
        estimated_source_tokens: row.get(10)?,
        outline_window_count: row.get(11)?,
        outline_completed_window_count: row.get(12)?,
        generation_window_count: row.get(13)?,
        generation_completed_window_count: row.get(14)?,
        generated_note_count: row.get(15)?,
        generated_card_count: row.get(16)?,
        generated_deck_id: row.get(17)?,
        safe_error_code: row.get(18)?,
        safe_error_message: row.get(19)?,
        created_at: row.get(20)?,
        updated_at: row.get(21)?,
        completed_at: row.get(22)?,
        archived_at: row.get(23)?,
        revision: row.get(24)?,
    })
}

fn active_generated_deck_exists`,
    "estimated_source_tokens: row.get(10)?",
  );

  source = source.replaceAll(
    `            format!("The Pdfium page renderer ({backend}) could not be loaded: {error}"),`,
    `            format!("The PDF page renderer ({backend}) could not be loaded: {error}"),`,
  );

  source = replaceOnce(
    source,
    "test document progress fixture",
    `            rendered_page_count: 1,
            generated_note_count: 0,`,
    `            rendered_page_count: 1,
            scale_mode: "standard".to_string(),
            estimated_source_tokens: 100,
            outline_window_count: 1,
            outline_completed_window_count: 1,
            generation_window_count: 1,
            generation_completed_window_count: 1,
            generated_note_count: 0,`,
  );
  return source;
});

await patch("src-tauri/src/document_pdf_tests.rs", (source) =>
  source.replaceAll(
    '"justpdf-render-0.1.2/pure-rust-parallel-v1"',
    '"justpdf-render-0.1.2/pure-rust-parallel-v2-120dpi"',
  ),
);
