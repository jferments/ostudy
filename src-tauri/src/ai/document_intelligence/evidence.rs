//! App-owned evidence supplied to card and guide authors. Provider adapters do
//! not own or reinterpret these source/knowledge relationships. Page images go
//! out once, to knowledge extraction; authors write from what it extracted.

use crate::knowledge::authoring::KnowledgeCatalog;

#[derive(Clone, Copy)]
pub(crate) struct AuthoringEvidence<'a> {
    pub knowledge: &'a KnowledgeCatalog,
}
