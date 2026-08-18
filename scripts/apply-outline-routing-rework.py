#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, value: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(value)


def replace_once(path: str, old: str, new: str) -> None:
    value = read(path)
    count = value.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}: {old[:100]!r}")
    write(path, value.replace(old, new, 1))


def replace_region(path: str, start: str, end: str, replacement: str) -> None:
    value = read(path)
    left = value.find(start)
    if left < 0:
        raise SystemExit(f"{path}: start marker not found: {start!r}")
    right = value.find(end, left)
    if right < 0:
        raise SystemExit(f"{path}: end marker not found: {end!r}")
    write(path, value[:left] + replacement + value[right:])


# futures-util is already present transitively at this exact version. Making it
# direct lets document window execution use bounded buffer_unordered without
# spawning tasks outside the document task-local cancellation/progress scope.
replace_once(
    "src-tauri/Cargo.toml",
    'chrono = "=0.4.45"\n',
    'chrono = "=0.4.45"\nfutures-util = "=0.3.33"\n',
)

replace_once(
    "src-tauri/src/ai/mod.rs",
    "mod catalog;\n",
    "mod catalog;\npub(crate) mod concurrency;\n",
)

write(
    "src-tauri/src/ai/concurrency.rs",
    '''//! Bounded provider concurrency for independent document windows.\n//!\n//! Window futures remain in the parent Tokio task (via `buffer_unordered`) so\n//! task-local cancellation, progress, credentials, and usage accounting remain\n//! authoritative. The limits intentionally avoid unbounded paid-provider fanout.\n\npub(crate) const OPENROUTER_DOCUMENT_PARALLELISM: usize = 4;\npub(crate) const DEEPSEEK_DOCUMENT_PARALLELISM: usize = 2;\n\npub(crate) fn document_request_parallelism(provider_kind: &str) -> usize {\n    match provider_kind {\n        "openrouter" => OPENROUTER_DOCUMENT_PARALLELISM,\n        "deepseek" => DEEPSEEK_DOCUMENT_PARALLELISM,\n        _ => 1,\n    }\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn remote_document_windows_are_bounded_and_local_inference_stays_serial() {\n        assert_eq!(document_request_parallelism("openrouter"), 4);\n        assert_eq!(document_request_parallelism("deepseek"), 2);\n        assert_eq!(document_request_parallelism("llama_cpp"), 1);\n    }\n}\n''',
)

# Context windows are request boundaries, not hierarchy boundaries. Give large
# models useful source batches while the existing capability calculation can
# still shrink these values for small-context models.
scale = "src-tauri/src/document_scale.rs"
for old, new in [
    ("outline_window_max_tokens: 3_200,", "outline_window_max_tokens: 6_000,"),
    ("outline_window_max_units: 8,", "outline_window_max_units: 12,"),
    ("outline_window_overlap_tokens: 350,", "outline_window_overlap_tokens: 300,"),
    ("generation_window_max_tokens: 5_000,", "generation_window_max_tokens: 7_000,"),
    ("generation_window_max_units: 10,", "generation_window_max_units: 12,"),
    ("generation_window_overlap_tokens: 700,", "generation_window_overlap_tokens: 500,"),
    ("outline_window_max_tokens: 10_000,", "outline_window_max_tokens: 14_000,"),
    ("outline_window_max_units: 18,", "outline_window_max_units: 24,"),
    ("outline_window_overlap_tokens: 500,", "outline_window_overlap_tokens: 400,"),
    ("generation_window_max_tokens: 7_000,", "generation_window_max_tokens: 10_000,"),
    ("generation_window_max_units: 12,", "generation_window_max_units: 18,"),
    ("generation_window_overlap_tokens: 600,", "generation_window_overlap_tokens: 500,"),
]:
    replace_once(scale, old, new)
replace_region(
    scale,
    "fn outline_completion_reserve(provider_kind: &str) -> usize {",
    "\nfn generation_completion_reserve",
    '''fn outline_completion_reserve(_provider_kind: &str) -> usize {\n    // Outline fragments are intentionally concise. Reserving 1.5k completion\n    // tokens prevents a local fragment from consuming a multi-thousand-token\n    // monologue while leaving ample room for the strict schema.\n    1_536\n}\n''',
)
replace_once(scale, "assert_eq!(plan.generation_window_max_tokens, 5_000);", "assert_eq!(plan.generation_window_max_tokens, 7_000);")
replace_once(scale, "assert!(plan.outline_window_max_tokens < 10_000);", "assert!(plan.outline_window_max_tokens < 14_000);")
replace_once(scale, "assert!(plan.generation_window_max_tokens < 7_000);", "assert!(plan.generation_window_max_tokens < 10_000);")
replace_once(scale, "assert_eq!(plan.outline_window_max_tokens, 10_000);", "assert_eq!(plan.outline_window_max_tokens, 14_000);")
replace_once(scale, "assert_eq!(plan.outline_window_max_units, 18);", "assert_eq!(plan.outline_window_max_units, 24);")
replace_once(scale, "assert_eq!(plan.generation_window_max_tokens, 7_000);", "assert_eq!(plan.generation_window_max_tokens, 10_000);")
replace_once(scale, "assert_eq!(plan.generation_window_max_units, 12);", "assert_eq!(plan.generation_window_max_units, 18);")
replace_once(scale, "assert_eq!(plan.outline_window_max_tokens, 10_000);\n        assert!(plan.outline_window_max_units > 8);\n        assert_eq!(plan.generation_window_max_tokens, 7_000);", "assert_eq!(plan.outline_window_max_tokens, 14_000);\n        assert!(plan.outline_window_max_units > 8);\n        assert_eq!(plan.generation_window_max_tokens, 10_000);")

