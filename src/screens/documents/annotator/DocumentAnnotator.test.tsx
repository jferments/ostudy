import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as annotationCardsIpc from "../../../ipc/annotationCards";
import * as annotationIpc from "../../../ipc/documentAnnotations";
import * as studyGuideIpc from "../../../ipc/studyGuides";
import { layoutWidths, matchesLayoutQuery } from "../../../test/viewport";
import { LayoutModeProvider } from "../../../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../../../ui/overlays/OverlayProvider";
import { DocumentAnnotator } from "./DocumentAnnotator";

vi.mock("../../../ipc/documentAnnotations", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../../ipc/documentAnnotations")>(),
  getDocumentAnnotations: vi.fn(),
  applyDocumentAnnotationChanges: vi.fn(),
  renderDocumentPageTile: vi.fn(),
  getDocumentPageText: vi.fn(),
}));
vi.mock("../../../ipc/studyGuides", () => ({ readDocumentSourceAsset: vi.fn() }));
vi.mock("../../../ipc/annotationCards", () => ({ getDocumentMarkedCards: vi.fn(), linkCardNoteToAnnotation: vi.fn() }));
// The card editor has its own tests; here it shows what it was given and saves on a click.
vi.mock("./MarkCardEditor", () => ({
  MarkCardEditor: ({ mark, suggestedDeckId, onSaved, onClose }: {
    mark: { id: string; quote: string };
    suggestedDeckId: string | null;
    onSaved: (closed: boolean) => void;
    onClose: () => void;
  }) => (
    <div>
      <p>Card from {mark.id}: {mark.quote} in {suggestedDeckId}</p>
      <button type="button" onClick={() => onSaved(true)}>Save card</button>
      <button type="button" onClick={onClose}>Close card</button>
    </div>
  ),
}));

const documentId = "a".repeat(32);
const title = "Cardiac physiology";
// Pages sit 100 CSS pixels below the top of the window, 20 apart, at one pixel per point.
const PAGE_TOP = 100;
const PAGE_GAP = 20;
const pages: annotationIpc.AnnotatedPage[] = [1, 2].map((pageNumber) => ({
  pageNumber, assetId: `${pageNumber}`.repeat(32), width: 612, height: 792, imageAvailable: true,
}));

const originalMatchMedia = window.matchMedia;

beforeEach(() => {
  vi.mocked(annotationIpc.getDocumentAnnotations).mockResolvedValue({ documentId, pages, annotations: [] });
  vi.mocked(annotationIpc.applyDocumentAnnotationChanges).mockResolvedValue({ annotations: [], deletedIds: [] });
  vi.mocked(studyGuideIpc.readDocumentSourceAsset).mockResolvedValue(new ArrayBuffer(8));
  vi.mocked(annotationIpc.renderDocumentPageTile).mockResolvedValue(new ArrayBuffer(8));
  vi.mocked(annotationIpc.getDocumentPageText).mockRejectedValue(new Error("This page has no text."));
  vi.mocked(annotationCardsIpc.getDocumentMarkedCards).mockResolvedValue({ documentId, cardIds: [], suggestedDeckId: null });
  Object.defineProperty(URL, "createObjectURL", { configurable: true, writable: true, value: vi.fn(() => "blob:page") });
  Object.defineProperty(URL, "revokeObjectURL", { configurable: true, writable: true, value: vi.fn() });
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function rect(this: HTMLElement) {
    const pageNumber = Number(this.dataset.annotatorPage);
    if (pageNumber > 0) return new DOMRect(0, PAGE_TOP + (pageNumber - 1) * (792 + PAGE_GAP), 612, 792);
    return new DOMRect(0, PAGE_TOP, 800, 600);
  });
});

afterEach(() => {
  window.matchMedia = originalMatchMedia;
  vi.restoreAllMocks();
});

