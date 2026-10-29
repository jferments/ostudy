import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as assetsIpc from "../../ipc/assets";
import * as decksIpc from "../../ipc/decks";
import * as documentAnnotationsIpc from "../../ipc/documentAnnotations";
import * as documentsIpc from "../../ipc/documents";
import * as externalFilesIpc from "../../ipc/externalFiles";
import * as notebooksIpc from "../../ipc/notebooks";
import * as studyGuidesIpc from "../../ipc/studyGuides";
import * as files from "../../platform/files";
import { LayoutModeProvider } from "../../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { openMenu } from "../../test/menus";
import { NotebookEditor } from "./NotebookEditor";
import * as pictures from "./pictures";

vi.mock("../../ipc/notebooks", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../ipc/notebooks")>(),
  getNotebook: vi.fn(),
  listNotebooks: vi.fn(),
  applyNotebookItemChanges: vi.fn(),
  addNotebookPage: vi.fn(),
  deleteNotebookPage: vi.fn(),
  moveNotebookPage: vi.fn(),
  prepareNotebookPdf: vi.fn(),
}));
vi.mock("../../ipc/externalFiles", () => ({ publishPreparedExport: vi.fn(), releasePreparedExport: vi.fn() }));
vi.mock("../../ipc/assets", () => ({ beginAssetDraft: vi.fn(), cancelAssetDraft: vi.fn(), importImage: vi.fn() }));
vi.mock("../../platform/assets", () => ({ managedAssetUrl: (id: string) => `blob:managed-${id}` }));
vi.mock("../../platform/files", () => ({ selectExternalImport: vi.fn() }));
vi.mock("./pictures", async (importOriginal) => ({
  ...await importOriginal<typeof import("./pictures")>(),
  naturalSize: vi.fn(),
}));
vi.mock("../../editor/FormulaDialog", () => ({
  // The formula editor itself is MathLive's; here it saves one formula.
  FormulaDialog: ({ session, onClose }: { session: import("../../editor/formula").FormulaEditSession; onClose: () => void }) => (
    <button type="button" onClick={() => { session.save({ latex: "x^2", placement: "inline" }); onClose(); }}>Save formula</button>
  ),
}));
vi.mock("../../ipc/documentAnnotations", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../ipc/documentAnnotations")>(),
  getDocumentAnnotations: vi.fn(),
  applyDocumentAnnotationChanges: vi.fn(),
}));
vi.mock("../../ipc/decks", () => ({ listDecks: vi.fn() }));
vi.mock("../../ipc/documents", () => ({ listDocuments: vi.fn() }));
vi.mock("../../ipc/studyGuides", () => ({ listStudyGuidePage: vi.fn(), readDocumentSourceAsset: vi.fn() }));

const notebookId = "b".repeat(32);
const deckId = "c".repeat(32);
const PAGE_TOP = 100;
const PAGE_GAP = 20;

function page(seed: string, position: number): notebooksIpc.NotebookPage {
  return { id: seed.repeat(32), position, paper: "grid", width: 612, height: 792, layout: "page" };
}

function board(seed: string, position: number): notebooksIpc.NotebookPage {
  return { id: seed.repeat(32), position, paper: "dots", width: 14400, height: 14400, layout: "board" };
}

function contents(overrides: Partial<notebooksIpc.NotebookContents> = {}): notebooksIpc.NotebookContents {
  return {
    notebook: {
      id: notebookId, subjectId: null, title: "Cardiac cycle", paper: "grid", pageSize: "letter",
      revision: 1, createdAt: 1, updatedAt: 1,
    },
    pages: [page("1", 1), page("2", 2)],
    items: [],
    unavailableLinkIds: [],
    clipDocuments: {},
    ...overrides,
  };
}

const deck: decksIpc.DeckSummary = {
  id: deckId, name: "Heart", description: "", sourceNoteCount: 4, generatedCardCount: 11, studyableDueCount: 3,
  archivedAt: null, createdAt: 1, updatedAt: 1, revision: 1,
};