# Hierarchy metadata remains in every chunk, but heading changes no longer split
# network requests. Capacity/context boundaries alone decide request packing.
semantics = "src-tauri/src/document_semantics.rs"
replace_region(
    semantics,
    "pub(crate) fn build_semantic_partitions(",
    "#[cfg(test)]\npub(crate) fn build_semantic_windows(",
    '''pub(crate) fn build_semantic_partitions(\n    chunks: &[SemanticChunk],\n    plan: DocumentScalePlan,\n) -> Vec<SemanticPartition> {\n    if chunks.is_empty() {\n        return Vec::new();\n    }\n\n    let mut partitions = Vec::new();\n    let mut start = 0_usize;\n    while start < chunks.len() {\n        let mut end = start;\n        let mut token_total = 0_usize;\n        let mut units = BTreeSet::new();\n\n        while end < chunks.len() {\n            let chunk = &chunks[end];\n            let chunk_tokens = effective_chunk_tokens(chunk);\n            let next_tokens = token_total.saturating_add(chunk_tokens);\n            let mut next_units = units.clone();\n            next_units.extend(chunk.page_numbers.iter().copied());\n\n            let exceeds_hard_boundary = end > start\n                && (next_tokens > plan.partition_max_tokens\n                    || next_units.len() > plan.partition_max_units);\n            let reached_target = token_total >= plan.partition_target_tokens\n                || units.len() >= plan.partition_target_units;\n\n            if exceeds_hard_boundary || (end > start && reached_target) {\n                break;\n            }\n\n            token_total = next_tokens;\n            units = next_units;\n            end += 1;\n        }\n\n        if end == start {\n            units.extend(chunks[start].page_numbers.iter().copied());\n            end = start + 1;\n        }\n\n        let unit_numbers = units.into_iter().collect::<Vec<_>>();\n        let unit_start = unit_numbers.first().copied().unwrap_or(1);\n        let unit_end = unit_numbers.last().copied().unwrap_or(unit_start);\n        let heading_titles = chunks[start..end]\n            .iter()\n            .filter_map(partition_heading)\n            .collect::<BTreeSet<_>>();\n        let title = if heading_titles.len() == 1 {\n            heading_titles\n                .into_iter()\n                .next()\n                .unwrap_or_else(|| format!("Source units {unit_start}–{unit_end}"))\n        } else {\n            format!("Source units {unit_start}–{unit_end}")\n        };\n        partitions.push(SemanticPartition {\n            partition_index: partitions.len(),\n            title,\n            chunk_start: start,\n            chunk_end: end,\n        });\n        start = end;\n    }\n    partitions\n}\n\n''',
)
replace_region(
    semantics,
    "    #[test]\n    fn heading_changes_create_local_partitions_before_hard_limits() {",
    "    #[test]\n    fn partitioned_windows_receive_unique_global_indices_within_one_partition() {",
    '''    #[test]\n    fn heading_changes_do_not_force_tiny_request_partitions() {\n        let mut chunks = Vec::new();\n        for index in 0..12 {\n            let mut value = chunk(index, &[index + 1], 900);\n            value.heading_context = Some(if index < 6 {\n                "Chapter 1 > Topic".to_string()\n            } else {\n                "Chapter 2 > Topic".to_string()\n            });\n            chunks.push(value);\n        }\n        let plan = DocumentScalePlan::for_unit_count(138);\n        let partitions = build_semantic_partitions(&chunks, plan);\n        assert_eq!(partitions.len(), 1);\n        assert_eq!(partitions[0].title, "Source units 1–12");\n        let windows = build_partitioned_semantic_windows(&chunks, plan, 14_000, 24, 0);\n        assert_eq!(windows.len(), 1);\n        assert!(windows[0].source_text.contains("Chapter 1 &gt; Topic"));\n        assert!(windows[0].source_text.contains("Chapter 2 &gt; Topic"));\n    }\n\n''',
)
replace_region(
    semantics,
    "    #[test]\n    fn partitioned_windows_do_not_overlap_across_chapters() {",
    "}\n",
    '''    #[test]\n    fn packed_windows_can_cross_heading_boundaries_without_losing_provenance() {\n        let mut chunks = Vec::new();\n        for index in 0..8 {\n            let mut value = chunk(index, &[index + 1], 700);\n            value.heading_context = Some(if index < 4 {\n                "Chapter A".to_string()\n            } else {\n                "Chapter B".to_string()\n            });\n            if index == 5 {\n                value.element_types = vec!["image".to_string()];\n            }\n            chunks.push(value);\n        }\n        let plan = DocumentScalePlan::for_unit_count(138);\n        let windows = build_partitioned_semantic_windows(&chunks, plan, 2_100, 3, 0);\n        assert!(windows.iter().any(|window| {\n            window.source_locator().first() <= 4 && window.source_locator().last() >= 5\n        }));\n        assert!(windows.iter().any(|window| window.source_text.contains("Chapter B")));\n        assert!(windows\n            .iter()\n            .any(|window| window.visual_page_numbers.contains(&6)));\n    }\n''',
)
# replace_region above consumed the module's final closing brace marker; put it back.
value = read(semantics)
if not value.rstrip().endswith("}"):
    write(semantics, value.rstrip() + "\n}\n")

