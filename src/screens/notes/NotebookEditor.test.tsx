import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as assetsIpc from "../../ipc/assets";
import * as decksIpc from "../../ipc/decks";
import * as documentsIpc from "../../ipc/documents";
import * as notebooksIpc from "../../ipc/notebooks";
import * as studyGuidesIpc from "../../ipc/studyGuides";
import * as files from "../../platform/files";
import { LayoutModeProvider } from "../../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
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
}));
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
vi.mock("../../ipc/decks", () => ({ listDecks: vi.fn() }));
vi.mock("../../ipc/documents", () => ({ listDocuments: vi.fn() }));
vi.mock("../../ipc/studyGuides", () => ({ listStudyGuidePage: vi.fn(), readDocumentSourceAsset: vi.fn() }));

const notebookId = "b".repeat(32);
const deckId = "c".repeat(32);
const PAGE_TOP = 100;
const PAGE_GAP = 20;

function page(seed: string, position: number): notebooksIpc.NotebookPage {
  return { id: seed.repeat(32), position, paper: "grid", width: 612, height: 792 };
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

async function openNotebook(overrides: Partial<Parameters<typeof NotebookEditor>[0]> = {}) {
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
  const canvas = screen.getByRole("group", { name: "Pages of Cardiac cycle" });
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
});
