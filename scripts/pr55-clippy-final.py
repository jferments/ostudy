#!/usr/bin/env python3
from __future__ import annotations

import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
SELF = pathlib.Path(__file__).resolve()
path = ROOT / "src-tauri/src/ai/document_intelligence.rs"
source = path.read_text()

# Group batch-specific request parameters so the semantic generation helper stays
# below Clippy's argument-count threshold without suppressing the lint.
old_signature = '''async fn generate_flashcard_batch(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    outline: &DocumentOutline,
    window: &SemanticWindow,
    images: &[SourcePageImage],
    targets: &[DocumentLearningTarget],
    maximum_cards: usize,
    completion_token_floor: usize,
    gap_pass: bool,
) -> Result<Vec<Value>, CommandError> {
    let outline_json = outline_context_for_window(outline, window)?;'''
new_signature = '''struct FlashcardBatchRequest<'a> {
    targets: &'a [DocumentLearningTarget],
    maximum_cards: usize,
    completion_token_floor: usize,
    gap_pass: bool,
}

async fn generate_flashcard_batch(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    outline: &DocumentOutline,
    window: &SemanticWindow,
    images: &[SourcePageImage],
    request: FlashcardBatchRequest<'_>,
) -> Result<Vec<Value>, CommandError> {
    let FlashcardBatchRequest {
        targets,
        maximum_cards,
        completion_token_floor,
        gap_pass,
    } = request;
    let outline_json = outline_context_for_window(outline, window)?;'''
if old_signature in source:
    source = source.replace(old_signature, new_signature, 1)
elif "struct FlashcardBatchRequest<'a>" not in source:
    raise RuntimeError("document_intelligence.rs: semantic batch helper signature not found")

old_empty = '''            images,
            &[],
            maximum_cards_per_response,
            limits.completion_token_floor,
            false,
        )'''
new_empty = '''            images,
            FlashcardBatchRequest {
                targets: &[],
                maximum_cards: maximum_cards_per_response,
                completion_token_floor: limits.completion_token_floor,
                gap_pass: false,
            },
        )'''
if old_empty in source:
    source = source.replace(old_empty, new_empty, 1)
elif "targets: &[]," not in source:
    raise RuntimeError("document_intelligence.rs: empty semantic batch call not found")

old_batch = '''            images,
            batch,
            batch.len().min(maximum_cards_per_response).max(1),
            limits.completion_token_floor,
            false,
        )'''
new_batch = '''            images,
            FlashcardBatchRequest {
                targets: batch,
                maximum_cards: batch.len().min(maximum_cards_per_response).max(1),
                completion_token_floor: limits.completion_token_floor,
                gap_pass: false,
            },
        )'''
if old_batch in source:
    source = source.replace(old_batch, new_batch, 1)
elif "targets: batch," not in source:
    raise RuntimeError("document_intelligence.rs: normal semantic batch call not found")

old_gap = '''            images,
            &gap_targets,
            gap_targets.len().min(maximum_cards_per_response).max(1),
            limits.completion_token_floor,
            true,
        )'''
new_gap = '''            images,
            FlashcardBatchRequest {
                targets: &gap_targets,
                maximum_cards: gap_targets.len().min(maximum_cards_per_response).max(1),
                completion_token_floor: limits.completion_token_floor,
                gap_pass: true,
            },
        )'''
if old_gap in source:
    source = source.replace(old_gap, new_gap, 1)
elif "targets: &gap_targets," not in source:
    raise RuntimeError("document_intelligence.rs: gap semantic batch call not found")

source = source.replace(
    "    candidates.sort_by(|left, right| right.2.cmp(&left.2));",
    "    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.2));",
    1,
)
if "candidates.sort_by(|left, right|" in source:
    raise RuntimeError("document_intelligence.rs: legacy semantic target sort still present")

path.write_text(source)
SELF.unlink()