# OpenRouter routing: cheapest high-quality endpoint meeting a useful throughput
# and latency preference, with q8/FP8 or higher precision only.
openrouter = "src-tauri/src/ai/providers/openrouter.rs"
replace_once(
    openrouter,
    'const REASONING_EFFORTS: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];\n',
    'const REASONING_EFFORTS: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];\nconst HIGH_PRECISION_QUANTIZATIONS: [&str; 6] = ["int8", "fp8", "mxfp8", "fp16", "bf16", "fp32"];\nconst PREFERRED_MIN_THROUGHPUT_TPS: u64 = 40;\nconst PREFERRED_MAX_P90_LATENCY_SECONDS: u64 = 30;\n',
)
replace_once(
    openrouter,
    '''    json!({\n        "require_parameters": require_parameters,\n        "data_collection": "deny",\n        "zdr": require_zero_data_retention,\n        "allow_fallbacks": true\n    })''',
    '''    json!({\n        "require_parameters": require_parameters,\n        "data_collection": "deny",\n        "zdr": require_zero_data_retention,\n        "allow_fallbacks": true,\n        "quantizations": HIGH_PRECISION_QUANTIZATIONS,\n        "sort": "price",\n        "preferred_min_throughput": {\n            "p50": PREFERRED_MIN_THROUGHPUT_TPS\n        },\n        "preferred_max_latency": {\n            "p90": PREFERRED_MAX_P90_LATENCY_SECONDS\n        }\n    })''',
)
replace_once(
    openrouter,
    '''        assert_eq!(strict.pointer("/require_parameters"), Some(&json!(true)));\n    }''',
    '''        assert_eq!(strict.pointer("/require_parameters"), Some(&json!(true)));\n        assert_eq!(strict.pointer("/sort"), Some(&json!("price")));\n        assert_eq!(\n            strict.pointer("/preferred_min_throughput/p50"),\n            Some(&json!(40))\n        );\n        assert_eq!(\n            strict.pointer("/preferred_max_latency/p90"),\n            Some(&json!(30))\n        );\n        assert_eq!(\n            strict.pointer("/quantizations"),\n            Some(&json!(["int8", "fp8", "mxfp8", "fp16", "bf16", "fp32"]))\n        );\n        let serialized = strict["quantizations"].to_string();\n        for rejected in ["int4", "fp4", "mxfp4", "nvfp4", "fp6", "unknown"] {\n            assert!(!serialized.contains(rejected));\n        }\n    }''',
)

# Default new installations to minimal reasoning for document extraction. Users
# can still choose none/low/medium/etc. Existing explicit settings are preserved.
settings = "src-tauri/src/settings.rs"
replace_once(settings, 'const DEFAULT_AI_REASONING_EFFORT: &str = "low";', 'const DEFAULT_AI_REASONING_EFFORT: &str = "minimal";')
replace_once(settings, 'assert_eq!(settings.ai_reasoning_effort, "low");', 'assert_eq!(settings.ai_reasoning_effort, "minimal");')
replace_once(settings, 'assert_eq!(current_ai_reasoning_effort(), "low");', 'assert_eq!(current_ai_reasoning_effort(), "minimal");')

# Tight, concurrent outline fragments. Hierarchy is preserved inside source
# chunks and deterministically assembled after bounded concurrent requests.
intel = "src-tauri/src/ai/document_intelligence.rs"
replace_once(
    intel,
    "use base64::{engine::general_purpose::STANDARD, Engine as _};\n",
    "use base64::{engine::general_purpose::STANDARD, Engine as _};\nuse futures_util::{stream, StreamExt};\n",
)
replace_once(
    intel,
    "    ai::providers, database::Database, document_semantics::SemanticWindow, error::CommandError,\n",
    "    ai::providers, database::Database, document_semantics::SemanticWindow, error::CommandError,\n    task_cancellation::set_current_task_phase,\n",
)
for old, new in [
    ("const MAX_FRAGMENT_OBJECTIVES: usize = 6;", "const MAX_FRAGMENT_OBJECTIVES: usize = 4;"),
    ("const MAX_FRAGMENT_CONCEPTS: usize = 10;", "const MAX_FRAGMENT_CONCEPTS: usize = 8;"),
    ("const MAX_FRAGMENT_SECTIONS: usize = 6;", "const MAX_FRAGMENT_SECTIONS: usize = 4;"),
    ("const MAX_FRAGMENT_SECTION_OBJECTIVES: usize = 4;", "const MAX_FRAGMENT_SECTION_OBJECTIVES: usize = 3;"),
    ("const MAX_FRAGMENT_SECTION_CONCEPTS: usize = 6;", "const MAX_FRAGMENT_SECTION_CONCEPTS: usize = 4;"),
]:
    replace_once(intel, old, new)

