//! Tables rebuilt from their drawn cell borders.
//!
//! The layout pass reads a ruled table as paragraphs that run across its
//! columns, so a row's values arrive scrambled. When a page draws a grid, the
//! grid's lines give the cells and each glyph belongs to the cell it sits in.

use std::io::{Read, Seek};

use oxidize_pdf::{
    graphics::extraction::{ExtractedGraphics, GraphicsExtractor, LineOrientation},
    parser::PdfDocument,
    pipeline::{Element, ElementBBox, ElementData, ElementMetadata, TableElementData},
    text::{ExtractionOptions, TextExtractor, TextFragment},
};

// Lines this close are one rule; drawn cells often repeat a shared edge.
const LINE_MERGE_TOLERANCE: f64 = 1.5;
// A row or column narrower than this is the gap of a doubled border.
const MIN_TRACK_SIZE: f64 = 4.0;
const MIN_TABLE_ROWS: usize = 2;
const MIN_TABLE_COLUMNS: usize = 2;
// Most cells of a real table hold text; a frame around prose does not.
const MIN_FILLED_CELL_SHARE: f64 = 0.5;
// Glyph gaps wider than this share of the font size are word breaks.
const WORD_GAP_SHARE: f64 = 0.2;
// Boxes around a diagram's labels also form small grids.
const MIN_TABLE_CELLS: usize = 8;
// A table's cells read as runs of the text it replaces. Rotated labels or a
// diagram's boxes come out as word pairs the page never had.
const MIN_SHARED_WORD_PAIRS: f64 = 0.6;

pub(crate) struct RuledTable {
    pub bbox: ElementBBox,
    pub rows: Vec<Vec<String>>,
}

#[derive(Default)]
pub(crate) struct RuledPage {
    pub tables: Vec<RuledTable>,
}

/// Replaces the elements a ruled table's cells cover with one table element
/// per table. Pages without a drawn grid are left as they are.
pub(crate) fn rebuild_ruled_tables<R: Read + Seek>(
    document: &PdfDocument<R>,
    page_count: usize,
    elements: Vec<Element>,
) -> Vec<Element> {
    let mut graphics = GraphicsExtractor::default();
    let mut text = TextExtractor::with_options(ExtractionOptions {
        preserve_layout: true,
        reconstruct_paragraphs: false,
        space_threshold: 0.3,
        detect_columns: false,
        ..ExtractionOptions::default()
    });
    let mut elements = elements;
    let mut rebuilt = 0_usize;
    for page in 0..page_count {
        let Ok(page_graphics) = graphics.extract_from_page(document, page) else {
            continue;
        };
        if !page_graphics.has_table_structure() {
            continue;
        }
        let Ok(page_index) = u32::try_from(page) else {
            continue;
        };
        let Ok(page_text) = text.extract_from_page(document, page_index) else {
            continue;
        };
        let ruled = ruled_tables(&page_graphics, &page_text.fragments);
        let (page_elements, page_rebuilt) =
            replace_on_page(elements, page_index, ruled, &page_text.fragments);
        elements = page_elements;
        rebuilt += page_rebuilt;
    }
    if rebuilt > 0 {
        crate::logging::info("pdf_ruled_tables_rebuilt", format!("tables={rebuilt}"));
    }
    elements
}