function usePhoneLayout(): void {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: query === "(pointer: coarse)" || matchesLayoutQuery(query, layoutWidths.compact),
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

async function openAnnotator(onClose = vi.fn(), showToast = vi.fn(), onPracticeCards?: (cardIds: string[]) => Promise<void>) {
  render(
    <LayoutModeProvider>
      <OverlayProvider>
        <DocumentAnnotator documentId={documentId} title={title} onClose={onClose} showToast={showToast}
          onPracticeCards={onPracticeCards} />
      </OverlayProvider>
    </LayoutModeProvider>,
  );
  await screen.findByRole("toolbar", { name: "Annotation tools" });
  const canvas = screen.getByRole("group", { name: `Pages of ${title}` });
  return { onClose, showToast, canvas };
}

/** A pointer at a spot on page 1, in points from its top-left corner. */
function onPage(x: number, y: number, pointerId = 1, pointerType = "mouse") {
  return { pointerId, pointerType, button: 0, pressure: 0.5, clientX: x, clientY: PAGE_TOP + y };
}

function drag(canvas: HTMLElement, points: Array<[number, number]>, pointerType = "mouse", pointerId = 1): void {
  const [first, ...rest] = points;
  if (!first) return;
  fireEvent.pointerDown(canvas, onPage(first[0], first[1], pointerId, pointerType));
  for (const [x, y] of rest) fireEvent.pointerMove(canvas, onPage(x, y, pointerId, pointerType));
  const last = rest[rest.length - 1] ?? first;
  fireEvent.pointerUp(canvas, onPage(last[0], last[1], pointerId, pointerType));
}

function savedChanges(call = 0): annotationIpc.DocumentAnnotationChange[] {
  return vi.mocked(annotationIpc.applyDocumentAnnotationChanges).mock.calls[call]?.[1] ?? [];
}

function pressed(name: string): string | null {
  return screen.getByRole("button", { name }).getAttribute("aria-pressed");
}

describe("document annotator", () => {
  it("draws a pen stroke, saves it in the background, and undoes and redoes it", async () => {
    const { canvas } = await openAnnotator();
    expect(pressed("Pen")).toBe("true");
    await waitFor(() => expect(studyGuideIpc.readDocumentSourceAsset).toHaveBeenCalledWith(documentId, pages[0]?.assetId));

    drag(canvas, [[100, 100], [150, 120], [200, 160]]);

    await waitFor(() => expect(annotationIpc.applyDocumentAnnotationChanges).toHaveBeenCalledTimes(1));
    const [put] = savedChanges();
    expect(put).toMatchObject({
      op: "put",
      pageNumber: 1,
      shape: { kind: "ink", points: [100, 100, 0.5, 150, 120, 0.5, 200, 160, 0.5], simulatePressure: true, color: "#1f2328" },
    });
    expect(await screen.findByText("Saved")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => expect(savedChanges(1)).toEqual([{ op: "delete", id: put?.id }]));
    fireEvent.keyDown(canvas, { key: "z", ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(savedChanges(2)).toEqual([put]));
  });

  it("types a text note with the keyboard and finds it again with the Select tool", async () => {
    const { canvas, onClose } = await openAnnotator();
    fireEvent.click(screen.getByRole("button", { name: "Text" }));
    drag(canvas, [[300, 300]]);

    const field = await screen.findByRole("textbox", { name: "New text note" });
    expect(document.activeElement).toBe(field);
    fireEvent.change(field, { target: { value: "Remember the valves" } });
    // Escape keeps the note and stays in the annotator.
    fireEvent.keyDown(field, { key: "Escape" });
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByText("Remember the valves")).toBeTruthy();

    await waitFor(() => expect(savedChanges()).toHaveLength(1));
    expect(savedChanges()[0]).toMatchObject({
      op: "put", pageNumber: 1, shape: { kind: "text", text: "Remember the valves", x: 300, fontSize: 14, width: 200 },
    });

    fireEvent.keyDown(canvas, { key: "v" });
    expect(pressed("Select")).toBe("true");
    const item = within(screen.getByRole("list", { name: "Annotations on page 1" }))
      .getByRole("button", { name: "Text note on page 1: Remember the valves" });
    act(() => item.focus());
    expect(item.getAttribute("aria-pressed")).toBe("true");
    const selection = screen.getByRole("toolbar", { name: "Selected annotation" });
    fireEvent.click(within(selection).getByRole("button", { name: "Large" }));
    await waitFor(() => expect(savedChanges(1)[0]).toMatchObject({ shape: { fontSize: 20 } }));

    fireEvent.click(within(selection).getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(savedChanges(2)).toEqual([{ op: "delete", id: savedChanges()[0]?.id }]));
    expect(screen.queryByText("Remember the valves")).toBeNull();
  });

  it("places a sticky note, writes long text in it, and opens it again", async () => {
    const { canvas, onClose } = await openAnnotator();
    fireEvent.keyDown(canvas, { key: "s" });
    expect(pressed("Sticky note")).toBe("true");
    drag(canvas, [[300, 300]]);

    const dialog = await screen.findByRole("dialog", { name: "New sticky note" });
    const field = within(dialog).getByRole("textbox", { name: "Note" });
    await waitFor(() => expect(document.activeElement).toBe(field));
    const text = `Ask in office hours\n${"Why does preload rise with volume? ".repeat(40)}`;
    fireEvent.change(field, { target: { value: text } });
    fireEvent.keyDown(field, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(onClose).not.toHaveBeenCalled();

    await waitFor(() => expect(savedChanges()).toHaveLength(1));
    expect(savedChanges()[0]).toMatchObject({
      op: "put", pageNumber: 1,
      shape: { kind: "text", sticky: true, text, x: 300, y: 300, width: 120, fontSize: 10, color: "#fde047" },
    });
    // The page shows the note's first lines.
    expect(document.querySelector("[data-sticky-note]")?.textContent).toContain("Ask in office hours");

    fireEvent.keyDown(canvas, { key: "v" });
    const item = within(screen.getByRole("list", { name: "Annotations on page 1" }))
      .getByRole("button", { name: /^Sticky note on page 1: Ask in office hours/ });
    fireEvent.click(item);
    const reopened = await screen.findByRole("dialog", { name: "Sticky note" });
    expect((within(reopened).getByRole("textbox", { name: "Note" }) as HTMLTextAreaElement).value).toBe(text);
    fireEvent.click(within(reopened).getByRole("button", { name: "Delete note" }));
    await waitFor(() => expect(savedChanges(1)).toEqual([{ op: "delete", id: savedChanges()[0]?.id }]));
    expect(document.querySelector("[data-sticky-note]")).toBeNull();
  });

  it("draws arrows and boxes, highlights, and erases", async () => {
    const { canvas } = await openAnnotator();
    fireEvent.click(screen.getByRole("button", { name: "Arrow" }));
    drag(canvas, [[100, 100], [200, 150]]);
    fireEvent.keyDown(canvas, { key: "r" });
    expect(pressed("Box")).toBe("true");
    drag(canvas, [[300, 300], [400, 380]]);
    fireEvent.keyDown(canvas, { key: "h" });
    fireEvent.click(screen.getByRole("button", { name: "Green" }));
    drag(canvas, [[50, 500], [150, 500], [250, 500]]);

    await waitFor(() => expect(savedChanges()).toHaveLength(3));
    expect(savedChanges().map((change) => change.op === "put" ? change.shape : null)).toEqual([
      { kind: "arrow", x1: 100, y1: 100, x2: 200, y2: 150, color: "#dc2626", strokeWidth: 3 },
      { kind: "rectangle", x: 300, y: 300, width: 100, height: 80, color: "#dc2626", strokeWidth: 3 },
      { kind: "highlight", points: [50, 500, 250, 500], color: "#86efac", strokeWidth: 16 },
    ]);

    fireEvent.keyDown(canvas, { key: "e" });
    drag(canvas, [[300, 340], [160, 500]]);
    await waitFor(() => expect(savedChanges(1)).toHaveLength(2));
    expect(savedChanges(1).every((change) => change.op === "delete")).toBe(true);
  });

  it("snaps the highlighter to the page's text and keeps the words it covers", async () => {
    vi.mocked(annotationIpc.getDocumentPageText).mockImplementation((_documentId, pageNumber) => pageNumber === 1
      ? Promise.resolve({
        pageNumber, width: 612, height: 792, characters: "Stroke volume",
        boxes: Array.from("Stroke volume").flatMap((_, index) => [72 + index * 6, 200, 6, 10]),
      })
      : Promise.reject(new Error("This page has no text.")));
    const { canvas } = await openAnnotator();
    expect(annotationIpc.getDocumentPageText).not.toHaveBeenCalled();
    fireEvent.keyDown(canvas, { key: "h" });
    await waitFor(() => expect(annotationIpc.getDocumentPageText).toHaveBeenCalledWith(documentId, 1));
    await act(async () => {});

    drag(canvas, [[75, 205], [100, 206], [147, 205]]);
    await waitFor(() => expect(savedChanges()).toHaveLength(1));
    expect(savedChanges()[0]).toMatchObject({
      op: "put",
      shape: { kind: "highlight", points: [], rects: [72, 200, 78, 10], text: "Stroke volume" },
    });
  });

  it("makes a card from a highlight and studies the cards made from marks", async () => {
    const highlightId = "h".repeat(32);
    const deckId = "d".repeat(32);
    vi.mocked(annotationIpc.getDocumentAnnotations).mockResolvedValue({
      documentId,
      pages,
      annotations: [{
        id: highlightId, pageNumber: 1, revision: 1, createdAt: 5, updatedAt: 5,
        shape: { kind: "highlight", points: [], color: "#facc15", strokeWidth: 14, rects: [100, 100, 200, 14], text: " The SA node sets the rate " },
      }],
    });
    vi.mocked(annotationCardsIpc.getDocumentMarkedCards)
      .mockResolvedValueOnce({ documentId, cardIds: [], suggestedDeckId: deckId })
      .mockResolvedValue({ documentId, cardIds: ["c".repeat(32)], suggestedDeckId: deckId });
    const onPracticeCards = vi.fn(async () => undefined);
    const { canvas } = await openAnnotator(vi.fn(), vi.fn(), onPracticeCards);
    // With no cards from marks yet, there is nothing to study.
    await waitFor(() => expect(annotationCardsIpc.getDocumentMarkedCards).toHaveBeenCalledWith(documentId));
    expect(screen.queryByRole("button", { name: "Study marked cards" })).toBeNull();

    fireEvent.keyDown(canvas, { key: "v" });
    const item = within(screen.getByRole("list", { name: "Annotations on page 1" }))
      .getByRole("button", { name: "Highlight on page 1: The SA node sets the rate" });
    act(() => item.focus());
    const selection = screen.getByRole("toolbar", { name: "Selected annotation" });
    fireEvent.click(within(selection).getByRole("button", { name: "Make a card" }));
    expect(await screen.findByText(`Card from ${highlightId}: The SA node sets the rate in ${deckId}`)).toBeTruthy();
    // One editor at a time.
    expect(within(selection).queryByRole("button", { name: "Make a card" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Save card" }));
    await waitFor(() => expect(screen.queryByText(/Card from/u)).toBeNull());
    const study = await screen.findByRole("button", { name: "Study marked cards" });
    fireEvent.click(study);
    await waitFor(() => expect(onPracticeCards).toHaveBeenCalledWith(["c".repeat(32)]));
  });

  it("zooms with the keyboard and with two fingers, without drawing", async () => {
    const { canvas, onClose } = await openAnnotator();
    const zoomButton = () => screen.getByRole("button", { name: /^Automatic zoom/ });
    expect(zoomButton().textContent).toBe("75%");
    fireEvent.keyDown(canvas, { key: "=", ctrlKey: true });
    expect(zoomButton().textContent).toBe("94%");
    expect(zoomButton().getAttribute("aria-pressed")).toBe("false");
    fireEvent.keyDown(canvas, { key: "0", ctrlKey: true });
    expect(zoomButton().textContent).toBe("75%");

    fireEvent.pointerDown(canvas, onPage(100, 100, 1, "touch"));
    fireEvent.pointerMove(canvas, onPage(110, 100, 1, "touch"));
    fireEvent.pointerDown(canvas, onPage(300, 100, 2, "touch"));
    fireEvent.pointerMove(canvas, onPage(500, 100, 2, "touch"));
    fireEvent.pointerUp(canvas, onPage(500, 100, 2, "touch"));
    fireEvent.pointerUp(canvas, onPage(110, 100, 1, "touch"));
    expect(zoomButton().textContent).toBe("154%");

    fireEvent.click(screen.getByRole("button", { name: "Back to documents" }));
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
    expect(annotationIpc.applyDocumentAnnotationChanges).not.toHaveBeenCalled();
  });

  it("draws the part of a page in view sharply once a zoom comes to rest", async () => {
    const { canvas } = await openAnnotator();
    const tiles = () => document.querySelectorAll("img[data-page-tile]");
    // At 75% the page images are sharp enough, so nothing is drawn again.
    await new Promise((resolve) => setTimeout(resolve, 250));
    expect(annotationIpc.renderDocumentPageTile).not.toHaveBeenCalled();

    for (let step = 0; step < 3; step += 1) fireEvent.keyDown(canvas, { key: "=", ctrlKey: true });
    expect(screen.getByRole("button", { name: /^Automatic zoom/ }).textContent).toBe("146%");
    await waitFor(() => expect(tiles().length).toBeGreaterThan(0));
    const requests = vi.mocked(annotationIpc.renderDocumentPageTile).mock.calls.map(([, tile]) => tile);
    // Page 1 fills the view; only the top of page 2 lies within the margin kept ready.
    expect(requests.filter((tile) => tile.pageNumber === 2).every((tile) => tile.top === 0)).toBe(true);
    // Tiles are drawn at 170 dpi, the next step above 146%.
    expect(requests[0]).toMatchObject({ pageWidth: 1442, pageHeight: 1867, left: 0, top: 0, width: 512, height: 512 });

    fireEvent.keyDown(canvas, { key: "0", ctrlKey: true });
    await waitFor(() => expect(tiles()).toHaveLength(0));
  });

  it("reports a failed save and asks before leaving without it", async () => {
    vi.mocked(annotationIpc.applyDocumentAnnotationChanges).mockRejectedValue(new Error("The library is busy."));
    const { canvas, onClose, showToast } = await openAnnotator();
    drag(canvas, [[100, 100], [200, 200]]);

    expect(await screen.findByText("Not saved")).toBeTruthy();
    expect(showToast).toHaveBeenCalledWith("error", "Annotations could not be saved: The library is busy.");

    fireEvent.click(screen.getByRole("button", { name: "Back to documents" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Leave without saving?" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Stay" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(onClose).not.toHaveBeenCalled();

    vi.mocked(annotationIpc.applyDocumentAnnotationChanges).mockResolvedValue({ annotations: [], deletedIds: [] });
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByText("Saved")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Back to documents" }));
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
  });

  it("shows the page images when React mounts the screen twice, as development builds do", async () => {
    render(
      <StrictMode>
        <LayoutModeProvider>
          <OverlayProvider>
            <DocumentAnnotator documentId={documentId} title={title} onClose={vi.fn()} showToast={vi.fn()} />
          </OverlayProvider>
        </LayoutModeProvider>
      </StrictMode>,
    );
    await screen.findByRole("toolbar", { name: "Annotation tools" });
    await waitFor(() => expect(document.querySelectorAll('img[src="blob:page"]')).toHaveLength(pages.length));
  });

  it("offers another try when the pages cannot be opened", async () => {
    vi.mocked(annotationIpc.getDocumentAnnotations)
      .mockRejectedValueOnce(new Error("The document is missing its page images."))
      .mockResolvedValue({ documentId, pages, annotations: [] });
    render(
      <OverlayProvider>
        <DocumentAnnotator documentId={documentId} title={title} onClose={vi.fn()} showToast={vi.fn()} />
      </OverlayProvider>,
    );
    expect(await screen.findByText("The document is missing its page images.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("toolbar", { name: "Annotation tools" })).toBeTruthy();
  });

  it("shows saved annotations and keeps every tool within reach on a phone", async () => {
    usePhoneLayout();
    vi.mocked(annotationIpc.getDocumentAnnotations).mockResolvedValue({
      documentId,
      pages,
      annotations: [{
        id: "n".repeat(32), pageNumber: 2, revision: 1, createdAt: 5, updatedAt: 5,
        shape: { kind: "text", x: 40, y: 40, width: 200, text: "Ask about preload", color: "#2563eb", fontSize: 14 },
      }],
    });
    const { canvas } = await openAnnotator();
    expect(screen.getByText("Ask about preload")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Box" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Text" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Notes" }));
    const notes = await screen.findByRole("dialog", { name: "Notes" });
    fireEvent.click(within(notes).getByRole("button", { name: "Sticky note" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(screen.getByRole("button", { name: "Notes: Sticky note" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Shapes" }));
    const shapes = await screen.findByRole("dialog", { name: "Shapes" });
    fireEvent.click(within(shapes).getByRole("button", { name: "Box" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(screen.getByRole("button", { name: "Shapes: Box" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Zoom (75%)" }));
    const zoom = await screen.findByRole("dialog", { name: "Zoom (75%)" });
    expect((within(zoom).getByRole("button", { name: "Automatic zoom" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(within(zoom).getByRole("button", { name: "Zoom in" }));
    expect(await screen.findByRole("button", { name: "Zoom (94%)" })).toBeTruthy();

    // A finger draws a box on a phone.
    drag(canvas, [[100, 100], [160, 140]], "touch");
    await waitFor(() => expect(savedChanges()[0]).toMatchObject({ shape: { kind: "rectangle", color: "#dc2626" } }));
    expect((screen.getByRole("button", { name: "Undo" }) as HTMLButtonElement).disabled).toBe(false);

    // Color and size open in a sheet; the next box takes the new color.
    fireEvent.click(screen.getByRole("button", { name: "Color and size: Red, Medium" }));
    const style = await screen.findByRole("dialog", { name: "Color and size" });
    fireEvent.click(within(style).getByRole("button", { name: "Blue" }));
    expect(within(style).getByRole("button", { name: "Blue" }).getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(within(style).getByRole("button", { name: "Close dialog" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(screen.getByRole("button", { name: "Color and size: Blue, Medium" })).toBeTruthy();
    drag(canvas, [[300, 300], [360, 340]], "touch");
    await waitFor(() => expect(savedChanges(1)[0]).toMatchObject({ shape: { kind: "rectangle", color: "#2563eb" } }));
  });
});
