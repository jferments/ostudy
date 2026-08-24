pub(crate) mod assets;
pub(crate) mod chunking;
pub(crate) mod model;
pub(crate) mod segmentation;

pub(crate) use assets::{ManagedAssetInput, ManagedAssetStore};
pub(crate) use chunking::build_semantic_chunks;
pub(crate) use model::{NormalizedBlock, UnitDraft};
pub(crate) use segmentation::{segment_units, unit_text_projection};
