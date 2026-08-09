#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const path = fileURLToPath(new URL("../src-tauri/src/document_semantics.rs", import.meta.url));
let source = await readFile(path, "utf8");
const original = source;

const oldChunk = `pub(crate) struct SemanticChunk {
    pub chunk_index: i64,
    pub text: String,
    pub full_text: String,
    pub element_types: Vec<String>,
    pub heading_context: Option<String>,
}`;
const newChunk = `pub(crate) struct SemanticChunk {
    pub chunk_index: i64,
    pub text: String,
    pub full_text: String,
    pub page_numbers: Vec<i64>,
    pub element_types: Vec<String>,
    pub heading_context: Option<String>,
    pub token_estimate: usize,
}`;
if (!source.includes(newChunk)) {
  if (!source.includes(oldChunk)) throw new Error("Could not find SemanticChunk definition");
  source = source.replace(oldChunk, newChunk);
}

const oldPartition = `pub(crate) struct SemanticPartition {
    pub partition_index: usize,
    pub title: String,
    pub page_numbers: Vec<i64>,
    pub page_start: i64,
    pub token_estimate: usize,
    chunk_start: usize,
    chunk_end: usize,
}`;
const newPartition = `pub(crate) struct SemanticPartition {
    pub partition_index: usize,
    pub title: String,
    pub page_start: i64,
    chunk_start: usize,
    chunk_end: usize,
}`;
if (!source.includes(newPartition)) {
  if (!source.includes(oldPartition)) throw new Error("Could not find SemanticPartition definition");
  source = source.replace(oldPartition, newPartition);
}

if (source !== original) await writeFile(path, source, "utf8");
