//! App-owned evidence supplied to card and guide authors. Provider adapters do
//! not own or reinterpret these source/knowledge relationships.

use crate::knowledge::authoring::KnowledgeCatalog;

use super::contracts::SourcePageImage;

#[derive(Clone, Copy)]
pub(crate) struct AuthoringEvidence<'a> {
    pub images: &'a [SourcePageImage],
    pub knowledge: &'a KnowledgeCatalog,
}
