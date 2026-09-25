//! Request-only structural dependency projection. Whole literal blocks are the
//! smallest selectable object; tables, captions, qualifiers and links stay whole.
//! Unknown locations or relationships fall back to the previous source-wide view.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::Value;

use crate::knowledge::{authoring::KnowledgeCatalog, evidence::EvidenceSelector};

#[derive(Default)]
struct SourceScope<'a> {
    hashes: BTreeSet<&'a str>,
    spans: Vec<(usize, usize)>,
    whole: bool,
}

fn scopes(catalog: &KnowledgeCatalog) -> BTreeMap<&str, SourceScope<'_>> {
    let mut scopes: BTreeMap<&str, SourceScope<'_>> = BTreeMap::new();
    for unit in &catalog.units {
        let primary = scopes.entry(unit.source_id.as_str()).or_default();
        primary.hashes.insert(&unit.source_hash);
        let Some(grounding) = &unit.content.grounding else {
            primary.whole = true;
            continue;
        };
        if !grounding
            .evidence
            .iter()
            .any(|reference| reference.source_id == unit.source_id)
        {
            primary.whole = true;
        }
        for reference in &grounding.evidence {
            let scope = scopes.entry(reference.source_id.as_str()).or_default();
            scope.hashes.insert(&reference.source_hash);
            match &reference.selector {
                EvidenceSelector::Text {
                    start_byte,
                    end_byte,
                } if start_byte < end_byte && end_byte - start_byte == reference.excerpt.len() => {
                    scope.spans.push((*start_byte, *end_byte));
                }
                _ => scope.whole = true,
            }
        }
    }
    scopes
}

fn relevant(value: &Value, scope: &SourceScope<'_>) -> bool {
    if scope.whole || scope.hashes.len() != 1 || scope.spans.is_empty() {
        return true;
    }
    let range = &value["sourceRange"];
    let bounds = range["startByte"].as_u64().zip(range["endByte"].as_u64());
    let Some((start, end)) = bounds else {
        return true;
    };
    if range["version"].as_u64() != Some(1)
        || start >= end
        || !range["sourceHash"]
            .as_str()
            .is_some_and(|hash| scope.hashes.contains(hash))
    {
        return true;
    }
    scope
        .spans
        .iter()
        .any(|&(left, right)| start < right as u64 && (left as u64) < end)
}

/// `candidates` is already restricted to admitted primary/supporting sources.
/// The graph never expands retrieval permission or invents a missing dependency.
/// Bidirectional closure conservatively includes a caption/footnote pointing to
/// a selected table, as well as a table pointing to its supporting context.
pub(super) fn select<'a>(catalog: &KnowledgeCatalog, candidates: Vec<&'a Value>) -> Vec<&'a Value> {
    let scopes = scopes(catalog);
    let mut occurrences: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, value) in candidates.iter().enumerate() {
        if let Some(id) = value["occurrenceId"].as_str() {
            occurrences.entry(id).or_default().push(index);
        }
    }
    let mut edges = vec![Vec::new(); candidates.len()];
    let mut ambiguous_sources = BTreeSet::new();
    for (index, value) in candidates.iter().enumerate() {
        let source = value["sourceId"].as_str().unwrap_or_default();
        let Some(links) = value["links"].as_array() else {
            ambiguous_sources.insert(source);
            continue;
        };
        if value["occurrenceId"].as_str().is_none() {
            ambiguous_sources.insert(source);
        }
        for link in links {
            let targets = link["targetOccurrenceId"]
                .as_str()
                .and_then(|id| occurrences.get(id));
            match targets {
                Some(targets) => {
                    for &target in targets {
                        edges[index].push(target);
                        edges[target].push(index);
                    }
                }
                // A node-only/unknown/external link cannot justify discarding
                // any context from this source. Keep the previous broad view.
                None => {
                    ambiguous_sources.insert(source);
                }
            }
        }
    }
    let mut selected = vec![false; candidates.len()];
    let mut pending = VecDeque::new();
    for (index, value) in candidates.iter().enumerate() {
        let source = value["sourceId"].as_str().unwrap_or_default();
        if ambiguous_sources.contains(source)
            || scopes
                .get(source)
                .is_none_or(|scope| relevant(value, scope))
        {
            selected[index] = true;
            pending.push_back(index);
        }
    }
    while let Some(index) = pending.pop_front() {
        for &dependency in &edges[index] {
            if !selected[dependency] {
                selected[dependency] = true;
                pending.push_back(dependency);
            }
        }
    }
    candidates
        .into_iter()
        .zip(selected)
        .filter_map(|(value, selected)| selected.then_some(value))
        .collect()
}

#[cfg(test)]
mod tests;