/// Swaps each accepted table in for the elements it covers: those centred in
/// it, and a table the layout pass read across it. Text that only such a
/// removed table held, such as a caption between two tables, comes back as
/// paragraphs read from the page's glyphs.
fn replace_on_page(
    elements: Vec<Element>,
    page: u32,
    ruled: RuledPage,
    glyphs: &[TextFragment],
) -> (Vec<Element>, usize) {
    let mut removed = vec![false; elements.len()];
    let mut removed_tables = Vec::new();
    let mut inserts = Vec::<(f64, Element)>::new();
    let mut rebuilt = 0;
    for table in ruled.tables {
        let covers = |element: &Element| {
            element.page() == page
                && !matches!(element, Element::Image(_))
                && (contains_center(&table.bbox, element.bbox())
                    || (matches!(element, Element::Table(_))
                        && overlap_area(&table.bbox, element.bbox()) * 2.0
                            >= table.bbox.width * table.bbox.height))
        };
        let covered = (0..elements.len())
            .filter(|&index| covers(&elements[index]))
            .collect::<Vec<_>>();
        let Some(&first) = covered.first() else {
            continue;
        };
        let replaced = covered
            .iter()
            .map(|&index| elements[index].display_text())
            .collect::<Vec<_>>();
        if shared_word_pair_share(&table.rows, &replaced) < MIN_SHARED_WORD_PAIRS {
            continue;
        }
        for &index in &covered {
            if !removed[index] && matches!(elements[index], Element::Table(_)) {
                removed_tables.push(*elements[index].bbox());
            }
            removed[index] = true;
        }
        let metadata = ElementMetadata {
            page,
            bbox: table.bbox,
            confidence: 1.0,
            font_name: None,
            font_size: None,
            is_bold: false,
            is_italic: false,
            ..elements[first].metadata().clone()
        };
        inserts.push((
            top(&table.bbox),
            Element::Table(TableElementData::new(table.rows, metadata)),
        ));
        rebuilt += 1;
    }
    // Glyphs already in a rebuilt table or a kept element are not leftovers.
    let claimed = inserts
        .iter()
        .map(|(_, element)| *element.bbox())
        .chain(
            elements
                .iter()
                .zip(&removed)
                .filter(|(element, removed)| !**removed && element.page() == page)
                .map(|(element, _)| *element.bbox()),
        )
        .collect::<Vec<_>>();
    for (bbox, text) in leftover_paragraphs(glyphs, &removed_tables, &claimed) {
        let metadata = ElementMetadata {
            page,
            bbox,
            confidence: 1.0,
            ..ElementMetadata::default()
        };
        inserts.push((
            top(&bbox),
            Element::Paragraph(ElementData { text, metadata }),
        ));
    }
    let Some(first) = removed.iter().position(|&is_removed| is_removed) else {
        return (elements, 0);
    };
    // Top of the page first, the order the reader meets them.
    inserts.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut inserts = Some(inserts.into_iter().map(|(_, element)| element));
    let mut result = Vec::with_capacity(elements.len());
    for (index, element) in elements.into_iter().enumerate() {
        if index == first {
            result.extend(inserts.take().into_iter().flatten());
        }
        if !removed[index] {
            result.push(element);
        }
    }
    (result, rebuilt)
}

/// Glyphs inside a removed layout table that nothing else claims, grouped
/// into paragraphs at blank-line gaps.
fn leftover_paragraphs(
    glyphs: &[TextFragment],
    removed: &[ElementBBox],
    claimed: &[ElementBBox],
) -> Vec<(ElementBBox, String)> {
    let mut left = glyphs
        .iter()
        .filter(|glyph| {
            let bbox = ElementBBox::new(glyph.x, glyph.y, glyph.width, glyph.height);
            removed.iter().any(|outer| contains_center(outer, &bbox))
                && !claimed.iter().any(|outer| contains_center(outer, &bbox))
        })
        .collect::<Vec<_>>();
    left.sort_by(|a, b| b.y.total_cmp(&a.y).then(a.x.total_cmp(&b.x)));
    let mut paragraphs: Vec<Vec<&TextFragment>> = Vec::new();
    for glyph in left {
        match paragraphs.last_mut() {
            Some(paragraph)
                if paragraph
                    .last()
                    .is_some_and(|last| last.y - glyph.y < last.height.max(glyph.height) * 1.8) =>
            {
                paragraph.push(glyph)
            }
            _ => paragraphs.push(vec![glyph]),
        }
    }
    paragraphs
        .into_iter()
        .filter_map(|paragraph| {
            let left = paragraph
                .iter()
                .map(|glyph| glyph.x)
                .fold(f64::INFINITY, f64::min);
            let right = paragraph
                .iter()
                .map(|glyph| glyph.x + glyph.width)
                .fold(f64::NEG_INFINITY, f64::max);
            let bottom = paragraph
                .iter()
                .map(|glyph| glyph.y)
                .fold(f64::INFINITY, f64::min);
            let top = paragraph
                .iter()
                .map(|glyph| glyph.y + glyph.height)
                .fold(f64::NEG_INFINITY, f64::max);
            let text = cell_text(paragraph);
            (!text.is_empty()).then(|| {
                (
                    ElementBBox::new(left, bottom, right - left, top - bottom),
                    text,
                )
            })
        })
        .collect()
}

