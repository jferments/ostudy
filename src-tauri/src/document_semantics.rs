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
/// not overflow smaller local models while sparse slide decks can span several
/// pages in one reasoning unit.
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

#[derive(Clone, Debug)]
struct SourceUnit<'a> {
    chunk: &'a SemanticChunk,
    segment_index: usize,
    segment_count: usize,
    page_numbers: Vec<i64>,
    text: String,
    token_estimate: usize,
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

    // Third-party semantic chunkers correctly try not to cut tables, headings,
    // or paragraphs, but a pathological source can still produce one chunk far
    // larger than the provider window. Hard request bounds take precedence at
    // the provider boundary. Only those oversized chunks are split, while their
    // original chunk identity and page provenance remain attached to every
    // continuation segment.
    let units = build_source_units(chunks, max_tokens, max_pages);
    let mut windows = Vec::new();
    let mut start = 0_usize;

    while start < units.len() {
        let mut end = start;
        let mut token_total = 0_usize;
        let mut pages = BTreeSet::new();

        while end < units.len() {
            let unit = &units[end];
            let next_tokens = token_total.saturating_add(unit.token_estimate.max(1));
            let mut next_pages = pages.clone();
            next_pages.extend(unit.page_numbers.iter().copied());

            let would_exceed =
                end > start && (next_tokens > max_tokens || next_pages.len() > max_pages);
            if would_exceed {
                break;
            }
            token_total = next_tokens;
            pages = next_pages;
            end += 1;
        }

        // build_source_units guarantees one unit is itself within the token/page
        // bound. This branch is defensive and guarantees progress if that helper
        // is changed in the future.
        if end == start {
            end = start + 1;
            token_total = units[start].token_estimate.max(1).min(max_tokens);
            pages.extend(units[start].page_numbers.iter().copied().take(max_pages));
        }

        let selected = &units[start..end];
        let page_numbers = pages.into_iter().collect::<Vec<_>>();
        let page_start = page_numbers.first().copied().unwrap_or(1);
        let page_end = page_numbers.last().copied().unwrap_or(page_start);
        let source_text = selected
            .iter()
            .map(format_unit_for_model)
            .collect::<Vec<_>>()
            .join("\n\n");

        windows.push(SemanticWindow {
            window_index: windows.len(),
            first_chunk_index: selected.first().map_or(0, |unit| unit.chunk.chunk_index),
            last_chunk_index: selected.last().map_or(0, |unit| unit.chunk.chunk_index),
            page_numbers,
            page_start,
            page_end,
            token_estimate: token_total.min(max_tokens),
            source_text,
        });

        if end >= units.len() {
            break;
        }

        // Carry complete source units forward until we have roughly the
        // configured overlap. Never restart at the same unit: progress remains
        // a hard invariant even when a persisted chunk was split into several
        // continuation units.
        let mut next_start = end;
        let mut overlap = 0_usize;
        while next_start > start + 1 && overlap < overlap_tokens {
            next_start -= 1;
            overlap = overlap.saturating_add(units[next_start].token_estimate.max(1));
        }
        if next_start <= start {
            next_start = start + 1;
        }
        start = next_start;
    }

    windows
}

fn build_source_units<'a>(
    chunks: &'a [SemanticChunk],
    max_tokens: usize,
    max_pages: usize,
) -> Vec<SourceUnit<'a>> {
    let mut units = Vec::new();
    for chunk in chunks {
        let text_chars = chunk.full_text.chars().count().max(1);
        let text_token_estimate = text_chars.div_ceil(4);
        let source_token_estimate = chunk.token_estimate.max(text_token_estimate).max(1);
        let token_segments = source_token_estimate.div_ceil(max_tokens).max(1);
        let page_segments = chunk.page_numbers.len().div_ceil(max_pages).max(1);
        let segment_count = token_segments.max(page_segments);
        let text_segments = split_text_into_segments(&chunk.full_text, segment_count);

        for segment_index in 0..segment_count {
            let text = text_segments
                .get(segment_index)
                .cloned()
                .unwrap_or_default();
            let page_numbers = segment_pages(&chunk.page_numbers, segment_index, segment_count);
            let distributed_estimate = source_token_estimate.div_ceil(segment_count);
            let segment_estimate = text.chars().count().div_ceil(4).max(distributed_estimate);
            units.push(SourceUnit {
                chunk,
                segment_index,
                segment_count,
                page_numbers,
                text,
                token_estimate: segment_estimate.clamp(1, max_tokens),
            });
        }
    }
    units
}

