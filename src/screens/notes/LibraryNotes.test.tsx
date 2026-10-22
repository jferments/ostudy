import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
}));

const SUBJECT = "5".repeat(32);

function summary(seed: string, title: string, subjectId: string | null): notebooksIpc.NotebookSummary {
  const id = seed.repeat(32);
  return {
    notebook: { id, subjectId, title, paper: "lined", pageSize: "letter", revision: 1, createdAt: 1, updatedAt: 1 },
    pageCount: 3,
    linkCount: 1,
    firstPage: { id: seed === "a" ? "1".repeat(32) : "2".repeat(32), position: 1, paper: "lined", width: 612, height: 792 },
    preview: [],
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
});

describe("Library notes", () => {
  it("shows the chosen subject's notebooks with their pages and links", async () => {
    renderNotes({ kind: "subject", subjectId: SUBJECT });
    expect(await screen.findByRole("button", { name: "Open Cardiac cycle" })).toBeTruthy();
    expect(screen.getByText("3 pages · 1 link")).toBeTruthy();
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
});