new_outline_builder = r'''pub(crate) async fn build_document_outline<F>(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    page_count: i64,
    sources: OutlineBuildSources<'_>,
    mut persist_fragment: F,
) -> Result<DocumentOutline, CommandError>
where
    F: FnMut(&SemanticWindow, &DocumentOutline) -> Result<(), CommandError>,
{
    let OutlineBuildSources {
        windows,
        cached_fragments,
        window_images,
    } = sources;
    if windows.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The PDF did not contain source windows that ostudy could analyze.",
        ));
    }

    let parallelism = crate::ai::concurrency::document_request_parallelism(&task.provider_kind)
        .clamp(1, windows.len());
    crate::logging::info(
        "document_outline_execution_plan",
        format!(
            "provider={} model={} windows={} parallelism={} warm_sticky_route={}",
            task.provider_kind,
            task.model_id,
            windows.len(),
            parallelism,
            task.provider_kind == "openrouter"
        ),
    );

    let mut resolved = vec![None; windows.len()];
    let mut missing = Vec::new();
    for (index, window) in windows.iter().enumerate() {
        if let Some(cached) = cached_fragments.get(index).and_then(Option::as_ref) {
            crate::logging::info(
                "document_outline_window_reused",
                format!(
                    "window={}/{} partition={} pages={}-{} source_tokens={}",
                    index + 1,
                    windows.len(),
                    window.partition_index + 1,
                    window.page_start,
                    window.page_end,
                    window.token_estimate,
                ),
            );
            resolved[index] = Some(cached.clone());
        } else {
            missing.push(index);
        }
    }

    let mut completed_count = windows.len().saturating_sub(missing.len());
    let warm_sticky_route = task.provider_kind == "openrouter" && missing.len() > 1;
    let mut cursor = 0_usize;
    while cursor < missing.len() {
        let batch_size = if warm_sticky_route && cursor == 0 {
            1
        } else {
            parallelism
        };
        let end = (cursor + batch_size).min(missing.len());
        let batch = &missing[cursor..end];
        let first = &windows[batch[0]];
        let last = &windows[*batch.last().unwrap_or(&batch[0])];
        set_current_task_phase(
            "outlining",
            Some(batch[0] + 1),
            completed_count,
            windows.len(),
            Some(first.page_start),
            Some(last.page_end),
            Some(first.partition_index + 1),
        );

        let mut batch_results = stream::iter(batch.iter().copied().map(|index| {
            let window = &windows[index];
            let images = window_images.get(index).map_or(&[][..], Vec::as_slice);
            async move {
                (
                    index,
                    build_outline_window(
                        runtime,
                        task,
                        document_title,
                        page_count,
                        windows.len(),
                        index,
                        window,
                        images,
                    )
                    .await,
                )
            }
        }))
        .buffer_unordered(batch.len().max(1))
        .collect::<Vec<_>>()
        .await;
        batch_results.sort_by_key(|(index, _)| *index);

        let mut first_error = None;
        for (index, result) in batch_results {
            match result {
                Ok(outline) => {
                    persist_fragment(&windows[index], &outline)?;
                    resolved[index] = Some(outline);
                    completed_count = completed_count.saturating_add(1);
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        cursor = end;
    }

    let mut fragments = Vec::with_capacity(windows.len());
    for (index, window) in windows.iter().enumerate() {
        let outline = resolved[index].take().ok_or_else(|| {
            CommandError::new(
                "document_outline_checkpoint_missing",
                "A completed outline window did not produce a reusable fragment.",
            )
        })?;
        crate::logging::info(
            "document_outline_window_completed",
            format!(
                "window={}/{} partition={} pages={}-{} sections={} objectives={} concepts={}",
                index + 1,
                windows.len(),
                window.partition_index + 1,
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

    let mut outline = assemble_outline_fragments(document_title, page_count, fragments);
    match synthesize_global_outline_context(runtime, task, document_title, page_count, &outline)
        .await
    {
        Ok(global) => {
            outline.summary = sanitize_outline_text(&global.summary, 8_000, &outline.summary);
            let objectives = normalize_outline_items(global.learning_objectives, 24, 1_200);
            if !objectives.is_empty() {
                outline.learning_objectives = objectives;
            }
            let concepts = normalize_outline_items(global.key_concepts, 40, 700);
            if !concepts.is_empty() {
                outline.key_concepts = concepts;
            }
            crate::logging::info(
                "document_global_outline_synthesized",
                format!(
                    "sections={} objectives={} concepts={} pages={}",
                    outline.sections.len(),
                    outline.learning_objectives.len(),
                    outline.key_concepts.len(),
                    page_count,
                ),
            );
        }
        Err(error) => {
            crate::logging::info(
                "document_global_outline_synthesis_skipped",
                format!(
                    "code={} detail={}",
                    error.code,
                    bounded_diagnostic(&error.message, 400)
                ),
            );
        }
    }
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

async fn build_outline_window(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    page_count: i64,
    total_windows: usize,
    index: usize,
    window: &SemanticWindow,
    images: &[SourcePageImage],
) -> Result<DocumentOutline, CommandError> {
    crate::logging::info(
        "document_outline_window_started",
        format!(
            "provider={} model={} window={}/{} partition={} pages={}-{} source_tokens={}",
            task.provider_kind,
            task.model_id,
            index + 1,
            total_windows,
            window.partition_index + 1,
            window.page_start,
            window.page_end,
            window.token_estimate,
        ),
    );
    match request_outline_fragment(
        runtime,
        task,
        document_title,
        page_count,
        window,
        images,
    )
    .await
    {
        Ok(outline) => Ok(outline),
        Err(error) if total_windows > 1 && recoverable_outline_window_error(&error) => {
            crate::logging::info(
                "document_outline_window_native_fallback",
                format!(
                    "window={}/{} partition={} pages={}-{} code={} detail={}",
                    index + 1,
                    total_windows,
                    window.partition_index + 1,
                    window.page_start,
                    window.page_end,
                    error.code,
                    bounded_diagnostic(&error.message, 400),
                ),
            );
            Ok(native_outline_fallback(document_title, window))
        }
        Err(error) => {
            crate::logging::error(
                "document_outline_window_failed",
                format!(
                    "provider={} model={} window={}/{} partition={} pages={}-{} source_tokens={} code={} detail={}",
                    task.provider_kind,
                    task.model_id,
                    index + 1,
                    total_windows,
                    window.partition_index + 1,
                    window.page_start,
                    window.page_end,
                    window.token_estimate,
                    error.code,
                    bounded_diagnostic(&error.message, 600),
                ),
            );
            Err(CommandError::new(
                error.code,
                format!(
                    "Outline pass failed in source window {}/{} (partition {}, PDF pages {}–{}, about {} source tokens): {}",
                    index + 1,
                    total_windows,
                    window.partition_index + 1,
                    window.page_start,
                    window.page_end,
                    window.token_estimate,
                    error.message,
                ),
            ))
        }
    }
}

'''
replace_region(
    intel,
    "pub(crate) async fn build_document_outline<F>(",
    "fn recoverable_outline_window_error",
    new_outline_builder,
)
replace_once(
    intel,
    '''         Build a CONCISE outline fragment for the educationally meaningful content in this bounded source window. Preserve section/chapter hierarchy and page spans actually supported by this window.          Return at most {MAX_FRAGMENT_OBJECTIVES} learning objectives, {MAX_FRAGMENT_CONCEPTS} key concepts, and {MAX_FRAGMENT_SECTIONS} sections. Keep the fragment summary to roughly 2–4 sentences; each section should have a short summary, at most {MAX_FRAGMENT_SECTION_OBJECTIVES} learning objectives, and at most {MAX_FRAGMENT_SECTION_CONCEPTS} key concepts.''',
    '''         Build a CONCISE outline fragment for the educationally meaningful content in this bounded source window. Preserve section/chapter hierarchy and page spans actually supported by this window.          Return at most {MAX_FRAGMENT_OBJECTIVES} learning objectives, {MAX_FRAGMENT_CONCEPTS} key concepts, and {MAX_FRAGMENT_SECTIONS} sections. Keep the fragment summary to roughly 1–3 sentences and each section summary to 1–2 short sentences; at most {MAX_FRAGMENT_SECTION_OBJECTIVES} section learning objectives and {MAX_FRAGMENT_SECTION_CONCEPTS} section key concepts. Do not spend output tokens restating source prose.''',
)
replace_region(
    intel,
    "fn outline_completion_tokens_for_provider(provider_kind: ProviderKind) -> usize {",
    "\nfn flashcard_completion_tokens",
    '''fn outline_completion_tokens_for_provider(_provider_kind: ProviderKind) -> usize {\n    1_536\n}\n''',
)
# Tighten schema string budgets. These limits remain comfortably above concise
# educational outline needs but prevent 3k-token local outline monologues.
for old, new in [
    ('"summary": string_schema(4_000),', '"summary": string_schema(1_800),'),
    ('"learningObjectives": string_array_schema(MAX_FRAGMENT_OBJECTIVES, 1_200),', '"learningObjectives": string_array_schema(MAX_FRAGMENT_OBJECTIVES, 500),'),
    ('"keyConcepts": string_array_schema(MAX_FRAGMENT_CONCEPTS, 700),', '"keyConcepts": string_array_schema(MAX_FRAGMENT_CONCEPTS, 240),'),
    ('"title": string_schema(500),', '"title": string_schema(240),'),
    ('"summary": string_schema(2_500),', '"summary": string_schema(900),'),
    ('"learningObjectives": string_array_schema(MAX_FRAGMENT_SECTION_OBJECTIVES, 1_000),', '"learningObjectives": string_array_schema(MAX_FRAGMENT_SECTION_OBJECTIVES, 420),'),
    ('"keyConcepts": string_array_schema(MAX_FRAGMENT_SECTION_CONCEPTS, 600)', '"keyConcepts": string_array_schema(MAX_FRAGMENT_SECTION_CONCEPTS, 220)'),
]:
    replace_once(intel, old, new)

