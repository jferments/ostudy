import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";

import * as documents from "../ipc/documents";
import * as externalFiles from "../ipc/externalFiles";
import * as studyMaterials from "../ipc/studyMaterials";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { CollectionStudyGuideDialog } from "./CollectionStudyGuideDialog";

vi.mock("../ipc/documents", () => ({ openDocumentSource: vi.fn() }));
vi.mock("../ipc/externalFiles", () => ({
  publishPreparedExport: vi.fn(),
  releasePreparedExport: vi.fn(),
}));
vi.mock("../ipc/studyMaterials", () => ({
  getCollectionStudyGuide: vi.fn(),
  prepareCollectionStudyGuidePdf: vi.fn(),
  saveCollectionStudyGuideOverlay: vi.fn(),
  discardCollectionStudyGuideOverlay: vi.fn(),
}));

const collectionId = "a".repeat(32);
const documentId = "b".repeat(32);

const guide: studyMaterials.CollectionStudyGuide = {
  id: "c".repeat(32),
  targetId: "d".repeat(32),
  collectionId,
  title: "Cardiovascular unit",
  revision: 2,
  sourceSnapshotId: "e".repeat(64),
  createdAt: 1,
  updatedAt: 2,
  publishedAt: 2,
  overlays: [],
  content: {
    schemaVersion: 1,
    collectionId,
    title: "Cardiovascular unit",
    overview: {
      summary: "Connect preload, afterload, and cardiac output.",
      learningObjectives: ["Compare preload and afterload."],
      keyConcepts: ["Preload", "Afterload"],
    },
    coverageNotes: ["Prepared knowledge exists without a current internal coverage receipt."],
    sections: [{
      blockKey: "f".repeat(32),
      title: "Preload",
      contentHash: "0".repeat(64),
      sourceDocumentId: documentId,
      sourceDocumentTitle: "Cardiac physiology",
      sourceGuideId: "1".repeat(32),
      sourceTopicId: "2".repeat(32),
      sourceLocator: { unitKind: "page", unitNumbers: [2, 3] },
      content: {
        overview: "Preload reflects ventricular filling before systole.",
        keyPoints: ["Venous return influences preload."],
        explanations: [],
        importantTerms: [],
        processSteps: [],
        comparisons: [],
        commonMisconceptions: [],
        callouts: [{ kind: "key_idea", title: "Remember", body: "Preload is a filling concept." }],
        tables: [{
          title: "Hemodynamic comparison",
          columns: ["Concept", "Meaning"],
          rows: [["Preload", "Filling"]],
          caption: "A compact comparison.",
        }],
        workedExamples: [],
        retrievalChecks: [{ prompt: "What raises preload?", answer: "Greater venous return." }],
        visualExplanations: [],
      },
    }],
  },
};

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(studyMaterials.getCollectionStudyGuide).mockResolvedValue(guide);
  vi.mocked(documents.openDocumentSource).mockResolvedValue(undefined);
  vi.mocked(studyMaterials.prepareCollectionStudyGuidePdf).mockResolvedValue({
    preparedExport: {
      id: "3".repeat(32),
      purpose: "collection_study_guide_pdf",
      suggestedFilename: "cardiovascular-unit-study-guide.pdf",
      mediaType: "application/pdf",
      byteSize: 100,
      sha256: "4".repeat(64),
      expiresAt: Date.now() + 60_000,
    },
    guideRevision: 2,
    sectionCount: 1,
    annotationCount: 0,
    mathSourceFallback: false,
  });
  vi.mocked(externalFiles.publishPreparedExport).mockResolvedValue({
    displayName: "cardiovascular-unit-study-guide.pdf",
  } as never);
  vi.mocked(externalFiles.releasePreparedExport).mockResolvedValue(undefined);
  vi.mocked(studyMaterials.saveCollectionStudyGuideOverlay).mockResolvedValue({
    id: "5".repeat(32),
    guideId: guide.id,
    targetId: guide.targetId,
    blockKey: guide.content.sections[0].blockKey,
    overlayKind: "body_override",
    baseContentHash: guide.content.sections[0].contentHash,
    richDoc: { type: "doc", content: [] },
    plainText: "Edited preload section.",
    state: "active",
    revision: 1,
    createdAt: 3,
    updatedAt: 3,
  });
  vi.mocked(studyMaterials.discardCollectionStudyGuideOverlay).mockResolvedValue(undefined);
});

