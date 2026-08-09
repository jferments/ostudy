#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const path = fileURLToPath(new URL("../src-tauri/src/document_semantics.rs", import.meta.url));
let source = await readFile(path, "utf8");
const original = source;

source = source.replace("    pub page_start: i64,\n", "");
source = source.replace("            page_start,\n", "");
source = source.replace("            token_total = effective_chunk_tokens(chunk);\n", "");

const oldAssertion = `        assert_eq!(partitions[1].title, "Chapter 2");
        assert_eq!(partitions[1].page_start, 7);`;
const newAssertion = `        assert_eq!(partitions[1].title, "Chapter 2");
        let windows = build_partitioned_semantic_windows(&chunks, plan, 2_000, 4, 0);
        assert!(windows
            .iter()
            .any(|window| window.partition_index == 1 && window.page_start == 7));`;
if (source.includes(oldAssertion)) source = source.replace(oldAssertion, newAssertion);

if (source !== original) await writeFile(path, source, "utf8");