fn split_text_into_segments(value: &str, segment_count: usize) -> Vec<String> {
    if segment_count <= 1 {
        return vec![value.trim().to_string()];
    }

    let characters = value.chars().collect::<Vec<_>>();
    if characters.is_empty() {
        return vec![String::new(); segment_count];
    }

    let mut segments = Vec::with_capacity(segment_count);
    let mut start = 0_usize;
    for segment_index in 0..segment_count {
        if segment_index + 1 == segment_count {
            segments.push(
                characters[start..]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .to_string(),
            );
            break;
        }

        let remaining_segments = segment_count - segment_index;
        let remaining_chars = characters.len().saturating_sub(start);
        let target = start + remaining_chars.div_ceil(remaining_segments);
        let search_radius = (remaining_chars / remaining_segments / 4).max(32);
        let floor = target.saturating_sub(search_radius).max(start + 1);
        let ceiling = (target + search_radius).min(characters.len() - 1);

        let mut split = None;
        for index in (floor..=target.min(ceiling)).rev() {
            if characters[index].is_whitespace() {
                split = Some(index + 1);
                break;
            }
        }
        if split.is_none() {
            for index in target.min(ceiling)..=ceiling {
                if characters[index].is_whitespace() {
                    split = Some(index + 1);
                    break;
                }
            }
        }
        let end = split.unwrap_or(target).clamp(start + 1, characters.len());
        segments.push(
            characters[start..end]
                .iter()
                .collect::<String>()
                .trim()
                .to_string(),
        );
        start = end;
        while start < characters.len() && characters[start].is_whitespace() {
            start += 1;
        }
    }

    while segments.len() < segment_count {
        segments.push(String::new());
    }
    segments
}

fn segment_pages(pages: &[i64], segment_index: usize, segment_count: usize) -> Vec<i64> {
    if pages.is_empty() {
        return Vec::new();
    }
    if segment_count <= 1 {
        return pages.to_vec();
    }

    let start = segment_index.saturating_mul(pages.len()) / segment_count;
    let end = (segment_index + 1)
        .saturating_mul(pages.len())
        .div_ceil(segment_count)
        .min(pages.len());
    if start < end {
        pages[start..end].to_vec()
    } else {
        vec![pages[start.min(pages.len() - 1)]]
    }
}

fn format_unit_for_model(unit: &SourceUnit<'_>) -> String {
    let chunk = unit.chunk;
    let heading = chunk.heading_context.as_deref().unwrap_or("(no heading)");
    let pages = unit
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
    let segment = if unit.segment_count > 1 {
        format!(
            " segment=\"{}/{}\"",
            unit.segment_index + 1,
            unit.segment_count
        )
    } else {
        String::new()
    };
    format!(
        "<ostudy-chunk index=\"{}\" pages=\"{}\"{} heading=\"{}\" element-types=\"{}\">\n{}\n</ostudy-chunk>",
        chunk.chunk_index,
        pages,
        segment,
        escape_attribute(heading),
        escape_attribute(&types),
        unit.text.trim()
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
    fn windows_cross_physical_page_boundaries_without_cutting_normal_chunks() {
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
        assert!(windows.iter().all(|window| window.token_estimate <= 1_500));
        assert!(windows.iter().all(|window| window.page_numbers.len() <= 3));
        assert_eq!(windows[0].last_chunk_index, windows[1].first_chunk_index);
        assert!(windows
            .iter()
            .all(|window| window.page_end >= window.page_start));
    }

    #[test]
    fn oversized_chunks_are_split_into_hard_bounded_continuations() {
        let mut oversized = chunk(0, &(1..=12).collect::<Vec<_>>(), 12_000);
        oversized.full_text = (0..12_000)
            .map(|index| format!("concept-{index} "))
            .collect::<String>();
        let chunks = vec![oversized, chunk(1, &[13], 100)];
        let windows = build_semantic_windows(&chunks, 1_000, 3, 100);

        assert!(windows.len() > 4);
        assert!(windows.iter().all(|window| window.token_estimate <= 1_000));
        assert!(windows.iter().all(|window| window.page_numbers.len() <= 3));
        assert!(windows
            .iter()
            .any(|window| window.source_text.contains("segment=\"2/")));
        assert!(windows.iter().any(|window| window.last_chunk_index == 1));
        assert!(windows
            .iter()
            .any(|window| window.source_text.contains("concept-11999")));
    }

    #[test]
    fn segment_text_preserves_tail_and_count() {
        let value = (0..1_000)
            .map(|index| format!("word-{index} "))
            .collect::<String>();
        let segments = split_text_into_segments(&value, 7);
        assert_eq!(segments.len(), 7);
        assert!(segments.first().unwrap().contains("word-0"));
        assert!(segments.last().unwrap().contains("word-999"));
        assert!(segments.iter().all(|segment| !segment.is_empty()));
    }
}
