#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const applicatorPath = join(root, "scripts", "apply-platform-p2-atomic-frontend.mjs");
let source = await readFile(applicatorPath, "utf8");
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
await writeFile(applicatorPath, source, "utf8");
await import(new URL("./apply-platform-p2-atomic-frontend.mjs", import.meta.url));

const templateTestPath = join(root, "src", "screens", "TemplatesScreen.assets.test.tsx");
let templateTest = await readFile(templateTestPath, "utf8");
const oldTemplateAssertions = `      expect(durableDraftMock.flush).toHaveBeenCalled();
      expect(durableDraftMock.acceptCanonicalCommit).toHaveBeenCalledWith(savedTemplate().id);`;
const newTemplateAssertions = `      expect(durableDraftMock.prepareCanonicalCommit).toHaveBeenCalled();
      expect(durableDraftMock.acceptCanonicalCommit).toHaveBeenCalledOnce();`;
if (!templateTest.includes(oldTemplateAssertions)) {
  throw new Error("The stale template atomic-commit assertions were not found.");
}
templateTest = templateTest.replace(oldTemplateAssertions, newTemplateAssertions);
await writeFile(templateTestPath, templateTest, "utf8");

const bulkTestPath = join(root, "src", "screens", "cards", "BulkEntryModal.test.tsx");
let bulkTest = await readFile(bulkTestPath, "utf8");
const oldBulkExpectation = `      expect(cardsIpc.bulkCreateCards).toHaveBeenCalledWith({
        deckId: deck.id,
        rows: expectedRows,
      });`;
const newBulkExpectation = `      expect(cardsIpc.bulkCreateCards).toHaveBeenCalledWith({
        deckId: deck.id,
        rows: expectedRows,
        editorDraft: { draftId: "d".repeat(32), revision: 1 },
      });`;
if (!bulkTest.includes(oldBulkExpectation)) {
  throw new Error("The stale bulk-entry canonical-write expectation was not found.");
}
bulkTest = bulkTest.replace(oldBulkExpectation, newBulkExpectation);
await writeFile(bulkTestPath, bulkTest, "utf8");

console.log("Finished atomic frontend expectation migration.");
