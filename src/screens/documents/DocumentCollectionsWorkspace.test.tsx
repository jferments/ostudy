import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as collections from "../../ipc/collections";
import type { DocumentSummary } from "../../ipc/documents";
import * as studyMaterials from "../../ipc/studyMaterials";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { DocumentCollectionsWorkspace } from "./DocumentCollectionsWorkspace";

vi.mock("../../ipc/collections", () => ({
  getDocumentCollectionOrganization: vi.fn(),
  createDocumentCollection: vi.fn(),
  updateDocumentCollection: vi.fn(),
  deleteDocumentCollection: vi.fn(),
  addDocumentCollectionMembership: vi.fn(),
  moveDocumentCollectionMembership: vi.fn(),
  removeDocumentCollectionMembership: vi.fn(),
  setDocumentCollectionRanges: vi.fn(),
  resolveDocumentSourceSelection: vi.fn(),
}));
vi.mock("../../ipc/studyMaterials", () => ({
  getStudyTargetBundle: vi.fn(),
  beginStudyTargetBundle: vi.fn(),
  composeStudyTargetBundle: vi.fn(),
  cancelStudyTargetBundle: vi.fn(),
}));
vi.mock("../../ipc/common", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../ipc/common")>();
  return { ...actual, createOperationId: () => "f".repeat(32) };
});

const folderId = "a".repeat(32);
const documentId = "b".repeat(32);
const membershipId = "c".repeat(32);

const organization: collections.DocumentCollectionOrganization = {
  revision: 2,
  collections: [{
    id: folderId,
    name: "Week 1",
    sortOrder: 1000,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
  }],
  memberships: [{
    id: membershipId,
    collectionId: folderId,
    documentId,
    sortOrder: 1000,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
  }],
  ranges: [{
    id: "d".repeat(32),
    membershipId,
    unitKind: "page",
    startUnit: 1,
    endUnit: 3,
    sourceSha256: "e".repeat(64),
    createdAt: 1,
  }],
};

const bundleStatus: studyMaterials.StudyTargetBundleStatus = {
  targetId: "t".repeat(32),
  target: { kind: "collection", id: folderId },
  title: "Week 1",
  revision: 2,
  sourceSnapshotId: "s".repeat(64),
  sourceChanged: false,
  deckId: "9".repeat(32),
  guideId: "8".repeat(32),
  guideRevision: 1,
  latestOperationId: "f".repeat(32),
  latestBatchState: "complete",
  flashcards: {
    kind: "flashcards",
    state: "succeeded",
    artifactId: "9".repeat(32),
    totalUnits: 1,
    completedUnits: 1,
    safeErrorCode: "",
    safeErrorMessage: "",
  },
  studyGuide: {
    kind: "study_guide",
    state: "succeeded",
    artifactId: "8".repeat(32),
    totalUnits: 1,
    completedUnits: 1,
    safeErrorCode: "",
    safeErrorMessage: "",
  },
};

