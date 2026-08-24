use std::collections::{BTreeMap, HashMap, HashSet};

use xberg::{
    types::document_structure::{
        ContentLayer, DocumentNode, DocumentStructure, NodeContent, NodeIndex,
    },
    Element, ElementType, ExtractedDocument, PageContent,
};

use crate::{
    document_canonical::CanonicalBlockRole,
    document_domain::DocumentFormat,
    document_ingestion::canonical::{segment_units, NormalizedBlock, UnitDraft},
    error::CommandError,
};

use super::{
    support::{
        bounded_text, deduplicate, MAX_ATTRIBUTE_VALUE_CHARS, MAX_BLOCK_TEXT_CHARS,
        MAX_HEADING_CHARS,
    },
    tables::{project_grid, project_table},
};

const MAX_NODE_ATTRIBUTES: usize = 24;
const MAX_ANCESTOR_DEPTH: usize = 64;

#[derive(Debug)]
pub(crate) struct NormalizedDocument {
    pub(crate) units: Vec<UnitDraft>,
    pub(crate) exact_unit_boundaries: bool,
    pub(crate) skipped_node_count: usize,
    pub(crate) external_image_reference_count: usize,
}

#[derive(Clone, Debug, Default)]
struct NodeContext {
    quoted: bool,
    admonition_kind: Option<String>,
    admonition_title: Option<String>,
}

#[derive(Default)]
struct MappingState {
    headings: Vec<String>,
    skipped_node_count: usize,
    external_image_reference_count: usize,
}

pub(crate) fn normalize_document(
    document: &ExtractedDocument,
    format: DocumentFormat,
    asset_ids: &HashMap<u32, String>,
) -> Result<NormalizedDocument, CommandError> {
    if authored_page_format(format)
        && document
            .pages
            .as_ref()
            .is_some_and(|pages| !pages.is_empty())
    {
        return normalize_authored_pages(document, format, asset_ids);
    }

    let mut state = MappingState::default();
    let mut blocks = if let Some(structure) = document.document.as_ref() {
        map_structure(structure, None, asset_ids, &mut state)?
    } else {
        Vec::new()
    };

    if blocks.is_empty() {
        if let Some(elements) = document.elements.as_ref() {
            blocks = map_elements(elements, asset_ids, &mut state);
        }
    }
    append_unrepresented_tables(&mut blocks, &document.tables)?;
    if blocks.is_empty() {
        let text = bounded_text(&document.content, MAX_BLOCK_TEXT_CHARS);
        if !text.is_empty() {
            blocks.push(block(
                CanonicalBlockRole::Paragraph,
                text,
                None,
                &state.headings,
                Vec::new(),
                BTreeMap::new(),
            ));
        }
    }

    Ok(NormalizedDocument {
        units: segment_units(format, blocks),
        exact_unit_boundaries: false,
        skipped_node_count: state.skipped_node_count,
        external_image_reference_count: state.external_image_reference_count,
    })
}

