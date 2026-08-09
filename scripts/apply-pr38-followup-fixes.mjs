#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

async function patch(relativePath, transform) {
  const path = fileURLToPath(new URL(`../${relativePath}`, import.meta.url));
  const source = await readFile(path, "utf8");
  const updated = transform(source);
  if (updated === source) {
    throw new Error(`No changes applied to ${relativePath}`);
  }
  await writeFile(path, updated, "utf8");
}

function required(source, label, search, replacement) {
  if (!source.includes(search)) throw new Error(`Could not find ${label}`);
  return source.replace(search, replacement);
}

function regexRequired(source, label, pattern, replacement) {
  if (!pattern.test(source)) throw new Error(`Could not find ${label}`);
  return source.replace(pattern, replacement);
}

await patch("src-tauri/src/document_operations.rs", (source) => {
  source = required(
    source,
    "document operations rusqlite import",
    "use rusqlite::{params, OptionalExtension, TransactionBehavior};",
    "use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};",
  );

  source = regexRequired(
    source,
    "document citation deck query",
    /        let mut deck_statement = transaction[\s\S]*?        drop\(deck_statement\);\n/u,
    `        let generated_decks =\n            load_surviving_generated_decks(&transaction, &input.document_id)?;\n`,
  );

  source = required(
    source,
    "document trash reconciliation marker",
    "pub(crate) fn reconcile_deleted_document_trash(paths: &AppPaths) -> Result<(), CommandError> {",
    `fn load_surviving_generated_decks(\n    connection: &Connection,\n    document_id: &str,\n) -> Result<Vec<(String, String)>, CommandError> {\n    let mut statement = connection\n        .prepare(\n            \"SELECT DISTINCT deck.id, deck.description\n             FROM document_decks provenance\n             JOIN decks deck ON deck.id = provenance.deck_id\n             WHERE provenance.document_id = ?1\n               AND deck.deleted_at IS NULL\n             ORDER BY deck.id\",\n        )\n        .map_err(CommandError::database)?;\n    statement\n        .query_map([document_id], |row| {\n            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))\n        })\n        .map_err(CommandError::database)?\n        .collect::<Result<Vec<_>, _>>()\n        .map_err(CommandError::database)\n}\n\npub(crate) fn reconcile_deleted_document_trash(paths: &AppPaths) -> Result<(), CommandError> {`,
  );

  const testInsert = `\n\n    #[test]\n    fn soft_deleted_generated_decks_are_not_counted_for_document_citation_preservation() {\n        let connection = Connection::open_in_memory().expect(\"fixture database should open\");\n        connection\n            .execute_batch(\n                \"CREATE TABLE decks (\n                     id TEXT PRIMARY KEY,\n                     description TEXT NOT NULL,\n                     deleted_at INTEGER\n                 );\n                 CREATE TABLE document_decks (\n                     document_id TEXT NOT NULL,\n                     deck_id TEXT NOT NULL\n                 );\n                 INSERT INTO decks (id, description, deleted_at) VALUES\n                     ('active', 'active deck', NULL),\n                     ('deleted', 'deleted deck', 123);\n                 INSERT INTO document_decks (document_id, deck_id) VALUES\n                     ('document', 'active'),\n                     ('document', 'deleted');\",\n            )\n            .expect(\"fixture schema should initialize\");\n\n        let decks = load_surviving_generated_decks(&connection, \"document\")\n            .expect(\"surviving deck lookup should succeed\");\n        assert_eq!(decks, vec![(\"active\".to_string(), \"active deck\".to_string())]);\n    }\n`;
  const end = source.lastIndexOf("\n}");
  if (end < 0) throw new Error("Could not find document_operations test module end");
  return `${source.slice(0, end)}${testInsert}${source.slice(end)}`;
});

await patch("src/screens/DocumentsScreen.tsx", (source) => {
  source = required(
    source,
    "document delete confirmation",
    `    const accepted = await confirm({\n      title: \"Delete this PDF permanently?\",\n      description:\n        \"The managed source PDF, rendered pages, extracted structure, outline, and generation history for this document will be removed. Generated flashcards and decks will remain; their source will be preserved as a text citation instead of a file link. This cannot be undone.\",`,
    `    const hasSurvivingGeneratedDeck = document.generatedDeckId !== null;\n    const accepted = await confirm({\n      title: \"Delete this PDF permanently?\",\n      description: hasSurvivingGeneratedDeck\n        ? \"The managed source PDF, rendered pages, extracted structure, outline, and generation history for this document will be removed. The surviving generated deck will remain and its source will be preserved as a text citation instead of a file link. This cannot be undone.\"\n        : \"The managed source PDF, rendered pages, extracted structure, outline, and generation history for this document will be removed. No active generated deck remains, so no source citation will be preserved. This cannot be undone.\",`,
  );
  source = required(
    source,
    "render processing phase labels",
    `  if (document.pageCount === 0 || document.renderedPageCount === 0) {\n    return \"Extracting text / rendering pages\";\n  }\n  if (document.renderedPageCount < document.pageCount) return \"Rendering pages\";`,
    `  if (document.pageCount === 0) return \"Extracting text\";\n  if (document.renderedPageCount < document.pageCount) return \"Rendering pages\";`,
  );
  return source;
});

