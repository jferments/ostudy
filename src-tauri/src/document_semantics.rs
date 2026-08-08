use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Persisted, engine-neutral semantic chunk used by all document-aware AI tasks.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SemanticChunk {
    pub chunk_index: i64,
    pub text: String,
    pub full_text: String,
    pub page_numbers: Vec<i64>,
    pub element_types: Vec<String>,
    pub heading_context: Option<String>,
    pub token_estimate: usize,
}

/// A contiguous, overlapping semantic source window. Windows are bounded both
/// by estimated tokens and by distinct physical pages, so dense textbooks do
/// not overflow smaller local models while sparse slide decks can span dozens
/// of pages in one reasoning unit.
#[derive(Clone, Debug)]
pub(crate) struct SemanticWindow {
    pub window_index: usize,
    pub first_chunk_index: i64,
    pub last_chunk_index: i64,
    pub page_numbers: Vec<i64>,
    pub page_start: i64,
    pub page_end: i64,
    pub token_estimate: usize,
    pub source_text: String,
}

pub(crate) fn build_semantic_windows(
    chunks: &[SemanticChunk],
    max_tokens: usize,
    max_pages: usize,
    overlap_tokens: usize,
) -> Vec<SemanticWindow> {
    if chunks.is_empty() || max_tokens == 0 || max_pages == 0 {
        return Vec::new();
    }

    let mut windows = Vec::new();
    let mut start = 0_usize;

    while start < chunks.len() {
        let mut end = start;
        let mut token_total = 0_usize;
        let mut pages = BTreeSet::new();

        while end < chunks.len() {
            let chunk = &chunks[end];
            let next_tokens = token_total.saturating_add(chunk.token_estimate.max(1));
            let mut next_pages = pages.clone();
            next_pages.extend(chunk.page_numbers.iter().copied());

            let would_exceed =
                end > start && (next_tokens > max_tokens || next_pages.len() > max_pages);
            if would_exceed {
                break;
            }
            token_total = next_tokens;
            pages = next_pages;
            end += 1;
        }

        // A single oversized semantic chunk is still kept intact rather than
        // cutting through a table, paragraph, or heading relationship.
        if end == start {
            end = start + 1;
            token_total = chunks[start].token_estimate.max(1);
            pages.extend(chunks[start].page_numbers.iter().copied());
        }

        let selected = &chunks[start..end];
        let page_numbers = pages.into_iter().collect::<Vec<_>>();
        let page_start = page_numbers.first().copied().unwrap_or(1);
        let page_end = page_numbers.last().copied().unwrap_or(page_start);
        let source_text = selected
            .iter()
            .map(format_chunk_for_model)
            .collect::<Vec<_>>()
            .join("\n\n");

        windows.push(SemanticWindow {
            window_index: windows.len(),
            first_chunk_index: selected.first().map_or(0, |chunk| chunk.chunk_index),
            last_chunk_index: selected.last().map_or(0, |chunk| chunk.chunk_index),
            page_numbers,
            page_start,
            page_end,
            token_estimate: token_total,
            source_text,
        });

        if end >= chunks.len() {
            break;
        }

        // Carry complete semantic chunks forward until we have roughly the
        // configured overlap. Never restart at the same chunk: progress is a
        // hard invariant even when one chunk is itself larger than the overlap.
        let mut next_start = end;
        let mut overlap = 0_usize;
        while next_start > start + 1 && overlap < overlap_tokens {
            next_start -= 1;
            overlap = overlap.saturating_add(chunks[next_start].token_estimate.max(1));
        }
        if next_start <= start {
            next_start = start + 1;
        }
        start = next_start;
    }

    windows
}

fn format_chunk_for_model(chunk: &SemanticChunk) -> String {
    let heading = chunk.heading_context.as_deref().unwrap_or("(no heading)");
    let pages = chunk
        .page_numbers
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let types = if chunk.element_types.is_empty() {
        "unknown".to_string()
    } else {
        chunk.element_types.join(",")
    };
    format!(
        "<ostudy-chunk index=\"{}\" pages=\"{}\" heading=\"{}\" element-types=\"{}\">\n{}\n</ostudy-chunk>",
        chunk.chunk_index,
        pages,
        escape_attribute(heading),
        escape_attribute(&types),
        chunk.full_text.trim()
    )
}

fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(index: i64, pages: &[i64], tokens: usize) -> SemanticChunk {
        SemanticChunk {
            chunk_index: index,
            text: format!("text {index}"),
            full_text: format!("Section {index}\ntext {index}"),
            page_numbers: pages.to_vec(),
            element_types: vec!["paragraph".to_string()],
            heading_context: Some(format!("Section {index}")),
            token_estimate: tokens,
        }
    }

    #[test]
    fn windows_cross_physical_page_boundaries_without_cutting_chunks() {
        let chunks = vec![
            chunk(0, &[1, 2], 400),
            chunk(1, &[2, 3], 400),
            chunk(2, &[3, 4], 400),
        ];
        let windows = build_semantic_windows(&chunks, 2_000, 20, 0);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].page_numbers, vec![1, 2, 3, 4]);
        assert!(windows[0].source_text.contains("Section 0"));
        assert!(windows[0].source_text.contains("Section 2"));
    }

    #[test]
    fn windows_honor_token_and_page_caps_with_semantic_overlap() {
        let chunks = (0..8)
            .map(|index| chunk(index, &[index + 1], 500))
            .collect::<Vec<_>>();
        let windows = build_semantic_windows(&chunks, 1_500, 3, 500);
        assert!(windows.len() >= 3);
        assert!(windows.iter().all(|window| window.page_numbers.len() <= 3));
        assert_eq!(windows[0].last_chunk_index, windows[1].first_chunk_index);
        assert!(windows
            .iter()
            .all(|window| window.page_end >= window.page_start));
    }

    #[test]
    fn one_oversized_chunk_is_preserved_and_progresses() {
        let chunks = vec![chunk(0, &[1, 2, 3], 50_000), chunk(1, &[4], 100)];
        let windows = build_semantic_windows(&chunks, 10_000, 2, 1_000);
        assert_eq!(windows[0].first_chunk_index, 0);
        assert_eq!(windows[0].last_chunk_index, 0);
        assert_eq!(windows[1].first_chunk_index, 1);
    }
}
