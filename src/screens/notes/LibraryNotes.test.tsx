import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as notebooksIpc from "../../ipc/notebooks";
import { LayoutModeProvider } from "../../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { LibraryNotes } from "./LibraryNotes";

vi.mock("../../ipc/notebooks", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../ipc/notebooks")>(),
  listNotebooks: vi.fn(),
  createNotebook: vi.fn(),
  getNotebook: vi.fn(),
  deleteNotebook: vi.fn(),
  listNoteFolders: vi.fn(),
  createNoteFolder: vi.fn(),
  putNotebookInFolder: vi.fn(),
  pinNotebook: vi.fn(),
}));

const SUBJECT = "5".repeat(32);

const FOLDER = "f".repeat(32);

function summary(
  seed: string,
  title: string,
  subjectId: string | null,
  shelf: Partial<Pick<notebooksIpc.NotebookSummary, "linkLabels" | "folderId" | "pinnedAt">> = {},
): notebooksIpc.NotebookSummary {
  const id = seed.repeat(32);
  return {
    notebook: { id, subjectId, title, paper: "lined", pageSize: "letter", revision: 1, createdAt: 1, updatedAt: 1 },
    pageCount: 3,
    linkCount: 1,
    firstPage: { id: seed === "a" ? "1".repeat(32) : "2".repeat(32), position: 1, paper: "lined", width: 612, height: 792 },
    preview: [],
    linkLabels: [],
    folderId: null,
    pinnedAt: null,
    ...shelf,
  };
}

function contentsOf(item: notebooksIpc.NotebookSummary): notebooksIpc.NotebookContents {
  return { notebook: item.notebook, pages: item.firstPage ? [item.firstPage] : [], items: [], unavailableLinkIds: [], clipDocuments: {} };
}

function renderNotes(subject: Parameters<typeof LibraryNotes>[0]["subject"]) {
  render(
    <LayoutModeProvider>
      <OverlayProvider>
        <LibraryNotes subject={subject} showToast={vi.fn()} onPracticeDeck={vi.fn()} onOpenGuide={vi.fn()} />
      </OverlayProvider>
    </LayoutModeProvider>,
  );
}

beforeEach(() => {
  vi.mocked(notebooksIpc.listNotebooks).mockResolvedValue([
    summary("a", "Cardiac cycle", SUBJECT),
    summary("b", "Loose ideas", null),
  ]);
  vi.mocked(notebooksIpc.listNoteFolders).mockResolvedValue([]);
});

async function chooseAction(menu: string, name: string): Promise<void> {
  const trigger = await screen.findByRole("button", { name: menu });
  for (let attempt = 0; attempt < 3 && trigger.getAttribute("aria-expanded") !== "true"; attempt += 1) {
    await act(async () => {
      fireEvent.click(trigger);
    });
  }
  fireEvent.click(await within(await screen.findByRole("menu")).findByRole("menuitem", { name }));
}

