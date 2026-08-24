use std::collections::{BTreeMap, HashMap};

use anydoc::model::{
    AssetId, Block, CellSlot, Document, ImageSource, Inline, List, NoteKind, Table, TableKind,
};

use crate::{document_canonical::CanonicalBlockRole, document_domain::DocumentFormat};

use super::{
    model::{InlineProjection, NormalizedBlock},
    support::{
        bounded_text, deduplicate, MAX_ATTRIBUTE_VALUE_CHARS, MAX_BLOCK_TEXT_CHARS,
        MAX_HEADING_CHARS,
    },
};

pub(crate) fn normalize_document_blocks(
    document: &Document,
    format: DocumentFormat,
    asset_identity: &HashMap<usize, String>,
) -> Vec<NormalizedBlock> {
    let mut output = Vec::new();
    let mut headings = Vec::<String>::new();
    for block in &document.blocks {
        normalize_block(block, format, asset_identity, &mut headings, &mut output, 0);
    }
    for note in &document.notes {
        let mut projection = InlineProjection::default();
        let text = blocks_plain_text(&note.blocks, asset_identity, &mut projection);
        if text.is_empty() && projection.asset_ids.is_empty() {
            continue;
        }
        let mut attributes = BTreeMap::new();
        attributes.insert(
            "noteId".to_string(),
            bounded_text(&note.id, MAX_ATTRIBUTE_VALUE_CHARS),
        );
        output.push(NormalizedBlock {
            role: match note.kind {
                NoteKind::Footnote => CanonicalBlockRole::Footnote,
                NoteKind::Endnote => CanonicalBlockRole::Endnote,
            },
            text,
            heading_level: None,
            parent_heading: headings.last().cloned(),
            heading_path: headings.clone(),
            asset_ids: deduplicate(projection.asset_ids),
            attributes,
        });
    }
    output
}

