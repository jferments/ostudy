#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

async function patchFile(relativePath, transform) {
  const path = fileURLToPath(new URL(`../${relativePath}`, import.meta.url));
  const source = await readFile(path, "utf8");
  const updated = transform(source);
  if (updated !== source) await writeFile(path, updated, "utf8");
}

function replaceRequired(source, label, search, replacement) {
  if (!source.includes(search)) throw new Error(`Could not find ${label}`);
  return source.replace(search, replacement);
}

function replaceBlock(source, label, startNeedle, endNeedle, replacement) {
  const start = source.indexOf(startNeedle);
  if (start < 0) throw new Error(`Could not find start of ${label}`);
  const end = source.indexOf(endNeedle, start);
  if (end < 0) throw new Error(`Could not find end of ${label}`);
  return `${source.slice(0, start)}${replacement}${source.slice(end)}`;
}

await patchFile("src-tauri/src/documents.rs", (source) => {
  source = replaceRequired(
    source,
    "document AI window constants",
    `const OUTLINE_WINDOW_MAX_TOKENS: usize = 32_000;
const OUTLINE_WINDOW_MAX_PAGES: usize = 48;
const OUTLINE_WINDOW_OVERLAP_TOKENS: usize = 2_000;
const GENERATION_WINDOW_MAX_TOKENS: usize = 20_000;
const GENERATION_WINDOW_MAX_PAGES: usize = 40;
const GENERATION_WINDOW_OVERLAP_TOKENS: usize = 2_000;`,
    `// Keep each provider request comfortably below common local llama-server
// context allocations. Large documents are handled by more semantic windows,
// not by making individual prompts approach the model's advertised maximum.
const OUTLINE_WINDOW_MAX_TOKENS: usize = 5_000;
const OUTLINE_WINDOW_MAX_PAGES: usize = 10;
const OUTLINE_WINDOW_OVERLAP_TOKENS: usize = 500;
const GENERATION_WINDOW_MAX_TOKENS: usize = 5_000;
const GENERATION_WINDOW_MAX_PAGES: usize = 10;
const GENERATION_WINDOW_OVERLAP_TOKENS: usize = 700;`,
  );
  return source;
});

await patchFile("src-tauri/src/ai/gateway.rs", (initial) => {
  if (initial.includes("finish_reason: Option<String>")) return initial;
  let source = initial;
  source = replaceRequired(
    source,
    "SSE finish reason field",
    `    tool_name: Option<String>,
    tool_arguments: String,
    wire_bytes: usize,`,
    `    tool_name: Option<String>,
    tool_arguments: String,
    finish_reason: Option<String>,
    wire_bytes: usize,`,
  );
  source = replaceRequired(
    source,
    "SSE finish reason initialization",
    `            tool_name: None,
            tool_arguments: String::new(),
            wire_bytes: 0,`,
    `            tool_name: None,
            tool_arguments: String::new(),
            finish_reason: None,
            wire_bytes: 0,`,
  );
  source = replaceRequired(
    source,
    "reconstructed streaming response",
    `        Ok(json!({ "choices": [{ "message": message }] }))`,
    `        Ok(json!({
            "choices": [{
                "message": message,
                "finish_reason": stream.finish_reason
            }]
        }))`,
  );
  source = replaceRequired(
    source,
    "SSE choice parsing",
    `        let delta = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("delta"));`,
    `        let choice = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first());
        if let Some(reason) = choice
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(Value::as_str)
            .filter(|reason| !reason.is_empty())
        {
            self.finish_reason = Some(reason.to_string());
        }
        let delta = choice.and_then(|choice| choice.get("delta"));`,
  );
  source = replaceRequired(
    source,
    "SSE finish-reason test anchor",
    `    #[test]
    fn sse_accumulator_bounds_generated_content() {`,
    `    #[test]
    fn sse_accumulator_preserves_finish_reason() {
        let mut stream = SseChatAccumulator::new(1024, 4096);
        stream
            .push_bytes(
                b"data: {\\"choices\\":[{\\"delta\\":{\\"content\\":\\"{}\\"},\\"finish_reason\\":\\"length\\"}]}\\n\\ndata: [DONE]\\n\\n",
            )
            .unwrap();
        stream.finish().unwrap();
        assert_eq!(stream.content, "{}");
        assert_eq!(stream.finish_reason.as_deref(), Some("length"));
    }

    #[test]
    fn sse_accumulator_bounds_generated_content() {`,
  );
  return source;
});