fn normalize_authored_pages(
    document: &ExtractedDocument,
    format: DocumentFormat,
    asset_ids: &HashMap<u32, String>,
) -> Result<NormalizedDocument, CommandError> {
    let pages = document.pages.as_deref().unwrap_or_default();
    let mut state = MappingState::default();
    let structure = document.document.as_ref();
    let mut units = Vec::with_capacity(pages.len());

    for page in pages {
        let mut page_state = MappingState::default();
        let mut blocks = structure
            .map(|structure| {
                map_structure(
                    structure,
                    Some(page.page_number),
                    asset_ids,
                    &mut page_state,
                )
            })
            .transpose()?
            .unwrap_or_default();

        append_page_tables(&mut blocks, page)?;
        append_page_images(&mut blocks, page, asset_ids);
        if blocks.is_empty() {
            let text = bounded_text(&page.content, MAX_BLOCK_TEXT_CHARS);
            if !text.is_empty() {
                let role = if spreadsheet_format(format) && text.contains('\t') {
                    CanonicalBlockRole::Table
                } else {
                    CanonicalBlockRole::Paragraph
                };
                blocks.push(block(role, text, None, &[], Vec::new(), BTreeMap::new()));
            }
        }

        let title = authored_unit_title(structure, page, &blocks, format);
        if let Some(notes) = page
            .speaker_notes
            .as_deref()
            .map(|value| bounded_text(value, MAX_BLOCK_TEXT_CHARS))
            .filter(|value| !value.is_empty())
        {
            blocks.push(NormalizedBlock {
                role: CanonicalBlockRole::SpeakerNote,
                text: notes,
                heading_level: None,
                parent_heading: title.clone(),
                heading_path: title.clone().into_iter().collect(),
                asset_ids: Vec::new(),
                attributes: BTreeMap::new(),
            });
        }
        if let Some(section_name) = page
            .section_name
            .as_deref()
            .map(|value| bounded_text(value, MAX_ATTRIBUTE_VALUE_CHARS))
            .filter(|value| !value.is_empty())
        {
            if let Some(first) = blocks.first_mut() {
                first
                    .attributes
                    .insert("presentationSection".to_string(), section_name);
            }
        }

        let label = if presentation_format(format) {
            format!("Slide {}", page.page_number)
        } else {
            page.sheet_name
                .as_deref()
                .map(|value| bounded_text(value, MAX_HEADING_CHARS))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| format!("Sheet {}", page.page_number))
        };
        units.push(UnitDraft {
            label: Some(label),
            title,
            text_chars: blocks.iter().map(|block| block.text.chars().count()).sum(),
            blocks,
        });
        state.skipped_node_count = state
            .skipped_node_count
            .saturating_add(page_state.skipped_node_count);
        state.external_image_reference_count = state
            .external_image_reference_count
            .saturating_add(page_state.external_image_reference_count);
    }

    Ok(NormalizedDocument {
        units,
        exact_unit_boundaries: true,
        skipped_node_count: state.skipped_node_count,
        external_image_reference_count: state.external_image_reference_count,
    })
}

fn map_structure(
    structure: &DocumentStructure,
    page_filter: Option<u32>,
    asset_ids: &HashMap<u32, String>,
    state: &mut MappingState,
) -> Result<Vec<NormalizedBlock>, CommandError> {
    let mut output = Vec::new();
    let mut headings = Vec::<String>::new();
    for (index, node) in structure.nodes.iter().enumerate() {
        if page_filter.is_some_and(|page| resolved_page(structure, index) != Some(page)) {
            continue;
        }
        let context = node_context(structure, index);
        if let Some(mut mapped) = map_node(node, asset_ids, &context, state)? {
            apply_heading_context(&mut mapped, &mut headings);
            if !is_adjacent_duplicate(output.last(), &mapped) {
                output.push(mapped);
            }
        }
    }
    state.headings = headings;
    Ok(output)
}

