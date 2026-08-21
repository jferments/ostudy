use std::collections::{BTreeSet, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    document_domain::{DocumentSourceDescriptor, SourceLocator, SourceUnitKind},
    document_scale::DocumentScalePlan,
    error::CommandError,
};

/// Persisted, engine-neutral semantic chunk used by all document-aware AI tasks.
///
/// Canonical source identity lives in `source_locator`; parser-specific source
/// concepts do not cross this boundary. Visual evidence is represented by stable
/// asset IDs carried by the locator.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SemanticChunk {
    pub chunk_index: i64,
    pub text: String,
    pub full_text: String,
    pub source_locator: SourceLocator,
    pub element_types: Vec<String>,
    pub heading_context: Option<String>,
    pub token_estimate: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct SemanticPartition {
    pub partition_index: usize,
    pub title: String,
    chunk_start: usize,
    chunk_end: usize,
}

/// A contiguous, overlapping semantic source window with canonical provenance.
///
/// `page_start` and `page_end` are a deliberately temporary compatibility
/// projection for the existing PDF outline/result schema and progress UI. They
/// are always derived from `source_locator`; new generic code must use the
/// locator directly. A later generic AI-schema/UI stage can remove these names
/// without changing canonical source identity.
#[derive(Clone, Debug)]
pub(crate) struct SemanticWindow {
    pub window_index: usize,
    pub partition_index: usize,
    pub partition_title: String,
    pub first_chunk_index: i64,
    pub last_chunk_index: i64,
    pub source_locator: SourceLocator,
    pub page_start: i64,
    pub page_end: i64,
    pub token_estimate: usize,
    pub source_text: String,
}

impl SemanticWindow {
    pub(crate) fn source_locator(&self) -> SourceLocator {
        self.source_locator.clone()
    }
}

#[derive(Clone, Debug)]
struct SourceUnit<'a> {
    chunk: &'a SemanticChunk,
    segment_index: usize,
    segment_count: usize,
    unit_numbers: Vec<i64>,
    text: String,
    token_estimate: usize,
}

pub(crate) fn build_partitioned_semantic_windows(
    chunks: &[SemanticChunk],
    plan: DocumentScalePlan,
    max_tokens: usize,
    max_units: usize,
    overlap_tokens: usize,
) -> Vec<SemanticWindow> {
    let partitions = build_semantic_partitions(chunks, plan);
    let mut windows = Vec::new();
    for partition in partitions {
        let mut partition_windows = build_semantic_windows_internal(
            &chunks[partition.chunk_start..partition.chunk_end],
            max_tokens,
            max_units,
            overlap_tokens,
            partition.partition_index,
            &partition.title,
        );
        let base_index = windows.len();
        for (offset, window) in partition_windows.iter_mut().enumerate() {
            window.window_index = base_index + offset;
        }
        windows.extend(partition_windows);
    }
    windows
}

pub(crate) fn validate_semantic_windows(
    windows: &[SemanticWindow],
    source: &DocumentSourceDescriptor,
) -> Result<(), CommandError> {
    source.validate()?;
    if windows.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The document did not contain bounded semantic source windows for analysis.",
        ));
    }

    let mut identities = HashSet::new();
    for (expected_index, window) in windows.iter().enumerate() {
        if window.window_index != expected_index || !identities.insert(window.window_index) {
            return Err(CommandError::new(
                "invalid_document_window_identity",
                "The semantic planner produced duplicate or non-contiguous source-window identities.",
            ));
        }
        if window.first_chunk_index < 0 || window.last_chunk_index < window.first_chunk_index {
            return Err(CommandError::new(
                "invalid_document_window_chunks",
                "A semantic source window has an invalid chunk range.",
            ));
        }
        if window.token_estimate == 0 || window.source_text.trim().is_empty() {
            return Err(CommandError::new(
                "invalid_document_window_content",
                "A semantic source window has no bounded source content.",
            ));
        }
        if window.source_locator.unit_kind != source.unit_kind {
            return Err(CommandError::new(
                "invalid_document_source_locator",
                "A semantic source window uses the wrong source-unit kind for this document.",
            ));
        }
        window.source_locator.validate_within(source.unit_count)?;
        if window.page_start != window.source_locator.first()
            || window.page_end != window.source_locator.last()
        {
            return Err(CommandError::new(
                "invalid_document_window_compatibility_range",
                "A semantic source window's compatibility range diverged from its canonical source locator.",
            ));
        }
    }
    Ok(())
}