describe("Library notes", () => {
  it("shows the chosen subject's notebooks with their pages and links", async () => {
    renderNotes({ kind: "subject", subjectId: SUBJECT });
    expect(await screen.findByRole("button", { name: "Open Cardiac cycle" })).toBeTruthy();
    expect(screen.getByText(/3 pages · 1 link$/u)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Open Loose ideas" })).toBeNull();
  });

  it("has nothing to show for a subject without notes but New notebook", async () => {
    renderNotes({ kind: "subject", subjectId: "6".repeat(32) });
    expect(await screen.findByText("No notes yet")).toBeTruthy();
    expect(screen.getByRole("button", { name: "New notebook" })).toBeTruthy();
  });

  it("creates a notebook in the chosen subject and opens it", async () => {
    const created = summary("c", "Valves", SUBJECT);
    vi.mocked(notebooksIpc.createNotebook).mockResolvedValue(contentsOf(created));
    vi.mocked(notebooksIpc.getNotebook).mockResolvedValue(contentsOf(created));
    renderNotes({ kind: "subject", subjectId: SUBJECT });
    fireEvent.click(await screen.findByRole("button", { name: "New notebook" }));
    const dialog = await screen.findByRole("dialog", { name: "New notebook" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: /Name/ }), { target: { value: "  Valves " } });
    fireEvent.click(within(dialog).getByRole("radio", { name: "Grid" }));
    fireEvent.click(within(dialog).getByRole("radio", { name: "Wide" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Create notebook" }));

    await waitFor(() => expect(notebooksIpc.createNotebook).toHaveBeenCalledWith({
      title: "Valves", subjectId: SUBJECT, paper: "grid", pageSize: "wide",
    }));
    expect(await screen.findByRole("toolbar", { name: "Annotation tools" })).toBeTruthy();
  });

  it("opens the notebook a document page was clipped into, whatever subject is chosen", async () => {
    const item = summary("a", "Cardiac cycle", SUBJECT);
    vi.mocked(notebooksIpc.getNotebook).mockResolvedValue(contentsOf(item));
    const onOpenRequestConsumed = vi.fn();
    render(
      <LayoutModeProvider>
        <OverlayProvider>
          <LibraryNotes subject={{ kind: "none" }} showToast={vi.fn()} onPracticeDeck={vi.fn()} onOpenGuide={vi.fn()}
            openRequest={{ requestId: 7, notebookId: item.notebook.id, pageId: "1".repeat(32) }}
            onOpenRequestConsumed={onOpenRequestConsumed} />
        </OverlayProvider>
      </LayoutModeProvider>,
    );
    expect(await screen.findByRole("toolbar", { name: "Annotation tools" })).toBeTruthy();
    expect(notebooksIpc.getNotebook).toHaveBeenCalledWith(item.notebook.id);
    expect(onOpenRequestConsumed).toHaveBeenCalledWith(7);
  });

  it("shows a notebook's links and pins it to the top", async () => {
    vi.mocked(notebooksIpc.listNotebooks).mockResolvedValue([
      summary("a", "Cardiac cycle", SUBJECT, { linkLabels: ["Costanzo p. 12", "Valves deck"] }),
    ]);
    vi.mocked(notebooksIpc.pinNotebook).mockResolvedValue(undefined);
    renderNotes({ kind: "all" });
    const links = await screen.findByRole("list", { name: "Links" });
    expect(within(links).getByText("Costanzo p. 12")).toBeTruthy();
    expect(within(links).getByText("Valves deck")).toBeTruthy();

    await chooseAction("Cardiac cycle actions", "Pin");
    await waitFor(() => expect(notebooksIpc.pinNotebook).toHaveBeenCalledWith("a".repeat(32), true));
  });

  it("shows a folder's notebooks and only the pinned ones under Pinned", async () => {
    vi.mocked(notebooksIpc.listNotebooks).mockResolvedValue([
      summary("a", "Cardiac cycle", SUBJECT, { folderId: FOLDER, pinnedAt: 5 }),
      summary("b", "Loose ideas", null),
    ]);
    vi.mocked(notebooksIpc.listNoteFolders).mockResolvedValue([
      { id: FOLDER, parentId: null, name: "Physiology", sortOrder: 0 },
    ]);
    renderNotes({ kind: "all" });
    const nav = await screen.findByRole("navigation", { name: "Note folders" });
    fireEvent.click(await within(nav).findByRole("button", { name: /^Physiology/u }));
    expect(await screen.findByRole("heading", { level: 2, name: "Physiology" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open Cardiac cycle" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Open Loose ideas" })).toBeNull();

    fireEvent.click(within(nav).getByRole("button", { name: /^Pinned/u }));
    expect(await screen.findByRole("heading", { level: 2, name: "Pinned" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open Cardiac cycle" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Open Loose ideas" })).toBeNull();
  });

  it("makes a folder and moves a notebook into it", async () => {
    const folder = { id: FOLDER, parentId: null, name: "Physiology", sortOrder: 0 };
    vi.mocked(notebooksIpc.createNoteFolder).mockResolvedValue(folder);
    vi.mocked(notebooksIpc.putNotebookInFolder).mockResolvedValue(undefined);
    renderNotes({ kind: "all" });
    const nav = await screen.findByRole("navigation", { name: "Note folders" });
    fireEvent.click(within(nav).getByRole("button", { name: "New folder" }));
    const dialog = await screen.findByRole("dialog", { name: "New folder" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Name" }), { target: { value: " Physiology " } });
    vi.mocked(notebooksIpc.listNoteFolders).mockResolvedValue([folder]);
    fireEvent.click(within(dialog).getByRole("button", { name: "Create folder" }));
    await waitFor(() => expect(notebooksIpc.createNoteFolder).toHaveBeenCalledWith("Physiology", null));
    // The new folder opens, empty.
    expect(await screen.findByRole("heading", { level: 2, name: "Physiology" })).toBeTruthy();
    expect(await within(nav).findByRole("button", { name: /^Physiology/u })).toBeTruthy();

    fireEvent.click(within(nav).getByRole("button", { name: /^All notes/u }));
    await chooseAction("Cardiac cycle actions", "Move to folder");
    const move = await screen.findByRole("dialog", { name: "Move Cardiac cycle" });
    fireEvent.change(within(move).getByRole("combobox", { name: "Folder" }), { target: { value: FOLDER } });
    fireEvent.click(within(move).getByRole("button", { name: "Move" }));
    await waitFor(() => expect(notebooksIpc.putNotebookInFolder).toHaveBeenCalledWith("a".repeat(32), FOLDER));
  });
});