fn map_node(
    node: &DocumentNode,
    asset_ids: &HashMap<u32, String>,
    context: &NodeContext,
    state: &mut MappingState,
) -> Result<Option<NormalizedBlock>, CommandError> {
    let mut attributes = bounded_attributes(node.attributes.as_ref());
    if let Some(kind) = context.admonition_kind.as_deref() {
        attributes.insert(
            "admonitionKind".to_string(),
            bounded_text(kind, MAX_ATTRIBUTE_VALUE_CHARS),
        );
    }
    if let Some(title) = context.admonition_title.as_deref() {
        attributes.insert(
            "admonitionTitle".to_string(),
            bounded_text(title, MAX_ATTRIBUTE_VALUE_CHARS),
        );
    }

    let mapped = match &node.content {
        NodeContent::Title { text } => {
            Some((CanonicalBlockRole::Title, text.clone(), Some(1), Vec::new()))
        }
        NodeContent::Heading { level, text } => Some((
            CanonicalBlockRole::Heading,
            text.clone(),
            Some((*level).clamp(1, 6)),
            Vec::new(),
        )),
        NodeContent::Paragraph { text } => Some((
            if context.quoted {
                CanonicalBlockRole::Quote
            } else {
                CanonicalBlockRole::Paragraph
            },
            text.clone(),
            None,
            Vec::new(),
        )),
        NodeContent::ListItem { text } => Some((
            if context.quoted {
                CanonicalBlockRole::Quote
            } else {
                CanonicalBlockRole::ListItem
            },
            text.clone(),
            None,
            Vec::new(),
        )),
        NodeContent::Table { grid } => {
            let projected = project_grid(grid)?;
            attributes.extend(projected.attributes);
            Some((CanonicalBlockRole::Table, projected.text, None, Vec::new()))
        }
        NodeContent::Image {
            description,
            image_index,
            src,
        } => {
            let asset_ids_for_node = image_index
                .and_then(|index| asset_ids.get(&index).cloned())
                .into_iter()
                .collect::<Vec<_>>();
            if asset_ids_for_node.is_empty()
                && src.as_deref().is_some_and(|value| !value.trim().is_empty())
            {
                state.external_image_reference_count =
                    state.external_image_reference_count.saturating_add(1);
                attributes.insert("externalImageReference".to_string(), "true".to_string());
            }
            Some((
                CanonicalBlockRole::Image,
                description.clone().unwrap_or_default(),
                None,
                asset_ids_for_node,
            ))
        }
        NodeContent::Code { text, language } => {
            if let Some(language) = language
                .as_deref()
                .map(|value| bounded_text(value, MAX_ATTRIBUTE_VALUE_CHARS))
                .filter(|value| !value.is_empty())
            {
                attributes.insert("language".to_string(), language);
            }
            Some((
                CanonicalBlockRole::CodeBlock,
                text.clone(),
                None,
                Vec::new(),
            ))
        }
        NodeContent::Formula { text } => {
            Some((CanonicalBlockRole::Equation, text.clone(), None, Vec::new()))
        }
        NodeContent::Footnote { text } => {
            Some((CanonicalBlockRole::Footnote, text.clone(), None, Vec::new()))
        }
        NodeContent::Comment { text } => {
            attributes.insert("sourceRole".to_string(), "comment".to_string());
            Some((CanonicalBlockRole::Footnote, text.clone(), None, Vec::new()))
        }
        NodeContent::DefinitionItem { term, definition } => {
            attributes.insert(
                "term".to_string(),
                bounded_text(term, MAX_ATTRIBUTE_VALUE_CHARS),
            );
            Some((
                CanonicalBlockRole::KeyValue,
                format!("{term}\n{definition}"),
                None,
                Vec::new(),
            ))
        }
        NodeContent::Citation { key, text } => {
            attributes.insert(
                "citationKey".to_string(),
                bounded_text(key, MAX_ATTRIBUTE_VALUE_CHARS),
            );
            Some((CanonicalBlockRole::Footnote, text.clone(), None, Vec::new()))
        }
        NodeContent::RawBlock { format, content } => {
            attributes.insert(
                "sourceFormat".to_string(),
                bounded_text(format, MAX_ATTRIBUTE_VALUE_CHARS),
            );
            Some((
                CanonicalBlockRole::CodeBlock,
                content.clone(),
                None,
                Vec::new(),
            ))
        }
        NodeContent::MetadataBlock { entries } => {
            let text = entries
                .iter()
                .map(|(key, value)| format!("{key}: {value}"))
                .collect::<Vec<_>>()
                .join("\n");
            Some((CanonicalBlockRole::KeyValue, text, None, Vec::new()))
        }
        NodeContent::Group {
            heading_level,
            heading_text,
            ..
        } => heading_text
            .as_ref()
            .filter(|text| !text.trim().is_empty())
            .map(|text| {
                (
                    CanonicalBlockRole::Heading,
                    text.clone(),
                    Some(heading_level.unwrap_or(1).clamp(1, 6)),
                    Vec::new(),
                )
            }),
        NodeContent::List { .. }
        | NodeContent::Quote
        | NodeContent::PageBreak
        | NodeContent::Slide { .. }
        | NodeContent::DefinitionList
        | NodeContent::Admonition { .. } => None,
    };

    let Some((role, text, heading_level, asset_ids)) = mapped else {
        return Ok(None);
    };
    let role = role_for_layer(role, node.content_layer);
    let text = bounded_text(&text, MAX_BLOCK_TEXT_CHARS);
    let asset_ids = deduplicate(asset_ids);
    if text.is_empty() && asset_ids.is_empty() {
        state.skipped_node_count = state.skipped_node_count.saturating_add(1);
        return Ok(None);
    }
    Ok(Some(NormalizedBlock {
        role,
        text,
        heading_level,
        parent_heading: None,
        heading_path: Vec::new(),
        asset_ids,
        attributes,
    }))
}