# Generation windows are independent after the global outline exists, so execute
# them in bounded waves and serialize checkpoint/database commits afterward.
generation = "src-tauri/src/document_generation.rs"
replace_once(
    generation,
    "use serde_json::Value;\n",
    "use futures_util::{stream, StreamExt};\nuse serde_json::Value;\n",
)
new_generation = r'''pub(crate) async fn execute_generation_windows(
    input: GenerationExecution<'_>,
) -> Result<Vec<Value>, CommandError> {
    let renders = if input.task.include_page_images {
        document_store::load_source_renders(input.database, input.document_id)?
    } else {
        Vec::new()
    };
    let mut cached = input.cached_cards;
    if cached.len() < input.windows.len() {
        cached.resize_with(input.windows.len(), || None);
    }

    let parallelism = crate::ai::concurrency::document_request_parallelism(&input.task.provider_kind)
        .clamp(1, input.windows.len().max(1));
    crate::logging::info(
        "document_generation_execution_plan",
        format!(
            "document={} provider={} model={} windows={} parallelism={} warm_sticky_route={}",
            input.document_id,
            input.task.provider_kind,
            input.task.model_id,
            input.windows.len(),
            parallelism,
            input.task.provider_kind == "openrouter",
        ),
    );

    let mut by_window = vec![None; input.windows.len()];
    let mut completed_count = 0_usize;
    let mut missing = Vec::new();
    for (position, window) in input.windows.iter().enumerate() {
        if let Some(cards) = cached.get_mut(position).and_then(Option::take) {
            crate::logging::info(
                "document_generation_window_reused",
                format!(
                    "document={} window={}/{} partition={} locator={} cards={}",
                    input.document_id,
                    window.window_index + 1,
                    input.windows.len(),
                    window.partition_index + 1,
                    window.source_locator().stable_identity(),
                    cards.len(),
                ),
            );
            document_store::mark_window_complete(
                input.database,
                input.document_id,
                input.job_id,
                window,
                cards.len(),
            )?;
            by_window[position] = Some(cards);
            completed_count = completed_count.saturating_add(1);
        } else {
            missing.push(position);
        }
    }
    document_store::update_generation_progress(
        input.database,
        input.document_id,
        completed_count,
    )?;

    let warm_sticky_route = input.task.provider_kind == "openrouter" && missing.len() > 1;
    let mut cursor = 0_usize;
    while cursor < missing.len() {
        let batch_size = if warm_sticky_route && cursor == 0 {
            1
        } else {
            parallelism
        };
        let end = (cursor + batch_size).min(missing.len());
        let batch = &missing[cursor..end];
        let first = &input.windows[batch[0]];
        let last = &input.windows[*batch.last().unwrap_or(&batch[0])];
        set_current_task_phase(
            "generating",
            Some(batch[0] + 1),
            completed_count,
            input.windows.len(),
            Some(first.page_start),
            Some(last.page_end),
            Some(first.partition_index + 1),
        );

        let mut prepared = Vec::with_capacity(batch.len());
        for position in batch.iter().copied() {
            let window = &input.windows[position];
            document_store::mark_window_running(
                input.database,
                input.document_id,
                input.job_id,
                window.window_index,
            )?;
            let images = if input.task.include_page_images {
                document_store::load_window_images(
                    input.paths,
                    &renders,
                    window,
                    input.plan.max_images_per_window,
                )?
            } else {
                Vec::new()
            };
            prepared.push((position, images));
        }

        let mut batch_results = stream::iter(prepared.iter().map(|(position, images)| {
            let position = *position;
            let window = &input.windows[position];
            async move {
                (
                    position,
                    generate_window_with_recovery(WindowGenerationRequest {
                        runtime: input.runtime,
                        task: input.task,
                        document_title: input.document_title,
                        outline: input.outline,
                        window,
                        images,
                        maximum_cards: input.plan.max_cards_per_window,
                        diagnostic_prefix: input.diagnostic_prefix,
                    })
                    .await,
                )
            }
        }))
        .buffer_unordered(batch.len().max(1))
        .collect::<Vec<_>>()
        .await;
        batch_results.sort_by_key(|(position, _)| *position);

        let mut first_error = None;
        for (position, result) in batch_results {
            match result {
                Ok(cards) => {
                    let window = &input.windows[position];
                    if let Some(contract) = input.checkpoint_contract {
                        document_checkpoints::persist_generation_cards_best_effort(
                            input.database,
                            input.document_id,
                            contract,
                            window,
                            &cards,
                        );
                    }
                    document_store::mark_window_complete(
                        input.database,
                        input.document_id,
                        input.job_id,
                        window,
                        cards.len(),
                    )?;
                    by_window[position] = Some(cards);
                    completed_count = completed_count.saturating_add(1);
                    document_store::update_generation_progress(
                        input.database,
                        input.document_id,
                        completed_count,
                    )?;
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        cursor = end;
    }

    let mut generated = Vec::new();
    for cards in by_window.into_iter().flatten() {
        if generated.len().saturating_add(cards.len()) > MAX_GENERATED_CARDS {
            return Err(CommandError::new(
                "too_many_generated_cards",
                format!(
                    "The model generated more than {MAX_GENERATED_CARDS} cards from this document."
                ),
            ));
        }
        generated.extend(cards);
    }
    set_current_task_phase(
        "curating",
        None,
        input.windows.len(),
        input.windows.len(),
        None,
        None,
        None,
    );
    Ok(generated)
}

'''
replace_region(
    generation,
    "pub(crate) async fn execute_generation_windows(",
    "async fn generate_window_with_recovery(",
    new_generation,
)

