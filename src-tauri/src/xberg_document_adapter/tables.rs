use std::collections::BTreeMap;

use crate::{document_ingestion::support::normalize_semantic_text, error::CommandError};
use xberg::types::document_structure::TableGrid;
use xberg::Table;

const MAX_TABLE_CELLS: usize = 100_000;
const MAX_TABLE_TEXT_CHARS: usize = 250_000;
const MAX_ATTRIBUTE_VALUE_CHARS: usize = 2_000;

pub(crate) struct TableProjection {
    pub(crate) text: String,
    pub(crate) attributes: BTreeMap<String, String>,
}

pub(crate) fn project_grid(grid: &TableGrid) -> Result<TableProjection, CommandError> {
    let rows = usize::try_from(grid.rows).unwrap_or(usize::MAX);
    let cols = usize::try_from(grid.cols).unwrap_or(usize::MAX);
    validate_dimensions(rows, cols)?;
    let mut matrix = vec![vec![String::new(); cols]; rows];
    let mut header_rows = 0_usize;
    for cell in &grid.cells {
        let row = usize::try_from(cell.row).unwrap_or(usize::MAX);
        let col = usize::try_from(cell.col).unwrap_or(usize::MAX);
        if row >= rows || col >= cols {
            return Err(invalid_table(
                "A structured table cell is outside its declared grid.",
            ));
        }
        matrix[row][col] = normalize_semantic_text(&cell.content, MAX_TABLE_TEXT_CHARS);
        if cell.is_header {
            header_rows = header_rows.max(row.saturating_add(1));
        }
    }
    Ok(projection(matrix, header_rows, None))
}

pub(crate) fn project_table(table: &Table) -> Result<TableProjection, CommandError> {
    let rows = table.cells.len();
    let cols = table.cells.iter().map(Vec::len).max().unwrap_or(0);
    validate_dimensions(rows, cols)?;
    let mut matrix = Vec::with_capacity(rows);
    for row in &table.cells {
        let mut normalized = row
            .iter()
            .map(|cell| normalize_semantic_text(cell, MAX_TABLE_TEXT_CHARS))
            .collect::<Vec<_>>();
        normalized.resize(cols, String::new());
        matrix.push(normalized);
    }
    Ok(projection(
        matrix,
        usize::from(table.columns.is_some()),
        table.table_id.as_deref(),
    ))
}

fn projection(
    matrix: Vec<Vec<String>>,
    header_rows: usize,
    table_id: Option<&str>,
) -> TableProjection {
    let row_count = matrix.len();
    let column_count = matrix.first().map(Vec::len).unwrap_or(0);
    let text = normalize_semantic_text(
        &matrix
            .iter()
            .map(|row| row.join("\t"))
            .collect::<Vec<_>>()
            .join("\n"),
        MAX_TABLE_TEXT_CHARS,
    );
    let mut attributes = BTreeMap::new();
    attributes.insert("rowCount".to_string(), row_count.to_string());
    attributes.insert("columnCount".to_string(), column_count.to_string());
    attributes.insert("headerRows".to_string(), header_rows.to_string());
    if let Some(table_id) = table_id {
        let table_id = normalize_semantic_text(table_id, MAX_ATTRIBUTE_VALUE_CHARS);
        if !table_id.is_empty() {
            attributes.insert("tableId".to_string(), table_id);
        }
    }
    TableProjection { text, attributes }
}

fn validate_dimensions(rows: usize, cols: usize) -> Result<(), CommandError> {
    if rows == 0 || cols == 0 {
        return Ok(());
    }
    if rows.saturating_mul(cols) > MAX_TABLE_CELLS {
        return Err(invalid_table(
            "A document table exceeds oStudy's 100,000-cell canonical limit.",
        ));
    }
    Ok(())
}

fn invalid_table(message: &str) -> CommandError {
    CommandError::new("document_table_too_large", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use xberg::types::document_structure::GridCell;

    #[test]
    fn structured_grid_is_projected_in_row_order() {
        let grid = TableGrid {
            rows: 2,
            cols: 2,
            cells: vec![
                GridCell {
                    content: "A".to_string(),
                    row: 0,
                    col: 0,
                    row_span: 1,
                    col_span: 1,
                    is_header: true,
                    bbox: None,
                },
                GridCell {
                    content: "B".to_string(),
                    row: 1,
                    col: 1,
                    row_span: 1,
                    col_span: 1,
                    is_header: false,
                    bbox: None,
                },
            ],
        };
        let projected = project_grid(&grid).unwrap();
        assert_eq!(projected.text, "A\t\n\tB");
        assert_eq!(
            projected.attributes.get("headerRows").map(String::as_str),
            Some("1")
        );
    }
}
