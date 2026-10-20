import { describe, expect, it } from "vitest";

import type { SourceCitation } from "../ipc/documents";
import { sourcePassageFor } from "./sourcePassage";

const citation: SourceCitation = {
  citationId: "a".repeat(32),
  schemaVersion: 1,
  sourceDocumentId: "b".repeat(32),
  sourceIdentity: `sha256:${"c".repeat(64)}`,
  sourceFingerprintSha256: "c".repeat(64),
  sourceTitle: "Shock physiology",
  originalFilename: "shock.pdf",
  sourceFormat: "pdf",
  sourceMediaType: "application/pdf",
  sourceUnitKind: "page",
  sourceUnitCount: 24,
  sourceRemoved: false,
  sourceLocator: { unitKind: "page", unitNumbers: [9, 8] },
};

describe("sourcePassageFor", () => {
  it("opens a PDF citation at its first cited page", () => {
    expect(sourcePassageFor(citation)).toEqual({ documentId: "b".repeat(32), title: "Shock physiology", pageNumber: 8 });
  });

  it("leaves citations that cannot open a page as text", () => {
    expect(sourcePassageFor(null)).toBeNull();
    expect(sourcePassageFor({ ...citation, sourceRemoved: true })).toBeNull();
    expect(sourcePassageFor({ ...citation, sourceLocator: null })).toBeNull();
    expect(sourcePassageFor({ ...citation, sourceFormat: "docx", sourceUnitKind: "section",
      sourceLocator: { unitKind: "section", unitNumbers: [2] } })).toBeNull();
    expect(sourcePassageFor({ ...citation, sourceLocator: { unitKind: "page", unitNumbers: [] } })).toBeNull();
  });
});