# OpenRouter stream watchdogs: initial-response timeout remains separate, but an
# accepted stream may no longer sit silent indefinitely or consume an entire
# bounded request for many minutes while trickling unusably slowly.
gateway = "src-tauri/src/ai/gateway.rs"
replace_once(
    gateway,
    "use tokio::time::{sleep, timeout};",
    "use tokio::time::{sleep, timeout, Instant};",
)
replace_once(
    gateway,
    "const DOCUMENT_RETRY_DELAY_MILLIS: u64 = 750;\n",
    "const DOCUMENT_RETRY_DELAY_MILLIS: u64 = 750;\nconst OPENROUTER_STREAM_IDLE_TIMEOUT_SECONDS: u64 = 90;\nconst OPENROUTER_STREAM_MAX_SECONDS: u64 = 300;\n",
)
replace_once(
    gateway,
    '''        let first_chunk = tokio::select! {\n            _ = cancellation.canceled() => return Err(canceled_error()),\n            result = response.chunk() => result.map_err(classify_reqwest_error)?,\n        };\n        let first_chunk = first_chunk.ok_or_else(|| {''',
    '''        let stream_started = Instant::now();\n        let stream_deadline = openrouter.then(|| {\n            stream_started + Duration::from_secs(OPENROUTER_STREAM_MAX_SECONDS)\n        });\n        let first_chunk = read_stream_chunk(\n            &mut response,\n            cancellation,\n            openrouter,\n            stream_deadline,\n        )\n        .await?;\n        let first_chunk = first_chunk.ok_or_else(|| {''',
)
replace_once(
    gateway,
    '''        while !stream.done {\n            let next = tokio::select! {\n                _ = cancellation.canceled() => return Err(canceled_error()),\n                result = response.chunk() => result.map_err(classify_reqwest_error)?,\n            };\n            let Some(chunk) = next else {\n                break;\n            };\n            stream.push_bytes(&chunk)?;\n        }\n        stream.finish()?;''',
    '''        while !stream.done {\n            let next = read_stream_chunk(\n                &mut response,\n                cancellation,\n                openrouter,\n                stream_deadline,\n            )\n            .await?;\n            let Some(chunk) = next else {\n                break;\n            };\n            stream.push_bytes(&chunk)?;\n        }\n        stream.finish()?;\n        if openrouter {\n            crate::logging::info_target(\n                "ai.gateway.openrouter",\n                "provider_stream_completed",\n                "An OpenRouter document request completed.",\n                json!({\n                    "generationId": stream.generation_id.as_deref(),\n                    "upstreamProvider": stream.upstream_provider.as_deref(),\n                    "finishReason": stream.finish_reason.as_deref(),\n                    "durationMs": u64::try_from(stream_started.elapsed().as_millis()).unwrap_or(u64::MAX),\n                }),\n            );\n        }''',
)
replace_once(
    gateway,
    "    usage: Option<Value>,\n    wire_bytes: usize,",
    "    usage: Option<Value>,\n    upstream_provider: Option<String>,\n    wire_bytes: usize,",
)
replace_once(
    gateway,
    "            usage: None,\n            wire_bytes: 0,",
    "            usage: None,\n            upstream_provider: None,\n            wire_bytes: 0,",
)
replace_once(
    gateway,
    '''        if let Some(usage) = safe_usage_envelope(&value) {\n            self.usage = Some(usage);\n        }''',
    '''        if let Some(usage) = safe_usage_envelope(&value) {\n            self.usage = Some(usage);\n        }\n        if self.openrouter {\n            if let Some(provider) = selected_openrouter_provider(&value) {\n                self.upstream_provider = Some(provider);\n            }\n        }''',
)
# Add stream-chunk watchdog helper before bounded body reader.
helper = r'''async fn read_stream_chunk(
    response: &mut Response,
    cancellation: &CancellationSignal,
    openrouter: bool,
    deadline: Option<Instant>,
) -> Result<Option<Vec<u8>>, AiGatewayError> {
    if !openrouter {
        return tokio::select! {
            _ = cancellation.canceled() => Err(canceled_error()),
            result = response.chunk() => result
                .map(|chunk| chunk.map(|bytes| bytes.to_vec()))
                .map_err(classify_reqwest_error),
        };
    }

    let now = Instant::now();
    let remaining = deadline
        .and_then(|deadline| deadline.checked_duration_since(now))
        .unwrap_or(Duration::ZERO);
    if remaining.is_zero() {
        return Err(error(
            "ai_provider_deadline",
            "OpenRouter kept this bounded document request open for more than five minutes. The partial response was discarded so ostudy can retry the window on a healthier endpoint.",
            true,
            None,
        ));
    }
    let wait = remaining.min(Duration::from_secs(OPENROUTER_STREAM_IDLE_TIMEOUT_SECONDS));
    let result = tokio::select! {
        _ = cancellation.canceled() => return Err(canceled_error()),
        result = timeout(wait, response.chunk()) => result,
    };
    match result {
        Ok(Ok(chunk)) => Ok(chunk.map(|bytes| bytes.to_vec())),
        Ok(Err(error_value)) => Err(classify_reqwest_error(error_value)),
        Err(_) if deadline.is_some_and(|deadline| Instant::now() >= deadline) => Err(error(
            "ai_provider_deadline",
            "OpenRouter kept this bounded document request open for more than five minutes. The partial response was discarded so ostudy can retry the window on a healthier endpoint.",
            true,
            None,
        )),
        Err(_) => Err(error(
            "ai_provider_stalled",
            "OpenRouter produced no stream activity for 90 seconds. The partial response was discarded so ostudy can retry the window on another healthy route.",
            true,
            None,
        )),
    }
}

'''
value = read(gateway)
marker = "async fn read_bounded_body("
pos = value.find(marker)
if pos < 0:
    raise SystemExit("gateway: read_bounded_body marker not found")
