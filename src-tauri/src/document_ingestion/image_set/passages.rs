//! Verified passages keep stable semantic identities separately from their order.

use crate::{
    document_canonical::{sha256_hex, CanonicalSemanticChunk},
    document_domain::{SourceLocator, SourceUnitKind},
    documents::image_sets::receipt::ChunkIdentity,
    error::CommandError,
};

use super::regions::{Frame, Region};

struct Passage {
    region: Region,
    stable_region_id: String,
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
                if !previous.units.contains(&unit) { previous.units.push(unit); }
                if region.id < previous.stable_region_id { previous.stable_region_id = region.id.clone(); }
                return (index, Some(previous.region.id.clone()));
            }
        }
        let index = self.values.len();
        self.values.push(Passage {
            region: region.clone(), stable_region_id: region.id.clone(),
            frame_identity: frame.identity.clone(), units: vec![unit],
        });
        (index, None)
    }

    pub(super) fn finish(self) -> Result<(Vec<CanonicalSemanticChunk>, Vec<ChunkIdentity>), CommandError> {
        let mut chunks = Vec::new();
        let mut identities = Vec::new();
        for (index, passage) in self.values.into_iter().enumerate() {
            let locator = SourceLocator::with_evidence(SourceUnitKind::Image, passage.units, None, Vec::new(), None)?;
            let text = passage.region.text;
            let fingerprint = serde_json::to_vec(&(
                "image-passage-v1", &text,
                passage.region.signature.as_ref().unwrap_or(&passage.frame_identity),
            )).map_err(CommandError::serialization)?;
            identities.push(ChunkIdentity {
                stable_key: format!("region:{}", passage.stable_region_id),
                evidence_sha256: sha256_hex(&fingerprint),
                projection_sha256: String::new(),
            });
            chunks.push(CanonicalSemanticChunk::new(
                index as i64, &text, &text, locator, vec!["paragraph".to_string()], None,
                text.chars().count().div_ceil(4), false,
            ));
        }
        Ok((chunks, identities))
    }
}

pub(super) fn add_occurrence(chunk: &mut CanonicalSemanticChunk, unit: i64) -> Result<(), CommandError> {
    let mut units = chunk.source_locator.unit_numbers.clone();
    if !units.contains(&unit) { units.push(unit); }
    let locator = SourceLocator::with_evidence(
        SourceUnitKind::Image, units, None, chunk.source_locator.asset_ids.clone(), None,
    )?;
    *chunk = CanonicalSemanticChunk::new(
        chunk.chunk_index, &chunk.text, &chunk.full_text, locator, chunk.element_types.clone(),
        chunk.heading_context.clone(), chunk.token_estimate, chunk.is_oversized,
    );
    Ok(())
}
