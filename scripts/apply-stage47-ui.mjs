#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

async function patch(relativePath, transform) {
  const path = fileURLToPath(new URL(`../${relativePath}`, import.meta.url));
  const source = await readFile(path, "utf8");
  const updated = transform(source);
  if (updated !== source) await writeFile(path, updated, "utf8");
}

function required(source, label, search, replacement) {
  if (source.includes(replacement)) return source;
  if (!source.includes(search)) throw new Error(`Could not find ${label}`);
  return source.replace(search, replacement);
}

await patch("src/screens/DocumentsScreen.tsx", (source) => {
  source = required(
    source,
    "document scale metric",
    `                    <div>
                      <dt>Cards</dt>
                      <dd>{document.generatedCardCount}</dd>
                    </div>`,
    `                    <div>
                      <dt>Mode</dt>
                      <dd>
                        {document.scaleMode === "large" ? "Large document" : "Standard"}
                        {document.estimatedSourceTokens > 0
                          ? ` · ~${formatCompactNumber(document.estimatedSourceTokens)} source tokens`
                          : ""}
                      </dd>
                    </div>
                    <div>
                      <dt>AI work</dt>
                      <dd>{formatAiProgress(document)}</dd>
                    </div>
                    <div>
                      <dt>Cards</dt>
                      <dd>{document.generatedCardCount}</dd>
                    </div>`,
  );
  source = required(
    source,
    "retry checkpoint note",
    `                  {document.safeErrorMessage ? (
                    <p className={styles.documentError}>{document.safeErrorMessage}</p>
                  ) : null}`,
    `                  {document.lifecycleState !== "processing" &&
                  document.generatedDeckId === null &&
                  document.outlineCompletedWindowCount + document.generationCompletedWindowCount > 0 ? (
                    <p className={styles.deckNote}>
                      Retry will reuse {document.outlineCompletedWindowCount +
                        document.generationCompletedWindowCount} completed AI source window
                      {document.outlineCompletedWindowCount +
                        document.generationCompletedWindowCount ===
                      1
                        ? ""
                        : "s"}.
                    </p>
                  ) : null}
                  {document.safeErrorMessage ? (
                    <p className={styles.documentError}>{document.safeErrorMessage}</p>
                  ) : null}`,
  );
  source = required(
    source,
    "segmented processing phase",
    `function processingPhaseLabel(document: DocumentSummary, canceling: boolean): string {
  if (canceling) return "Stopping";
  if (document.generatedDeckId !== null) return "Generating additional cards";
  if (document.pageCount === 0) return "Extracting text";
  if (document.renderedPageCount < document.pageCount) return "Rendering pages";
  return "Building outline / generating cards";
}`,
    `function processingPhaseLabel(document: DocumentSummary, canceling: boolean): string {
  if (canceling) return "Stopping";
  if (document.pageCount === 0) return "Extracting text";
  if (document.renderedPageCount < document.pageCount) return "Rendering pages";
  if (
    document.outlineWindowCount > 0 &&
    document.outlineCompletedWindowCount < document.outlineWindowCount
  ) {
    return `Analyzing source ${document.outlineCompletedWindowCount}/${document.outlineWindowCount}`;
  }
  if (
    document.generationWindowCount > 0 &&
    document.generationCompletedWindowCount < document.generationWindowCount
  ) {
    const prefix = document.generatedDeckId !== null ? "Generating additional cards" : "Generating cards";
    return `${prefix} ${document.generationCompletedWindowCount}/${document.generationWindowCount}`;
  }
  return document.generatedDeckId !== null ? "Finalizing additional cards" : "Finalizing generated deck";
}

function formatAiProgress(document: DocumentSummary): string {
  if (document.pageCount === 0) return "Not planned";
  if (document.outlineWindowCount === 0 && document.generationWindowCount === 0) {
    return document.generatedDeckId ? "Complete" : "Pending";
  }
  return `${document.outlineCompletedWindowCount}/${document.outlineWindowCount} analysis · ${document.generationCompletedWindowCount}/${document.generationWindowCount} generation`;
}

function formatCompactNumber(value: number): string {
  return new Intl.NumberFormat(undefined, {
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(value);
}`,
  );
  return source;
});

await patch("src/screens/DocumentsScreen.test.tsx", (source) => {
  source = required(
    source,
    "document progress fixture fields",
    `    renderedPageCount: 10,
    generatedNoteCount: 6,`,
    `    renderedPageCount: 10,
    scaleMode: "standard",
    estimatedSourceTokens: 12_000,
    outlineWindowCount: 2,
    outlineCompletedWindowCount: 2,
    generationWindowCount: 3,
    generationCompletedWindowCount: 3,
    generatedNoteCount: 6,`,
  );
  const marker = `  it("offers an exact stop action while a document generation task is processing", async () => {`;
  if (!source.includes("shows segmented large-document AI progress and resumable work")) {
    if (!source.includes(marker)) throw new Error("Could not find DocumentsScreen stop test marker");
    source = source.replace(
      marker,
      `  it("shows segmented large-document AI progress and resumable work", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({
        lifecycleState: "processing",
        pageCount: 138,
        renderedPageCount: 138,
        scaleMode: "large",
        estimatedSourceTokens: 180_000,
        outlineWindowCount: 42,
        outlineCompletedWindowCount: 17,
        generationWindowCount: 55,
        generationCompletedWindowCount: 0,
        generatedDeckId: null,
        generatedCardCount: 0,
        completedAt: null,
      }),
    ]);
    renderDocuments();

    expect(await screen.findByText("Analyzing source 17/42")).toBeTruthy();
    expect(screen.getByText(/Large document/u)).toBeTruthy();
    expect(screen.getByText(/17\/42 analysis/u)).toBeTruthy();
  });

  it("shows reusable checkpoints after a large-document provider failure", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({
        lifecycleState: "failed",
        pageCount: 138,
        renderedPageCount: 138,
        scaleMode: "large",
        outlineWindowCount: 42,
        outlineCompletedWindowCount: 42,
        generationWindowCount: 55,
        generationCompletedWindowCount: 31,
        generatedDeckId: null,
        generatedCardCount: 0,
        safeErrorCode: "ai_document_output_truncated",
        safeErrorMessage: "One bounded generation window reached its completion limit.",
      }),
    ]);
    renderDocuments();

    expect(await screen.findByText(/Retry will reuse 73 completed AI source windows/u)).toBeTruthy();
  });

${marker}`,
    );
  }
  return source;
});
