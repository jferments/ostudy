//! Preserve extracted grids; plain text is a projection, not table authority.
use std::collections::BTreeMap;

use xberg::{types::document_structure::TableGrid, Table};

use crate::{
    document_canonical::structure::{SourceBounds, TableCell, TableStructure},
    error::CommandError,
};

pub(super) struct TableProjection {
    pub(super) text: String,
    pub(super) attributes: BTreeMap<String, String>,
}

pub(super) fn bounds(value: &xberg::types::extraction::BoundingBox) -> SourceBounds {
    SourceBounds {
        x0: value.x0,
        y0: value.y0,
        x1: value.x1,
        y1: value.y1,
    }
}

pub(super) fn structure_from_grid(grid: &TableGrid) -> Result<TableStructure, CommandError> {
    admit_dimensions(grid.rows as usize, grid.cols as usize, grid.cells.len())?;
    let mut characters = 0_usize;
    for cell in &grid.cells {
        characters = characters.saturating_add(cell.content.chars().count());
        if characters > 250_000 {
            return Err(invalid_table());
        }
    }
    let table = TableStructure {
        rows: grid.rows as usize,
        columns: grid.cols as usize,
        cells: grid
            .cells
            .iter()
            .map(|cell| TableCell {
                row: cell.row as usize,
                column: cell.col as usize,
                row_span: cell.row_span as usize,
                column_span: cell.col_span as usize,
                header: Some(cell.is_header),
                text: cell.content.clone(),
                bounds: cell.bbox.as_ref().map(bounds),
            })
            .collect(),
        column_headers: None,
    };
    table.validate().map_err(|_| invalid_table())?;
    Ok(table)
}

pub(super) fn structure_from_table(value: &Table) -> Result<TableStructure, CommandError> {
    let columns = value.cells.iter().map(Vec::len).max().unwrap_or(0);
    let columns = columns.max(value.columns.as_ref().map_or(0, Vec::len));
    let count = value.cells.iter().map(Vec::len).sum();
    admit_dimensions(value.cells.len(), columns, count)?;
    let mut characters = 0_usize;
    for text in value
        .cells
        .iter()
        .flatten()
        .chain(value.columns.iter().flatten())
    {
        characters = characters.saturating_add(text.chars().count());
        if characters > 250_000 {
            return Err(invalid_table());
        }
    }
    let cells = value
        .cells
        .iter()
        .enumerate()
        .flat_map(|(row, values)| {
            values
                .iter()
                .enumerate()
                .map(move |(column, text)| TableCell {
                    row,
                    column,
                    row_span: 1,
                    column_span: 1,
                    header: None,
                    text: text.clone(),
                    bounds: None,
                })
        })
        .collect();
    let table = TableStructure {
        rows: value.cells.len(),
        columns,
        cells,
        column_headers: value.columns.clone(),
    };
    table.validate().map_err(|_| invalid_table())?;
    Ok(table)
}

pub(super) fn project_grid(grid: &TableGrid) -> Result<TableProjection, CommandError> {
    project(&structure_from_grid(grid)?)
}

pub(super) fn project_table(table: &Table) -> Result<TableProjection, CommandError> {
    project(&structure_from_table(table)?)
}

fn project(table: &TableStructure) -> Result<TableProjection, CommandError> {
    let header_rows = table
        .cells
        .iter()
        .filter(|cell| cell.header == Some(true))
        .map(|cell| cell.row + cell.row_span)
        .max()
        .unwrap_or(0);
    Ok(TableProjection {
        text: table.text_projection()?,
        attributes: BTreeMap::from([
            ("rowCount".to_string(), table.rows.to_string()),
            ("columnCount".to_string(), table.columns.to_string()),
            ("headerRows".to_string(), header_rows.to_string()),
        ]),
    })
}

fn admit_dimensions(rows: usize, columns: usize, cells: usize) -> Result<(), CommandError> {
    if rows > 100_000
        || columns > 100_000
        || cells > 100_000
        || rows.saturating_mul(columns) > 100_000
    {
        return Err(invalid_table());
    }
    Ok(())
}

fn invalid_table() -> CommandError {
    CommandError::new(
        "document_table_too_large",
        "The extracted table is invalid or exceeds its bounded cell/text limits.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use xberg::types::document_structure::GridCell;

    #[test]
    fn table_grid_preserves_cells_and_header_metadata() {
        let grid = TableGrid {
            rows: 2,
            cols: 2,
            cells: vec![
                GridCell {
                    row: 0,
                    col: 0,
                    row_span: 1,
                    col_span: 1,
                    is_header: true,
                    content: "A".to_string(),
                    bbox: None,
                },
                GridCell {
                    row: 1,
                    col: 1,
                    row_span: 1,
                    col_span: 1,
                    is_header: false,
                    content: "B".to_string(),
                    bbox: None,
                },
            ],
        };
        let projected = project_grid(&grid).unwrap();
        assert_eq!(projected.text, "A\t\n\tB");
        assert_eq!(projected.attributes["headerRows"], "1");
        let structure = structure_from_grid(&grid).unwrap();
        assert_eq!(structure.cells[0].header, Some(true));
        assert_eq!(structure.cells[1].column, 1);
    }
}
