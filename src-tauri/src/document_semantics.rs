use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::document_scale::DocumentScalePlan;

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

/// A local, heading-aware document partition. Partitions are never sent as one
/// unbounded prompt: they define independent semantic/checkpoint boundaries,
/// and each partition is further divided into hard-bounded provider windows.
#[derive(Clone, Debug)]
pub(crate) struct SemanticPartition {
    pub partition_index: usize,
    pub title: String,
    pub page_start: i64,
    chunk_start: usize,
    chunk_end: usize,
}

/// A contiguous, overlapping semantic source window. Windows are bounded both
/// by estimated tokens and by distinct physical pages, so dense textbooks do
/// not overflow smaller local models while sparse slide decks can span several
/// pages in one reasoning unit.
#[derive(Clone, Debug)]
pub(crate) struct SemanticWindow {
    pub window_index: usize,
    pub partition_index: usize,
    pub partition_title: String,
    pub first_chunk_index: i64,
    pub last_chunk_index: i64,
    pub page_numbers: Vec<i64>,
    pub visual_page_numbers: Vec<i64>,
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

pub(crate) fn build_partitioned_semantic_windows(
    chunks: &[SemanticChunk],
    plan: DocumentScalePlan,
    max_tokens: usize,
    max_pages: usize,
    overlap_tokens: usize,
) -> Vec<SemanticWindow> {
    let partitions = build_semantic_partitions(chunks, plan);
    let mut windows = Vec::new();
    for partition in partitions {
        let mut partition_windows = build_semantic_windows_internal(
            &chunks[partition.chunk_start..partition.chunk_end],
            max_tokens,
            max_pages,
            overlap_tokens,
            partition.partition_index,
            &partition.title,
        );
        for window in &mut partition_windows {
            window.window_index = windows.len();
        }
        windows.extend(partition_windows);
    }
    windows
}

pub(crate) fn build_semantic_partitions(
    chunks: &[SemanticChunk],
    plan: DocumentScalePlan,
) -> Vec<SemanticPartition> {
    if chunks.is_empty() {
        return Vec::new();
    }

    let mut partitions = Vec::new();
    let mut start = 0_usize;
    while start < chunks.len() {
        let mut end = start;
        let mut token_total = 0_usize;
        let mut pages = BTreeSet::new();
        let mut title = partition_heading(&chunks[start]);

        while end < chunks.len() {
            let chunk = &chunks[end];
            let next_heading = partition_heading(chunk);
            let chunk_tokens = effective_chunk_tokens(chunk);
            let next_tokens = token_total.saturating_add(chunk_tokens);
            let mut next_pages = pages.clone();
            next_pages.extend(chunk.page_numbers.iter().copied());

            let exceeds_hard_boundary = end > start
                && (next_tokens > plan.partition_max_tokens
                    || next_pages.len() > plan.partition_max_pages);
            let heading_changed =
                end > start && title.is_some() && next_heading.is_some() && title != next_heading;
            let enough_for_semantic_split = token_total
                >= plan.partition_target_tokens.saturating_div(3).max(1_500)
                || pages.len() >= plan.partition_target_pages.saturating_div(3).max(4);
            let reached_target = token_total >= plan.partition_target_tokens
                || pages.len() >= plan.partition_target_pages;

            if exceeds_hard_boundary
                || (heading_changed && enough_for_semantic_split)
                || (end > start && reached_target)
            {
                break;
            }

            if title.is_none() {
                title = next_heading;
            }
            token_total = next_tokens;
            pages = next_pages;
            end += 1;
        }

        if end == start {
            let chunk = &chunks[start];
            token_total = effective_chunk_tokens(chunk);
            pages.extend(chunk.page_numbers.iter().copied());
            title = title.or_else(|| partition_heading(chunk));
            end = start + 1;
        }

        let page_numbers = pages.into_iter().collect::<Vec<_>>();
        let page_start = page_numbers.first().copied().unwrap_or(1);
        let page_end = page_numbers.last().copied().unwrap_or(page_start);
        let title = title.unwrap_or_else(|| format!("PDF pages {page_start}–{page_end}"));
        partitions.push(SemanticPartition {
            partition_index: partitions.len(),
            title,
            page_start,
            chunk_start: start,
            chunk_end: end,
        });
        start = end;
    }
    partitions
}

#[cfg(test)]
pub(crate) fn build_semantic_windows(
    chunks: &[SemanticChunk],
    max_tokens: usize,
    max_pages: usize,
    overlap_tokens: usize,
) -> Vec<SemanticWindow> {
    let title = chunks
        .iter()
        .find_map(partition_heading)
        .unwrap_or_else(|| "Document source".to_string());
    build_semantic_windows_internal(chunks, max_tokens, max_pages, overlap_tokens, 0, &title)
}

fn build_semantic_windows_internal(
    chunks: &[SemanticChunk],
    max_tokens: usize,
    max_pages: usize,
    overlap_tokens: usize,
    partition_index: usize,
    partition_title: &str,
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
        let visual_page_numbers = selected
            .iter()
            .filter(|unit| chunk_needs_visual_grounding(unit.chunk))
            .flat_map(|unit| unit.page_numbers.iter().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let chunks_text = selected
            .iter()
            .map(format_unit_for_model)
            .collect::<Vec<_>>()
            .join("\n\n");
        let source_text = format!(
            "<ostudy-partition index=\"{}\" title=\"{}\" pages=\"{}-{}\">\n{}\n</ostudy-partition>",
            partition_index,
            escape_attribute(partition_title),
            page_start,
            page_end,
            chunks_text
        );

        windows.push(SemanticWindow {
            window_index: windows.len(),
            partition_index,
            partition_title: partition_title.to_string(),
            first_chunk_index: selected.first().map_or(0, |unit| unit.chunk.chunk_index),
            last_chunk_index: selected.last().map_or(0, |unit| unit.chunk.chunk_index),
            page_numbers,
            visual_page_numbers,
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

fn effective_chunk_tokens(chunk: &SemanticChunk) -> usize {
    chunk
        .token_estimate
        .max(chunk.full_text.chars().count().div_ceil(4))
        .max(1)
}

fn partition_heading(chunk: &SemanticChunk) -> Option<String> {
    let heading = chunk.heading_context.as_deref()?.trim();
    if heading.is_empty() {
        return None;
    }
    let first_line = heading.lines().find(|line| !line.trim().is_empty())?.trim();
    let root = [" > ", " › ", " :: ", " / ", " | "]
        .iter()
        .filter_map(|separator| {
            first_line
                .split_once(separator)
                .map(|(left, _)| left.trim())
        })
        .find(|value| !value.is_empty())
        .unwrap_or(first_line);
    let bounded = root
        .chars()
        .filter(|character| !character.is_control())
        .take(180)
        .collect::<String>()
        .trim()
        .to_string();
    (!bounded.is_empty()).then_some(bounded)
}

fn chunk_needs_visual_grounding(chunk: &SemanticChunk) -> bool {
    chunk
        .element_types
        .iter()
        .any(|element_type| matches!(element_type.as_str(), "image" | "table" | "key_value"))
        || chunk
            .full_text
            .contains("has no extractable structured text")
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
        if start >= characters.len() {
            segments.push(String::new());
            continue;
        }
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
        let before_end = target.min(ceiling);

        let mut split = characters[floor..=before_end]
            .iter()
            .rposition(|character| character.is_whitespace())
            .map(|offset| floor + offset + 1);
        if split.is_none() {
            let after_start = target.min(ceiling);
            split = characters[after_start..=ceiling]
                .iter()
                .position(|character| character.is_whitespace())
                .map(|offset| after_start + offset + 1);
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

    #[test]
    fn heading_changes_create_local_partitions_before_hard_limits() {
        let mut chunks = Vec::new();
        for index in 0..12 {
            let mut value = chunk(index, &[index + 1], 900);
            value.heading_context = Some(if index < 6 {
                "Chapter 1 > Topic".to_string()
            } else {
                "Chapter 2 > Topic".to_string()
            });
            chunks.push(value);
        }
        let plan = DocumentScalePlan::for_page_count(138);
        let partitions = build_semantic_partitions(&chunks, plan);
        assert_eq!(partitions.len(), 2);
        assert_eq!(partitions[0].title, "Chapter 1");
        assert_eq!(partitions[1].title, "Chapter 2");
        assert_eq!(partitions[1].page_start, 7);
    }

    #[test]
    fn partitioned_windows_do_not_overlap_across_chapters() {
        let mut chunks = Vec::new();
        for index in 0..8 {
            let mut value = chunk(index, &[index + 1], 700);
            value.heading_context = Some(if index < 4 {
                "Chapter A".to_string()
            } else {
                "Chapter B".to_string()
            });
            if index == 5 {
                value.element_types = vec!["image".to_string()];
            }
            chunks.push(value);
        }
        let plan = DocumentScalePlan::for_page_count(138);
        let windows = build_partitioned_semantic_windows(&chunks, plan, 1_400, 2, 300);
        assert!(windows
            .iter()
            .any(|window| window.partition_title == "Chapter A"));
        assert!(windows
            .iter()
            .any(|window| window.partition_title == "Chapter B"));
        assert!(windows
            .iter()
            .all(|window| !(window.page_start <= 4 && window.page_end >= 5)));
        assert!(windows
            .iter()
            .any(|window| window.visual_page_numbers.contains(&6)));
    }
}
