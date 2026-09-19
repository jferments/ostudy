//! Conservative native overlap proof. A match is a whole-image identity or a
//! forward-scrolling suffix/prefix with multiple unique, pixel-exact anchors.

use std::collections::BTreeMap;

use super::regions::{Frame, Region};

pub(super) const CONTRACT: &str = "image-overlap-exact-v1";

pub(super) struct Alignment {
    pub targets: Vec<Option<usize>>,
    pub whole_image: bool,
    pub ambiguous: bool,
}

impl Alignment {
    pub(super) fn between(previous: Option<&Frame>, current: &Frame) -> Self {
        let mut result = Self {
            targets: vec![None; current.regions.len()],
            whole_image: false,
            ambiguous: false,
        };
        let Some(previous) = previous else {
            return result;
        };
        if previous.identity == current.identity
            && previous.available == current.available
            && previous.unresolved == current.unresolved
            && previous.regions.len() == current.regions.len()
            && previous
                .regions
                .iter()
                .zip(&current.regions)
                .all(|(a, b)| {
                    a.text == b.text && a.bounds == b.bounds && a.signature == b.signature
                })
        {
            result.targets = (0..current.regions.len()).map(Some).collect();
            result.whole_image = true;
            return result;
        }
        let old_counts = counts(&previous.regions);
        let new_counts = counts(&current.regions);
        result.ambiguous = old_counts.keys().any(|text| new_counts.contains_key(text));
        if previous.unresolved > 0 || current.unresolved > 0 {
            return result;
        }
        let Some(first) = current.regions.first() else {
            return result;
        };
        if old_counts.get(first.text.as_str()) != Some(&1) {
            return result;
        }
        let Some(start) = previous
            .regions
            .iter()
            .position(|region| region.text == first.text)
        else {
            return result;
        };
        let tail = &previous.regions[start..];
        let Some(prefix) = current.regions.get(..tail.len()) else {
            return result;
        };
        if tail.len() < 2
            || tail.iter().map(|region| region.text.chars().count()).sum::<usize>() < 48
            || !reading_order(tail)
            || !reading_order(prefix)
        {
            return result;
        }
        let mut displacement = None;
        for (old, new) in tail.iter().zip(prefix) {
            if old.text != new.text
                || old.signature.is_none()
                || old.signature != new.signature
                || old_counts.get(old.text.as_str()) != Some(&1)
                || new_counts.get(new.text.as_str()) != Some(&1)
            {
                return result;
            }
            let Some((a, b)) = old.bounds.zip(new.bounds) else {
                return result;
            };
            let delta = i64::from(a.top) - i64::from(b.top);
            if delta <= 0 || displacement.is_some_and(|previous| previous != delta) {
                return result;
            }
            displacement = Some(delta);
        }
        for (offset, target) in result.targets.iter_mut().take(tail.len()).enumerate() {
            *target = Some(start + offset);
        }
        result.ambiguous = false;
        result
    }
}

fn counts(regions: &[Region]) -> BTreeMap<&str, usize> {
    let mut result = BTreeMap::new();
    for region in regions {
        *result.entry(region.text.as_str()).or_default() += 1;
    }
    result
}

fn reading_order(regions: &[Region]) -> bool {
    regions.iter().all(|region| region.ordered)
        && regions.windows(2).all(|pair| {
            pair[0]
                .bounds
                .zip(pair[1].bounds)
                .is_some_and(|(a, b)| a.bottom <= b.top)
        })
}