await patch("src/screens/DocumentsScreen.test.tsx", (source) => {
  source = required(
    source,
    "processing phase test label",
    `    expect(screen.getByText(\"Extracting text / rendering pages\")).toBeTruthy();`,
    `    expect(screen.getByText(\"Extracting text\")).toBeTruthy();`,
  );

  source = required(
    source,
    "stop generation test marker",
    `  it(\"offers an exact stop action while a document generation task is processing\", async () => {`,
    `  it(\"shows live rendered-page progress once the total page count is known\", async () => {\n    vi.mocked(documentIpc.listDocuments).mockResolvedValue([\n      makeDocument({\n        lifecycleState: \"processing\",\n        pageCount: 11,\n        extractedPageCount: 11,\n        renderedPageCount: 4,\n        generatedDeckId: null,\n        generatedCardCount: 0,\n        completedAt: null,\n      }),\n    ]);\n    renderDocuments();\n\n    expect(await screen.findByText(\"4/11 rendered\")).toBeTruthy();\n    expect(screen.getByText(\"Rendering pages\")).toBeTruthy();\n  });\n\n  it(\"offers an exact stop action while a document generation task is processing\", async () => {`,
  );

  const extraDeleteTest = `\n\n  it(\"does not claim citation preservation after the only generated deck was deleted\", async () => {\n    const withoutDeck = makeDocument({\n      generatedDeckId: null,\n      lifecycleState: \"ready\",\n      generatedCardCount: 7,\n    });\n    vi.mocked(documentIpc.listDocuments)\n      .mockResolvedValueOnce([withoutDeck])\n      .mockResolvedValue([]);\n    vi.mocked(documentIpc.deletePdfDocument).mockResolvedValue({\n      documentId,\n      preservedDeckCitationCount: 0,\n    });\n    const { showToast } = renderDocuments();\n\n    await screen.findByRole(\"heading\", { name: \"Cardiac physiology\" });\n    fireEvent.click(screen.getByRole(\"button\", { name: \"Delete PDF\" }));\n    const dialog = await screen.findByRole(\"alertdialog\", {\n      name: \"Delete this PDF permanently?\",\n    });\n    expect(within(dialog).getByText(/No active generated deck remains/u)).toBeTruthy();\n    fireEvent.click(within(dialog).getByRole(\"button\", { name: \"Delete PDF\" }));\n\n    await waitFor(() => {\n      expect(showToast).toHaveBeenCalledWith(\"success\", \"PDF deleted from Documents.\");\n    });\n  });\n`;
  const end = source.lastIndexOf("\n});");
  if (end < 0) throw new Error("Could not find DocumentsScreen test suite end");
  return `${source.slice(0, end)}${extraDeleteTest}${source.slice(end)}`;
});

await patch("src/screens/SettingsScreen.tsx", (source) => {
  source = required(
    source,
    "obsolete AI model shell import",
    `import aiModelStyles from \"./settings/DefaultAiModelSettings.module.css\";\n`,
    "",
  );
  source = required(
    source,
    "AI settings panel layout",
    `      panel: (\n        <div className={aiModelStyles.shell}>\n          <DefaultAiModelSettings controller={aiController} />\n          <AiLlmSettings controller={aiController} />\n        </div>\n      ),`,
    `      panel: (\n        <AiLlmSettings\n          controller={aiController}\n          sidebarTop={<DefaultAiModelSettings controller={aiController} />}\n        />\n      ),`,
  );
  return source;
});

