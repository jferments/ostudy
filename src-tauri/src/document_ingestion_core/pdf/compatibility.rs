use rusqlite::params;

use crate::{
    error::CommandError,
    pdf_document_ingestion::IngestedDocument,
};

use super::super::support::{normalize_semantic_text, sha256_hex, to_i64};

pub(crate) fn clear_pdf_compatibility_derivatives(
    transaction: &rusqlite::Transaction<'_>,
    document_id: &str,
) -> Result<(), CommandError> {
    for table in [
        "document_pages",
        "document_semantic_elements",
        "document_semantic_chunks",
        "document_outlines",
        "document_outline_checkpoints",
        "document_generation_checkpoints",
    ] {
        transaction
            .execute(
                &format!("DELETE FROM {table} WHERE document_id = ?1"),
                [document_id],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_ingestion_clear_derivatives_failed",
                    format!("document={document_id} table={table}"),
                    error,
                )
            })?;
    }
    Ok(())
}

pub(crate) fn persist_pdf_compatibility_projection(
    transaction: &rusqlite::Transaction<'_>,
    document_id: &str,
    processed: &IngestedDocument,
    now: i64,
) -> Result<(), CommandError> {
    for page in &processed.pages {
        transaction
            .execute(
                "INSERT INTO document_pages (
                     document_id, page_number, plain_text, text_sha256,
                     render_relative_path, render_mime_type, render_byte_size,
                     render_sha256, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 'image/png', ?6, ?7, ?8, ?8)",
                params![
                    document_id,
                    page.unit_number,
                    page.plain_text,
                    page.text_sha256,
                    page.render_relative_path,
                    page.render_byte_size,
                    page.render_sha256,
                    now,
                ],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_ingestion_page_insert_failed",
                    format!("document={document_id} unit={}", page.unit_number),
                    error,
                )
            })?;
    }

    for element in &processed.semantic.elements {
        let text = normalize_semantic_text(&element.text, 250_000);
        transaction
            .execute(
                "INSERT INTO document_semantic_elements (
                     document_id, element_index, page_number, element_type,
                     text, parent_heading, heading_path_json,
                     bbox_x, bbox_y, bbox_width, bbox_height, confidence,
                     font_name, font_size, is_bold, is_italic,
                     text_sha256, created_at
                 ) VALUES (
                     ?1, ?2, ?3, ?4, ?5, ?6, ?7,
                     ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18
                 )",
                params![
                    document_id,
                    to_i64(element.element_index),
                    to_i64(element.page_number),
                    normalize_element_type(&element.element_type)?,
                    text,
                    element.parent_heading,
                    serde_json::to_string(&element.heading_path)
                        .map_err(CommandError::serialization)?,
                    element.bbox.x,
                    element.bbox.y,
                    element.bbox.width.max(0.0),
                    element.bbox.height.max(0.0),
                    element.confidence.clamp(0.0, 1.0),
                    element.font_name,
                    element.font_size,
                    i64::from(element.is_bold),
                    i64::from(element.is_italic),
                    sha256_hex(text.as_bytes()),
                    now,
                ],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_ingestion_element_insert_failed",
                    format!("document={document_id} element={}", element.element_index),
                    error,
                )
            })?;
    }

    for chunk in &processed.semantic.chunks {
        let text = normalize_semantic_text(&chunk.text, 1_000_000);
        let full_text = normalize_semantic_text(&chunk.full_text, 1_100_000);
        let unit_numbers = chunk
            .page_numbers
            .iter()
            .map(|unit| to_i64(*unit))
            .collect::<Vec<_>>();
        transaction
            .execute(
                "INSERT INTO document_semantic_chunks (
                     document_id, chunk_index, text, full_text,
                     page_numbers_json, bounding_boxes_json, element_types_json,
                     heading_context, token_estimate, is_oversized,
                     text_sha256, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    document_id,
                    to_i64(chunk.chunk_index),
                    text,
                    full_text,
                    serde_json::to_string(&unit_numbers).map_err(CommandError::serialization)?,
                    serde_json::to_string(&chunk.bounding_boxes)
                        .map_err(CommandError::serialization)?,
                    serde_json::to_string(&chunk.element_types)
                        .map_err(CommandError::serialization)?,
                    chunk.heading_context,
                    to_i64(chunk.token_estimate),
                    i64::from(chunk.is_oversized),
                    sha256_hex(text.as_bytes()),
                    now,
                ],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_ingestion_chunk_insert_failed",
                    format!("document={document_id} chunk={}", chunk.chunk_index),
                    error,
                )
            })?;
    }
    Ok(())
}

fn normalize_element_type(value: &str) -> Result<&'static str, CommandError> {
    match value.to_ascii_lowercase().replace('-', "_").as_str() {
        "title" => Ok("title"),
        "paragraph" => Ok("paragraph"),
        "table" => Ok("table"),
        "header" => Ok("header"),
        "footer" => Ok("footer"),
        "listitem" | "list_item" => Ok("list_item"),
        "image" => Ok("image"),
        "codeblock" | "code_block" => Ok("code_block"),
        "keyvalue" | "key_value" => Ok("key_value"),
        _ => Err(CommandError::new(
            "document_semantic_element_unsupported",
            "The extraction adapter returned an unsupported semantic element type.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_element_type_mapping_remains_bounded() {
        assert_eq!(normalize_element_type("list-item").unwrap(), "list_item");
        assert_eq!(normalize_element_type("keyValue").unwrap(), "key_value");
        assert_eq!(
            normalize_element_type("unsupported").unwrap_err().code,
            "document_semantic_element_unsupported"
        );
    }
}
