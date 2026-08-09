#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const path = fileURLToPath(new URL("../src-tauri/src/document_more.rs", import.meta.url));
let source = await readFile(path, "utf8");
const original = source;

const oldChunks = `        statement
            .query_map([document_id], |row| {
                let pages_json: String = row.get(3)?;
                let types_json: String = row.get(4)?;
                let token_estimate: i64 = row.get(6)?;
                let page_numbers = serde_json::from_str(&pages_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                let element_types = serde_json::from_str(&types_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(SemanticChunk {
                    chunk_index: row.get(0)?,
                    text: row.get(1)?,
                    full_text: row.get(2)?,
                    page_numbers,
                    element_types,
                    heading_context: row.get(5)?,
                    token_estimate: usize::try_from(token_estimate).unwrap_or(usize::MAX),
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)
`;
const newChunks = `        let chunks = statement
            .query_map([document_id], |row| {
                let pages_json: String = row.get(3)?;
                let types_json: String = row.get(4)?;
                let token_estimate: i64 = row.get(6)?;
                let page_numbers = serde_json::from_str(&pages_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                let element_types = serde_json::from_str(&types_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(SemanticChunk {
                    chunk_index: row.get(0)?,
                    text: row.get(1)?,
                    full_text: row.get(2)?,
                    page_numbers,
                    element_types,
                    heading_context: row.get(5)?,
                    token_estimate: usize::try_from(token_estimate).unwrap_or(usize::MAX),
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        Ok(chunks)
`;

const oldPages = `        statement
            .query_map([document_id], |row| {
                Ok(PageRenderRecord {
                    page_number: row.get(0)?,
                    render_relative_path: row.get(1)?,
                    render_byte_size: row.get(2)?,
                    render_sha256: row.get(3)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)
`;
const newPages = `        let pages = statement
            .query_map([document_id], |row| {
                Ok(PageRenderRecord {
                    page_number: row.get(0)?,
                    render_relative_path: row.get(1)?,
                    render_byte_size: row.get(2)?,
                    render_sha256: row.get(3)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        Ok(pages)
`;

if (source.includes(oldChunks)) source = source.replace(oldChunks, newChunks);
if (source.includes(oldPages)) source = source.replace(oldPages, newPages);

if (!source.includes("let chunks = statement") || !source.includes("let pages = statement")) {
  throw new Error("Could not apply document-more lifetime corrections");
}

if (source !== original) await writeFile(path, source, "utf8");
