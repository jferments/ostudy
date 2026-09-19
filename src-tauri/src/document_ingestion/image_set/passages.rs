//! Unique text projections with independently retained screenshot occurrences.
//! Every new occurrence is checked against the representative, not just a peer.

use crate::{
    document_canonical::CanonicalSemanticChunk,
    document_domain::{SourceLocator, SourceUnitKind},
    error::CommandError,
};

use super::regions::{Frame, Region};

struct Passage {
    region: Region,
    frame_identity: String,
    units: Vec<i64>,
}

#[derive(Default)]
pub(super) struct Passages {
    values: Vec<Passage>,
}

impl Passages {
    pub(super) fn admit(
        &mut self,
        frame: &Frame,
        region: &Region,
        unit: i64,
        target: Option<usize>,
        whole_image: bool,
    ) -> (usize, Option<String>) {
        if let Some((index, previous)) =
            target.and_then(|index| self.values.get_mut(index).map(|passage| (index, passage)))
        {
            let exact = region.text == previous.region.text
                && ((region.signature.is_some() && region.signature == previous.region.signature)
                    || (whole_image && frame.identity == previous.frame_identity));
            if exact {
                if !previous.units.contains(&unit) {
                    previous.units.push(unit);
                }
                return (index, Some(previous.region.id.clone()));
            }
        }
        let index = self.values.len();
        self.values.push(Passage {
            region: region.clone(),
            frame_identity: frame.identity.clone(),
            units: vec![unit],
        });
        (index, None)
    }

    pub(super) fn finish(self) -> Result<Vec<CanonicalSemanticChunk>, CommandError> {
        self.values
            .into_iter()
            .enumerate()
            .map(|(index, passage)| {
                let locator = SourceLocator::with_evidence(
                    SourceUnitKind::Image,
                    passage.units,
                    None,
                    Vec::new(),
                    None,
                )?;
                let text = passage.region.text;
                Ok(CanonicalSemanticChunk::new(
                    index as i64,
                    &text,
                    &text,
                    locator,
                    vec!["paragraph".to_string()],
                    None,
                    text.chars().count().div_ceil(4),
                    false,
                ))
            })
            .collect()
    }
}

pub(super) fn add_occurrence(
    chunk: &mut CanonicalSemanticChunk,
    unit: i64,
) -> Result<(), CommandError> {
    let mut units = chunk.source_locator.unit_numbers.clone();
    if !units.contains(&unit) {
        units.push(unit);
    }
    let locator = SourceLocator::with_evidence(
        SourceUnitKind::Image,
        units,
        None,
        chunk.source_locator.asset_ids.clone(),
        None,
    )?;
    *chunk = CanonicalSemanticChunk::new(
        chunk.chunk_index,
        &chunk.text,
        &chunk.full_text,
        locator,
        chunk.element_types.clone(),
        chunk.heading_context.clone(),
        chunk.token_estimate,
        chunk.is_oversized,
    );
    Ok(())
}
