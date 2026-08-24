use std::collections::BTreeMap;

use crate::document_canonical::CanonicalBlockRole;

#[derive(Clone, Debug)]
pub(crate) struct NormalizedBlock {
    pub(crate) role: CanonicalBlockRole,
    pub(crate) text: String,
    pub(crate) heading_level: Option<u8>,
    pub(crate) parent_heading: Option<String>,
    pub(crate) heading_path: Vec<String>,
    pub(crate) asset_ids: Vec<String>,
    pub(crate) attributes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct UnitDraft {
    pub(crate) label: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) blocks: Vec<NormalizedBlock>,
    pub(crate) text_chars: usize,
}

impl UnitDraft {
    pub(crate) fn push(&mut self, block: NormalizedBlock) {
        if self.title.is_none()
            && matches!(
                block.role,
                CanonicalBlockRole::Title | CanonicalBlockRole::Heading
            )
        {
            self.title = (!block.text.is_empty()).then(|| block.text.clone());
        }
        self.text_chars = self.text_chars.saturating_add(block.text.chars().count());
        self.blocks.push(block);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}
