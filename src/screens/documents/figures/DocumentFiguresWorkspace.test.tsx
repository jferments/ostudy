import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as annotationIpc from "../../../ipc/documentAnnotations";
import * as figureIpc from "../../../ipc/documentFigures";
import type { DocumentFigure, DocumentFigures } from "../../../ipc/documentFigures";
import { OverlayProvider } from "../../../ui/overlays/OverlayProvider";
import { DocumentFiguresWorkspace } from "./DocumentFiguresWorkspace";

vi.mock("../../../ipc/documentAnnotations", () => ({ renderDocumentPageTile: vi.fn() }));
vi.mock("../../../ipc/documentFigures", () => ({
  getDocumentFigures: vi.fn(),
  findDocumentFigures: vi.fn(),
  setDocumentFiguresUseImages: vi.fn(),
  setDocumentFigureHidden: vi.fn(),
}));
vi.mock("../../library/LibraryFrame", () => ({ useLibraryFullPage: vi.fn() }));

const documentId = "d".repeat(32);

function figure(overrides: Partial<DocumentFigure> = {}): DocumentFigure {
  return {
    figureId: "p00002-01",
    pageNumber: 2,
    pageWidth: 612,
    pageHeight: 792,
    kind: "image",
    bounds: { left: 72, top: 100, width: 300, height: 200 },
    captionBounds: { left: 72, top: 306, width: 300, height: 22 },
    label: "Figure 3.2",
    caption: "Figure 3.2 The four chambers of the heart.",
    hidden: false,
    passages: [
      { pageNumber: 5, kind: "mention", excerpt: "As Figure 3.2 shows, the right atrium fills first." },
      { pageNumber: 1, kind: "neighbour", excerpt: null },
      { pageNumber: 3, kind: "neighbour", excerpt: null },
    ],
    ...overrides,
  };
}

function figures(overrides: Partial<DocumentFigures> = {}): DocumentFigures {
  return { documentId, useImages: true, state: "found", scannedAt: 1, figures: [figure()], ...overrides };
}

function renderWorkspace() {
  const onClose = vi.fn();
  const showToast = vi.fn();
  render(
    <OverlayProvider>
      <DocumentFiguresWorkspace documentId={documentId} title="Cardiac physiology" onClose={onClose} showToast={showToast} />
    </OverlayProvider>,
  );
  return { onClose, showToast };
}

describe("DocumentFiguresWorkspace", () => {
  beforeEach(() => {
    vi.mocked(annotationIpc.renderDocumentPageTile).mockResolvedValue(new ArrayBuffer(8));
    URL.createObjectURL = vi.fn(() => "blob:figure");
    URL.revokeObjectURL = vi.fn();
  });
  afterEach(() => vi.clearAllMocks());

  it("shows each figure drawn from its page with its label, caption and explaining pages", async () => {
    vi.mocked(figureIpc.getDocumentFigures).mockResolvedValue(figures());
    renderWorkspace();

    const card = await screen.findByRole("listitem", { name: /Figure 3\.2/u });
    expect(within(card).getByText("· Page 2", { exact: false })).toBeTruthy();
    expect(await within(card).findByRole("img", { name: "Figure 3.2 The four chambers of the heart." })).toBeTruthy();
    expect(within(card).getByText("As Figure 3.2 shows, the right atrium fills first.")).toBeTruthy();
    expect(within(card).getByText("Explained on pages 1 and 3")).toBeTruthy();
    // The caption is drawn into the picture by default, so it is not repeated as text.
    expect(annotationIpc.renderDocumentPageTile).toHaveBeenCalledWith(documentId, expect.objectContaining({ pageNumber: 2 }));
    const withCaption = vi.mocked(annotationIpc.renderDocumentPageTile).mock.calls[0]?.[1];

    fireEvent.click(screen.getByRole("checkbox", { name: "Show captions in the picture" }));
    expect(await within(card).findByText("Figure 3.2 The four chambers of the heart.")).toBeTruthy();
    await waitFor(() => expect(annotationIpc.renderDocumentPageTile).toHaveBeenCalledTimes(2));
    const withoutCaption = vi.mocked(annotationIpc.renderDocumentPageTile).mock.calls[1]?.[1];
    expect(withoutCaption?.height).toBeLessThan(withCaption?.height ?? 0);
  });

  it("hides a figure and lists it under Hidden", async () => {
    vi.mocked(figureIpc.getDocumentFigures).mockResolvedValue(figures());
    vi.mocked(figureIpc.setDocumentFigureHidden).mockResolvedValue(figures({ figures: [figure({ hidden: true })] }));
    renderWorkspace();

    fireEvent.click(await screen.findByRole("button", { name: "Hide Figure 3.2 on page 2" }));
    await waitFor(() => expect(figureIpc.setDocumentFigureHidden).toHaveBeenCalledWith(documentId, "p00002-01", true));
    expect(await screen.findByText("0 figures, 1 hidden")).toBeTruthy();
    expect(screen.getByText("Hidden (1)")).toBeTruthy();
  });

  it("finds figures in a document that was never searched", async () => {
    vi.mocked(figureIpc.getDocumentFigures).mockResolvedValue(figures({ state: "none", scannedAt: null, figures: [] }));
    vi.mocked(figureIpc.findDocumentFigures).mockResolvedValue(figures());
    renderWorkspace();

    fireEvent.click(await screen.findByRole("button", { name: "Find figures" }));
    expect(await screen.findByRole("listitem", { name: /Figure 3\.2/u })).toBeTruthy();
    expect(figureIpc.findDocumentFigures).toHaveBeenCalledWith(documentId);
  });

  it("turns images back on and searches when they were off", async () => {
    vi.mocked(figureIpc.getDocumentFigures).mockResolvedValue(figures({ useImages: false, state: "pending", figures: [] }));
    vi.mocked(figureIpc.setDocumentFiguresUseImages).mockResolvedValue(figures({ state: "pending", figures: [] }));
    vi.mocked(figureIpc.findDocumentFigures).mockResolvedValue(figures());
    renderWorkspace();

    fireEvent.click(await screen.findByRole("button", { name: "Use images" }));
    expect(await screen.findByRole("listitem", { name: /Figure 3\.2/u })).toBeTruthy();
    expect(figureIpc.setDocumentFiguresUseImages).toHaveBeenCalledWith(documentId, true);
  });

  it("says when this device cannot search for figures", async () => {
    vi.mocked(figureIpc.getDocumentFigures).mockResolvedValue(figures({ state: "unsupported", figures: [] }));
    renderWorkspace();
    expect(await screen.findByText("Figures are found on desktop")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Find figures" })).toBeNull();
  });

  it("returns to the documents", async () => {
    vi.mocked(figureIpc.getDocumentFigures).mockResolvedValue(figures());
    const { onClose } = renderWorkspace();
    fireEvent.click(await screen.findByRole("button", { name: "Back to documents" }));
    expect(onClose).toHaveBeenCalled();
  });
});
