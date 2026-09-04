import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as documents from "../ipc/documents";
import { SourceCitationPanel } from "./SourceCitationPanel";

vi.mock("../ipc/documents", () => ({
  listDeckSourceCitations: vi.fn(),
  getNoteSourceCitation: vi.fn(),
}));

const citation: documents.SourceCitation = {
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
  sourceLocator: {
    unitKind: "page",
    unitNumbers: [8, 9],
    sectionLabel: "Distributive shock",
  },
};

describe("SourceCitationPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(documents.listDeckSourceCitations).mockResolvedValue([]);
    vi.mocked(documents.getNoteSourceCitation).mockResolvedValue(null);
  });

  it("renders a deck citation and exact source locator", async () => {
    vi.mocked(documents.listDeckSourceCitations).mockResolvedValue([citation]);

    render(<SourceCitationPanel deckId={"d".repeat(32)} />);

    expect(await screen.findByRole("region", { name: "Source" })).toBeTruthy();
    expect(screen.getByText("Shock physiology")).toBeTruthy();
    expect(screen.getByText("PDF · shock.pdf")).toBeTruthy();
    expect(screen.getByText("Distributive shock · page 8, 9")).toBeTruthy();
    expect(documents.listDeckSourceCitations).toHaveBeenCalledWith("d".repeat(32));
  });

  it("keeps a removed source visibly attributable", async () => {
    vi.mocked(documents.getNoteSourceCitation).mockResolvedValue({
      ...citation,
      sourceRemoved: true,
    });

    render(<SourceCitationPanel noteId={"e".repeat(32)} compact />);

    expect(await screen.findByText("Source removed · citation preserved")).toBeTruthy();
    expect(documents.getNoteSourceCitation).toHaveBeenCalledWith("e".repeat(32));
  });

  it("stays absent when an ordinary deck has no document source", async () => {
    const { container } = render(<SourceCitationPanel deckId={"f".repeat(32)} />);

    await vi.waitFor(() =>
      expect(documents.listDeckSourceCitations).toHaveBeenCalledWith("f".repeat(32)),
    );
    expect(container.querySelector("[data-source-citation-panel]")).toBeNull();
  });
});