write(gateway, value[:pos] + helper + value[pos:])
replace_once(
    gateway,
    '''            | "ai_request_timeout"\n            | "ai_initial_response_timeout"''',
    '''            | "ai_request_timeout"\n            | "ai_initial_response_timeout"\n            | "ai_provider_stalled"\n            | "ai_provider_deadline"\n            | "ai_rate_limited"\n            | "ai_http_rate_limited"''',
)
replace_once(
    gateway,
    "        Some(seconds @ 1..=5) => Duration::from_secs(seconds),",
    "        Some(seconds @ 1..=30) => Duration::from_secs(seconds),",
)
replace_once(
    gateway,
    '''        assert!(should_retry_document_stream_error(&error(\n            "ai_network_error",\n            "network",\n            true,\n            None,\n        )));''',
    '''        assert!(should_retry_document_stream_error(&error(\n            "ai_network_error",\n            "network",\n            true,\n            None,\n        )));\n        assert!(should_retry_document_stream_error(&error(\n            "ai_provider_stalled",\n            "stalled",\n            true,\n            None,\n        )));\n        assert!(should_retry_document_stream_error(&error(\n            "ai_rate_limited",\n            "rate limited",\n            true,\n            Some(4),\n        )));''',
)

# Documentation: hierarchy and transport batching are independent. All providers
# use bounded requests; OpenRouter adds high-precision cost/performance routing.
architecture = "docs/architecture/ai-and-document-processing.md"
replace_once(
    architecture,
    '''Provider source windows are deliberately conservative. Large documents scale through **more bounded requests**, not larger individual prompts. A pathological semantic chunk larger than the provider boundary is split into bounded continuation units while retaining original source/page provenance.\n\nLocal llama.cpp/Unsloth jobs are **window-bounded, not aggregate-job-bounded**. The former whole-job 1.2M-token ceiling and cloud-only source-token guard do not apply to local execution. Individual requests remain constrained by the configured/reported model context. Metered/cloud providers retain conservative aggregate runaway-cost protection.''',
    '''Provider source windows are deliberately conservative. Large documents scale through **more bounded requests**, not ever-growing individual prompts. A pathological semantic chunk larger than the provider boundary is split into bounded continuation units while retaining original source/page provenance. Heading and chapter metadata stay inside each chunk but no longer force a new network request; request packing is driven by the source-token/source-unit budget and the model's reported context window.\n\nAll providers are **window-bounded, not aggregate-job-bounded**. Whole-document token estimates are diagnostic/cost information rather than a provider-specific refusal threshold. A smaller-context model therefore receives more source windows while a larger-context model can use fewer, larger windows.\n\nIndependent provider windows execute with bounded concurrency: OpenRouter permits up to four active document requests, direct DeepSeek up to two, and local llama.cpp remains serial to avoid competing with one local inference engine. OpenRouter warms one request first so its stable document `session_id` can establish sticky routing before subsequent windows fan out. Successful windows are checkpointed and final hierarchy/card order remains deterministic regardless of completion order.''',
)

