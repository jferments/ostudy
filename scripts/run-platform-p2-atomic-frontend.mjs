#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const path = join(root, "scripts", "apply-platform-p2-atomic-frontend.mjs");
let source = await readFile(path, "utf8");
const original = `    for (const indent of ["    ", "  "]) {
      const anchor = \`${"${indent}"}commit: vi.fn().mockResolvedValue(undefined),\\n\`;
      const replacement = \`${"${indent}"}prepareCanonicalCommit: vi.fn().mockResolvedValue({ draftId: "d".repeat(32), revision: 1 }),\\n${"${indent}"}acceptCanonicalCommit: vi.fn(),\\n${"${anchor}"}\`;
      if (text.includes(anchor)) text = text.replaceAll(anchor, replacement);
    }`;
const replacement = `    for (const indent of ["    ", "  "]) {
      const anchor = \`${"${indent}"}commit: vi.fn().mockResolvedValue(undefined),\\n\`;
      const replacement = \`${"${indent}"}prepareCanonicalCommit: vi.fn().mockResolvedValue({ draftId: "d".repeat(32), revision: 1 }),\\n${"${indent}"}acceptCanonicalCommit: vi.fn(),\\n${"${anchor}"}\`;
      if (text.includes(anchor)) {
        text = text.replaceAll(anchor, replacement);
        break;
      }
    }`;
if (!source.includes(original)) {
  throw new Error("The atomic frontend test-mock migration loop was not found.");
}
source = source.replace(original, replacement);
await writeFile(path, source, "utf8");
await import(new URL("./apply-platform-p2-atomic-frontend.mjs", import.meta.url));
