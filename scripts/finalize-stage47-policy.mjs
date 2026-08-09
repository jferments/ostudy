#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

async function edit(relative, transform) {
  const path = fileURLToPath(new URL(`../${relative}`, import.meta.url));
  const source = await readFile(path, "utf8");
  const updated = transform(source);
  if (updated !== source) await writeFile(path, updated, "utf8");
}

function replaceRequired(source, label, search, replacement) {
  if (source.includes(replacement)) return source;
  const index = source.indexOf(search);
  if (index < 0) throw new Error(`Could not find ${label}`);
  return `${source.slice(0, index)}${replacement}${source.slice(index + search.length)}`;
}

function replaceFunction(source, startMarker, endMarker, replacement) {
  const start = source.indexOf(startMarker);
  if (start < 0) throw new Error(`Could not find function start: ${startMarker}`);
  const end = source.indexOf(endMarker, start);
  if (end < 0) throw new Error(`Could not find function end: ${endMarker}`);
  return `${source.slice(0, start)}${replacement}${source.slice(end)}`;
}

await edit("src-tauri/src/documents.rs", (source) => {
  source = replaceRequired(
    source,
    "document_scale self import",
    "document_scale::{DocumentScalePlan, MAX_SUPPORTED_PDF_PAGES},",
    "document_scale::{self, DocumentScalePlan, MAX_SUPPORTED_PDF_PAGES},",
  );

  const planMarker = "    let plan = DocumentScalePlan::for_page_count(pages.len());";
  if (source.includes(planMarker)) {
    source = source.replace(
      planMarker,
      "    let task = document_intelligence::load_document_task_context(database.inner())?;\n    let plan = DocumentScalePlan::for_provider(pages.len(), &task.provider_kind);",
    );
    const duplicate = "    let task = document_intelligence::load_document_task_context(database.inner())?;";
    const first = source.indexOf(duplicate);
    const second = source.indexOf(duplicate, first + duplicate.length);
    if (second >= 0) {
      source = `${source.slice(0, second)}${source.slice(second + duplicate.length + 1)}`;
    }
  }

  if (!source.includes("document_ai_scale_plan")) {
    const marker = "    let outline_contract = document_checkpoints::outline_contract_key(&task, plan);";
    const index = source.indexOf(marker);
    if (index < 0) throw new Error("Could not find outline contract marker");
    const insertion = `    let budget = document_scale::validate_automatic_job(\n        pages.len(),\n        estimated_source_tokens,\n        plan,\n        outline_windows.len(),\n        generation_windows.len(),\n        &task.provider_kind,\n    )?;\n    crate::logging::info(\n        \"document_ai_scale_plan\",\n        format!(\n            \"document={} provider={} scale={} pages={} source_tokens={} outline_windows={} generation_windows={} card_capacity={} estimated_job_tokens={} ceiling={}\",\n            document_id,\n            task.provider_kind,\n            plan.label(),\n            pages.len(),\n            estimated_source_tokens,\n            outline_windows.len(),\n            generation_windows.len(),\n            budget.generation_card_capacity,\n            budget.estimated_tokens,\n            budget.ceiling_tokens,\n        ),\n    );\n\n`;
    source = `${source.slice(0, index)}${insertion}${source.slice(index)}`;
  }

  const recovery = `async fn generate_window_with_recovery(
    runtime: &AiRuntimeState,
    task: &document_intelligence::DocumentTaskContext,
    document_title: &str,
    outline: &document_intelligence::DocumentOutline,
    window: &SemanticWindow,
    images: &[document_intelligence::SourcePageImage],
    maximum_cards: usize,
) -> Result<Vec<serde_json::Value>, CommandError> {
    let first = document_intelligence::generate_window_flashcards(
        runtime,
        task,
        document_title,
        outline,
        window,
        images,
        maximum_cards.max(1),
    )
    .await;
    match first {
        Err(error)
            if maximum_cards > 2
                && (error.code == "ai_document_output_truncated"
                    || (error.code == "ai_document_output_invalid"
                        && error.message.contains("more than"))) =>
        {
            // One reduced retry only. Never recursively keep halving and
            // spending provider tokens after a completion/context failure.
            let reduced_budget = (maximum_cards / 2).max(2);
            crate::logging::info(
                "document_generation_window_compact_retry",
                format!(
                    "window={} partition={} pages={}-{} old_budget={} new_budget={} code={}",
                    window.window_index + 1,
                    window.partition_index + 1,
                    window.page_start,
                    window.page_end,
                    maximum_cards,
                    reduced_budget,
                    error.code,
                ),
            );
            document_intelligence::generate_window_flashcards(
                runtime,
                task,
                document_title,
                outline,
                window,
                images,
                reduced_budget,
            )
            .await
        }
        other => other,
    }
}

`;
  if (source.includes("async fn generate_window_with_recovery(") && !source.includes("document_generation_window_compact_retry")) {
    source = replaceFunction(
      source,
      "async fn generate_window_with_recovery(",
      "fn extract_and_render_pdf",
      recovery,
    );
  }
  return source;
});