it("renders collection sections through the shared semantic reader with one overflow menu", async () => {
  render(
    <OverlayProvider>
      <CollectionStudyGuideDialog
        collectionId={collectionId}
        collectionTitle="Cardiovascular unit"
        showToast={vi.fn()}
        onClose={vi.fn()}
      />
    </OverlayProvider>,
  );

  expect(await screen.findByRole("heading", { name: "Preload" })).toBeTruthy();
  expect(screen.getByText("Hemodynamic comparison")).toBeTruthy();
  expect(screen.getByText("Remember")).toBeTruthy();
  expect(screen.getByText("What raises preload?")).toBeTruthy();
  expect(screen.getByText("Source-grounded sections synthesized from the collection’s retained document knowledge.")).toBeTruthy();
  expect(screen.queryByText(/Prepared knowledge exists without/iu)).toBeNull();
  expect(screen.queryByRole("heading", { name: "Coverage notes" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Open source" })).toBeNull();

  fireEvent.click(screen.getByRole("button", { name: /Section actions for Preload/u }));
  const menu = await screen.findByRole("menu", { name: /Section actions for Preload/u });
  fireEvent.click(within(menu).getByText("Open source"));
  await waitFor(() => expect(documents.openDocumentSource).toHaveBeenCalledWith(documentId));
});

it("exports the same collection guide snapshot through the native PDF boundary", async () => {
  const showToast = vi.fn();
  render(
    <OverlayProvider>
      <CollectionStudyGuideDialog
        collectionId={collectionId}
        collectionTitle="Cardiovascular unit"
        showToast={showToast}
        onClose={vi.fn()}
      />
    </OverlayProvider>,
  );

  fireEvent.click(await screen.findByRole("button", { name: "Export PDF" }));
  await waitFor(() => {
    expect(studyMaterials.prepareCollectionStudyGuidePdf).toHaveBeenCalledWith(collectionId, true);
    expect(externalFiles.publishPreparedExport).toHaveBeenCalledWith("3".repeat(32));
    expect(showToast).toHaveBeenCalledWith(
      "success",
      "Exported collection study guide with 1 section.",
    );
  });
  expect(externalFiles.releasePreparedExport).toHaveBeenCalledWith("3".repeat(32));
});


it("saves source-bound section overrides from the overflow menu with the exact base hash", async () => {
  vi.mocked(studyMaterials.getCollectionStudyGuide)
    .mockResolvedValueOnce(guide)
    .mockResolvedValue(guide);
  render(
    <OverlayProvider>
      <CollectionStudyGuideDialog
        collectionId={collectionId}
        collectionTitle="Cardiovascular unit"
        showToast={vi.fn()}
        onClose={vi.fn()}
      />
    </OverlayProvider>,
  );

  fireEvent.click(await screen.findByRole("button", { name: /Section actions for Preload/u }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Edit section" }));
  const editor = screen.getByRole("textbox", { name: "Edit generated section" });
  fireEvent.change(editor, { target: { value: "Edited preload section." } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));

  await waitFor(() => {
    expect(studyMaterials.saveCollectionStudyGuideOverlay).toHaveBeenCalledWith(
      expect.objectContaining({
        collectionId,
        guideId: guide.id,
        blockKey: guide.content.sections[0].blockKey,
        overlayKind: "body_override",
        baseContentHash: guide.content.sections[0].contentHash,
        plainText: "Edited preload section.",
      }),
    );
  });
});

it("keeps section note actions inside the overflow menu", async () => {
  render(
    <OverlayProvider>
      <CollectionStudyGuideDialog
        collectionId={collectionId}
        collectionTitle="Cardiovascular unit"
        showToast={vi.fn()}
        onClose={vi.fn()}
      />
    </OverlayProvider>,
  );
  expect(await screen.findByRole("heading", { name: "Preload" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Add note" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: /Section actions for Preload/u }));
  expect(await screen.findByRole("menuitem", { name: "Add note" })).toBeTruthy();
  expect(screen.getByRole("menuitem", { name: "Edit section" })).toBeTruthy();
});