/// Group semantic chunks into coarse capacity partitions.
///
/// Heading changes deliberately do not create request boundaries. Heading
/// context remains attached to every chunk and is visible to the model, while
/// transport packing is controlled only by bounded token/source-unit capacity.
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
        let mut units = BTreeSet::new();

        while end < chunks.len() {
            let chunk = &chunks[end];
            let chunk_tokens = effective_chunk_tokens(chunk);
            let next_tokens = token_total.saturating_add(chunk_tokens);
            let mut next_units = units.clone();
            next_units.extend(chunk.source_locator.unit_numbers.iter().copied());

            let exceeds_hard_boundary = end > start
                && (next_tokens > plan.partition_max_tokens
                    || next_units.len() > plan.partition_max_units);
            let reached_target = token_total >= plan.partition_target_tokens
                || units.len() >= plan.partition_target_units;

            if exceeds_hard_boundary || (end > start && reached_target) {
                break;
            }

            token_total = next_tokens;
            units = next_units;
            end += 1;
        }

        if end == start {
            units.extend(chunks[start].source_locator.unit_numbers.iter().copied());
            end = start + 1;
        }

        let unit_numbers = units.into_iter().collect::<Vec<_>>();
        let unit_start = unit_numbers.first().copied().unwrap_or(1);
        let unit_end = unit_numbers.last().copied().unwrap_or(unit_start);
        let heading_titles = chunks[start..end]
            .iter()
            .filter_map(partition_heading)
            .collect::<BTreeSet<_>>();
        let title = if heading_titles.len() == 1 {
            heading_titles
                .into_iter()
                .next()
                .unwrap_or_else(|| format!("Source units {unit_start}–{unit_end}"))
        } else {
            format!("Source units {unit_start}–{unit_end}")
        };
        partitions.push(SemanticPartition {
            partition_index: partitions.len(),
            title,
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
    max_units: usize,
    overlap_tokens: usize,
) -> Vec<SemanticWindow> {
    let title = chunks
        .iter()
        .find_map(partition_heading)
        .unwrap_or_else(|| "Document source".to_string());
    build_semantic_windows_internal(chunks, max_tokens, max_units, overlap_tokens, 0, &title)
}

fn build_semantic_windows_internal(
    chunks: &[SemanticChunk],
    max_tokens: usize,
    max_units: usize,
    overlap_tokens: usize,
    partition_index: usize,
    partition_title: &str,
) -> Vec<SemanticWindow> {
    if chunks.is_empty() || max_tokens == 0 || max_units == 0 {
        return Vec::new();
    }

    let unit_kind = chunks
        .first()
        .map(|chunk| chunk.source_locator.unit_kind)
        .unwrap_or(SourceUnitKind::Section);
    let units = build_source_units(chunks, max_tokens, max_units);
    let mut windows = Vec::new();
    let mut start = 0_usize;

    while start < units.len() {
        let mut end = start;
        let mut token_total = 0_usize;
        let mut source_units = BTreeSet::new();

        while end < units.len() {
            let unit = &units[end];
            let next_tokens = token_total.saturating_add(unit.token_estimate.max(1));
            let mut next_units = source_units.clone();
            next_units.extend(unit.unit_numbers.iter().copied());

            let would_exceed =
                end > start && (next_tokens > max_tokens || next_units.len() > max_units);
            if would_exceed {
                break;
            }
            token_total = next_tokens;
            source_units = next_units;
            end += 1;
        }

        if end == start {
            end = start + 1;
            token_total = units[start].token_estimate.max(1).min(max_tokens);
            source_units.extend(units[start].unit_numbers.iter().copied().take(max_units));
        }

        let selected = &units[start..end];
        let mut unit_numbers = source_units.into_iter().collect::<Vec<_>>();
        if unit_numbers.is_empty() {
            unit_numbers.push(1);
        }
        let unit_start = unit_numbers.first().copied().unwrap_or(1);
        let unit_end = unit_numbers.last().copied().unwrap_or(unit_start);
        let asset_ids = selected
            .iter()
            .flat_map(|unit| unit.chunk.source_locator.asset_ids.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let chunks_text = selected
            .iter()
            .map(format_unit_for_model)
            .collect::<Vec<_>>()
            .join("\n\n");
        let source_text = format!(
            "<ostudy-partition index=\"{}\" title=\"{}\" source-units=\"{}-{}\">\n{}\n</ostudy-partition>",
            partition_index,
            escape_attribute(partition_title),
            unit_start,
            unit_end,
            chunks_text
        );
        let source_locator = SourceLocator::with_evidence(
            unit_kind,
            unit_numbers,
            Some(partition_title.to_string()),
            asset_ids,
            None,
        )
        .expect("persisted semantic chunks contain validated canonical source locators");

        windows.push(SemanticWindow {
            window_index: windows.len(),
            partition_index,
            partition_title: partition_title.to_string(),
            first_chunk_index: selected.first().map_or(0, |unit| unit.chunk.chunk_index),
            last_chunk_index: selected.last().map_or(0, |unit| unit.chunk.chunk_index),
            page_start: source_locator.first(),
            page_end: source_locator.last(),
            source_locator,
            token_estimate: token_total.min(max_tokens),
            source_text,
        });

        if end >= units.len() {
            break;
        }
        let mut next_start = end;
        let mut overlap = 0_usize;
        while next_start > start + 1 && overlap < overlap_tokens {
            next_start -= 1;
            overlap = overlap.saturating_add(units[next_start].token_estimate.max(1));
        }
        start = next_start.max(start + 1);
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

fn build_source_units<'a>(
    chunks: &'a [SemanticChunk],
    max_tokens: usize,
    max_units: usize,
) -> Vec<SourceUnit<'a>> {
    let mut units = Vec::new();
    for chunk in chunks {
        let text_token_estimate = chunk.full_text.chars().count().max(1).div_ceil(4);
        let source_token_estimate = chunk.token_estimate.max(text_token_estimate).max(1);
        let token_segments = source_token_estimate.div_ceil(max_tokens).max(1);
        let unit_segments = chunk
            .source_locator
            .unit_numbers
            .len()
            .div_ceil(max_units)
            .max(1);
        let segment_count = token_segments.max(unit_segments);
        let text_segments = split_text_into_segments(&chunk.full_text, segment_count);

        for segment_index in 0..segment_count {
            let text = text_segments
                .get(segment_index)
                .cloned()
                .unwrap_or_default();
            let unit_numbers = segment_source_units(
                &chunk.source_locator.unit_numbers,
                segment_index,
                segment_count,
            );
            let distributed_estimate = source_token_estimate.div_ceil(segment_count);
            let segment_estimate = text.chars().count().div_ceil(4).max(distributed_estimate);
            units.push(SourceUnit {
                chunk,
                segment_index,
                segment_count,
                unit_numbers,
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

fn segment_source_units(units: &[i64], segment_index: usize, segment_count: usize) -> Vec<i64> {
    if units.is_empty() {
        return Vec::new();
    }
    if segment_count <= 1 {
        return units.to_vec();
    }
    let start = segment_index.saturating_mul(units.len()) / segment_count;
    let end = (segment_index + 1)
        .saturating_mul(units.len())
        .div_ceil(segment_count)
        .min(units.len());
    if start < end {
        units[start..end].to_vec()
    } else {
        vec![units[start.min(units.len() - 1)]]
    }
}

fn format_unit_for_model(unit: &SourceUnit<'_>) -> String {
    let chunk = unit.chunk;
    let heading = chunk.heading_context.as_deref().unwrap_or("(no heading)");
    let source_units = unit
        .unit_numbers
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
        "<ostudy-chunk index=\"{}\" source-units=\"{}\"{} heading=\"{}\" element-types=\"{}\">\n{}\n</ostudy-chunk>",
        chunk.chunk_index,
        source_units,
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

    fn chunk(index: i64, units: &[i64], tokens: usize) -> SemanticChunk {
        SemanticChunk {
            chunk_index: index,
            text: format!("text {index}"),
            full_text: format!("Section {index}\ntext {index}"),
            source_locator: SourceLocator::new(
                SourceUnitKind::Page,
                units.to_vec(),
                Some(format!("Section {index}")),
            )
            .unwrap(),
            element_types: vec!["paragraph".to_string()],
            heading_context: Some(format!("Section {index}")),
            token_estimate: tokens,
        }
    }

    #[test]
    fn windows_cross_source_unit_boundaries_without_cutting_normal_chunks() {
        let chunks = vec![
            chunk(0, &[1, 2], 400),
            chunk(1, &[2, 3], 400),
            chunk(2, &[3, 4], 400),
        ];
        let windows = build_semantic_windows(&chunks, 2_000, 20, 0);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].source_locator.unit_numbers, vec![1, 2, 3, 4]);
        assert_eq!((windows[0].page_start, windows[0].page_end), (1, 4));
    }

    #[test]
    fn windows_honor_token_and_unit_caps_with_semantic_overlap() {
        let chunks = (0..8)
            .map(|index| chunk(index, &[index + 1], 500))
            .collect::<Vec<_>>();
        let windows = build_semantic_windows(&chunks, 1_500, 3, 500);
        assert!(windows.len() >= 3);
        assert!(windows.iter().all(|window| window.token_estimate <= 1_500));
        assert!(windows
            .iter()
            .all(|window| window.source_locator.unit_numbers.len() <= 3));
        assert_eq!(windows[0].last_chunk_index, windows[1].first_chunk_index);
    }

    #[test]
    fn oversized_chunks_are_split_into_hard_bounded_continuations() {
        let mut oversized = chunk(0, &(1..=12).collect::<Vec<_>>(), 12_000);
        oversized.full_text = (0..12_000)
            .map(|index| format!("concept-{index} "))
            .collect::<String>();
        let windows = build_semantic_windows(&[oversized, chunk(1, &[13], 100)], 1_000, 3, 100);
        assert!(windows.len() > 4);
        assert!(windows.iter().all(|window| window.token_estimate <= 1_000));
        assert!(windows
            .iter()
            .all(|window| window.source_locator.unit_numbers.len() <= 3));
        assert!(windows
            .iter()
            .any(|window| window.source_text.contains("segment=\"2/")));
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
    fn heading_changes_do_not_force_tiny_request_partitions() {
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
        let plan = DocumentScalePlan::for_unit_count(138);
        let partitions = build_semantic_partitions(&chunks, plan);
        assert_eq!(partitions.len(), 1);
        assert_eq!(partitions[0].title, "Source units 1–12");
    }

    #[test]
    fn partitioned_windows_have_unique_global_indices() {
        let chunks = (0..11)
            .map(|index| chunk(index, &[index + 1], 100))
            .collect::<Vec<_>>();
        let plan = DocumentScalePlan::for_unit_count(11);
        let windows = build_partitioned_semantic_windows(&chunks, plan, 10_000, 10, 0);
        assert_eq!(windows.len(), 2);
        assert_eq!(
            windows
                .iter()
                .map(|window| window.window_index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        validate_semantic_windows(&windows, &DocumentSourceDescriptor::pdf(11)).unwrap();
    }

    #[test]
    fn packed_windows_preserve_visual_asset_evidence() {
        let mut visual = chunk(1, &[2], 200);
        visual.source_locator = SourceLocator::with_evidence(
            SourceUnitKind::Page,
            vec![2],
            Some("Diagram".to_string()),
            vec!["unit-render:000002".to_string()],
            None,
        )
        .unwrap();
        let windows = build_semantic_windows(&[chunk(0, &[1], 200), visual], 1_000, 4, 0);
        assert_eq!(
            windows[0].source_locator.asset_ids,
            vec!["unit-render:000002"]
        );
    }

    #[test]
    fn non_pdf_source_units_remain_non_pdf_through_window_planning() {
        let chunk = SemanticChunk {
            chunk_index: 0,
            text: "diagram".to_string(),
            full_text: "A flow diagram connects preload to stroke volume.".to_string(),
            source_locator: SourceLocator::with_evidence(
                SourceUnitKind::Slide,
                vec![3],
                Some("Frank-Starling mechanism".to_string()),
                vec!["slide-render:000003".to_string()],
                None,
            )
            .unwrap(),
            element_types: vec!["image".to_string()],
            heading_context: Some("Frank-Starling mechanism".to_string()),
            token_estimate: 20,
        };
        let windows = build_semantic_windows(&[chunk], 1_000, 10, 0);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].source_locator.unit_kind, SourceUnitKind::Slide);
        assert_eq!(
            windows[0].source_locator.asset_ids,
            vec!["slide-render:000003"]
        );
        assert_eq!((windows[0].page_start, windows[0].page_end), (3, 3));
    }
}