fn map_elements(
    elements: &[Element],
    asset_ids: &HashMap<u32, String>,
    state: &mut MappingState,
) -> Vec<NormalizedBlock> {
    let mut output = Vec::new();
    let mut headings = Vec::new();
    for element in elements {
        let mut attributes = element
            .metadata
            .additional
            .iter()
            .take(MAX_NODE_ATTRIBUTES)
            .map(|(key, value)| {
                (
                    bounded_text(key, 120),
                    bounded_text(value, MAX_ATTRIBUTE_VALUE_CHARS),
                )
            })
            .filter(|(key, value)| !key.is_empty() && !value.is_empty())
            .collect::<BTreeMap<_, _>>();
        let (role, heading_level) = match element.element_type {
            ElementType::Title => (CanonicalBlockRole::Title, Some(1)),
            ElementType::Heading => (CanonicalBlockRole::Heading, Some(1)),
            ElementType::NarrativeText => (CanonicalBlockRole::Paragraph, None),
            ElementType::ListItem => (CanonicalBlockRole::ListItem, None),
            ElementType::Table => (CanonicalBlockRole::Table, None),
            ElementType::Image => (CanonicalBlockRole::Image, None),
            ElementType::CodeBlock => (CanonicalBlockRole::CodeBlock, None),
            ElementType::Formula => (CanonicalBlockRole::Equation, None),
            ElementType::BlockQuote => (CanonicalBlockRole::Quote, None),
            ElementType::Footer => (CanonicalBlockRole::Footer, None),
            ElementType::Header => (CanonicalBlockRole::Header, None),
            ElementType::PageBreak => continue,
        };
        let mut element_asset_ids = Vec::new();
        if role == CanonicalBlockRole::Image {
            for key in ["image_index", "imageIndex"] {
                if let Some(index) = element
                    .metadata
                    .additional
                    .get(key)
                    .and_then(|value| value.parse::<u32>().ok())
                {
                    if let Some(asset_id) = asset_ids.get(&index) {
                        element_asset_ids.push(asset_id.clone());
                    }
                }
            }
            if element_asset_ids.is_empty() {
                attributes.insert("imageAssetUnavailable".to_string(), "true".to_string());
            }
        }
        let text = bounded_text(&element.text, MAX_BLOCK_TEXT_CHARS);
        if text.is_empty() && element_asset_ids.is_empty() {
            state.skipped_node_count = state.skipped_node_count.saturating_add(1);
            continue;
        }
        let mut mapped = NormalizedBlock {
            role,
            text,
            heading_level,
            parent_heading: None,
            heading_path: Vec::new(),
            asset_ids: deduplicate(element_asset_ids),
            attributes,
        };
        apply_heading_context(&mut mapped, &mut headings);
        if !is_adjacent_duplicate(output.last(), &mapped) {
            output.push(mapped);
        }
    }
    state.headings = headings;
    output
}

fn append_unrepresented_tables(
    blocks: &mut Vec<NormalizedBlock>,
    tables: &[xberg::Table],
) -> Result<(), CommandError> {
    let mut existing = blocks
        .iter()
        .filter(|block| block.role == CanonicalBlockRole::Table)
        .map(|block| block.text.clone())
        .collect::<HashSet<_>>();
    for table in tables {
        let projected = project_table(table)?;
        if projected.text.is_empty() || !existing.insert(projected.text.clone()) {
            continue;
        }
        blocks.push(NormalizedBlock {
            role: CanonicalBlockRole::Table,
            text: projected.text,
            heading_level: None,
            parent_heading: None,
            heading_path: Vec::new(),
            asset_ids: Vec::new(),
            attributes: projected.attributes,
        });
    }
    Ok(())
}

fn append_page_tables(
    blocks: &mut Vec<NormalizedBlock>,
    page: &PageContent,
) -> Result<(), CommandError> {
    let mut existing = blocks
        .iter()
        .filter(|block| block.role == CanonicalBlockRole::Table)
        .map(|block| block.text.clone())
        .collect::<HashSet<_>>();
    for table in &page.tables {
        let projected = project_table(table.as_ref())?;
        if projected.text.is_empty() || !existing.insert(projected.text.clone()) {
            continue;
        }
        blocks.push(NormalizedBlock {
            role: CanonicalBlockRole::Table,
            text: projected.text,
            heading_level: None,
            parent_heading: None,
            heading_path: Vec::new(),
            asset_ids: Vec::new(),
            attributes: projected.attributes,
        });
    }
    Ok(())
}