fn top(bbox: &ElementBBox) -> f64 {
    bbox.y + bbox.height
}

fn overlap_area(a: &ElementBBox, b: &ElementBBox) -> f64 {
    let width = (a.x + a.width).min(b.x + b.width) - a.x.max(b.x);
    let height = (a.y + a.height).min(b.y + b.height) - a.y.max(b.y);
    width.max(0.0) * height.max(0.0)
}

/// The share of adjacent word pairs inside the table's cells that also occur
/// in the text it would replace.
fn shared_word_pair_share(rows: &[Vec<String>], replaced: &[String]) -> f64 {
    fn pairs(text: &str) -> Vec<(String, String)> {
        let words = text
            .split(|character: char| !character.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(str::to_lowercase)
            .collect::<Vec<_>>();
        words
            .windows(2)
            .map(|pair| (pair[0].clone(), pair[1].clone()))
            .collect()
    }
    let known = replaced
        .iter()
        .flat_map(|text| pairs(text))
        .collect::<std::collections::HashSet<_>>();
    let table_pairs = rows
        .iter()
        .flatten()
        .flat_map(|cell| pairs(cell))
        .collect::<std::collections::HashSet<_>>();
    if table_pairs.is_empty() {
        return 0.0;
    }
    table_pairs
        .iter()
        .filter(|pair| known.contains(*pair))
        .count() as f64
        / table_pairs.len() as f64
}

fn contains_center(outer: &ElementBBox, inner: &ElementBBox) -> bool {
    let x = inner.x + inner.width / 2.0;
    let y = inner.y + inner.height / 2.0;
    x >= outer.x && x <= outer.x + outer.width && y >= outer.y && y <= outer.y + outer.height
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rule {
    /// The fixed coordinate: y for a horizontal rule, x for a vertical one.
    at: f64,
    start: f64,
    end: f64,
}

impl Rule {
    fn covers(&self, position: f64) -> bool {
        position >= self.start - LINE_MERGE_TOLERANCE && position <= self.end + LINE_MERGE_TOLERANCE
    }
}

pub(crate) fn ruled_tables(graphics: &ExtractedGraphics, glyphs: &[TextFragment]) -> RuledPage {
    let mut horizontal = Vec::new();
    let mut vertical = Vec::new();
    for line in &graphics.lines {
        match line.orientation {
            LineOrientation::Horizontal => horizontal.push(Rule {
                at: (line.y1 + line.y2) / 2.0,
                start: line.x1.min(line.x2),
                end: line.x1.max(line.x2),
            }),
            LineOrientation::Vertical => vertical.push(Rule {
                at: (line.x1 + line.x2) / 2.0,
                start: line.y1.min(line.y2),
                end: line.y1.max(line.y2),
            }),
            _ => {}
        }
    }
    let horizontal = merge_rules(horizontal);
    let vertical = merge_rules(vertical);
    let mut page = RuledPage::default();
    for (rows, columns) in grid_components(&horizontal, &vertical) {
        build_tables(&rows, &columns, glyphs, &mut page);
    }
    page
}

/// Joins collinear pieces of one rule into a single span.
fn merge_rules(mut rules: Vec<Rule>) -> Vec<Rule> {
    rules.retain(|rule| {
        [rule.at, rule.start, rule.end]
            .iter()
            .all(|value| value.is_finite())
    });
    rules.sort_by(|a, b| a.at.total_cmp(&b.at).then(a.start.total_cmp(&b.start)));
    let mut merged: Vec<Rule> = Vec::new();
    for rule in rules {
        if let Some(last) = merged.iter_mut().rev().find(|last| {
            (last.at - rule.at).abs() <= LINE_MERGE_TOLERANCE
                && rule.start <= last.end + LINE_MERGE_TOLERANCE
        }) {
            last.end = last.end.max(rule.end);
            continue;
        }
        merged.push(rule);
    }
    merged
}

/// Groups rules that touch into separate grids, each as (horizontal, vertical).
fn grid_components(horizontal: &[Rule], vertical: &[Rule]) -> Vec<(Vec<Rule>, Vec<Rule>)> {
    let count = horizontal.len() + vertical.len();
    let mut parent = (0..count).collect::<Vec<_>>();
    fn root(parent: &mut [usize], mut node: usize) -> usize {
        while parent[node] != node {
            parent[node] = parent[parent[node]];
            node = parent[node];
        }
        node
    }
    for (h_index, h) in horizontal.iter().enumerate() {
        for (v_index, v) in vertical.iter().enumerate() {
            if h.covers(v.at) && v.covers(h.at) {
                let a = root(&mut parent, h_index);
                let b = root(&mut parent, horizontal.len() + v_index);
                parent[a] = b;
            }
        }
    }
    let mut groups = std::collections::BTreeMap::<usize, (Vec<Rule>, Vec<Rule>)>::new();
    for (index, rule) in horizontal.iter().enumerate() {
        groups
            .entry(root(&mut parent, index))
            .or_default()
            .0
            .push(*rule);
    }
    for (index, rule) in vertical.iter().enumerate() {
        let key = root(&mut parent, horizontal.len() + index);
        groups.entry(key).or_default().1.push(*rule);
    }
    groups.into_values().collect()
}

/// Distinct track edges, merging positions closer than a doubled border.
fn edges(rules: &[Rule]) -> Vec<f64> {
    let mut positions = rules.iter().map(|rule| rule.at).collect::<Vec<_>>();
    positions.sort_by(f64::total_cmp);
    let mut edges: Vec<f64> = Vec::new();
    for position in positions {
        match edges.last() {
            Some(last) if position - last < MIN_TRACK_SIZE => {}
            _ => edges.push(position),
        }
    }
    edges
}

/// Reads one grid. A row with no inner borders (a caption, a heading or prose
/// inside a page frame) is not part of a table, so it splits the grid.
fn build_tables(
    horizontal: &[Rule],
    vertical: &[Rule],
    glyphs: &[TextFragment],
    page: &mut RuledPage,
) {
    let xs = edges(vertical);
    // Rows read top to bottom, and PDF y grows upward.
    let mut ys = edges(horizontal);
    ys.reverse();
    let (Some(columns), Some(rows)) = (xs.len().checked_sub(1), ys.len().checked_sub(1)) else {
        return;
    };
    if rows < MIN_TABLE_ROWS || columns < MIN_TABLE_COLUMNS {
        return;
    }

    // Each grid slot points at the slot that owns its merged cell.
    let mut owner = (0..rows * columns).collect::<Vec<_>>();
    for row in 0..rows {
        let middle_y = (ys[row] + ys[row + 1]) / 2.0;
        for column in 0..columns {
            let middle_x = (xs[column] + xs[column + 1]) / 2.0;
            let slot = row * columns + column;
            if column > 0 && !has_rule(vertical, xs[column], middle_y) {
                owner[slot] = owner[slot - 1];
            } else if row > 0 && !has_rule(horizontal, ys[row], middle_x) {
                owner[slot] = owner[slot - columns];
            }
        }
    }

    let mut cell_glyphs = vec![Vec::<&TextFragment>::new(); rows * columns];
    for glyph in glyphs {
        let x = glyph.x + glyph.width / 2.0;
        let y = glyph.y + glyph.height / 2.0;
        let Some(column) = (0..columns).find(|&c| x >= xs[c] && x < xs[c + 1]) else {
            continue;
        };
        let Some(row) = (0..rows).find(|&r| y <= ys[r] && y > ys[r + 1]) else {
            continue;
        };
        cell_glyphs[owner[row * columns + column]].push(glyph);
    }
    let texts = cell_glyphs.into_iter().map(cell_text).collect::<Vec<_>>();

    let is_band = |row: usize| {
        (1..columns).all(|column| owner[row * columns + column] == owner[row * columns])
    };
    let mut row = 0;
    while row < rows {
        if is_band(row) {
            row += 1;
            continue;
        }
        let first = row;
        while row < rows && !is_band(row) {
            row += 1;
        }
        page.tables.extend(table_from_rows(
            &xs,
            &ys,
            columns,
            first..row,
            &owner,
            &texts,
        ));
    }
}

fn table_from_rows(
    xs: &[f64],
    ys: &[f64],
    columns: usize,
    rows: std::ops::Range<usize>,
    owner: &[usize],
    texts: &[String],
) -> Option<RuledTable> {
    let owned = rows
        .clone()
        .flat_map(|row| (0..columns).map(move |column| row * columns + column))
        .filter(|&slot| owner[slot] == slot)
        .collect::<Vec<_>>();
    let filled = owned
        .iter()
        .filter(|&&slot| !texts[slot].is_empty())
        .count();
    if filled < MIN_TABLE_CELLS || (filled as f64) < MIN_FILLED_CELL_SHARE * owned.len() as f64 {
        return None;
    }
    let mut table_rows = rows
        .clone()
        .map(|row| {
            (0..columns)
                .map(|column| {
                    let slot = row * columns + column;
                    if owner[slot] == slot {
                        texts[slot].clone()
                    } else {
                        String::new()
                    }
                })
                .collect::<Vec<_>>()
        })
        .filter(|cells| cells.iter().any(|cell| !cell.is_empty()))
        .collect::<Vec<_>>();
    // A column no row writes into is the gap of a doubled border, or a
    // column that belongs to another table in the same frame.
    let used = (0..columns)
        .filter(|&column| table_rows.iter().any(|row| !row[column].is_empty()))
        .collect::<Vec<_>>();
    for row in &mut table_rows {
        *row = used
            .iter()
            .map(|&column| std::mem::take(&mut row[column]))
            .collect();
    }
    if table_rows.len() < MIN_TABLE_ROWS || used.len() < MIN_TABLE_COLUMNS {
        return None;
    }
    // Prose set in a ruled box fills one cell per row.
    let multi_cell_rows = table_rows
        .iter()
        .filter(|row| row.iter().filter(|cell| !cell.is_empty()).count() >= 2)
        .count();
    if multi_cell_rows * 2 < table_rows.len() {
        return None;
    }
    // A merged cell reaches past the column that holds its text.
    let written = rows
        .clone()
        .flat_map(|row| (0..columns).map(move |column| (row * columns + column, column)))
        .filter(|&(slot, _)| !texts[owner[slot]].is_empty())
        .map(|(_, column)| column)
        .collect::<Vec<_>>();
    let left = written
        .iter()
        .map(|&column| xs[column])
        .fold(f64::INFINITY, f64::min);
    let right = written
        .iter()
        .map(|&column| xs[column + 1])
        .fold(f64::NEG_INFINITY, f64::max);
    let (top, bottom) = (ys[rows.start], ys[rows.end]);
    Some(RuledTable {
        bbox: ElementBBox::new(left, bottom, right - left, top - bottom),
        rows: table_rows,
    })
}

fn has_rule(rules: &[Rule], at: f64, position: f64) -> bool {
    rules
        .iter()
        .any(|rule| (rule.at - at).abs() < MIN_TRACK_SIZE && rule.covers(position))
}

/// Reads a cell's glyphs line by line, top to bottom and left to right.
fn cell_text(mut glyphs: Vec<&TextFragment>) -> String {
    glyphs.sort_by(|a, b| b.y.total_cmp(&a.y).then(a.x.total_cmp(&b.x)));
    let mut lines: Vec<Vec<&TextFragment>> = Vec::new();
    for glyph in glyphs {
        match lines.last_mut() {
            Some(line) if (line[0].y - glyph.y).abs() < line[0].height.max(glyph.height) * 0.5 => {
                line.push(glyph)
            }
            _ => lines.push(vec![glyph]),
        }
    }
    let mut text = String::new();
    for mut line in lines {
        line.sort_by(|a, b| a.x.total_cmp(&b.x));
        if !text.is_empty() && !text.ends_with('-') {
            text.push(' ');
        }
        let mut previous_end: Option<f64> = None;
        for glyph in line {
            if glyph.text.trim().is_empty() {
                text.push(' ');
                continue;
            }
            let gap_limit = glyph.font_size.max(1.0) * WORD_GAP_SHARE;
            if previous_end.is_some_and(|end| glyph.x - end > gap_limit) && !text.ends_with(' ') {
                text.push(' ');
            }
            text.push_str(glyph.text.trim());
            previous_end = Some(glyph.x + glyph.width);
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidize_pdf::graphics::extraction::VectorLine;

    fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> VectorLine {
        VectorLine::new(x1, y1, x2, y2, 1.0, true, None)
    }

    fn graphics(lines: Vec<VectorLine>) -> ExtractedGraphics {
        let mut graphics = ExtractedGraphics::new();
        lines.into_iter().for_each(|line| graphics.add_line(line));
        graphics
    }

    fn glyphs(text: &str, x: f64, y: f64) -> Vec<TextFragment> {
        text.chars()
            .enumerate()
            .filter(|(_, character)| *character != ' ')
            .map(|(index, character)| {
                TextFragment::new(
                    character.to_string(),
                    x + index as f64 * 5.0,
                    y,
                    4.5,
                    9.0,
                    9.0,
                )
            })
            .collect()
    }

    /// Two columns by three rows, each cell drawn as its own rectangle, the
    /// way word processors export ruled tables.
    fn drawn_grid(xs: &[f64], ys: &[f64]) -> ExtractedGraphics {
        let mut lines = Vec::new();
        for row in 0..ys.len() - 1 {
            for column in 0..xs.len() - 1 {
                let (left, right, top, bottom) = (xs[column], xs[column + 1], ys[row], ys[row + 1]);
                lines.push(line(left, top, right, top));
                lines.push(line(right, top, right, bottom));
                lines.push(line(right, bottom, left, bottom));
                lines.push(line(left, bottom, left, top));
            }
        }
        graphics(lines)
    }

    #[test]
    fn glyphs_land_in_the_cell_whose_borders_contain_them() {
        let graphics = drawn_grid(&[40.0, 140.0, 300.0], &[700.0, 680.0, 640.0, 610.0, 590.0]);
        let mut text = glyphs("Vitamin", 45.0, 686.0);
        text.extend(glyphs("Dose", 145.0, 686.0));
        text.extend(glyphs("K", 45.0, 660.0));
        text.extend(glyphs("90–120 µg", 145.0, 660.0));
        // A second line of the same cell.
        text.extend(glyphs("daily", 145.0, 648.0));
        text.extend(glyphs("B1", 45.0, 620.0));
        text.extend(glyphs("1.1 mg", 145.0, 620.0));
        text.extend(glyphs("C", 45.0, 596.0));
        text.extend(glyphs("75 mg", 145.0, 596.0));

        let tables = ruled_tables(&graphics, &text).tables;
        assert_eq!(tables.len(), 1);
        assert_eq!(
            tables[0].rows,
            vec![
                vec!["Vitamin", "Dose"],
                vec!["K", "90–120 µg daily"],
                vec!["B1", "1.1 mg"],
                vec!["C", "75 mg"],
            ]
        );
    }

    #[test]
    fn a_frame_around_prose_is_not_a_table() {
        let graphics = drawn_grid(&[40.0, 572.0], &[748.0, 50.0]);
        let text = glyphs("Plain paragraph text", 45.0, 400.0);
        assert!(ruled_tables(&graphics, &text).tables.is_empty());
    }

    #[test]
    fn a_missing_inner_border_merges_the_cells() {
        let mut lines =
            drawn_grid(&[40.0, 140.0, 240.0, 340.0], &[700.0, 680.0, 640.0, 620.0]).lines;
        // Remove the vertical rule between the first two cells of the first row.
        lines.retain(|rule| {
            !(rule.orientation == LineOrientation::Vertical
                && (rule.x1 - 140.0).abs() < 0.1
                && rule.y1.max(rule.y2) > 690.0)
        });
        let mut text = glyphs("Group heading", 45.0, 686.0);
        text.extend(glyphs("C", 245.0, 686.0));
        text.extend(glyphs("A", 45.0, 660.0));
        text.extend(glyphs("B", 145.0, 660.0));
        text.extend(glyphs("D", 245.0, 660.0));
        for (x, label) in [(45.0, "E"), (145.0, "F"), (245.0, "G")] {
            text.extend(glyphs(label, x, 626.0));
        }
        let tables = ruled_tables(&graphics(lines), &text).tables;
        assert_eq!(
            tables[0].rows,
            vec![
                vec!["Group heading", "", "C"],
                vec!["A", "B", "D"],
                vec!["E", "F", "G"],
            ]
        );
    }

    #[test]
    fn a_caption_row_inside_a_page_frame_splits_two_tables() {
        let xs = [40.0, 140.0, 300.0];
        let upper = [700.0, 680.0, 660.0, 640.0, 620.0];
        let lower = [580.0, 560.0, 540.0, 520.0, 500.0];
        let mut lines = drawn_grid(&xs, &upper).lines;
        lines.extend(drawn_grid(&xs, &lower).lines);
        // The page frame joins both tables into one set of lines.
        lines.extend(drawn_grid(&[40.0, 300.0], &[700.0, 500.0]).lines);
        let mut text = Vec::new();
        let vitamins = [
            ("A", "900 µg"),
            ("D", "15 µg"),
            ("E", "15 mg"),
            ("K", "120 µg"),
        ];
        let minerals = [
            ("Iron", "8 mg"),
            ("Zinc", "11 mg"),
            ("Copper", "900 µg"),
            ("Iodine", "150 µg"),
        ];
        for (top, rows) in [(upper, vitamins), (lower, minerals)] {
            for (row, (name, dose)) in rows.iter().enumerate() {
                text.extend(glyphs(name, 45.0, top[row] - 14.0));
                text.extend(glyphs(dose, 145.0, top[row] - 14.0));
            }
        }
        text.extend(glyphs("Table 2 Minerals", 45.0, 600.0));
        let tables = ruled_tables(&graphics(lines), &text).tables;
        let rows = tables
            .iter()
            .map(|table| table.rows.clone())
            .collect::<Vec<_>>();
        let expected = |rows: [(&str, &str); 4]| {
            rows.iter()
                .map(|(name, dose)| vec![name.to_string(), dose.to_string()])
                .collect::<Vec<_>>()
        };
        assert_eq!(rows, vec![expected(vitamins), expected(minerals)]);
        assert!(tables[1].bbox.y >= 499.0 && tables[1].bbox.y + tables[1].bbox.height <= 581.0);
    }

    #[test]
    fn prose_in_a_ruled_box_is_not_a_table() {
        let ys = [700.0, 680.0, 660.0, 640.0, 620.0, 600.0];
        let graphics = drawn_grid(&[40.0, 300.0, 400.0], &ys);
        let mut text = Vec::new();
        for (row, top) in ys.iter().take(4).enumerate() {
            text.extend(glyphs(
                &format!("Figure caption line {row}"),
                45.0,
                top - 14.0,
            ));
        }
        text.extend(glyphs("side note", 305.0, 606.0));
        assert!(ruled_tables(&graphics, &text).tables.is_empty());
    }

    #[test]
    fn a_table_must_read_as_runs_of_the_text_it_replaces() {
        let rows = vec![
            vec![
                "Problems associated".to_string(),
                "with deficiency".to_string(),
            ],
            vec!["Hemorrhagic disease".to_string(), "of newborn".to_string()],
        ];
        let scrambled_paragraphs = vec![
            "Problems associated Hemorrhagic disease".to_string(),
            "with deficiency of newborn".to_string(),
        ];
        assert_eq!(shared_word_pair_share(&rows, &scrambled_paragraphs), 1.0);
        let rotated_labels = vec!["sgassaP riA ciliated epithelium".to_string()];
        assert_eq!(shared_word_pair_share(&rows, &rotated_labels), 0.0);
    }
}