await patch("src/screens/settings/AiLlmSettings.tsx", (source) => {
  source = required(
    source,
    "ReactNode import",
    `import type { AiProfilesController } from \"../../controllers/aiProfilesController\";`,
    `import type { ReactNode } from \"react\";\n\nimport type { AiProfilesController } from \"../../controllers/aiProfilesController\";`,
  );
  source = required(
    source,
    "AI settings props",
    `interface AiLlmSettingsProps {\n  controller: AiProfilesController;\n}\n\nexport function AiLlmSettings({ controller }: AiLlmSettingsProps) {`,
    `interface AiLlmSettingsProps {\n  controller: AiProfilesController;\n  sidebarTop?: ReactNode;\n}\n\nexport function AiLlmSettings({ controller, sidebarTop }: AiLlmSettingsProps) {`,
  );
  source = required(
    source,
    "AI sidebar start",
    `      <aside className={styles.aiSidebar} aria-label=\"AI provider profiles\">`,
    `      <div className={styles.aiSidebarStack}>\n        {sidebarTop}\n        <aside className={styles.aiSidebar} aria-label=\"AI provider profiles\">`,
  );
  source = required(
    source,
    "AI sidebar end",
    `      </aside>\n\n      <main className={styles.aiDetail}>`,
    `        </aside>\n      </div>\n\n      <main className={styles.aiDetail}>`,
  );
  return source;
});

await patch("src/screens/settings/Settings.module.css", (source) => {
  source = required(
    source,
    "AI sidebar style marker",
    `.aiSidebar {\n  position: sticky;`,
    `.aiSidebarStack {\n  display: grid;\n  min-width: 0;\n  align-self: start;\n  gap: 18px;\n}\n\n.aiSidebar {\n  position: sticky;`,
  );
  return source;
});