fn append_page_images(
    blocks: &mut Vec<NormalizedBlock>,
    page: &PageContent,
    asset_ids: &HashMap<u32, String>,
) {
    let mut referenced = blocks
        .iter()
        .flat_map(|block| block.asset_ids.iter().cloned())
        .collect::<HashSet<_>>();
    let missing = page
        .image_indices
        .iter()
        .filter_map(|index| asset_ids.get(index).cloned())
        .filter(|asset_id| referenced.insert(asset_id.clone()))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        blocks.push(NormalizedBlock {
            role: CanonicalBlockRole::Image,
            text: String::new(),
            heading_level: None,
            parent_heading: None,
            heading_path: Vec::new(),
            asset_ids: missing,
            attributes: BTreeMap::new(),
        });
    }
}

fn authored_unit_title(
    structure: Option<&DocumentStructure>,
    page: &PageContent,
    blocks: &[NormalizedBlock],
    format: DocumentFormat,
) -> Option<String> {
    if presentation_format(format) {
        if let Some(title) = structure.and_then(|structure| {
            structure.nodes.iter().find_map(|node| match &node.content {
                NodeContent::Slide { number, title } if *number == page.page_number => {
                    title.clone()
                }
                _ => None,
            })
        }) {
            let title = bounded_text(&title, MAX_HEADING_CHARS);
            if !title.is_empty() {
                return Some(title);
            }
        }
    }
    blocks
        .iter()
        .find(|block| {
            matches!(
                block.role,
                CanonicalBlockRole::Title | CanonicalBlockRole::Heading
            )
        })
        .map(|block| block.text.clone())
        .or_else(|| {
            if spreadsheet_format(format) {
                page.sheet_name.clone()
            } else {
                page.section_name.clone()
            }
        })
        .map(|value| bounded_text(&value, MAX_HEADING_CHARS))
        .filter(|value| !value.is_empty())
}

fn apply_heading_context(block: &mut NormalizedBlock, headings: &mut Vec<String>) {
    if let Some(level) = block.heading_level {
        let level = usize::from(level.clamp(1, 6));
        while headings.len() >= level {
            headings.pop();
        }
        block.parent_heading = headings.last().cloned();
        headings.push(block.text.clone());
        block.heading_path = headings.clone();
    } else {
        block.parent_heading = headings.last().cloned();
        block.heading_path = headings.clone();
    }
}

fn block(
    role: CanonicalBlockRole,
    text: String,
    heading_level: Option<u8>,
    headings: &[String],
    asset_ids: Vec<String>,
    attributes: BTreeMap<String, String>,
) -> NormalizedBlock {
    NormalizedBlock {
        role,
        text,
        heading_level,
        parent_heading: headings.last().cloned(),
        heading_path: headings.to_vec(),
        asset_ids,
        attributes,
    }
}

fn role_for_layer(role: CanonicalBlockRole, layer: ContentLayer) -> CanonicalBlockRole {
    if matches!(
        role,
        CanonicalBlockRole::Image | CanonicalBlockRole::Table | CanonicalBlockRole::Equation
    ) {
        return role;
    }
    match layer {
        ContentLayer::Body => role,
        ContentLayer::Header => CanonicalBlockRole::Header,
        ContentLayer::Footer => CanonicalBlockRole::Footer,
        ContentLayer::Footnote => CanonicalBlockRole::Footnote,
    }
}

fn bounded_attributes(attributes: Option<&HashMap<String, String>>) -> BTreeMap<String, String> {
    attributes
        .into_iter()
        .flat_map(|attributes| attributes.iter())
        .take(MAX_NODE_ATTRIBUTES)
        .map(|(key, value)| {
            (
                bounded_text(key, 120),
                bounded_text(value, MAX_ATTRIBUTE_VALUE_CHARS),
            )
        })
        .filter(|(key, value)| !key.is_empty() && !value.is_empty())
        .collect()
}