const document: DocumentSummary = {
  id: documentId,
  title: "Cardiac physiology",
  originalFilename: "cardiac.pdf",
  byteSize: 100,
  sha256: "e".repeat(64),
  lifecycleState: "ready",
  sourceFormat: "pdf",
  sourceUnitKind: "page",
  sourceUnitCount: 10,
  pageCount: 10,
  extractedPageCount: 10,
  renderedPageCount: 10,
  extractionMode: "native",
  ocrPageCount: 0,
  ocrCompletedPageCount: 0,
  ocrMeanConfidence: null,
  ocrEngine: "",
  scaleMode: "standard",
  estimatedSourceTokens: 10,
  outlineWindowCount: 1,
  outlineCompletedWindowCount: 1,
  generationWindowCount: 1,
  generationCompletedWindowCount: 1,
  generatedNoteCount: 0,
  generatedCardCount: 0,
  generatedDeckId: null,
  safeErrorCode: "",
  safeErrorMessage: "",
  createdAt: 1,
  updatedAt: 1,
  completedAt: 1,
  revision: 1,
  processingDurationMs: null,
  apiTokenCount: null,
  apiTokenUsageComplete: false,
};

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(collections.getDocumentCollectionOrganization).mockResolvedValue(organization);
  vi.mocked(collections.createDocumentCollection).mockResolvedValue({
    ...organization,
    revision: 3,
    collections: [...organization.collections, {
      id: "1".repeat(32), name: "Week 2", sortOrder: 2000,
createdAt: 2, updatedAt: 2, revision: 1,
    }],
  });
  vi.mocked(collections.addDocumentCollectionMembership).mockResolvedValue(organization);
  vi.mocked(collections.setDocumentCollectionRanges).mockResolvedValue(organization);
  vi.mocked(studyMaterials.getStudyTargetBundle).mockResolvedValue(null);
  vi.mocked(studyMaterials.beginStudyTargetBundle).mockResolvedValue({
    ...bundleStatus,
    deckId: null,
    guideId: null,
    guideRevision: null,
    latestBatchState: "queued",
    flashcards: { ...bundleStatus.flashcards, state: "pending", artifactId: null },
    studyGuide: { ...bundleStatus.studyGuide, state: "pending", artifactId: null },
  });
  vi.mocked(studyMaterials.composeStudyTargetBundle).mockResolvedValue(bundleStatus);
  vi.mocked(studyMaterials.cancelStudyTargetBundle).mockResolvedValue(undefined);
});

function renderWorkspace({
  refreshDecks = vi.fn().mockResolvedValue([]),
  onOpenDeck = vi.fn(),
  onOpenGuide = vi.fn(),
}: {
  refreshDecks?: () => Promise<unknown>;
  onOpenDeck?: (deckId: string) => void;
  onOpenGuide?: (collectionId: string, title: string) => void;
} = {}) {
  render(
    <OverlayProvider>
      <DocumentCollectionsWorkspace
        documents={[document]}
        refreshDecks={refreshDecks}
        onOpenDeck={onOpenDeck}
        onOpenGuide={onOpenGuide}
      />
    </OverlayProvider>,
  );
  return { refreshDecks, onOpenDeck, onOpenGuide };
}

