//! Bind native normalization selectors to exact extractor records once per import.
use std::collections::{BTreeMap, HashMap};

use xberg::{
    types::document_structure::{NodeContent, RelationshipKind},
    ExtractedDocument,
};

use crate::{
    document_canonical::structure::{BlockStructure, StructureLink, StructureLinkKind},
    error::CommandError,
};

use super::super::tables::{bounds, structure_from_grid, structure_from_table};

pub(super) const NODE: &str = "_ostudyNode";
pub(super) const TABLE: &str = "_ostudyTable";
pub(super) const PAGE: &str = "_ostudyPageIndex";

pub(crate) struct StructureCatalog<'a> {
    document: &'a ExtractedDocument,
    links: HashMap<u32, Vec<StructureLink>>,
}

impl<'a> StructureCatalog<'a> {
    pub(crate) fn new(document: &'a ExtractedDocument) -> Result<Self, CommandError> {
        let mut links = HashMap::<u32, Vec<StructureLink>>::new();
        if let Some(structure) = &document.document {
            if structure.nodes.len() > 1_000_000 || structure.relationships.len() > 1_000_000 {
                return Err(invalid());
            }
            for relation in &structure.relationships {
                if relation.source.0 as usize >= structure.nodes.len()
                    || relation.target.0 >= 1_000_000
                {
                    return Err(invalid());
                }
                let kind = match relation.kind {
                    RelationshipKind::FootnoteReference => StructureLinkKind::Footnote,
                    RelationshipKind::CitationReference => StructureLinkKind::Citation,
                    RelationshipKind::InternalLink => StructureLinkKind::InternalLink,
                    RelationshipKind::Caption => StructureLinkKind::Caption,
                    RelationshipKind::Label => StructureLinkKind::Label,
                    RelationshipKind::TocEntry => StructureLinkKind::Contents,
                    RelationshipKind::CrossReference => StructureLinkKind::CrossReference,
                };
                let entries = links.entry(relation.source.0).or_default();
                let link = StructureLink {
                    target_node: relation.target.0,
                    kind,
                };
                if !entries.contains(&link) {
                    if entries.len() >= 128 {
                        return Err(invalid());
                    }
                    entries.push(link);
                }
            }
        }
        Ok(Self { document, links })
    }

    pub(crate) fn resolve(
        &self,
        attributes: &BTreeMap<String, String>,
    ) -> Result<Option<BlockStructure>, CommandError> {
        if let Some(index) = selector(attributes, NODE)? {
            let node = self
                .document
                .document
                .as_ref()
                .and_then(|structure| structure.nodes.get(index))
                .ok_or_else(invalid)?;
            let table = match &node.content {
                NodeContent::Table { grid } => Some(structure_from_grid(grid)?),
                _ => None,
            };
            return Ok(Some(BlockStructure {
                node: Some(u32::try_from(index).map_err(|_| invalid())?),
                bounds: node.bbox.as_ref().map(bounds),
                table,
                links: self.links.get(&(index as u32)).cloned().unwrap_or_default(),
                ..BlockStructure::default()
            }));
        }
        if let Some(index) = selector(attributes, TABLE)? {
            let table = if let Some(page) = selector(attributes, PAGE)? {
                self.document
                    .pages
                    .as_ref()
                    .and_then(|pages| pages.get(page))
                    .and_then(|page| page.tables.get(index))
                    .map(AsRef::as_ref)
            } else {
                self.document.tables.get(index)
            }
            .ok_or_else(invalid)?;
            return Ok(Some(BlockStructure {
                bounds: table.bounding_box.as_ref().map(bounds),
                table: Some(structure_from_table(table)?),
                ..BlockStructure::default()
            }));
        }
        Ok(None)
    }
}

fn selector(
    attributes: &BTreeMap<String, String>,
    key: &str,
) -> Result<Option<usize>, CommandError> {
    attributes
        .get(key)
        .map(|value| value.parse::<usize>().map_err(|_| invalid()))
        .transpose()
}

pub(crate) fn literal_text(value: &str) -> Result<String, CommandError> {
    if value.chars().count() > 250_000
        || value
            .chars()
            .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\t'))
    {
        return Err(CommandError::new(
            "document_content_invalid",
            "The extracted content exceeds its bound or contains unsupported controls; it was not truncated.",
        ));
    }
    Ok(value.to_string())
}

fn invalid() -> CommandError {
    CommandError::new(
        "document_structure_invalid",
        "Invalid native document structure or provenance.",
    )
}

#[cfg(test)]
mod tests;