fn node_context(structure: &DocumentStructure, index: usize) -> NodeContext {
    let mut context = NodeContext::default();
    let mut parent = structure.nodes.get(index).and_then(|node| node.parent);
    for _ in 0..MAX_ANCESTOR_DEPTH {
        let Some(NodeIndex(parent_index)) = parent else {
            break;
        };
        let Some(node) = structure.nodes.get(parent_index as usize) else {
            break;
        };
        match &node.content {
            NodeContent::Quote => context.quoted = true,
            NodeContent::Admonition { kind, title } if context.admonition_kind.is_none() => {
                context.admonition_kind = Some(kind.clone());
                context.admonition_title = title.clone();
            }
            _ => {}
        }
        parent = node.parent;
    }
    context
}

fn resolved_page(structure: &DocumentStructure, index: usize) -> Option<u32> {
    let node = structure.nodes.get(index)?;
    if let Some(page) = node.page {
        return Some(page);
    }
    if let NodeContent::Slide { number, .. } = &node.content {
        return Some(*number);
    }
    let mut parent = node.parent;
    for _ in 0..MAX_ANCESTOR_DEPTH {
        let NodeIndex(parent_index) = parent?;
        let parent_node = structure.nodes.get(parent_index as usize)?;
        if let Some(page) = parent_node.page {
            return Some(page);
        }
        if let NodeContent::Slide { number, .. } = &parent_node.content {
            return Some(*number);
        }
        parent = parent_node.parent;
    }
    None
}

fn is_adjacent_duplicate(previous: Option<&NormalizedBlock>, next: &NormalizedBlock) -> bool {
    previous.is_some_and(|previous| {
        previous.role == next.role
            && previous.text == next.text
            && previous.asset_ids == next.asset_ids
            && previous.heading_level == next.heading_level
    })
}

fn authored_page_format(format: DocumentFormat) -> bool {
    presentation_format(format) || spreadsheet_format(format)
}

fn presentation_format(format: DocumentFormat) -> bool {
    matches!(
        format,
        DocumentFormat::Ppt | DocumentFormat::Pptx | DocumentFormat::Odp
    )
}

fn spreadsheet_format(format: DocumentFormat) -> bool {
    matches!(format, DocumentFormat::Excel | DocumentFormat::Ods)
}

#[cfg(test)]
mod tests {
    use super::*;
    use xberg::types::document_structure::{DocumentNode, NodeContent, TableGrid};

    #[test]
    fn empty_container_nodes_are_not_reported_as_degradation() {
        let mut document = ExtractedDocument::default();
        document.document = Some(DocumentStructure {
            nodes: vec![DocumentNode {
                id: "list".to_string(),
                content: NodeContent::List { ordered: false },
                parent: None,
                children: Vec::new(),
                content_layer: ContentLayer::Body,
                page: None,
                page_end: None,
                bbox: None,
                annotations: Vec::new(),
                attributes: None,
            }],
            source_format: Some("docx".to_string()),
            relationships: Vec::new(),
            node_types: Vec::new(),
        });
        document.content = "fallback".to_string();

        let normalized =
            normalize_document(&document, DocumentFormat::Docx, &HashMap::new()).unwrap();
        assert_eq!(normalized.skipped_node_count, 0);
        assert_eq!(normalized.units.len(), 1);
    }

    #[test]
    fn invalid_table_dimensions_are_rejected_before_canonicalization() {
        let mut document = ExtractedDocument::default();
        document.document = Some(DocumentStructure {
            nodes: vec![DocumentNode {
                id: "table".to_string(),
                content: NodeContent::Table {
                    grid: TableGrid {
                        rows: 100_001,
                        cols: 2,
                        cells: Vec::new(),
                    },
                },
                parent: None,
                children: Vec::new(),
                content_layer: ContentLayer::Body,
                page: None,
                page_end: None,
                bbox: None,
                annotations: Vec::new(),
                attributes: None,
            }],
            source_format: Some("docx".to_string()),
            relationships: Vec::new(),
            node_types: Vec::new(),
        });

        assert_eq!(
            normalize_document(&document, DocumentFormat::Docx, &HashMap::new())
                .unwrap_err()
                .code,
            "document_table_too_large"
        );
    }
}