describe("Document collections workspace", () => {
  it("keeps collection discovery quiet and preserves the empty chrome geometry", async () => {
    let finish!: (value: collections.DocumentCollectionOrganization) => void;
    vi.mocked(collections.getDocumentCollectionOrganization).mockReturnValueOnce(
      new Promise((resolve) => { finish = resolve; }),
    );
    renderWorkspace();

    expect(
      (screen.getByRole("button", { name: "+ New collection" }) as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(screen.queryByRole("heading", { name: "Document collections" })).toBeNull();
    expect(screen.queryByText("Loading collections…")).toBeNull();

    await act(async () => {
      finish({ revision: 3, collections: [], memberships: [], ranges: [] });
      await Promise.resolve();
    });
    await waitFor(() => {
      expect((screen.getByRole("button", { name: "+ New collection" }) as HTMLButtonElement).disabled)
        .toBe(false);
    });
  });

  it("keeps an empty collection area to the single create action", async () => {
    vi.mocked(collections.getDocumentCollectionOrganization).mockResolvedValueOnce({
      revision: 3,
      collections: [],
      memberships: [],
      ranges: [],
    });
    renderWorkspace();

    expect(await screen.findByRole("button", { name: "+ New collection" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Document collections" })).toBeNull();
    expect(screen.queryByText("No collections yet.")).toBeNull();
    expect(screen.queryByText(/Organize related sources/iu)).toBeNull();
    expect(studyMaterials.getStudyTargetBundle).not.toHaveBeenCalled();
  });

  it("browses collections locally and exposes canonical source ranges without resolving or preparing them", async () => {
    renderWorkspace();
    expect(await screen.findByRole("heading", { name: "Week 1" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "← Collections" })).toBeNull();
    expect(screen.queryByText(/subfolder/iu)).toBeNull();
    expect(screen.queryByText(/Folder organization is local/iu)).toBeNull();
    expect((await screen.findAllByText("Pages 1–3")).length).toBeGreaterThan(0);
    expect(collections.getDocumentCollectionOrganization).toHaveBeenCalledTimes(1);
    await waitFor(() => {
      expect(studyMaterials.getStudyTargetBundle).toHaveBeenCalledWith({
        target: { kind: "collection", id: folderId },
      });
    });
    expect(collections.resolveDocumentSourceSelection).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Edit range" }));
    const dialog = await screen.findByRole("dialog", { name: "Reading range" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Pages" }), {
      target: { value: "2-4, 7" },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save range" }));
    await waitFor(() => {
      expect(collections.setDocumentCollectionRanges).toHaveBeenCalledWith({
        operationId: "f".repeat(32),
        membershipId,
        ranges: [
          { unitKind: "page", startUnit: 2, endUnit: 4 },
          { unitKind: "page", startUnit: 7, endUnit: 7 },
        ],
        revision: 1,
        expectedOrganizationRevision: 2,
      });
    });
  });

  it("shows determinate collection-generation progress from durable artifact status", async () => {
    let finish!: (value: studyMaterials.StudyTargetBundleStatus) => void;
    const running = {
      ...bundleStatus,
      deckId: null,
      guideId: null,
      guideRevision: null,
      latestBatchState: "composing",
      flashcards: { ...bundleStatus.flashcards, state: "succeeded", artifactId: "9".repeat(32) },
      studyGuide: {
        ...bundleStatus.studyGuide,
        state: "running",
        artifactId: null,
        totalUnits: 8,
        completedUnits: 3,
      },
    };
    vi.mocked(studyMaterials.beginStudyTargetBundle).mockResolvedValueOnce(running);
    vi.mocked(studyMaterials.composeStudyTargetBundle).mockReturnValueOnce(
      new Promise((resolve) => { finish = resolve; }),
    );
    renderWorkspace();

    await screen.findByRole("heading", { name: "Week 1" });
    fireEvent.click(await screen.findByRole("button", { name: "Create study materials" }));

    expect(await screen.findByText("Writing collection study guide")).toBeTruthy();
    expect(screen.getByText("3 / 8")).toBeTruthy();
    const progress = screen.getByRole("progressbar", { name: "Collection study-material generation progress" });
    expect(progress.getAttribute("aria-valuenow")).toBe("3");
    expect(progress.getAttribute("aria-valuemax")).toBe("8");

    await act(async () => {
      finish(bundleStatus);
      await Promise.resolve();
    });
  });

  it("stops the active native collection operation without leaving the page", async () => {
    let finish!: (value: studyMaterials.StudyTargetBundleStatus) => void;
    const running = {
      ...bundleStatus,
      latestBatchState: "composing",
      flashcards: { ...bundleStatus.flashcards, state: "succeeded" },
      studyGuide: { ...bundleStatus.studyGuide, state: "running", totalUnits: 5, completedUnits: 2 },
    };
    vi.mocked(studyMaterials.beginStudyTargetBundle).mockResolvedValueOnce(running);
    vi.mocked(studyMaterials.composeStudyTargetBundle).mockReturnValueOnce(
      new Promise((resolve) => { finish = resolve; }),
    );
    renderWorkspace();

    await screen.findByRole("heading", { name: "Week 1" });
    fireEvent.click(screen.getByRole("button", { name: "Create study materials" }));
    const stop = await screen.findByRole("button", { name: "Stop collection generation" });
    fireEvent.click(stop);

    await waitFor(() => {
      expect(studyMaterials.cancelStudyTargetBundle).toHaveBeenCalledWith("f".repeat(32));
    });

    await act(async () => {
      finish({ ...running, latestBatchState: "canceled",
        studyGuide: { ...running.studyGuide, state: "canceled" } });
      await Promise.resolve();
    });
  });

  it("creates one coherent bundle and opens its ordinary flashcard deck from compact status", async () => {
    const refreshDecks = vi.fn().mockResolvedValue([]);
    const onOpenDeck = vi.fn();
    const onOpenGuide = vi.fn();
    renderWorkspace({ refreshDecks, onOpenDeck, onOpenGuide });

    await screen.findByRole("heading", { name: "Week 1" });
    fireEvent.click(await screen.findByRole("button", { name: "Create study materials" }));

    await waitFor(() => {
      expect(studyMaterials.beginStudyTargetBundle).toHaveBeenCalledWith({
        operationId: "f".repeat(32),
        target: { kind: "collection", id: folderId },
        outputPolicy: { flashcards: true, studyGuide: true },
        recreateArtifacts: [],
      });
      expect(studyMaterials.composeStudyTargetBundle).toHaveBeenCalledWith({
        operationId: "f".repeat(32),
      });
    });
    expect(refreshDecks).toHaveBeenCalledTimes(1);
    const open = await screen.findByRole("button", { name: "Open Flashcards deck" });
    expect(open.textContent).toContain("Ready");
    fireEvent.click(open);
    expect(onOpenDeck).toHaveBeenCalledWith("9".repeat(32));
    const openGuide = screen.getByRole("button", { name: "Open collection study guide" });
    expect(openGuide.textContent).toContain("Ready");
    fireEvent.click(openGuide);
    expect(onOpenGuide).toHaveBeenCalledWith(folderId, "Week 1");
  });

  it("uses the committed mutation result directly instead of refetching after a folder write", async () => {
    renderWorkspace();
    await screen.findByRole("button", { name: "+ New collection" });
    await waitFor(() => {
      expect((screen.getByRole("button", { name: "+ New collection" }) as HTMLButtonElement).disabled)
        .toBe(false);
    });
    fireEvent.click(screen.getByRole("button", { name: "+ New collection" }));
    const dialog = await screen.findByRole("dialog", { name: "Create document collection" });
    expect(dialog.getAttribute("data-width")).toBe("small");
    const body = dialog.querySelector("[data-collection-dialog-body]");
    expect(body?.className).toContain("tw:p-4");
    expect(body?.className).toContain("tw:min-w-0");
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Name" }), {
      target: { value: "Week 2" },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("heading", { name: "Week 2" })).toBeTruthy();
    expect(collections.createDocumentCollection).toHaveBeenCalledWith(expect.objectContaining({
      operationId: "f".repeat(32),
      name: "Week 2",
        expectedOrganizationRevision: 2,
    }));
    expect(collections.getDocumentCollectionOrganization).toHaveBeenCalledTimes(1);
  });

  it("adds an existing document by dropping it on a folder without source resolution or preparation", async () => {
    const empty = { ...organization, revision: 3, memberships: [], ranges: [] };
    const added = {
      ...empty,
      revision: 4,
      memberships: [{ ...organization.memberships[0], collectionId: folderId }],
    };
    vi.mocked(collections.getDocumentCollectionOrganization).mockResolvedValueOnce(empty);
    vi.mocked(collections.addDocumentCollectionMembership).mockResolvedValueOnce(added);
    renderWorkspace();

    const values = new Map<string, string>([
      ["application/x-ostudy-document-id", documentId],
    ]);
    const transfer = {
      effectAllowed: "copy",
      dropEffect: "none",
      get types() { return Array.from(values.keys()); },
      setData(type: string, value: string) { values.set(type, value); },
      getData(type: string) { return values.get(type) ?? ""; },
    } as unknown as DataTransfer;

    const dropTarget = await waitFor(() => {
      const element = globalThis.document.querySelector(`[data-document-folder-drop="${folderId}"]`);
      expect(element).toBeTruthy();
      return element as HTMLElement;
    });
    fireEvent.dragOver(dropTarget, { dataTransfer: transfer });
    fireEvent.drop(dropTarget, { dataTransfer: transfer });

    await waitFor(() => {
      expect(collections.addDocumentCollectionMembership).toHaveBeenCalledWith({
        operationId: "f".repeat(32),
        collectionId: folderId,
        documentId,
        expectedOrganizationRevision: 3,
      });
    });
    expect(collections.resolveDocumentSourceSelection).not.toHaveBeenCalled();
    expect(studyMaterials.beginStudyTargetBundle).not.toHaveBeenCalled();
    expect(studyMaterials.composeStudyTargetBundle).not.toHaveBeenCalled();
  });

});