user_guide = "docs/user-guide/ai-providers.md"
replace_once(
    user_guide,
    '''For document generation, ostudy requests strict JSON Schema output only from parameter-compatible endpoints and sets OpenRouter's provider-data-collection policy to `deny`. It explicitly disables provider-side context compression so document windows are not silently changed, and it never enables prompt/body debugging. Enable **Require zero data retention** on a profile to send `provider.zdr=true`; this stronger restriction can reduce endpoint availability, and the connection test reports that as a routing/capability failure rather than silently weakening the request.''',
    '''For document generation, ostudy requests strict JSON Schema output only from parameter-compatible endpoints and sets OpenRouter's provider-data-collection policy to `deny`. Routing filters out provider endpoints below 8-bit precision (`int4`, FP4-family, FP6, and unknown quantization are excluded), sorts eligible endpoints by price, and prefers endpoints whose recent throughput and latency clear oStudy's batch-processing performance floor. The throughput/latency preferences are soft so a healthy high-precision fallback remains available; the quantization and privacy filters are hard.\n\nostudy explicitly disables provider-side context compression so document windows are not silently changed, and it never enables prompt/body debugging. A stable per-document `session_id` establishes OpenRouter sticky routing after the first successful request; oStudy deliberately warms that route before issuing up to four independent document windows concurrently. A stalled OpenRouter stream is abandoned after 90 seconds without activity, and each bounded request has a five-minute maximum, allowing the existing single transient retry to choose a healthier route instead of leaving one outline window stuck indefinitely.\n\nEnable **Require zero data retention** on a profile to send `provider.zdr=true`; this stronger restriction can reduce endpoint availability, and the connection test reports that as a routing/capability failure rather than silently weakening the request.''',
)

print("Applied outline packing, bounded concurrency, OpenRouter routing, and stream watchdog rework.")