await patchFile("src-tauri/src/ai/document_intelligence.rs", (initial) => {
  if (initial.includes("fn assemble_outline_fragments(")) return initial;
  let source = initial;

  let updatedConstants = source
    .replace("const OUTLINE_PROMPT_VERSION: i64 = 1;", "const OUTLINE_PROMPT_VERSION: i64 = 2;")
    .replace("const MAX_OUTLINES_PER_MERGE: usize = 8;\n", "");

  let withRecord = replaceRequired(
    updatedConstants,
    "outline fragment record insertion",
    `#[derive(Clone, Debug)]
pub(crate) struct SourcePageImage {
    pub page_number: i64,
    pub bytes: Vec<u8>,
}
`,
    `#[derive(Clone, Debug)]
pub(crate) struct SourcePageImage {
    pub page_number: i64,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
struct OutlineFragmentRecord {
    window_index: usize,
    page_start: i64,
    page_end: i64,
    source_tokens: usize,
    outline: DocumentOutline,
}
`,
  );

  const buildOutline = `pub(crate) async fn build_document_outline(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    page_count: i64,
    windows: &[SemanticWindow],
    window_images: &[Vec<SourcePageImage>],
) -> Result<DocumentOutline, CommandError> {
    if windows.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The PDF did not contain source windows that ostudy could analyze.",
        ));
    }

    let mut fragments = Vec::with_capacity(windows.len());
    for (index, window) in windows.iter().enumerate() {
        let images = window_images.get(index).map_or(&[][..], Vec::as_slice);
        crate::logging::info(
            "document_outline_window_started",
            format!(
                "provider={} model={} window={}/{} pages={}-{} source_tokens={}",
                task.provider_kind,
                task.model_id,
                index + 1,
                windows.len(),
                window.page_start,
                window.page_end,
                window.token_estimate,
            ),
        );
        let outline = match request_outline_fragment(
            runtime,
            task,
            document_title,
            page_count,
            window,
            images,
        )
        .await
        {
            Ok(outline) => outline,
            Err(error) => {
                crate::logging::error(
                    "document_outline_window_failed",
                    format!(
                        "provider={} model={} window={}/{} pages={}-{} source_tokens={} code={} detail={}",
                        task.provider_kind,
                        task.model_id,
                        index + 1,
                        windows.len(),
                        window.page_start,
                        window.page_end,
                        window.token_estimate,
                        error.code,
                        bounded_diagnostic(&error.message, 600),
                    ),
                );
                return Err(CommandError::new(
                    error.code,
                    format!(
                        "Outline pass failed in source window {}/{} (PDF pages {}–{}, about {} source tokens): {}",
                        index + 1,
                        windows.len(),
                        window.page_start,
                        window.page_end,
                        window.token_estimate,
                        error.message,
                    ),
                ));
            }
        };
        crate::logging::info(
            "document_outline_window_completed",
            format!(
                "window={}/{} pages={}-{} sections={} objectives={} concepts={}",
                index + 1,
                windows.len(),
                window.page_start,
                window.page_end,
                outline.sections.len(),
                outline.learning_objectives.len(),
                outline.key_concepts.len(),
            ),
        );
        fragments.push(OutlineFragmentRecord {
            window_index: index,
            page_start: window.page_start,
            page_end: window.page_end,
            source_tokens: window.token_estimate,
            outline,
        });
    }

    let outline = assemble_outline_fragments(document_title, page_count, fragments);
    crate::logging::info(
        "document_outline_assembled",
        format!(
            "sections={} objectives={} concepts={} pages={}",
            outline.sections.len(),
            outline.learning_objectives.len(),
            outline.key_concepts.len(),
            page_count,
        ),
    );
    Ok(outline)
}

`;
  let withBuild = replaceBlock(
    withRecord,
    "model outline assembly",
    "pub(crate) async fn build_document_outline(",
    "pub(crate) async fn generate_window_flashcards(",
    buildOutline,
  );

  let generateStart = withBuild.indexOf("pub(crate) async fn generate_window_flashcards(");
  const curateStartNeedle = "pub(crate) async fn curate_document_cards(";
  const curateStart = withBuild.indexOf(curateStartNeedle, generateStart);
  if (generateStart < 0 || curateStart < 0) throw new Error("Could not find generation/curation functions");
  let generationBlock = withBuild.slice(generateStart, curateStart);
  generationBlock = generationBlock.replace(
    "    let outline_json = serde_json::to_string(outline).map_err(CommandError::serialization)?;",
    "    let outline_json = outline_context_for_window(outline, window)?;",
  );
  generationBlock = generationBlock.replace(
    "The validated global document outline follows. Use it to judge the GLOBAL importance of local material.",
    "A compact validated outline context for this source window follows. Use it to judge the GLOBAL importance of local material without requiring the full document outline in every request.",
  );
  const withGeneration = `${withBuild.slice(0, generateStart)}${generationBlock}${withBuild.slice(curateStart)}`;

  const curateStart2 = withGeneration.indexOf(curateStartNeedle);
  const fragmentStartNeedle = "async fn request_outline_fragment(";
  const fragmentStart = withGeneration.indexOf(fragmentStartNeedle, curateStart2);
  if (curateStart2 < 0 || fragmentStart < 0) throw new Error("Could not find curation block");
  let curateBlock = withGeneration.slice(curateStart2, fragmentStart);
  curateBlock = curateBlock.replace(
    "    let outline_json = serde_json::to_string(outline).map_err(CommandError::serialization)?;",
    "    let outline_json = outline_context_for_curation(outline)?;",
  );
  const withCuration = `${withGeneration.slice(0, curateStart2)}${curateBlock}${withGeneration.slice(fragmentStart)}`;

  const outlineRequestBlock = `async fn request_outline_fragment(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    page_count: i64,
    window: &SemanticWindow,
    images: &[SourcePageImage],
) -> Result<DocumentOutline, CommandError> {
    let user = format!(
        "Document: {document_title}\\nTotal PDF pages: {page_count}\\nCurrent source window: pages {}–{}\\n\\n\\
         <ostudy-source-window>\\n{}\\n</ostudy-source-window>\\n\\n\\
         Build an outline fragment for the educationally meaningful content in this bounded source window. Preserve section/chapter hierarchy and page spans that are actually supported by this window. \\
         If rendered pages are attached, use them as source evidence for diagrams, tables, equations, and pages with little extractable text. \\
         Exclude bibliography/reference material, legal/publishing scaffolding, indexes/navigation, and incidental trivia.",
        window.page_start,
        window.page_end,
        window.source_text,
    );
    request_outline_with_retry(
        runtime,
        task,
        &outline_system_prompt(),
        &user,
        images,
        window.page_start,
        window.page_end,
        page_count,
    )
    .await
}

async fn request_outline_with_retry(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    system: &str,
    user: &str,
    images: &[SourcePageImage],
    page_start: i64,
    page_end: i64,
    page_count: i64,
) -> Result<DocumentOutline, CommandError> {
    let schema = outline_schema();
    let first = request_json(
        runtime,
        task,
        JsonTaskRequest {
            system_prompt: system,
            user_text: user,
            schema: &schema,
            max_tokens: 3_072,
            images,
        },
    )
    .await
    .and_then(|value| parse_outline_fragment(value, page_start, page_end, page_count));

    match first {
        Ok(outline) => Ok(outline),
        Err(error) if retryable_contract_error(&error) => {
            let repaired = format!(
                "{user}\\n\\nA prior response could not be decoded as the required outline object. Generate a fresh bounded outline fragment for these pages and return only the required structured result."
            );
            request_json(
                runtime,
                task,
                JsonTaskRequest {
                    system_prompt: system,
                    user_text: &repaired,
                    schema: &schema,
                    max_tokens: 3_072,
                    images,
                },
            )
            .await
            .and_then(|value| parse_outline_fragment(value, page_start, page_end, page_count))
            .map_err(|retry_error| collapse_contract_error(retry_error, "outline fragment"))
        }
        Err(error) => Err(error),
    }
}

`;
  const withOutlineRequests = replaceBlock(
    withCuration,
    "outline request and model merge functions",
    fragmentStartNeedle,
    "async fn request_flashcards_with_retry(",
    outlineRequestBlock,
  );

  const flashStart = withOutlineRequests.indexOf("async fn request_flashcards_with_retry(");
  const curationRequestStart = withOutlineRequests.indexOf("async fn request_curation_with_retry(", flashStart);
  let flashBlock = withOutlineRequests.slice(flashStart, curationRequestStart);
  flashBlock = flashBlock.replaceAll("max_tokens: 8_192,", "max_tokens: 5_120,");
  const withFlashLimits = `${withOutlineRequests.slice(0, flashStart)}${flashBlock}${withOutlineRequests.slice(curationRequestStart)}`;

  const responseMarker = `    .map_err(gateway_to_command)?;

    match task.profile.provider_kind {`;
  const withFinishCheck = replaceRequired(
    withFlashLimits,
    "document completion finish-reason check",
    responseMarker,
    `    .map_err(gateway_to_command)?;

    ensure_document_response_complete(&response)?;

    match task.profile.provider_kind {`,
  );

  const parseOutlineStart = withFinishCheck.indexOf("fn parse_outline(value: Value)");
  const parseFlashStart = withFinishCheck.indexOf("fn parse_flashcards(value: Value)", parseOutlineStart);
  if (parseOutlineStart < 0 || parseFlashStart < 0) throw new Error("Could not find parse_outline block");
  const parseOutlineReplacement = `fn parse_outline_fragment(
    value: Value,
    page_start: i64,
    page_end: i64,
    page_count: i64,
) -> Result<DocumentOutline, CommandError> {
    let outline: DocumentOutline = serde_json::from_value(value).map_err(|error| {
        output_error(format!(
            "The provider returned an outline object that could not be decoded: {}",
            bounded_diagnostic(&error.to_string(), 320)
        ))
    })?;
    Ok(normalize_outline_fragment(
        outline,
        page_start,
        page_end,
        page_count,
    ))
}

`;
  const withoutStrictOutlineValidation = `${withFinishCheck.slice(0, parseOutlineStart)}${parseOutlineReplacement}${withFinishCheck.slice(parseFlashStart)}`;

  const helperAnchor = "fn outline_system_prompt() -> String {";
  const helpers = `fn assemble_outline_fragments(
    document_title: &str,
    page_count: i64,
    mut fragments: Vec<OutlineFragmentRecord>,
) -> DocumentOutline {
    fragments.sort_by_key(|fragment| fragment.window_index);
    let mut summary_parts = Vec::new();
    let mut objectives = Vec::new();
    let mut objective_keys = HashSet::new();
    let mut concepts = Vec::new();
    let mut concept_keys = HashSet::new();
    let mut sections = Vec::new();

    for fragment in fragments {
        if summary_parts.len() < 24 {
            summary_parts.push(sanitize_outline_text(&fragment.outline.summary, 2_000, ""));
        }
        extend_unique_items(
            &mut objectives,
            &mut objective_keys,
            fragment.outline.learning_objectives,
            MAX_OUTLINE_OBJECTIVES,
        );
        extend_unique_items(
            &mut concepts,
            &mut concept_keys,
            fragment.outline.key_concepts,
            MAX_OUTLINE_CONCEPTS,
        );
        sections.extend(fragment.outline.sections);
        crate::logging::info(
            "document_outline_fragment_merged",
            format!(
                "window={} pages={}-{} source_tokens={}",
                fragment.window_index + 1,
                fragment.page_start,
                fragment.page_end,
                fragment.source_tokens,
            ),
        );
    }

    sections.sort_by_key(|section| (section.page_start, section.page_end));
    let mut merged_sections: Vec<OutlineSection> = Vec::new();
    for section in sections {
        if let Some(previous) = merged_sections.last_mut() {
            let same_heading = normalized_outline_key(&previous.title)
                == normalized_outline_key(&section.title);
            let touches = section.page_start <= previous.page_end.saturating_add(2);
            if same_heading && touches {
                previous.page_end = previous.page_end.max(section.page_end).min(page_count.max(1));
                previous.summary = sanitize_outline_text(
                    &format!("{} {}", previous.summary, section.summary),
                    10_000,
                    &previous.title,
                );
                merge_outline_items(
                    &mut previous.learning_objectives,
                    section.learning_objectives,
                    32,
                );
                merge_outline_items(&mut previous.key_concepts, section.key_concepts, 48);
                continue;
            }
        }
        if merged_sections.len() < MAX_OUTLINE_SECTIONS {
            merged_sections.push(section);
        }
    }

    let fallback = format!("Learning outline for {document_title}.");
    let joined_summary = summary_parts
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    DocumentOutline {
        summary: sanitize_outline_text(&joined_summary, 20_000, &fallback),
        learning_objectives: objectives,
        key_concepts: concepts,
        sections: merged_sections,
    }
}

fn normalize_outline_fragment(
    outline: DocumentOutline,
    page_start: i64,
    page_end: i64,
    page_count: i64,
) -> DocumentOutline {
    let document_end = page_count.max(1);
    let source_start = page_start.clamp(1, document_end);
    let source_end = page_end.clamp(source_start, document_end);
    let fallback_summary = format!("Educational material from PDF pages {source_start}–{source_end}.");
    let summary = sanitize_outline_text(&outline.summary, 8_000, &fallback_summary);
    let learning_objectives = normalize_outline_items(outline.learning_objectives, 24, 1_200);
    let key_concepts = normalize_outline_items(outline.key_concepts, 40, 700);
    let mut sections = outline
        .sections
        .into_iter()
        .take(48)
        .map(|section| {
            let mut start = section.page_start.clamp(source_start, source_end);
            let mut end = section.page_end.clamp(source_start, source_end);
            if end < start {
                std::mem::swap(&mut start, &mut end);
            }
            let fallback_title = format!("Pages {start}–{end}");
            let title = sanitize_outline_text(&section.title, 500, &fallback_title);
            let summary = sanitize_outline_text(&section.summary, 4_000, &title);
            OutlineSection {
                title,
                page_start: start,
                page_end: end,
                summary,
                learning_objectives: normalize_outline_items(
                    section.learning_objectives,
                    12,
                    1_000,
                ),
                key_concepts: normalize_outline_items(section.key_concepts, 18, 600),
            }
        })
        .collect::<Vec<_>>();

    if sections.is_empty() {
        sections.push(OutlineSection {
            title: format!("Pages {source_start}–{source_end}"),
            page_start: source_start,
            page_end: source_end,
            summary: summary.clone(),
            learning_objectives: learning_objectives.iter().take(8).cloned().collect(),
            key_concepts: key_concepts.iter().take(12).cloned().collect(),
        });
    }

    DocumentOutline {
        summary,
        learning_objectives,
        key_concepts,
        sections,
    }
}

fn outline_context_for_window(
    outline: &DocumentOutline,
    window: &SemanticWindow,
) -> Result<String, CommandError> {
    let sections = outline
        .sections
        .iter()
        .filter(|section| {
            section.page_end >= window.page_start && section.page_start <= window.page_end
        })
        .take(8)
        .map(|section| {
            json!({
                "title": sanitize_outline_text(&section.title, 300, "Section"),
                "pageStart": section.page_start,
                "pageEnd": section.page_end,
                "summary": sanitize_outline_text(&section.summary, 1_200, ""),
                "learningObjectives": section.learning_objectives.iter().take(5).map(|value| sanitize_outline_text(value, 500, "")).collect::<Vec<_>>(),
                "keyConcepts": section.key_concepts.iter().take(8).map(|value| sanitize_outline_text(value, 300, "")).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&json!({
        "summary": sanitize_outline_text(&outline.summary, 2_500, "Document learning outline"),
        "learningObjectives": outline.learning_objectives.iter().take(16).map(|value| sanitize_outline_text(value, 500, "")).collect::<Vec<_>>(),
        "keyConcepts": outline.key_concepts.iter().take(24).map(|value| sanitize_outline_text(value, 300, "")).collect::<Vec<_>>(),
        "relevantSections": sections
    }))
    .map_err(CommandError::serialization)
}

fn outline_context_for_curation(outline: &DocumentOutline) -> Result<String, CommandError> {
    serde_json::to_string(&json!({
        "summary": sanitize_outline_text(&outline.summary, 3_000, "Document learning outline"),
        "learningObjectives": outline.learning_objectives.iter().take(32).map(|value| sanitize_outline_text(value, 500, "")).collect::<Vec<_>>(),
        "keyConcepts": outline.key_concepts.iter().take(48).map(|value| sanitize_outline_text(value, 300, "")).collect::<Vec<_>>()
    }))
    .map_err(CommandError::serialization)
}

fn normalize_outline_items(values: Vec<String>, max_items: usize, max_chars: usize) -> Vec<String> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for value in values {
        let normalized = sanitize_outline_text(&value, max_chars, "");
        if normalized.is_empty() {
            continue;
        }
        let key = normalized_outline_key(&normalized);
        if seen.insert(key) {
            result.push(normalized);
            if result.len() >= max_items {
                break;
            }
        }
    }
    result
}

fn extend_unique_items(
    target: &mut Vec<String>,
    seen: &mut HashSet<String>,
    values: Vec<String>,
    limit: usize,
) {
    for value in values {
        if target.len() >= limit {
            break;
        }
        let key = normalized_outline_key(&value);
        if !key.is_empty() && seen.insert(key) {
            target.push(value);
        }
    }
}

fn merge_outline_items(target: &mut Vec<String>, values: Vec<String>, limit: usize) {
    let mut seen = target
        .iter()
        .map(|value| normalized_outline_key(value))
        .collect::<HashSet<_>>();
    for value in values {
        if target.len() >= limit {
            break;
        }
        let key = normalized_outline_key(&value);
        if !key.is_empty() && seen.insert(key) {
            target.push(value);
        }
    }
}

fn sanitize_outline_text(value: &str, maximum_chars: usize, fallback: &str) -> String {
    let normalized = value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\\n' | '\\t'))
        .take(maximum_chars)
        .collect::<String>();
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalized_outline_key(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn bounded_diagnostic(value: &str, maximum_chars: usize) -> String {
    value.chars().take(maximum_chars).collect()
}

fn ensure_document_response_complete(value: &Value) -> Result<(), CommandError> {
    match value
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
    {
        Some("length") => Err(CommandError::new(
            "ai_document_output_truncated",
            "The AI provider reached its completion limit before finishing the structured document result. ostudy now uses smaller source windows; if this still occurs, increase the model/server context or output allowance and retry.",
        )),
        Some("content_filter") => Err(CommandError::new(
            "ai_document_output_filtered",
            "The AI provider stopped the document result because of its content policy.",
        )),
        Some("insufficient_system_resource") => Err(CommandError::new(
            "ai_provider_resource_exhausted",
            "The AI provider stopped the document request because it did not have enough inference resources.",
        )),
        _ => Ok(()),
    }
}

`;
  const withHelpers = replaceRequired(
    withoutStrictOutlineValidation,
    "outline helper insertion",
    helperAnchor,
    `${helpers}${helperAnchor}`,
  );

  const oldCollapse = `fn collapse_contract_error(error: CommandError, task: &str) -> CommandError {
    if retryable_contract_error(&error) {
        CommandError::new(
            "ai_document_output_invalid",
            format!(
                "The AI provider twice returned {task} that did not match ostudy's contract. Retry the document or choose a different model."
            ),
        )
    } else {
        error
    }
}`;
  const newCollapse = `fn collapse_contract_error(error: CommandError, task: &str) -> CommandError {
    if retryable_contract_error(&error) {
        CommandError::new(
            "ai_document_output_invalid",
            format!(
                "The AI provider returned invalid {task} twice. Last validation detail: {}",
                bounded_diagnostic(&error.message, 500)
            ),
        )
    } else {
        error
    }
}`;
  const withDetailedCollapse = replaceRequired(
    withHelpers,
    "detailed contract failure",
    oldCollapse,
    newCollapse,
  );

  const testAnchor = `    #[test]
    fn schemas_cover_all_generation_forms() {`;
  const tests = `    #[test]
    fn outline_fragments_are_normalized_and_merged_without_another_model_call() {
        let fragments = vec![
            OutlineFragmentRecord {
                window_index: 0,
                page_start: 1,
                page_end: 10,
                source_tokens: 4_000,
                outline: normalize_outline_fragment(
                    DocumentOutline {
                        summary: "First fragment".to_string(),
                        learning_objectives: vec!["Explain gas exchange".to_string()],
                        key_concepts: vec!["Diffusion".to_string()],
                        sections: vec![OutlineSection {
                            title: "Gas Exchange".to_string(),
                            page_start: 8,
                            page_end: 2,
                            summary: "Mechanisms".to_string(),
                            learning_objectives: vec!["Explain gas exchange".to_string()],
                            key_concepts: vec!["Diffusion".to_string()],
                        }],
                    },
                    1,
                    10,
                    20,
                ),
            },
            OutlineFragmentRecord {
                window_index: 1,
                page_start: 9,
                page_end: 20,
                source_tokens: 4_000,
                outline: normalize_outline_fragment(
                    DocumentOutline {
                        summary: "Second fragment".to_string(),
                        learning_objectives: vec!["Apply ventilation concepts".to_string()],
                        key_concepts: vec!["Ventilation".to_string()],
                        sections: vec![OutlineSection {
                            title: "Gas Exchange".to_string(),
                            page_start: 9,
                            page_end: 14,
                            summary: "Applications".to_string(),
                            learning_objectives: vec!["Apply ventilation concepts".to_string()],
                            key_concepts: vec!["Ventilation".to_string()],
                        }],
                    },
                    9,
                    20,
                    20,
                ),
            },
        ];
        let outline = assemble_outline_fragments("Respiratory", 20, fragments);
        assert_eq!(outline.sections.len(), 1);
        assert_eq!(outline.sections[0].page_start, 2);
        assert_eq!(outline.sections[0].page_end, 14);
        assert_eq!(outline.learning_objectives.len(), 2);
        assert!(outline.summary.contains("First fragment"));
        assert!(outline.summary.contains("Second fragment"));
    }

    #[test]
    fn window_outline_context_excludes_distant_sections() {
        let outline = DocumentOutline {
            summary: "Whole document".to_string(),
            learning_objectives: vec!["Objective".to_string()],
            key_concepts: vec!["Concept".to_string()],
            sections: vec![
                OutlineSection {
                    title: "Nearby".to_string(),
                    page_start: 10,
                    page_end: 20,
                    summary: "Relevant".to_string(),
                    learning_objectives: Vec::new(),
                    key_concepts: Vec::new(),
                },
                OutlineSection {
                    title: "Distant".to_string(),
                    page_start: 100,
                    page_end: 110,
                    summary: "Not relevant".to_string(),
                    learning_objectives: Vec::new(),
                    key_concepts: Vec::new(),
                },
            ],
        };
        let window = SemanticWindow {
            window_index: 0,
            first_chunk_index: 0,
            last_chunk_index: 0,
            page_numbers: vec![12],
            page_start: 12,
            page_end: 15,
            token_estimate: 1_000,
            source_text: "source".to_string(),
        };
        let context = outline_context_for_window(&outline, &window).unwrap();
        assert!(context.contains("Nearby"));
        assert!(!context.contains("Distant"));
    }

    #[test]
    fn truncated_provider_output_is_reported_separately_from_schema_drift() {
        let error = ensure_document_response_complete(&json!({
            "choices": [{ "finish_reason": "length" }]
        }))
        .unwrap_err();
        assert_eq!(error.code, "ai_document_output_truncated");
        assert!(error.message.contains("completion limit"));
    }

`;
  return replaceRequired(
    withDetailedCollapse,
    "large-document outline tests",
    testAnchor,
    `${tests}${testAnchor}`,
  );
});
