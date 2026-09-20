//! Collection-wide equivalence is proved before projection. Upload adjacency is
//! not evidence: inspect every bounded pair in both scrolling directions.

use crate::error::CommandError;

use super::{overlap::Alignment, regions::Frame};

#[derive(Clone, Copy)]
pub(super) struct RegionTarget {
    pub member: usize,
    pub region: usize,
    pub whole_image: bool,
}

pub(super) struct FrameAlignment {
    pub targets: Vec<Option<RegionTarget>>,
    pub whole_image: Option<usize>,
    pub ambiguous: bool,
}

pub(super) fn align(
    frames: &[&Frame],
    mut check: impl FnMut() -> Result<(), CommandError>,
) -> Result<Vec<FrameAlignment>, CommandError> {
    let mut offsets = Vec::with_capacity(frames.len());
    let mut owners = Vec::new();
    let mut plans = Vec::with_capacity(frames.len());
    for (member, frame) in frames.iter().enumerate() {
        offsets.push(owners.len());
        owners.extend((0..frame.regions.len()).map(|region| (member, region)));
        plans.push(FrameAlignment {
            targets: vec![None; frame.regions.len()],
            whole_image: None,
            ambiguous: false,
        });
    }
    let mut parents = (0..owners.len()).collect::<Vec<_>>();
    for current in 0..frames.len() {
        for previous in 0..current {
            check()?;
            let forward = Alignment::between(Some(frames[previous]), frames[current]);
            let backward = Alignment::between(Some(frames[current]), frames[previous]);
            let proved = forward.whole_image
                || forward.targets.iter().any(Option::is_some)
                || backward.targets.iter().any(Option::is_some);
            if !proved && (forward.ambiguous || backward.ambiguous) {
                plans[current].ambiguous = true;
            }
            if forward.whole_image && plans[current].whole_image.is_none() {
                plans[current].whole_image = Some(previous);
            }
            for (region, target) in forward.targets.iter().enumerate() {
                if let Some(target) = target {
                    join(
                        &mut parents,
                        offsets[current] + region,
                        offsets[previous] + target,
                    );
                }
            }
            for (region, target) in backward.targets.iter().enumerate() {
                if let Some(target) = target {
                    join(
                        &mut parents,
                        offsets[previous] + region,
                        offsets[current] + target,
                    );
                }
            }
        }
    }
    for (node, &(member, region)) in owners.iter().enumerate() {
        let representative = root(&mut parents, node);
        if representative == node {
            continue;
        }
        let (previous, target) = owners[representative];
        let a = &frames[previous].regions[target];
        let b = &frames[member].regions[region];
        let whole_image = frames[previous].identity == frames[member].identity;
        // Recheck directly against the representative. A chain of overlaps alone
        // must not launder changed text or pixel evidence into an equivalence.
        if previous < member
            && a.text == b.text
            && ((a.signature.is_some() && a.signature == b.signature) || whole_image)
        {
            plans[member].targets[region] = Some(RegionTarget {
                member: previous,
                region: target,
                whole_image,
            });
        } else {
            plans[member].ambiguous = true;
        }
    }
    Ok(plans)
}

fn root(parents: &mut [usize], mut node: usize) -> usize {
    while parents[node] != node {
        parents[node] = parents[parents[node]];
        node = parents[node];
    }
    node
}

fn join(parents: &mut [usize], a: usize, b: usize) {
    let a = root(parents, a);
    let b = root(parents, b);
    // The earliest retained occurrence is available when projection reaches a
    // later member. This does not impose an educational reading order.
    parents[a.max(b)] = a.min(b);
}
