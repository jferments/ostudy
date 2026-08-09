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

await edit("src-tauri/src/document_more.rs", (source) => {
  for (const line of [
    "const GENERATION_WINDOW_MAX_TOKENS: usize = 5_000;\n",
    "const GENERATION_WINDOW_MAX_PAGES: usize = 10;\n",
    "const GENERATION_WINDOW_OVERLAP_TOKENS: usize = 700;\n",
    "const MAX_IMAGES_PER_GENERATION_WINDOW: usize = 6;\n",
  ]) source = source.replace(line, "");
  return source;
});

await edit("src-tauri/src/document_scale.rs", (source) => {
  const marker = "    pub(crate) fn for_page_count(page_count: usize) -> Self {";
  if (source.includes(marker) && !source.includes("    #[cfg(test)]\n    pub(crate) fn for_page_count")) {
    source = source.replace(marker, `    #[cfg(test)]\n${marker}`);
  }
  return source;
});

await edit("src-tauri/src/document_semantics.rs", (source) => {
  for (const line of [
    "    pub first_chunk_index: i64,\n",
    "    pub last_chunk_index: i64,\n",
    "    pub page_numbers: Vec<i64>,\n",
    "    pub page_end: i64,\n",
    "    pub token_estimate: usize,\n",
  ]) source = source.replace(line, "");
  for (const line of [
    "            first_chunk_index: chunks[start].chunk_index,\n",
    "            last_chunk_index: chunks[end - 1].chunk_index,\n",
    "            page_numbers,\n",
    "            page_end,\n",
    "            token_estimate: token_total,\n",
  ]) source = source.replace(line, "");
  const marker = "pub(crate) fn build_semantic_windows(\n";
  if (source.includes(marker) && !source.includes("#[cfg(test)]\npub(crate) fn build_semantic_windows")) {
    source = source.replace(marker, `#[cfg(test)]\n${marker}`);
  }
  return source;
});

await edit("src-tauri/src/documents.rs", (source) => {
  for (const line of [
    "const MAX_PDF_PAGES: usize = 2_000;\n",
    "const OUTLINE_WINDOW_MAX_TOKENS: usize = 3_200;\n",
    "const OUTLINE_WINDOW_MAX_PAGES: usize = 8;\n",
    "const OUTLINE_WINDOW_OVERLAP_TOKENS: usize = 350;\n",
    "const GENERATION_WINDOW_MAX_TOKENS: usize = 5_000;\n",
    "const GENERATION_WINDOW_MAX_PAGES: usize = 10;\n",
    "const GENERATION_WINDOW_OVERLAP_TOKENS: usize = 700;\n",
    "const MAX_IMAGES_PER_GENERATION_WINDOW: usize = 6;\n",
  ]) source = source.replace(line, "");
  source = source.replace(
    "generation_windows.iter().zip(cached_generation.into_iter())",
    "generation_windows.iter().zip(cached_generation)",
  );

  if (!source.includes("struct DocumentAiProgressPlan")) {
    const marker = "fn initialize_document_ai_plan(\n";
    const index = source.indexOf(marker);
    if (index < 0) throw new Error("Could not find initialize_document_ai_plan");
    const struct = `#[derive(Clone, Copy, Debug)]\nstruct DocumentAiProgressPlan {\n    estimated_source_tokens: usize,\n    outline_window_count: usize,\n    outline_completed_window_count: usize,\n    generation_window_count: usize,\n    generation_completed_window_count: usize,\n}\n\n`;
    source = `${source.slice(0, index)}${struct}${source.slice(index)}`;
  }
  const oldSignature = `fn initialize_document_ai_plan(\n    database: &Database,\n    document_id: &str,\n    plan: DocumentScalePlan,\n    estimated_source_tokens: usize,\n    outline_window_count: usize,\n    outline_completed_window_count: usize,\n    generation_window_count: usize,\n    generation_completed_window_count: usize,\n) -> Result<(), CommandError> {`;
  const newSignature = `fn initialize_document_ai_plan(\n    database: &Database,\n    document_id: &str,\n    plan: DocumentScalePlan,\n    progress: DocumentAiProgressPlan,\n) -> Result<(), CommandError> {\n    let DocumentAiProgressPlan {\n        estimated_source_tokens,\n        outline_window_count,\n        outline_completed_window_count,\n        generation_window_count,\n        generation_completed_window_count,\n    } = progress;`;
  if (source.includes(oldSignature)) source = source.replace(oldSignature, newSignature);

  const callA = `        initialize_document_ai_plan(\n            database.inner(),\n            document_id,\n            plan,\n            estimated_source_tokens,\n            outline_windows.len(),\n            outline_windows.len(),\n            generation_windows.len(),\n            0,\n        )?;`;
  const replA = `        initialize_document_ai_plan(\n            database.inner(),\n            document_id,\n            plan,\n            DocumentAiProgressPlan {\n                estimated_source_tokens,\n                outline_window_count: outline_windows.len(),\n                outline_completed_window_count: outline_windows.len(),\n                generation_window_count: generation_windows.len(),\n                generation_completed_window_count: 0,\n            },\n        )?;`;
  if (source.includes(callA)) source = source.replace(callA, replA);
  const callB = `        initialize_document_ai_plan(\n            database.inner(),\n            document_id,\n            plan,\n            estimated_source_tokens,\n            outline_windows.len(),\n            outline_completed,\n            generation_windows.len(),\n            0,\n        )?;`;
  const replB = `        initialize_document_ai_plan(\n            database.inner(),\n            document_id,\n            plan,\n            DocumentAiProgressPlan {\n                estimated_source_tokens,\n                outline_window_count: outline_windows.len(),\n                outline_completed_window_count: outline_completed,\n                generation_window_count: generation_windows.len(),\n                generation_completed_window_count: 0,\n            },\n        )?;`;
  if (source.includes(callB)) source = source.replace(callB, replB);
  return source;
});

