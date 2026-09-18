//! Extractor-supplied structure, not model interpretations or executable layout.
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{invalid_canonical, validate_text, CanonicalBlockRole, CommandError};

const MAX_CELLS: usize = 100_000;
const MAX_TEXT: usize = 250_000;
const MAX_NODE: u32 = 1_000_000;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SourceBounds {
    // Coordinates are retained in the extractor's source space. They are NOT
    // normalized image-mask coordinates; rendering requires a known transform.
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl SourceBounds {
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        if [self.x0, self.y0, self.x1, self.y1]
            .iter()
            .any(|value| !value.is_finite())
            || self.x1 < self.x0
            || self.y1 < self.y0
        {
            return Err(invalid_canonical("Invalid extractor-space geometry."));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StructureLinkKind {
    Footnote,
    Citation,
    InternalLink,
    Caption,
    Label,
    Contents,
    CrossReference,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StructureLink {
    pub target_node: u32,
    pub kind: StructureLinkKind,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BlockStructure {
    pub version: u32,
    pub node: Option<u32>,
    pub bounds: Option<SourceBounds>,
    pub table: Option<TableStructure>,
    pub links: Vec<StructureLink>,
}

impl Default for BlockStructure {
    fn default() -> Self {
        Self {
            version: 1,
            node: None,
            bounds: None,
            table: None,
            links: Vec::new(),
        }
    }
}

impl BlockStructure {
    pub(crate) fn validate(&self, role: CanonicalBlockRole) -> Result<(), CommandError> {
        if self.version != 1 || self.node.is_some_and(|node| node >= MAX_NODE) {
            return Err(invalid_canonical("Unsupported canonical block structure."));
        }
        if let Some(bounds) = &self.bounds {
            bounds.validate()?;
        }
        let mut links = BTreeSet::new();
        if self.links.len() > 128 {
            return Err(invalid_canonical("Too many canonical structural links."));
        }
        for link in &self.links {
            if self.node.is_none()
                || link.target_node >= MAX_NODE
                || !links.insert((link.target_node, link.kind))
            {
                return Err(invalid_canonical("Invalid canonical structural link."));
            }
        }
        if let Some(table) = &self.table {
            if role != CanonicalBlockRole::Table {
                return Err(invalid_canonical("Table structure belongs to a table block."));
            }
            table.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TableCell {
    pub row: usize,
    pub column: usize,
    pub row_span: usize,
    pub column_span: usize,
    pub header: Option<bool>,
    pub text: String,
    pub bounds: Option<SourceBounds>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TableStructure {
    pub rows: usize,
    pub columns: usize,
    pub cells: Vec<TableCell>,
    // Some extractors retain headers supplied by another fragment. Do not
    // pretend they are a row physically present in this fragment.
    pub column_headers: Option<Vec<String>>,
}

impl TableStructure {
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        if self.rows > MAX_CELLS
            || self.columns > MAX_CELLS
            || self.rows.saturating_mul(self.columns) > MAX_CELLS
            || self.cells.len() > MAX_CELLS
        {
            return Err(invalid_canonical("The structured table exceeds its cell budget."));
        }
        let mut occupied = vec![false; self.rows * self.columns];
        let mut characters = 0_usize;
        for cell in &self.cells {
            let end_row = cell.row.checked_add(cell.row_span);
            let end_column = cell.column.checked_add(cell.column_span);
            if cell.row_span == 0
                || cell.column_span == 0
                || end_row.is_none_or(|end| end > self.rows)
                || end_column.is_none_or(|end| end > self.columns)
            {
                return Err(invalid_canonical("A table cell has an invalid span."));
            }
            validate_text(&cell.text, MAX_TEXT, true)?;
            characters = characters.saturating_add(cell.text.chars().count());
            if let Some(bounds) = &cell.bounds {
                bounds.validate()?;
            }
            for row in cell.row..end_row.unwrap_or(cell.row) {
                for column in cell.column..end_column.unwrap_or(cell.column) {
                    let slot = &mut occupied[row * self.columns + column];
                    if *slot {
                        return Err(invalid_canonical("Overlapping table cells are ambiguous."));
                    }
                    *slot = true;
                }
            }
        }
        if let Some(headers) = &self.column_headers {
            if headers.len() != self.columns {
                return Err(invalid_canonical("Table headers do not match its columns."));
            }
            for header in headers {
                validate_text(header, MAX_TEXT, true)?;
                characters = characters.saturating_add(header.chars().count());
            }
        }
        if characters > MAX_TEXT {
            return Err(invalid_canonical("The structured table exceeds its text budget."));
        }
        Ok(())
    }

    /// A deterministic search projection. Cell values and header roles remain
    /// authoritative in the structure; this projection does not infer spans.
    pub(crate) fn text_projection(&self) -> Result<String, CommandError> {
        self.validate()?;
        let mut matrix = vec![vec![""; self.columns]; self.rows];
        for cell in &self.cells {
            matrix[cell.row][cell.column] = cell.text.trim();
        }
        let text = matrix
            .iter()
            .map(|row| row.join("\t"))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();
        if text.chars().count() > MAX_TEXT {
            return Err(invalid_canonical("The table text projection exceeds its budget."));
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests;