beforeEach(() => {
  vi.mocked(notebooksIpc.getNotebook).mockResolvedValue(contents());
  vi.mocked(notebooksIpc.listNotebooks).mockResolvedValue([]);
  vi.mocked(notebooksIpc.applyNotebookItemChanges).mockResolvedValue({ items: [], deletedIds: [] });
  vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);
  vi.mocked(documentsIpc.listDocuments).mockResolvedValue([]);
  vi.mocked(studyGuidesIpc.listStudyGuidePage).mockResolvedValue({ items: [], totalCount: 0, offset: 0, limit: 100 });
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function rect(this: HTMLElement) {
    const pageNumber = Number(this.dataset.annotatorPage);
    if (pageNumber > 0) return new DOMRect(0, PAGE_TOP + (pageNumber - 1) * (792 + PAGE_GAP), 612, 792);
    return new DOMRect(0, PAGE_TOP, 800, 600);
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});

async function openNotebook(overrides: Partial<Parameters<typeof NotebookEditor>[0]> = {}, canvasName = "Pages of Cardiac cycle") {
  const props = {
    notebookId,
    title: "Cardiac cycle",
    onClose: vi.fn(),
    showToast: vi.fn(),
    onPracticeDeck: vi.fn(),
    onOpenGuide: vi.fn(),
    onOpenNotebook: vi.fn(),
    ...overrides,
  };
  render(
    <LayoutModeProvider>
      <OverlayProvider>
        <NotebookEditor {...props} />
      </OverlayProvider>
    </LayoutModeProvider>,
  );
  await screen.findByRole("toolbar", { name: "Annotation tools" });
  const canvas = screen.getByRole("group", { name: canvasName });
  return { ...props, canvas };
}

function onPage(x: number, y: number) {
  return { pointerId: 1, pointerType: "mouse", button: 0, pressure: 0.5, clientX: x, clientY: PAGE_TOP + y };
}

function savedChanges(call = 0): notebooksIpc.NotebookItemChange[] {
  return vi.mocked(notebooksIpc.applyNotebookItemChanges).mock.calls[call]?.[1] ?? [];
}