await edit("src-tauri/src/ai/document_intelligence.rs", (source) => {
  source = source.replace(
    "let maximum_cards = maximum_cards.max(1).min(MAX_WINDOW_CARDS);",
    "let maximum_cards = maximum_cards.clamp(1, MAX_WINDOW_CARDS);",
  );
  if (!source.includes("struct OutlineBuildSources")) {
    const marker = "pub(crate) async fn build_document_outline<F>(\n";
    const index = source.indexOf(marker);
    if (index < 0) throw new Error("Could not find build_document_outline");
    const definition = `pub(crate) struct OutlineBuildSources<'a> {\n    pub windows: &'a [SemanticWindow],\n    pub cached_fragments: &'a [Option<DocumentOutline>],\n    pub window_images: &'a [Vec<SourcePageImage>],\n}\n\n`;
    source = `${source.slice(0, index)}${definition}${source.slice(index)}`;
  }
  const oldSignature = `pub(crate) async fn build_document_outline<F>(\n    runtime: &AiRuntimeState,\n    task: &DocumentTaskContext,\n    document_title: &str,\n    page_count: i64,\n    windows: &[SemanticWindow],\n    cached_fragments: &[Option<DocumentOutline>],\n    window_images: &[Vec<SourcePageImage>],\n    mut persist_fragment: F,\n) -> Result<DocumentOutline, CommandError>\nwhere\n    F: FnMut(&SemanticWindow, &DocumentOutline) -> Result<(), CommandError>,\n{`;
  const newSignature = `pub(crate) async fn build_document_outline<F>(\n    runtime: &AiRuntimeState,\n    task: &DocumentTaskContext,\n    document_title: &str,\n    page_count: i64,\n    sources: OutlineBuildSources<'_>,\n    mut persist_fragment: F,\n) -> Result<DocumentOutline, CommandError>\nwhere\n    F: FnMut(&SemanticWindow, &DocumentOutline) -> Result<(), CommandError>,\n{\n    let OutlineBuildSources {\n        windows,\n        cached_fragments,\n        window_images,\n    } = sources;`;
  if (source.includes(oldSignature)) source = source.replace(oldSignature, newSignature);
  return source;
});

await edit("src-tauri/src/ai/document_intelligence_adapter.rs", (source) => {
  const oldExport = `    curate_document_cards, generate_window_flashcards, load_document_task_context, DocumentOutline,\n    DocumentTaskContext, SourcePageImage, OUTLINE_PROMPT_VERSION, OUTLINE_SCHEMA_VERSION,`;
  const newExport = `    curate_document_cards, generate_window_flashcards, load_document_task_context, DocumentOutline,\n    DocumentTaskContext, OutlineBuildSources, SourcePageImage, OUTLINE_PROMPT_VERSION,\n    OUTLINE_SCHEMA_VERSION,`;
  if (source.includes(oldExport)) source = source.replace(oldExport, newExport);
  const oldCall = `    implementation::build_document_outline(\n        runtime,\n        task,\n        document_title,\n        page_count,\n        windows,\n        cached_fragments,\n        &images,\n        persist_fragment,\n    )`;
  const newCall = `    implementation::build_document_outline(\n        runtime,\n        task,\n        document_title,\n        page_count,\n        OutlineBuildSources {\n            windows,\n            cached_fragments,\n            window_images: &images,\n        },\n        persist_fragment,\n    )`;
  if (source.includes(oldCall)) source = source.replace(oldCall, newCall);
  return source;
});