fn normalize_block(
    block: &Block,
    _format: DocumentFormat,
    asset_identity: &HashMap<usize, String>,
    headings: &mut Vec<String>,
    output: &mut Vec<NormalizedBlock>,
    depth: usize,
) {
    match block {
        Block::Heading { level, content, .. } => {
            let projection = project_inlines(content, asset_identity);
            let text = bounded_text(&projection.text, MAX_HEADING_CHARS);
            if text.is_empty() && projection.asset_ids.is_empty() {
                return;
            }
            let level = (*level).clamp(1, 64);
            let parent = headings.get(usize::from(level).saturating_sub(2)).cloned();
            headings.truncate(usize::from(level).saturating_sub(1));
            if !text.is_empty() {
                headings.push(text.clone());
            }
            let mut attributes = BTreeMap::new();
            attributes.insert("headingLevel".to_string(), level.to_string());
            add_external_image_count(&mut attributes, projection.external_image_count);
            output.push(NormalizedBlock {
                role: if output.is_empty() && level == 1 {
                    CanonicalBlockRole::Title
                } else {
                    CanonicalBlockRole::Heading
                },
                text,
                heading_level: Some(level),
                parent_heading: parent,
                heading_path: headings.clone(),
                asset_ids: deduplicate(projection.asset_ids),
                attributes,
            });
        }
        Block::Paragraph(inlines) => {
            let projection = project_inlines(inlines, asset_identity);
            if projection.text.trim().is_empty() && projection.asset_ids.is_empty() {
                return;
            }
            let role = if projection.text.trim().is_empty() && !projection.asset_ids.is_empty() {
                CanonicalBlockRole::Image
            } else {
                CanonicalBlockRole::Paragraph
            };
            let mut attributes = BTreeMap::new();
            add_external_image_count(&mut attributes, projection.external_image_count);
            output.push(NormalizedBlock {
                role,
                text: bounded_text(&projection.text, MAX_BLOCK_TEXT_CHARS),
                heading_level: None,
                parent_heading: headings.last().cloned(),
                heading_path: headings.clone(),
                asset_ids: deduplicate(projection.asset_ids),
                attributes,
            });
        }
        Block::List(list) => normalize_list(list, asset_identity, headings, output, depth),
        Block::Table(table) => {
            let mut projection = InlineProjection::default();
            let text = table_plain_text(table, asset_identity, &mut projection);
            let mut attributes = BTreeMap::new();
            attributes.insert("headerRows".to_string(), table.header_rows.to_string());
            attributes.insert("rowCount".to_string(), table.grid.len().to_string());
            attributes.insert(
                "tableKind".to_string(),
                match table.kind {
                    TableKind::Data => "data",
                    TableKind::Layout => "layout",
                }
                .to_string(),
            );
            add_external_image_count(&mut attributes, projection.external_image_count);
            output.push(NormalizedBlock {
                role: CanonicalBlockRole::Table,
                text: bounded_text(&text, MAX_BLOCK_TEXT_CHARS),
                heading_level: None,
                parent_heading: headings.last().cloned(),
                heading_path: headings.clone(),
                asset_ids: deduplicate(projection.asset_ids),
                attributes,
            });
        }
        Block::BlockQuote(blocks) => {
            let mut projection = InlineProjection::default();
            let text = blocks_plain_text(blocks, asset_identity, &mut projection);
            if text.is_empty() && projection.asset_ids.is_empty() {
                return;
            }
            output.push(NormalizedBlock {
                role: CanonicalBlockRole::Quote,
                text: bounded_text(&text, MAX_BLOCK_TEXT_CHARS),
                heading_level: None,
                parent_heading: headings.last().cloned(),
                heading_path: headings.clone(),
                asset_ids: deduplicate(projection.asset_ids),
                attributes: BTreeMap::new(),
            });
        }
        Block::CodeBlock { lang, text } => {
            let mut attributes = BTreeMap::new();
            if let Some(language) = lang {
                let language = bounded_text(language, MAX_ATTRIBUTE_VALUE_CHARS);
                if !language.is_empty() {
                    attributes.insert("language".to_string(), language);
                }
            }
            output.push(NormalizedBlock {
                role: CanonicalBlockRole::CodeBlock,
                text: bounded_text(text, MAX_BLOCK_TEXT_CHARS),
                heading_level: None,
                parent_heading: headings.last().cloned(),
                heading_path: headings.clone(),
                asset_ids: Vec::new(),
                attributes,
            });
        }
        Block::Math(tex) => output.push(NormalizedBlock {
            role: CanonicalBlockRole::Equation,
            text: bounded_text(tex, MAX_BLOCK_TEXT_CHARS),
            heading_level: None,
            parent_heading: headings.last().cloned(),
            heading_path: headings.clone(),
            asset_ids: Vec::new(),
            attributes: BTreeMap::new(),
        }),
        Block::Rule => {}
    }
}

fn normalize_list(
    list: &List,
    asset_identity: &HashMap<usize, String>,
    headings: &mut Vec<String>,
    output: &mut Vec<NormalizedBlock>,
    depth: usize,
) {
    for (index, item) in list.items.iter().enumerate() {
        let mut projection = InlineProjection::default();
        let text = blocks_plain_text(&item.blocks, asset_identity, &mut projection);
        if text.is_empty() && projection.asset_ids.is_empty() {
            continue;
        }
        let ordinal = list.start.saturating_add(index as u64);
        let marker = item
            .marker_label
            .clone()
            .unwrap_or_else(|| list.marker.label(ordinal));
        let mut attributes = BTreeMap::new();
        attributes.insert(
            "listMarker".to_string(),
            bounded_text(&marker, MAX_ATTRIBUTE_VALUE_CHARS),
        );
        attributes.insert("listDepth".to_string(), depth.to_string());
        attributes.insert("ordered".to_string(), list.ordered().to_string());
        add_external_image_count(&mut attributes, projection.external_image_count);
        output.push(NormalizedBlock {
            role: CanonicalBlockRole::ListItem,
            text: bounded_text(&text, MAX_BLOCK_TEXT_CHARS),
            heading_level: None,
            parent_heading: headings.last().cloned(),
            heading_path: headings.clone(),
            asset_ids: deduplicate(projection.asset_ids),
            attributes,
        });
        for nested in &item.blocks {
            if let Block::List(nested_list) = nested {
                normalize_list(
                    nested_list,
                    asset_identity,
                    headings,
                    output,
                    depth.saturating_add(1),
                );
            }
        }
    }
}