describe("notebook editor", () => {
  it("draws on grid paper and saves the stroke to its page", async () => {
    const { canvas } = await openNotebook();
    expect(document.querySelectorAll("[data-paper='grid']").length).toBeGreaterThan(0);
    // No page images are read for paper.
    expect(studyGuidesIpc.readDocumentSourceAsset).not.toHaveBeenCalled();

    fireEvent.pointerDown(canvas, onPage(100, 100));
    fireEvent.pointerMove(canvas, onPage(160, 140));
    fireEvent.pointerUp(canvas, onPage(160, 140));

    await waitFor(() => expect(notebooksIpc.applyNotebookItemChanges).toHaveBeenCalledTimes(1));
    expect(vi.mocked(notebooksIpc.applyNotebookItemChanges).mock.calls[0]?.[0]).toBe(notebookId);
    expect(savedChanges()[0]).toMatchObject({ op: "put", pageId: "1".repeat(32), shape: { kind: "ink" } });
  });

  it("adds a Study button for a deck that starts Practice once the page is saved", async () => {
    const { onPracticeDeck } = await openNotebook();
    fireEvent.click(screen.getByRole("button", { name: "Add link" }));
    const dialog = await screen.findByRole("dialog", { name: "Add a link" });
    fireEvent.change(within(dialog).getByRole("combobox", { name: "Link to" }), { target: { value: "deck" } });
    const select = within(dialog).getByRole("combobox", { name: "Deck" });
    await waitFor(() => expect(within(select).getByRole("option", { name: "Heart" })).toBeTruthy());
    fireEvent.change(select, { target: { value: deckId } });
    expect((within(dialog).getByRole("textbox", { name: "Label" }) as HTMLInputElement).value).toBe("Study Heart");
    expect(within(dialog).getByRole("radio", { name: "Button" }).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(within(dialog).getByRole("button", { name: "Add link" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(document.querySelector("[data-note-link='button']")?.textContent).toBe("Study Heart");
    fireEvent.click(screen.getByRole("button", { name: "Open link" }));
    await waitFor(() => expect(onPracticeDeck).toHaveBeenCalledWith(deckId));
    // The link was saved before leaving.
    expect(savedChanges()[0]).toMatchObject({
      op: "put", pageId: "1".repeat(32),
      shape: { kind: "text", text: "Study Heart", link: { target: { kind: "deck", deckId }, button: true } },
    });
  });

  it("adds a picture that fits the page and keeps its image with the first save", async () => {
    const draftId = "d".repeat(32);
    const assetId = "e".repeat(32);
    vi.mocked(assetsIpc.beginAssetDraft).mockResolvedValue({ id: draftId, purpose: "import", expiresAt: 1 });
    vi.mocked(files.selectExternalImport).mockResolvedValue({ id: "staged-1", fileName: "heart.png" } as never);
    vi.mocked(assetsIpc.importImage).mockResolvedValue({
      id: assetId, originalFilename: "heart.png", mimeType: "image/png", byteSize: 10, createdAt: 1,
    });
    vi.mocked(pictures.naturalSize).mockResolvedValue({ width: 1032, height: 516 });
    await openNotebook();
    fireEvent.click(screen.getByRole("button", { name: "Add picture" }));

    await waitFor(() => expect(notebooksIpc.applyNotebookItemChanges).toHaveBeenCalledTimes(1));
    expect(assetsIpc.beginAssetDraft).toHaveBeenCalledWith("import", notebookId);
    expect(files.selectExternalImport).toHaveBeenCalledWith("image");
    expect(assetsIpc.importImage).toHaveBeenCalledWith("staged-1", draftId);
    // As wide as the page's margins allow, keeping its shape, and centered.
    expect(savedChanges()[0]).toMatchObject({
      op: "put", pageId: "1".repeat(32), assetDraftId: draftId,
      shape: { kind: "image", x: 48, y: 48, width: 516, height: 258, assetId },
    });
    expect(document.querySelector(`image[href*='${assetId}']`)).not.toBeNull();

    // Once saved, the picture no longer names the draft.
    fireEvent.keyDown(screen.getByRole("group", { name: "Pages of Cardiac cycle" }), { key: "ArrowRight" });
    await waitFor(() => expect(notebooksIpc.applyNotebookItemChanges).toHaveBeenCalledTimes(2));
    expect(savedChanges(1)[0]).toMatchObject({ op: "put", shape: { kind: "image", assetId } });
    expect(savedChanges(1)[0]).not.toHaveProperty("assetDraftId");
  });

  it("drops the picture's draft when no image is chosen", async () => {
    const draftId = "d".repeat(32);
    vi.mocked(assetsIpc.beginAssetDraft).mockResolvedValue({ id: draftId, purpose: "import", expiresAt: 1 });
    vi.mocked(assetsIpc.cancelAssetDraft).mockResolvedValue(undefined);
    vi.mocked(files.selectExternalImport).mockResolvedValue(null);
    await openNotebook();
    fireEvent.click(screen.getByRole("button", { name: "Add picture" }));

    await waitFor(() => expect(assetsIpc.cancelAssetDraft).toHaveBeenCalledWith(draftId));
    expect(assetsIpc.importImage).not.toHaveBeenCalled();
    expect(notebooksIpc.applyNotebookItemChanges).not.toHaveBeenCalled();
  });

  it("types a checklist with the toolbar and Enter, adds a formula, and ticks a box with Select", async () => {
    const { canvas } = await openNotebook();
    fireEvent.click(screen.getByRole("button", { name: "Text" }));
    fireEvent.pointerDown(canvas, onPage(100, 100));
    fireEvent.pointerUp(canvas, onPage(100, 100));
    const field = await screen.findByRole("textbox", { name: "New text note" }) as HTMLTextAreaElement;
    fireEvent.change(field, { target: { value: "Valves" } });
    fireEvent.click(screen.getByRole("button", { name: "Checklist" }));
    expect(field.value).toBe("[ ] Valves");
    field.setSelectionRange(field.value.length, field.value.length);
    fireEvent.keyDown(field, { key: "Enter" });
    expect(field.value).toBe("[ ] Valves\n[ ] ");
    fireEvent.change(field, { target: { value: "[ ] Valves\n[ ] Area " } });
    field.setSelectionRange(field.value.length, field.value.length);

    fireEvent.click(screen.getByRole("button", { name: "Insert formula" }));
    // The note stays open while the formula editor has the focus.
    fireEvent.blur(field);
    expect(screen.getByRole("textbox", { name: "New text note" })).toBe(field);
    fireEvent.click(screen.getByRole("button", { name: "Save formula" }));
    expect(field.value).toBe("[ ] Valves\n[ ] Area $x^2$");
    fireEvent.keyDown(field, { key: "Escape" });

    expect(document.querySelectorAll("[data-note-checkbox='unchecked']")).toHaveLength(2);
    expect(document.querySelector("[data-study-math='inline']")).not.toBeNull();
    await waitFor(() => expect(notebooksIpc.applyNotebookItemChanges).toHaveBeenCalledTimes(1));
    expect(savedChanges()[0]).toMatchObject({ shape: { kind: "text", text: "[ ] Valves\n[ ] Area $x^2$" } });

    fireEvent.keyDown(canvas, { key: "v" });
    fireEvent.pointerDown(document.querySelector("[data-note-checkbox]")!);
    expect(document.querySelectorAll("[data-note-checkbox='checked']")).toHaveLength(1);
    await waitFor(() => expect(notebooksIpc.applyNotebookItemChanges).toHaveBeenCalledTimes(2));
    expect(savedChanges(1)[0]).toMatchObject({ shape: { text: "[x] Valves\n[ ] Area $x^2$" } });
  });

  it("opens a document beside the notes and saves both before closing", async () => {
    const documentId = "f".repeat(32);
    vi.mocked(documentsIpc.listDocuments).mockResolvedValue([
      { id: documentId, title: "Lecture 7", sourceUnitKind: "page", sourceUnitCount: 12 } as documentsIpc.DocumentSummary,
    ]);
    vi.mocked(documentAnnotationsIpc.getDocumentAnnotations).mockResolvedValue({
      pages: Array.from({ length: 12 }, (_, index) => ({
        pageNumber: index + 1, assetId: "", width: 612, height: 792, imageAvailable: false,
      })),
      annotations: [],
    } as unknown as documentAnnotationsIpc.DocumentAnnotations);
    const { onClose } = await openNotebook();
    fireEvent.click(screen.getByRole("button", { name: "Document beside" }));
    const dialog = await screen.findByRole("dialog", { name: "Open a document beside your notes" });
    const select = within(dialog).getByRole("combobox", { name: "Document" });
    await waitFor(() => expect(within(select).getByRole("option", { name: "Lecture 7" })).toBeTruthy());
    fireEvent.change(select, { target: { value: documentId } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Open" }));

    const beside = await screen.findByRole("region", { name: "Lecture 7, beside your notes" });
    expect(await within(beside).findByRole("group", { name: "Pages of Lecture 7" })).toBeTruthy();
    expect(documentAnnotationsIpc.getDocumentAnnotations).toHaveBeenCalledWith(documentId);
    // The notebook stays open beside it, with its own tools.
    expect(screen.getByRole("group", { name: "Pages of Cardiac cycle" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Change document" })).toBeTruthy();

    fireEvent.click(within(beside).getByRole("button", { name: "Close document" }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "Lecture 7, beside your notes" })).toBeNull());
    expect(onClose).not.toHaveBeenCalled();
  });

  it("keeps a link whose target is gone, struck through, and opens nothing", async () => {
    const linkId = "d".repeat(32);
    vi.mocked(notebooksIpc.getNotebook).mockResolvedValue(contents({
      items: [{
        id: linkId, pageId: "1".repeat(32), revision: 1, createdAt: 1, updatedAt: 1,
        shape: {
          kind: "text", x: 40, y: 40, width: 200, text: "Lecture 7 · p. 9", color: "#1f2328", fontSize: 14,
          link: { target: { kind: "documentPage", documentId: "e".repeat(32), pageNumber: 9 } },
        },
      }],
      unavailableLinkIds: [linkId],
    }));
    const { onPracticeDeck, showToast } = await openNotebook();
    expect(document.querySelector("[data-note-link='link']")?.getAttribute("data-unavailable")).toBe("true");

    fireEvent.click(screen.getByRole("button", { name: "Select" }));
    const item = await screen.findByRole("button", { name: /Lecture 7 · p\. 9/ });
    item.focus();
    fireEvent.click(await screen.findByRole("button", { name: "Removed" }));
    expect(showToast).toHaveBeenCalledWith("error", "What this link opened is no longer in your library.");
    expect(onPracticeDeck).not.toHaveBeenCalled();
  });

  it("saves the notebook as a PDF once what is pending is saved, and says what was left out", async () => {
    const exportId = "9".repeat(32);
    vi.mocked(notebooksIpc.prepareNotebookPdf).mockResolvedValue({
      preparedExport: { id: exportId } as notebooksIpc.PreparedNotebookPdf["preparedExport"],
      pageCount: 2,
      missingPictureCount: 1,
    });
    vi.mocked(externalFilesIpc.publishPreparedExport).mockResolvedValue({} as Awaited<ReturnType<typeof externalFilesIpc.publishPreparedExport>>);
    vi.mocked(externalFilesIpc.releasePreparedExport).mockResolvedValue();
    const { canvas, showToast } = await openNotebook();
    fireEvent.pointerDown(canvas, onPage(100, 100));
    fireEvent.pointerMove(canvas, onPage(150, 140));
    fireEvent.pointerUp(canvas, onPage(150, 140));

    fireEvent.click(screen.getByRole("button", { name: "Save notebook as PDF" }));

    await waitFor(() => expect(externalFilesIpc.releasePreparedExport).toHaveBeenCalledWith(exportId));
    const saved = vi.mocked(notebooksIpc.applyNotebookItemChanges).mock.invocationCallOrder.at(0) ?? Infinity;
    const prepared = vi.mocked(notebooksIpc.prepareNotebookPdf).mock.invocationCallOrder.at(0) ?? -Infinity;
    expect(saved).toBeLessThan(prepared);
    expect(notebooksIpc.prepareNotebookPdf).toHaveBeenCalledWith(notebookId);
    expect(externalFilesIpc.publishPreparedExport).toHaveBeenCalledWith(exportId);
    expect(showToast).toHaveBeenCalledWith("success", "Saved 2 pages as a PDF. 1 picture could not be read and was left out.");
  });

  it("opens the document page a clip came from, and says when that document is gone", async () => {
    const documentId = "e".repeat(32);
    const goneId = "9".repeat(32);
    const clip = (id: string, y: number, source: string) => ({
      id, pageId: "1".repeat(32), revision: 1, createdAt: 1, updatedAt: 1,
      shape: { kind: "image" as const, x: 100, y, width: 200, height: 100, assetId: "a".repeat(32), source: { documentId: source, pageNumber: 4 } },
    });
    vi.mocked(notebooksIpc.getNotebook).mockResolvedValue(contents({
      items: [clip("7".repeat(32), 100, documentId), clip("8".repeat(32), 400, goneId)],
      clipDocuments: { [documentId]: "Lecture 7" },
    }));
    vi.mocked(documentAnnotationsIpc.getDocumentAnnotations).mockResolvedValue({
      pages: Array.from({ length: 6 }, (_, index) => ({
        pageNumber: index + 1, assetId: "", width: 612, height: 792, imageAvailable: false,
      })),
      annotations: [],
    } as unknown as documentAnnotationsIpc.DocumentAnnotations);
    const { canvas, showToast } = await openNotebook();
    fireEvent.click(screen.getByRole("button", { name: "Select" }));

    fireEvent.pointerDown(canvas, onPage(200, 150));
    fireEvent.pointerUp(canvas, onPage(200, 150));
    fireEvent.click(await screen.findByRole("button", { name: "From Lecture 7 · p. 4" }));
    expect(await screen.findByRole("dialog", { name: "Lecture 7" })).toBeTruthy();
    expect(documentAnnotationsIpc.getDocumentAnnotations).toHaveBeenCalledWith(documentId);
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Lecture 7" })).toBeNull());

    fireEvent.pointerDown(canvas, onPage(200, 450));
    fireEvent.pointerUp(canvas, onPage(200, 450));
    fireEvent.click(await screen.findByRole("button", { name: "From p. 4 (removed)" }));
    expect(showToast).toHaveBeenCalledWith("error", "The document this was clipped from is no longer in your library.");
  });

  it("adds a page after the one in view and opens the notebook again", async () => {
    vi.mocked(notebooksIpc.addNotebookPage).mockResolvedValue(contents({ pages: [page("1", 1), page("3", 2), page("2", 3)] }));
    await openNotebook();
    expect(screen.getByText("Page 1 of 2")).toBeTruthy();
    vi.mocked(notebooksIpc.getNotebook).mockResolvedValue(contents({ pages: [page("1", 1), page("3", 2), page("2", 3)] }));

    fireEvent.click(screen.getByRole("button", { name: "Add page" }));

    await waitFor(() => expect(notebooksIpc.addNotebookPage).toHaveBeenCalledWith(notebookId, 2));
    await waitFor(() => expect(notebooksIpc.getNotebook).toHaveBeenCalledTimes(2));
    expect(await screen.findByText(/of 3$/)).toBeTruthy();
  });

  it("opens a notebook of boards on its board, with the Hand tool, Fit all and an overview", async () => {
    vi.mocked(notebooksIpc.getNotebook).mockResolvedValue(contents({ pages: [board("1", 1)] }));
    const { canvas } = await openNotebook({}, "Pages of Cardiac cycle, Board 1");
    expect(screen.getByText("Board 1")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Add page" })).toBeNull();
    expect(screen.getByRole("button", { name: /^Fit all/u })).toBeTruthy();
    expect(screen.getByRole("img", { name: "Board overview" })).toBeTruthy();

    // Holding Space picks the Hand tool for as long as it is held.
    const pen = screen.getByRole("button", { name: "Pen" });
    fireEvent.click(pen);
    fireEvent.keyDown(canvas, { key: " " });
    expect(screen.getByRole("button", { name: "Hand" }).getAttribute("aria-pressed")).toBe("true");
    fireEvent.keyUp(canvas, { key: " " });
    expect(screen.getByRole("button", { name: "Pen" }).getAttribute("aria-pressed")).toBe("true");
  });

  it("shows pages apart from boards, adds a board and goes to it", async () => {
    const withBoard = contents({ pages: [page("1", 1), page("2", 2), board("3", 3)] });
    vi.mocked(notebooksIpc.getNotebook).mockResolvedValue(contents());
    vi.mocked(notebooksIpc.addNotebookPage).mockResolvedValue(withBoard);
    await openNotebook();
    expect(screen.queryByRole("button", { name: "Hand" })).toBeNull();

    vi.mocked(notebooksIpc.getNotebook).mockResolvedValue(withBoard);
    const actions = screen.getByRole("button", { name: "Page 1 actions" });
    fireEvent.click(await within(await openMenu(actions)).findByRole("menuitem", { name: "Add board" }));
    await waitFor(() => expect(notebooksIpc.addNotebookPage).toHaveBeenCalledWith(notebookId, 3, undefined, "board"));
    expect(await screen.findByRole("group", { name: "Pages of Cardiac cycle, Board 1" })).toBeTruthy();

    // The place in the notebook is a menu of its pages and boards.
    const place = screen.getByRole("button", { name: "Board 1" });
    fireEvent.click(await within(await openMenu(place)).findByRole("menuitemcheckbox", { name: "Pages (2)" }));
    expect(await screen.findByRole("group", { name: "Pages of Cardiac cycle" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Page 1 of 2" })).toBeTruthy();
  });

  it("asks how boards go into a saved PDF", async () => {
    const exportId = "9".repeat(32);
    vi.mocked(notebooksIpc.getNotebook).mockResolvedValue(contents({ pages: [page("1", 1), board("2", 2)] }));
    vi.mocked(notebooksIpc.prepareNotebookPdf).mockResolvedValue({
      preparedExport: { id: exportId } as notebooksIpc.PreparedNotebookPdf["preparedExport"],
      pageCount: 3,
      missingPictureCount: 0,
    });
    vi.mocked(externalFilesIpc.publishPreparedExport).mockResolvedValue({} as Awaited<ReturnType<typeof externalFilesIpc.publishPreparedExport>>);
    vi.mocked(externalFilesIpc.releasePreparedExport).mockResolvedValue();
    await openNotebook();

    const save = screen.getByRole("button", { name: "Save PDF" });
    fireEvent.click(await within(await openMenu(save)).findByRole("menuitem", { name: "Boards across Letter pages" }));
    await waitFor(() => expect(notebooksIpc.prepareNotebookPdf).toHaveBeenCalledWith(notebookId, undefined, { split: "letter" }));
  });

  it("builds a mind map with the keyboard, folds a branch and saves each topic's place", async () => {
    const { canvas } = await openNotebook();
    fireEvent.keyDown(canvas, { key: "m" });
    expect(screen.getByRole("button", { name: "Mind map" }).getAttribute("aria-pressed")).toBe("true");
    fireEvent.pointerDown(canvas, onPage(300, 200));
    fireEvent.pointerUp(canvas, onPage(300, 200));

    // Enter goes on to the next topic and Tab to a branch, as typing goes.
    let field = await screen.findByRole("textbox", { name: "New topic" });
    fireEvent.change(field, { target: { value: "Heart" } });
    fireEvent.keyDown(field, { key: "Tab" });
    field = await screen.findByRole("textbox", { name: "New topic" });
    fireEvent.change(field, { target: { value: "Systole" } });
    fireEvent.keyDown(field, { key: "Tab" });
    field = await screen.findByRole("textbox", { name: "New topic" });
    fireEvent.change(field, { target: { value: "Ejection" } });
    fireEvent.keyDown(field, { key: "Enter" });
    field = await screen.findByRole("textbox", { name: "New topic" });
    fireEvent.change(field, { target: { value: "Contraction" } });
    fireEvent.keyDown(field, { key: "Escape" });

    await waitFor(() => expect(document.querySelectorAll("[data-map-branch]")).toHaveLength(3));
    const saved = () => vi.mocked(notebooksIpc.applyNotebookItemChanges).mock.calls
      .flatMap((call) => call[1])
      .flatMap((change) => (change.op === "put" && change.shape.kind === "text" ? [change.shape] : []))
      .reverse();
    await waitFor(() => expect(new Set(saved().map((shape) => shape.text))).toEqual(new Set(["Heart", "Systole", "Ejection", "Contraction"])));
    const heart = saved().find((shape) => shape.text === "Heart");
    const systole = saved().find((shape) => shape.text === "Systole");
    expect(heart?.topic).toMatchObject({ order: 0 });
    expect(heart?.topic?.parent).toBeUndefined();
    expect(systole?.topic).toMatchObject({ map: heart?.topic?.map, order: 0 });

    // Up to the branch with the arrow key, then fold it from the toolbar.
    fireEvent.keyDown(canvas, { key: "ArrowLeft" });
    fireEvent.click(screen.getByRole("button", { name: "Fold branches" }));
    expect(document.querySelectorAll("[data-map-branch]")).toHaveLength(1);
    expect(document.querySelector("[data-fold-badge]")?.getAttribute("data-fold-badge")).toBe("2");
    fireEvent.keyDown(canvas, { key: "/" });
    expect(document.querySelectorAll("[data-map-branch]")).toHaveLength(3);
  });

  it("makes a map of a pasted outline, copies it back as a list, and keeps an arrow on its topics", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
    const { canvas, showToast } = await openNotebook();
    fireEvent.paste(canvas, { clipboardData: { getData: () => "# Heart\n- Systole\n  - Ejection\n- Diastole" } });
    await waitFor(() => expect(document.querySelectorAll("[data-map-branch]")).toHaveLength(3));

    // The central topic opens selected, with the map's actions.
    await openMenu(screen.getByRole("button", { name: "More map actions" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Copy the map as a list" }));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith("- Heart\n  - Systole\n    - Ejection\n  - Diastole"));
    expect(showToast).toHaveBeenCalledWith("success", "Copied the map as a list.");

    const topics = () => new Map(vi.mocked(notebooksIpc.applyNotebookItemChanges).mock.calls
      .flatMap((call) => call[1])
      .flatMap((change) => (change.op === "put" && change.shape.kind === "text" ? [[change.shape.text, { id: change.id, shape: change.shape }] as const] : [])));
    await waitFor(() => expect(topics().size).toBe(4));
    const middle = (text: string) => {
      const shape = topics().get(text)?.shape;
      return shape ? { x: shape.x + shape.width / 2, y: shape.y + 12 } : { x: 0, y: 0 };
    };
    const from = middle("Ejection");
    const to = middle("Diastole");
    fireEvent.keyDown(canvas, { key: "a" });
    fireEvent.pointerDown(canvas, onPage(from.x, from.y));
    fireEvent.pointerMove(canvas, onPage(to.x, to.y));
    fireEvent.pointerUp(canvas, onPage(to.x, to.y));
    await waitFor(() => {
      const arrow = vi.mocked(notebooksIpc.applyNotebookItemChanges).mock.calls
        .flatMap((call) => call[1])
        .find((change) => change.op === "put" && change.shape.kind === "arrow");
      expect(arrow?.op === "put" && arrow.shape.kind === "arrow" ? [arrow.shape.from, arrow.shape.to] : null)
        .toEqual([topics().get("Ejection")?.id, topics().get("Diastole")?.id]);
    });
  });
});
