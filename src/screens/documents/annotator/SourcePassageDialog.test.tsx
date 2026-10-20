import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as annotationIpc from "../../../ipc/documentAnnotations";
import * as studyGuideIpc from "../../../ipc/studyGuides";
import { LayoutModeProvider } from "../../../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../../../ui/overlays/OverlayProvider";
import { SourcePassageDialog } from "./SourcePassageDialog";

vi.mock("../../../ipc/documentAnnotations", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../../ipc/documentAnnotations")>(),
  getDocumentAnnotations: vi.fn(),
  applyDocumentAnnotationChanges: vi.fn(),
  renderDocumentPageTile: vi.fn(),
  getDocumentPageText: vi.fn(),
}));
vi.mock("../../../ipc/studyGuides", () => ({ readDocumentSourceAsset: vi.fn() }));

const documentId = "a".repeat(32);
const pages: annotationIpc.AnnotatedPage[] = [1, 2, 3].map((pageNumber) => ({
  pageNumber, assetId: `${pageNumber}`.repeat(32), width: 612, height: 792, imageAvailable: true,
}));
// Pages are stacked 20 CSS pixels apart below a 16 pixel gutter, at one pixel per point.
const PAGE_GAP = 20;

beforeEach(() => {
  vi.mocked(annotationIpc.getDocumentAnnotations).mockResolvedValue({ documentId, pages, annotations: [] });
  vi.mocked(annotationIpc.applyDocumentAnnotationChanges).mockResolvedValue({ annotations: [], deletedIds: [] });
  vi.mocked(studyGuideIpc.readDocumentSourceAsset).mockResolvedValue(new ArrayBuffer(8));
  vi.mocked(annotationIpc.renderDocumentPageTile).mockResolvedValue(new ArrayBuffer(8));
  vi.mocked(annotationIpc.getDocumentPageText).mockRejectedValue(new Error("This page has no text."));
  Object.defineProperty(URL, "createObjectURL", { configurable: true, writable: true, value: vi.fn(() => "blob:page") });
  Object.defineProperty(URL, "revokeObjectURL", { configurable: true, writable: true, value: vi.fn() });
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(612);
  vi.spyOn(HTMLElement.prototype, "offsetTop", "get").mockImplementation(function offsetTop(this: HTMLElement) {
    const pageNumber = Number(this.dataset.annotatorPage);
    return pageNumber > 0 ? 16 + (pageNumber - 1) * (792 + PAGE_GAP) : 0;
  });
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

function openPassage(onClose = vi.fn()) {
  render(
    <LayoutModeProvider>
      <OverlayProvider>
        <SourcePassageDialog
          passage={{ documentId, title: "Shock physiology", pageNumber: 2 }}
          onClose={onClose}
          showToast={vi.fn()}
        />
      </OverlayProvider>
    </LayoutModeProvider>,
  );
  return onClose;
}

describe("source passage", () => {
  it("opens the document at the cited page and points it out", async () => {
    openPassage();

    expect(await screen.findByRole("dialog", { name: "Shock physiology" })).toBeTruthy();
    const canvas = await screen.findByRole("group", { name: "Pages of Shock physiology" });
    const cited = screen.getByRole("group", { name: "Page 2" });
    expect(canvas.scrollTop).toBe(16 + 792 + PAGE_GAP - 8);
    expect(cited.dataset.marked).toBe("true");
    expect(screen.getByText("Opened at page 2")).toBeTruthy();
    expect(screen.getByRole("group", { name: "Page 1" }).dataset.marked).toBeUndefined();
    // The outline fades once the learner has found the page.
    await waitFor(() => expect(cited.dataset.marked).toBeUndefined(), { timeout: 4000 });
  });

  it("closes back to the card", async () => {
    const onClose = openPassage();
    await screen.findByRole("toolbar", { name: "Annotation tools" });

    fireEvent.click(screen.getByRole("button", { name: "Close dialog" }));

    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  });
});