fn project_inlines(
    inlines: &[Inline],
    asset_identity: &HashMap<usize, String>,
) -> InlineProjection {
    let mut projection = InlineProjection::default();
    project_inline_slice(inlines, asset_identity, &mut projection);
    projection.text = bounded_text(&projection.text, MAX_BLOCK_TEXT_CHARS);
    projection
}

fn project_inline_slice(
    inlines: &[Inline],
    asset_identity: &HashMap<usize, String>,
    projection: &mut InlineProjection,
) {
    for inline in inlines {
        match inline {
            Inline::Text { text, .. } => projection.text.push_str(text),
            Inline::Link { content, .. } => {
                project_inline_slice(content, asset_identity, projection)
            }
            Inline::Image { alt, source } => {
                if !alt.trim().is_empty() {
                    projection.text.push_str(alt);
                }
                match source {
                    ImageSource::Asset(AssetId(index)) => {
                        if let Some(asset_id) = asset_identity.get(index) {
                            projection.asset_ids.push(asset_id.clone());
                        }
                    }
                    ImageSource::External(_) => {
                        projection.external_image_count =
                            projection.external_image_count.saturating_add(1);
                    }
                    ImageSource::Unavailable => {}
                }
            }
            Inline::Anchor(_) | Inline::NoteRef(_) => {}
            Inline::LineBreak => projection.text.push('\n'),
            Inline::Math(tex) => {
                projection.text.push('$');
                projection.text.push_str(tex);
                projection.text.push('$');
            }
            Inline::Checkbox(checked) => {
                projection
                    .text
                    .push_str(if *checked { "[x]" } else { "[ ]" })
            }
        }
    }
}

fn blocks_plain_text(
    blocks: &[Block],
    asset_identity: &HashMap<usize, String>,
    projection: &mut InlineProjection,
) -> String {
    let mut parts = Vec::new();
    for block in blocks {
        match block {
            Block::Heading { content, .. } | Block::Paragraph(content) => {
                let before = projection.text.len();
                project_inline_slice(content, asset_identity, projection);
                let projected = projection.text[before..].trim().to_string();
                projection.text.truncate(before);
                if !projected.is_empty() {
                    parts.push(projected);
                }
            }
            Block::List(list) => {
                for (index, item) in list.items.iter().enumerate() {
                    let item_text = blocks_plain_text(&item.blocks, asset_identity, projection);
                    if !item_text.is_empty() {
                        let marker = item.marker_label.clone().unwrap_or_else(|| {
                            list.marker.label(list.start.saturating_add(index as u64))
                        });
                        parts.push(format!("{marker} {item_text}"));
                    }
                }
            }
            Block::Table(table) => {
                let text = table_plain_text(table, asset_identity, projection);
                if !text.is_empty() {
                    parts.push(text);
                }
            }
            Block::BlockQuote(nested) => {
                let text = blocks_plain_text(nested, asset_identity, projection);
                if !text.is_empty() {
                    parts.push(text);
                }
            }
            Block::CodeBlock { text, .. } | Block::Math(text) => {
                if !text.trim().is_empty() {
                    parts.push(text.trim().to_string());
                }
            }
            Block::Rule => {}
        }
    }
    bounded_text(&parts.join("\n"), MAX_BLOCK_TEXT_CHARS)
}

fn table_plain_text(
    table: &Table,
    asset_identity: &HashMap<usize, String>,
    projection: &mut InlineProjection,
) -> String {
    let rows = table
        .grid
        .iter()
        .map(|row| {
            row.iter()
                .map(|slot| match slot {
                    CellSlot::Origin(cell) => {
                        blocks_plain_text(&cell.blocks, asset_identity, projection)
                    }
                    CellSlot::Covered { .. } => String::new(),
                })
                .collect::<Vec<_>>()
                .join("\t")
        })
        .collect::<Vec<_>>();
    bounded_text(&rows.join("\n"), MAX_BLOCK_TEXT_CHARS)
}

fn add_external_image_count(attributes: &mut BTreeMap<String, String>, count: usize) {
    if count > 0 {
        attributes.insert("externalImageReferenceCount".to_string(), count.to_string());
    }
}