await patch("src/screens/settings/DefaultAiModelSettings.module.css", (source) => {
  source = regexRequired(
    source,
    "legacy default model shell rules",
    /^\.shell \{[\s\S]*?(?=\.card \{)/u,
    "",
  );
  source = source.replace(/  grid-column: 1;\n  grid-row: 1;\n/u, "");
  source = regexRequired(
    source,
    "mobile legacy shell rules",
    /@media \(max-width: 760px\) \{[\s\S]*?\n\}\n\n(?=@media \(max-width: 520px\))/u,
    "",
  );
  return source;
});

await patch("src-tauri/src/documents.rs", (source) => {
  source = required(
    source,
    "outline window limits",
    `const OUTLINE_WINDOW_MAX_TOKENS: usize = 5_000;\nconst OUTLINE_WINDOW_MAX_PAGES: usize = 10;\nconst OUTLINE_WINDOW_OVERLAP_TOKENS: usize = 500;`,
    `const OUTLINE_WINDOW_MAX_TOKENS: usize = 3_200;\nconst OUTLINE_WINDOW_MAX_PAGES: usize = 8;\nconst OUTLINE_WINDOW_OVERLAP_TOKENS: usize = 350;`,
  );

  source = required(
    source,
    "spawn blocking extraction/render",
    `        let paths_owned = paths.inner().clone();\n        let source_owned = source_path.clone();\n        let id_owned = document_id.to_string();\n        let processed = tauri::async_runtime::spawn_blocking(move || {\n            extract_and_render_pdf(&paths_owned, &render_runtime, &id_owned, &source_owned)\n        })\n        .await\n        .map_err(|error| {\n            CommandError::new(\n                \"document_processing_failed\",\n                format!(\"The PDF processing task stopped unexpectedly: {error}\"),\n            )\n        })??;\n        persist_processed_document(database.inner(), document_id, &processed)?;`,
    `        let paths_owned = paths.inner().clone();\n        let source_owned = source_path.clone();\n        let id_owned = document_id.to_string();\n        let (progress_sender, mut progress_receiver) =\n            tokio::sync::mpsc::unbounded_channel::<(usize, usize)>();\n        let processing_task = tauri::async_runtime::spawn_blocking(move || {\n            extract_and_render_pdf(\n                &paths_owned,\n                &render_runtime,\n                &id_owned,\n                &source_owned,\n                |completed, total| {\n                    progress_sender.send((completed, total)).map_err(|_| {\n                        CommandError::new(\n                            \"document_processing_stopped\",\n                            \"The PDF processing progress channel closed unexpectedly.\",\n                        )\n                    })\n                },\n            )\n        });\n        while let Some((completed, total)) = progress_receiver.recv().await {\n            update_render_progress(database.inner(), document_id, completed, total)?;\n        }\n        let processed = processing_task.await.map_err(|error| {\n            CommandError::new(\n                \"document_processing_failed\",\n                format!(\"The PDF processing task stopped unexpectedly: {error}\"),\n            )\n        })??;\n        persist_processed_document(database.inner(), document_id, &processed)?;`,
  );

  source = required(
    source,
    "extract render signature",
    `fn extract_and_render_pdf(\n    paths: &AppPaths,\n    render_runtime: &PdfRenderRuntime,\n    document_id: &str,\n    source_path: &Path,\n) -> Result<ProcessedDocument, CommandError> {`,
    `fn extract_and_render_pdf<F>(\n    paths: &AppPaths,\n    render_runtime: &PdfRenderRuntime,\n    document_id: &str,\n    source_path: &Path,\n    mut report_progress: F,\n) -> Result<ProcessedDocument, CommandError>\nwhere\n    F: FnMut(usize, usize) -> Result<(), CommandError>,\n{`,
  );

  source = required(
    source,
    "render page directory marker",
    `    let page_directory = paths.document_pages_dir(document_id);`,
    `    report_progress(0, semantic.page_count)?;\n\n    let page_directory = paths.document_pages_dir(document_id);`,
  );

  source = required(
    source,
    "rendered push progress",
    `            rendered.push((\n                page_number,\n                relative,\n                i64::try_from(png.len()).unwrap_or(i64::MAX),\n                sha256_hex(&png),\n            ));\n            Ok(())`,
    `            rendered.push((\n                page_number,\n                relative,\n                i64::try_from(png.len()).unwrap_or(i64::MAX),\n                sha256_hex(&png),\n            ));\n            report_progress(rendered.len(), semantic.page_count)?;\n            Ok(())`,
  );

  source = required(
    source,
    "persist processed marker",
    `fn persist_processed_document(\n    database: &Database,`,
    `fn update_render_progress(\n    database: &Database,\n    document_id: &str,\n    rendered_pages: usize,\n    total_pages: usize,\n) -> Result<(), CommandError> {\n    let total = i64::try_from(total_pages).unwrap_or(i64::MAX);\n    let rendered = i64::try_from(rendered_pages).unwrap_or(i64::MAX).min(total);\n    let now = unix_time_millis()?;\n    database.with_connection(|connection| {\n        let changed = connection\n            .execute(\n                \"UPDATE documents\n                 SET page_count = ?1,\n                     extracted_page_count = ?1,\n                     rendered_page_count = ?2,\n                     updated_at = ?3\n                 WHERE id = ?4 AND lifecycle_state = 'processing'\",\n                params![total, rendered, now, document_id],\n            )\n            .map_err(CommandError::database)?;\n        if changed != 1 {\n            return Err(CommandError::new(\n                \"document_changed\",\n                \"The document changed while PDF rendering progress was being recorded.\",\n            ));\n        }\n        Ok(())\n    })\n}\n\nfn persist_processed_document(\n    database: &Database,`,
  );
  return source;
});

await patch("src-tauri/src/ai/gateway.rs", (source) => {
  source = required(
    source,
    "empty stream diagnostic",
    `        if stream.content.trim().is_empty() && stream.tool_arguments.trim().is_empty() {\n            return Err(error(\n                \"ai_empty_response\",\n                \"The model returned no final document-task content.\",\n                true,\n                None,\n            ));\n        }`,
    `        if stream.content.trim().is_empty() && stream.tool_arguments.trim().is_empty() {\n            let finish = stream.finish_reason.as_deref().unwrap_or(\"not provided\");\n            return Err(error(\n                \"ai_empty_response\",\n                &format!(\n                    \"The provider ended the streamed document request with finish_reason={finish}, but returned no final content or structured tool arguments.\"\n                ),\n                true,\n                None,\n            ));\n        }`,
  );

  source = required(
    source,
    "stream delta content block",
    `        if let Some(content) = delta\n            .and_then(|delta| delta.get(\"content\"))\n            .and_then(Value::as_str)\n        {\n            if self.content.len().saturating_add(content.len()) > self.maximum_content_bytes {\n                return Err(error(\n                    \"ai_response_too_large\",\n                    \"The AI provider returned more generated content than ostudy accepts.\",\n                    false,\n                    None,\n                ));\n            }\n            self.content.push_str(content);\n        }`,
    `        if let Some(content) = delta\n            .and_then(|delta| delta.get(\"content\"))\n            .and_then(Value::as_str)\n        {\n            self.append_compat_content(content)?;\n        } else if let Some(content) = choice\n            .and_then(|choice| choice.get(\"message\"))\n            .and_then(|message| message.get(\"content\"))\n            .and_then(Value::as_str)\n            .or_else(|| choice.and_then(|choice| choice.get(\"text\")).and_then(Value::as_str))\n            .or_else(|| value.get(\"content\").and_then(Value::as_str))\n        {\n            // Some llama.cpp/Unsloth-compatible streaming builds emit a final\n            // full message, completion text, or native top-level content field\n            // instead of OpenAI delta.content chunks. Accept those safe text\n            // variants without duplicating a final full message after deltas.\n            self.append_compat_content(content)?;\n        }`,
  );

  source = required(
    source,
    "SSE process line end",
    `        Ok(())\n    }\n}\n\nfn trim_ascii`,
    `        Ok(())\n    }\n\n    fn append_compat_content(&mut self, content: &str) -> Result<(), AiGatewayError> {\n        if content.is_empty() {\n            return Ok(());\n        }\n        if !self.content.is_empty() && content.starts_with(&self.content) {\n            // A provider may emit one final full-message snapshot after already\n            // streaming its deltas. In that case the snapshot adds no new text.\n            return Ok(());\n        }\n        if self.content.len().saturating_add(content.len()) > self.maximum_content_bytes {\n            return Err(error(\n                \"ai_response_too_large\",\n                \"The AI provider returned more generated content than ostudy accepts.\",\n                false,\n                None,\n            ));\n        }\n        self.content.push_str(content);\n        Ok(())\n    }\n}\n\nfn trim_ascii`,
  );

  const extraTests = `\n\n    #[test]\n    fn sse_accumulator_accepts_llama_full_message_compatibility_events() {\n        let mut stream = SseChatAccumulator::new(2048, 8192);\n        stream\n            .push_bytes(\n                br#\"data: {\"choices\":[{\"message\":{\"content\":\"{\\\"summary\\\":\\\"ok\\\"}\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\"#,\n            )\n            .unwrap();\n        stream.finish().unwrap();\n        assert_eq!(stream.content, \"{\\\"summary\\\":\\\"ok\\\"}\");\n        assert_eq!(stream.finish_reason.as_deref(), Some(\"stop\"));\n    }\n\n    #[test]\n    fn sse_accumulator_accepts_llama_native_top_level_content() {\n        let mut stream = SseChatAccumulator::new(2048, 8192);\n        stream\n            .push_bytes(\n                br#\"data: {\"content\":\"{\\\"summary\\\":\"}\n\ndata: {\"content\":\"\\\"ok\\\"}\"}\n\ndata: [DONE]\n\"#,\n            )\n            .unwrap();\n        stream.finish().unwrap();\n        assert_eq!(stream.content, \"{\\\"summary\\\":\\\"ok\\\"}\");\n    }\n`;
  const end = source.lastIndexOf("\n}");
  if (end < 0) throw new Error("Could not find gateway test module end");
  return `${source.slice(0, end)}${extraTests}${source.slice(end)}`;
});

await patch("src-tauri/src/ai/document_intelligence.rs", (source) => {
  source = required(source, "outline prompt version", "const OUTLINE_PROMPT_VERSION: i64 = 2;", "const OUTLINE_PROMPT_VERSION: i64 = 3;");
  source = required(
    source,
    "outline constants marker",
    `const MAX_INLINE_IMAGES: usize = 6;`,
    `const MAX_INLINE_IMAGES: usize = 6;\nconst MAX_FRAGMENT_OBJECTIVES: usize = 10;\nconst MAX_FRAGMENT_CONCEPTS: usize = 16;\nconst MAX_FRAGMENT_SECTIONS: usize = 10;\nconst MAX_FRAGMENT_SECTION_OBJECTIVES: usize = 6;\nconst MAX_FRAGMENT_SECTION_CONCEPTS: usize = 10;\nconst MAX_GLOBAL_SYNTHESIS_INPUT_CHARS: usize = 12_000;`,
  );

  source = required(
    source,
    "global context struct marker",
    `#[derive(Debug, Deserialize)]\n#[serde(deny_unknown_fields)]\nstruct FlashcardOutput {`,
    `#[derive(Debug, Deserialize)]\n#[serde(rename_all = \"camelCase\", deny_unknown_fields)]\nstruct GlobalOutlineSynthesis {\n    summary: String,\n    learning_objectives: Vec<String>,\n    key_concepts: Vec<String>,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(deny_unknown_fields)]\nstruct FlashcardOutput {`,
  );

  source = required(
    source,
    "native outline assembly return",
    `    let outline = assemble_outline_fragments(document_title, page_count, fragments);\n    crate::logging::info(\n        \"document_outline_assembled\",\n        format!(\n            \"sections={} objectives={} concepts={} pages={}\",\n            outline.sections.len(),\n            outline.learning_objectives.len(),\n            outline.key_concepts.len(),\n            page_count,\n        ),\n    );\n    Ok(outline)`,
    `    let mut outline = assemble_outline_fragments(document_title, page_count, fragments);\n    match synthesize_global_outline_context(runtime, task, document_title, page_count, &outline).await {\n        Ok(global) => {\n            outline.summary = sanitize_outline_text(\n                &global.summary,\n                8_000,\n                &outline.summary,\n            );\n            let objectives = normalize_outline_items(global.learning_objectives, 24, 1_200);\n            if !objectives.is_empty() {\n                outline.learning_objectives = objectives;\n            }\n            let concepts = normalize_outline_items(global.key_concepts, 40, 700);\n            if !concepts.is_empty() {\n                outline.key_concepts = concepts;\n            }\n            crate::logging::info(\n                \"document_global_outline_synthesized\",\n                format!(\n                    \"sections={} objectives={} concepts={} pages={}\",\n                    outline.sections.len(),\n                    outline.learning_objectives.len(),\n                    outline.key_concepts.len(),\n                    page_count,\n                ),\n            );\n        }\n        Err(error) => {\n            // Native assembly is already a valid whole-document context. The\n            // global LLM synthesis improves semantic prioritization but is never\n            // allowed to make document processing brittle.\n            crate::logging::info(\n                \"document_global_outline_synthesis_skipped\",\n                format!(\"code={} detail={}\", error.code, bounded_diagnostic(&error.message, 400)),\n            );\n        }\n    }\n    crate::logging::info(\n        \"document_outline_assembled\",\n        format!(\n            \"sections={} objectives={} concepts={} pages={}\",\n            outline.sections.len(),\n            outline.learning_objectives.len(),\n            outline.key_concepts.len(),\n            page_count,\n        ),\n    );\n    Ok(outline)`,
  );

  source = required(
    source,
    "outline fragment request guidance",
    `         Build an outline fragment for the educationally meaningful content in this bounded source window. Preserve section/chapter hierarchy and page spans that are actually supported by this window. \\\n         If rendered pages are attached, use them as source evidence for diagrams, tables, equations, and pages with little extractable text. \\\n         Exclude bibliography/reference material, legal/publishing scaffolding, indexes/navigation, and incidental trivia.\",`,
    `         Build a CONCISE outline fragment for the educationally meaningful content in this bounded source window. Preserve section/chapter hierarchy and page spans actually supported by this window. \\\n         Return at most 10 learning objectives, 16 key concepts, and 10 sections. Keep the fragment summary to roughly 3–6 sentences; each section should have a short summary, at most 6 learning objectives, and at most 10 key concepts. \\\n         If rendered pages are attached, use them as source evidence for diagrams, tables, equations, and pages with little extractable text. \\\n         Exclude bibliography/reference material, legal/publishing scaffolding, indexes/navigation, and incidental trivia.\",`,
  );

  source = required(source, "outline schema selection", "    let schema = outline_schema();", "    let schema = outline_fragment_schema();");
  source = required(
    source,
    "outline first completion allowance",
    `            max_tokens: 3_072,\n            images,`,
    `            max_tokens: outline_completion_tokens(task),\n            images,`,
  );
  source = required(
    source,
    "outline retry completion allowance",
    `                    max_tokens: 3_072,\n                    images,`,
    `                    max_tokens: outline_completion_tokens(task),\n                    images,`,
  );

  source = required(
    source,
    "flashcards retry marker",
    `async fn request_flashcards_with_retry(`,
    `async fn synthesize_global_outline_context(\n    runtime: &AiRuntimeState,\n    task: &DocumentTaskContext,\n    document_title: &str,\n    page_count: i64,\n    outline: &DocumentOutline,\n) -> Result<GlobalOutlineSynthesis, CommandError> {\n    let digest = global_outline_digest(outline);\n    let schema = global_outline_schema();\n    let system = format!(\n        \"You are ostudy's bounded whole-document learning planner. The supplied digest was assembled from every validated local outline fragment in the document. Synthesize a concise GLOBAL learning view for later flashcard generation. Do not recreate section hierarchy, invent source facts, or expand minor details. Return only the required structured result. Outline prompt version: {OUTLINE_PROMPT_VERSION}.\"\n    );\n    let user = format!(\n        \"Document: {document_title}\\nTotal PDF pages: {page_count}\\n\\n<ostudy-whole-document-digest>\\n{digest}\\n</ostudy-whole-document-digest>\\n\\nProduce one concise whole-document summary, at most 16 major learning objectives, and at most 24 key concepts. These fields must represent the document as a whole rather than only its beginning.\"\n    );\n    request_json(\n        runtime,\n        task,\n        JsonTaskRequest {\n            system_prompt: &system,\n            user_text: &user,\n            schema: &schema,\n            max_tokens: 2_048,\n            images: &[],\n        },\n    )\n    .await\n    .and_then(|value| {\n        serde_json::from_value(value).map_err(|error| {\n            output_error(format!(\n                \"The provider returned a global outline synthesis that could not be decoded: {}\",\n                bounded_diagnostic(&error.to_string(), 320)\n            ))\n        })\n    })\n}\n\nfn outline_completion_tokens(task: &DocumentTaskContext) -> usize {\n    match task.profile.provider_kind {\n        ProviderKind::Deepseek => 6_144,\n        ProviderKind::LlamaCpp => 3_072,\n        _ => 3_072,\n    }\n}\n\nasync fn request_flashcards_with_retry(`,
  );

  source = required(
    source,
    "outline schema function name",
    `fn outline_schema() -> Value {`,
    `fn outline_fragment_schema() -> Value {`,
  );
  source = required(source, "outline summary schema bound", `            \"summary\": string_schema(MAX_OUTLINE_TEXT_CHARS),`, `            \"summary\": string_schema(4_000),`);
  source = required(source, "outline objective schema bound", `            \"learningObjectives\": string_array_schema(MAX_OUTLINE_OBJECTIVES, 2_000),`, `            \"learningObjectives\": string_array_schema(MAX_FRAGMENT_OBJECTIVES, 1_200),`);
  source = required(source, "outline concept schema bound", `            \"keyConcepts\": string_array_schema(MAX_OUTLINE_CONCEPTS, 1_000),`, `            \"keyConcepts\": string_array_schema(MAX_FRAGMENT_CONCEPTS, 700),`);
  source = required(source, "outline section count schema", `                \"maxItems\": MAX_OUTLINE_SECTIONS,`, `                \"maxItems\": MAX_FRAGMENT_SECTIONS,`);
  source = required(source, "outline title schema bound", `                        \"title\": string_schema(2_000),`, `                        \"title\": string_schema(500),`);
  source = required(source, "outline section summary schema bound", `                        \"summary\": string_schema(10_000),`, `                        \"summary\": string_schema(2_500),`);
  source = required(source, "outline section objective schema bound", `                        \"learningObjectives\": string_array_schema(32, 2_000),`, `                        \"learningObjectives\": string_array_schema(MAX_FRAGMENT_SECTION_OBJECTIVES, 1_000),`);
  source = required(source, "outline section concept schema bound", `                        \"keyConcepts\": string_array_schema(48, 1_000)`, `                        \"keyConcepts\": string_array_schema(MAX_FRAGMENT_SECTION_CONCEPTS, 600)`);

  source = required(
    source,
    "flashcard schema marker for global schema",
    `fn flashcard_schema() -> Value {`,
    `fn global_outline_schema() -> Value {\n    json!({\n        \"type\": \"object\",\n        \"additionalProperties\": false,\n        \"required\": [\"summary\", \"learningObjectives\", \"keyConcepts\"],\n        \"properties\": {\n            \"summary\": string_schema(8_000),\n            \"learningObjectives\": string_array_schema(16, 1_200),\n            \"keyConcepts\": string_array_schema(24, 700)\n        }\n    })\n}\n\nfn flashcard_schema() -> Value {`,
  );

  source = required(
    source,
    "outline context marker for digest",
    `fn outline_context_for_window(`,
    `fn global_outline_digest(outline: &DocumentOutline) -> String {\n    let section_count = outline.sections.len().max(1);\n    let section_budget = (MAX_GLOBAL_SYNTHESIS_INPUT_CHARS / section_count).clamp(120, 650);\n    let mut digest = String::new();\n    digest.push_str(\"Native whole-document summary: \" );\n    digest.push_str(&sanitize_outline_text(&outline.summary, 2_000, \"\"));\n    digest.push_str(\"\\nMajor objectives collected across fragments:\\n\");\n    for objective in outline.learning_objectives.iter().take(32) {\n        digest.push_str(\"- \" );\n        digest.push_str(&sanitize_outline_text(objective, 280, \"\"));\n        digest.push('\\n');\n    }\n    digest.push_str(\"Major concepts collected across fragments:\\n\");\n    for concept in outline.key_concepts.iter().take(48) {\n        digest.push_str(\"- \" );\n        digest.push_str(&sanitize_outline_text(concept, 180, \"\"));\n        digest.push('\\n');\n    }\n    digest.push_str(\"Sections spanning the full document:\\n\");\n    for section in &outline.sections {\n        let title = sanitize_outline_text(&section.title, 160, \"Section\");\n        let summary_budget = section_budget.saturating_sub(title.chars().count()).max(40);\n        let summary = sanitize_outline_text(&section.summary, summary_budget, \"\");\n        digest.push_str(&format!(\n            \"- pages {}–{} | {} | {}\\n\",\n            section.page_start, section.page_end, title, summary\n        ));\n    }\n    digest.chars().take(MAX_GLOBAL_SYNTHESIS_INPUT_CHARS).collect()\n}\n\nfn outline_context_for_window(`,
  );

  source = required(source, "fragment normalization objectives", `    let learning_objectives = normalize_outline_items(outline.learning_objectives, 24, 1_200);`, `    let learning_objectives = normalize_outline_items(outline.learning_objectives, MAX_FRAGMENT_OBJECTIVES, 1_000);`);
  source = required(source, "fragment normalization concepts", `    let key_concepts = normalize_outline_items(outline.key_concepts, 40, 700);`, `    let key_concepts = normalize_outline_items(outline.key_concepts, MAX_FRAGMENT_CONCEPTS, 600);`);
  source = required(source, "fragment normalization sections", `        .take(48)`, `        .take(MAX_FRAGMENT_SECTIONS)`);
  source = required(source, "fragment section objectives", `                    12,\n                    1_000,`, `                    MAX_FRAGMENT_SECTION_OBJECTIVES,\n                    900,`);
  source = required(source, "fragment section concepts", `                key_concepts: normalize_outline_items(section.key_concepts, 18, 600),`, `                key_concepts: normalize_outline_items(\n                    section.key_concepts,\n                    MAX_FRAGMENT_SECTION_CONCEPTS,\n                    500,\n                ),`);

  const tests = `\n\n    #[test]\n    fn outline_fragment_schema_is_deliberately_small() {\n        let schema = outline_fragment_schema();\n        assert_eq!(schema.pointer(\"/properties/sections/maxItems\").and_then(Value::as_u64), Some(MAX_FRAGMENT_SECTIONS as u64));\n        assert_eq!(outline_completion_tokens(&DocumentTaskContext {\n            profile_id: \"x\".to_string(),\n            provider_kind: \"deepseek\".to_string(),\n            model_id: \"m\".to_string(),\n            profile_revision: 1,\n            credential_epoch: 0,\n            network_scope: \"cloud\".to_string(),\n            include_page_images: false,\n            profile: ProfileSnapshot {\n                id: \"x\".to_string(),\n                display_name: \"x\".to_string(),\n                provider_kind: ProviderKind::Deepseek,\n                base_url: \"https://api.deepseek.com/\".to_string(),\n                model_id: \"m\".to_string(),\n                auth_mode: super::super::catalog::AuthMode::Bearer,\n                enabled: true,\n                allow_insecure_http: false,\n                request_timeout_ms: 60_000,\n                revision: 1,\n                credential_epoch: 0,\n            },\n            credential_storage: CredentialStorage::None,\n        }), 6_144);\n    }\n\n    #[test]\n    fn global_outline_digest_reaches_the_end_of_the_document() {\n        let outline = DocumentOutline {\n            summary: \"Whole document\".to_string(),\n            learning_objectives: vec![\"Global objective\".to_string()],\n            key_concepts: vec![\"Global concept\".to_string()],\n            sections: vec![\n                OutlineSection {\n                    title: \"Beginning\".to_string(),\n                    page_start: 1,\n                    page_end: 5,\n                    summary: \"Opening concepts\".to_string(),\n                    learning_objectives: Vec::new(),\n                    key_concepts: Vec::new(),\n                },\n                OutlineSection {\n                    title: \"Ending\".to_string(),\n                    page_start: 130,\n                    page_end: 138,\n                    summary: \"Final concepts\".to_string(),\n                    learning_objectives: Vec::new(),\n                    key_concepts: Vec::new(),\n                },\n            ],\n        };\n        let digest = global_outline_digest(&outline);\n        assert!(digest.contains(\"Beginning\"));\n        assert!(digest.contains(\"Ending\"));\n        assert!(digest.contains(\"130–138\"));\n        assert!(digest.chars().count() <= MAX_GLOBAL_SYNTHESIS_INPUT_CHARS);\n    }\n`;
  const end = source.lastIndexOf("\n}");
  if (end < 0) throw new Error("Could not find document intelligence test module end");
  return `${source.slice(0, end)}${tests}${source.slice(end)}`;
});

await patch("src-tauri/src/document_pdf_tests.rs", (source) => {
  source = required(
    source,
    "render_pages test call",
    `    let page_count = pdf_render_backend::render_pages(&runtime, &source, |page_number, png| {`,
    `    let page_count = pdf_render_backend::render_pages(&runtime, &source, |page_number, png| {`,
  );
  // This file is intentionally touched only if later renderer API changes are needed.
  return `${source}\n`;
});
