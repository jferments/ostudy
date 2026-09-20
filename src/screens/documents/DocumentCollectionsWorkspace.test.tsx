import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
    parentId: null,
    name: "Week 1",
    sortOrder: 1000,
    archivedAt: null,
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
      id: "1".repeat(32), parentId: null, name: "Week 2", sortOrder: 2000,
      archivedAt: null, createdAt: 2, updatedAt: 2, revision: 1,
    }],
  });
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
  it("browses folders locally and exposes canonical source ranges without resolving or preparing them", async () => {
    renderWorkspace();
    fireEvent.click(await screen.findByRole("button", { name: /Week 1/ }));
    expect(screen.getByRole("navigation", { name: "Document folder breadcrumbs" })).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "Sources" }));
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

  it("creates one coherent bundle and opens its ordinary flashcard deck from compact status", async () => {
    const refreshDecks = vi.fn().mockResolvedValue([]);
    const onOpenDeck = vi.fn();
    const onOpenGuide = vi.fn();
    renderWorkspace({ refreshDecks, onOpenDeck, onOpenGuide });

    fireEvent.click(await screen.findByRole("button", { name: /Week 1/ }));
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
    fireEvent.click(await screen.findByRole("button", { name: "+ New folder" }));
    const dialog = await screen.findByRole("dialog", { name: "Create document folder" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Name" }), {
      target: { value: "Week 2" },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("button", { name: /Week 2/ })).toBeTruthy();
    expect(collections.createDocumentCollection).toHaveBeenCalledWith(expect.objectContaining({
      operationId: "f".repeat(32),
      name: "Week 2",
      parentId: null,
      expectedOrganizationRevision: 2,
    }));
    expect(collections.getDocumentCollectionOrganization).toHaveBeenCalledTimes(1);
  });
});
