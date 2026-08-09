#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const path = fileURLToPath(new URL("../src-tauri/src/ai/document_intelligence.rs", import.meta.url));
let source = await readFile(path, "utf8");
const original = source;
const oldText = `        assert_eq!(
            outline_completion_tokens_for_provider(ProviderKind::Deepseek),
            6_144
        );`;
const newText = `        assert_eq!(
            outline_completion_tokens_for_provider(ProviderKind::Deepseek),
            3_072
        );`;
if (!source.includes(newText)) {
  if (!source.includes(oldText)) throw new Error("Could not find stale DeepSeek outline allowance assertion");
  source = source.replace(oldText, newText);
}
if (source !== original) await writeFile(path, source, "utf8");