await edit("src-tauri/src/document_more.rs", (source) => {
  source = replaceRequired(
    source,
    "additional document_scale self import",
    "document_scale::DocumentScalePlan,",
    "document_scale::{self, DocumentScalePlan},",
  );

  const planMarker = "    let plan = DocumentScalePlan::for_page_count(page_count);";
  if (source.includes(planMarker)) {
    source = source.replace(
      planMarker,
      "    let task = document_intelligence::load_document_task_context(database.inner())?;\n    let plan = DocumentScalePlan::for_provider(page_count, &task.provider_kind);",
    );
    const duplicate = "    let task = document_intelligence::load_document_task_context(database.inner())?;";
    const first = source.indexOf(duplicate);
    const second = source.indexOf(duplicate, first + duplicate.length);
    if (second >= 0) {
      source = `${source.slice(0, second)}${source.slice(second + duplicate.length + 1)}`;
    }
  }

  if (!source.includes("document_ai_additional_scale_plan")) {
    const marker = "    initialize_additional_progress(";
    const index = source.indexOf(marker);
    if (index < 0) throw new Error("Could not find additional progress marker");
    const insertion = `    let estimated_source_tokens = chunks.iter().fold(0_usize, |total, chunk| {\n        total.saturating_add(\n            chunk\n                .token_estimate\n                .max(chunk.full_text.chars().count().div_ceil(4)),\n        )\n    });\n    let budget = document_scale::validate_automatic_job(\n        page_count,\n        estimated_source_tokens,\n        plan,\n        0,\n        windows.len(),\n        &task.provider_kind,\n    )?;\n    crate::logging::info(\n        \"document_ai_additional_scale_plan\",\n        format!(\n            \"document={} provider={} scale={} pages={} source_tokens={} generation_windows={} card_capacity={} estimated_job_tokens={} ceiling={}\",\n            input.document_id,\n            task.provider_kind,\n            plan.label(),\n            page_count,\n            estimated_source_tokens,\n            windows.len(),\n            budget.generation_card_capacity,\n            budget.estimated_tokens,\n            budget.ceiling_tokens,\n        ),\n    );\n\n`;
    source = `${source.slice(0, index)}${insertion}${source.slice(index)}`;
  }

  const recovery = `async fn generate_window_with_recovery(
    runtime: &AiRuntimeState,
    task: &document_intelligence::DocumentTaskContext,
    document_title: &str,
    outline: &document_intelligence::DocumentOutline,
    window: &SemanticWindow,
    images: &[document_intelligence::SourcePageImage],
    maximum_cards: usize,
) -> Result<Vec<Value>, CommandError> {
    let first = document_intelligence::generate_window_flashcards(
        runtime,
        task,
        document_title,
        outline,
        window,
        images,
        maximum_cards.max(1),
    )
    .await;
    match first {
        Err(error)
            if maximum_cards > 2
                && (error.code == "ai_document_output_truncated"
                    || (error.code == "ai_document_output_invalid"
                        && error.message.contains("more than"))) =>
        {
            let reduced_budget = (maximum_cards / 2).max(2);
            crate::logging::info(
                "document_additional_window_compact_retry",
                format!(
                    "window={} partition={} pages={}-{} old_budget={} new_budget={} code={}",
                    window.window_index + 1,
                    window.partition_index + 1,
                    window.page_start,
                    window.page_end,
                    maximum_cards,
                    reduced_budget,
                    error.code,
                ),
            );
            document_intelligence::generate_window_flashcards(
                runtime,
                task,
                document_title,
                outline,
                window,
                images,
                reduced_budget,
            )
            .await
        }
        other => other,
    }
}

`;
  if (source.includes("async fn generate_window_with_recovery(") && !source.includes("document_additional_window_compact_retry")) {
    source = replaceFunction(
      source,
      "async fn generate_window_with_recovery(",
      "fn initialize_additional_progress",
      recovery,
    );
  }
  return source;
});
