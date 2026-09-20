import { useState } from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DocumentJobsProvider } from "../../controllers/DocumentJobsProvider";
import type { DocumentSummary } from "../../ipc/documents";
import type { AmendImageSetInput, ImageSetDetails, ImportImageSetInput } from "../../ipc/imageSets";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { ImageSetWorkspace } from "./ImageSetWorkspace";

const mocks = vi.hoisted(() => ({
  stage: vi.fn(), import: vi.fn(), amend: vi.fn(), details: vi.fn(), preview: vi.fn(),
  prepare: vi.fn(), cards: vi.fn(), more: vi.fn(), guide: vi.fn(), list: vi.fn(),
  cancel: vi.fn(), cancelGuide: vi.fn(), toast: vi.fn(), refresh: vi.fn(),
}));
vi.mock("../../platform/imageInput", async (original) => ({
  ...await original<typeof import("../../platform/imageInput")>(), stageBrowserImage: mocks.stage,
}));
vi.mock("../../ipc/imageSets", () => ({
  importImageSet: mocks.import, amendImageSet: mocks.amend, getImageSet: mocks.details,
  getImageSetPreview: mocks.preview, prepareImageSet: mocks.prepare,
}));
vi.mock("../../ipc/documents", () => ({
  listDocuments: mocks.list, getDocumentGenerationProgress: vi.fn().mockResolvedValue(null),
  retryDocumentFlashcardGeneration: mocks.cards, retryPdfFlashcardGeneration: mocks.cards,
  generateMoreDocumentFlashcards: mocks.more, generateMorePdfFlashcards: mocks.more,
  cancelPdfFlashcardGeneration: mocks.cancel, deletePdfDocument: vi.fn(),
  importPdfAndGenerateFlashcards: vi.fn(), openDocumentSource: vi.fn(),
}));
vi.mock("../../ipc/studyGuides", () => ({
  generateDocumentStudyGuide: mocks.guide, cancelDocumentStudyGuideGeneration: mocks.cancelGuide,
}));

const id = "a".repeat(32);
let saved: ImageSetDetails | null;
let deckId: string | null;
function image(name = "lecture.png") { return new File(["pixels"], name, { type: "image/png" }); }
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((yes) => { resolve = yes; });
  return { promise, resolve };
}
function member(name: string, index: number) {
  return { id: index.toString(16).padStart(32, "0"), originalFilename: name,
    mediaType: "image/png", byteSize: 6, width: 80, height: 60,
    quarterTurns: 0 as const, inclusion: null, processing: null };
}
function Routes() {
  const [visible, setVisible] = useState(true);
  const [source, setSource] = useState<string | null>(null);
  return <>
    <button onClick={() => { setSource(saved?.documentId ?? null); setVisible(true); }}>Open collection</button>
    {visible ? <ImageSetWorkspace documentId={source} onClose={() => setVisible(false)} /> : <p>Another page</p>}
  </>;
}
function open() {
  render(<OverlayProvider><DocumentJobsProvider refreshDecks={mocks.refresh} showToast={mocks.toast}>
    <Routes />
  </DocumentJobsProvider></OverlayProvider>);
}
function select(files: File[]) {
  fireEvent.change(screen.getByLabelText("Choose images"), { target: { files } });
}
async function ready() {
  await waitFor(() => expect(screen.getByRole("status").textContent).toContain("Study materials are ready"));
}

beforeEach(() => {
  vi.resetAllMocks();
  saved = null;
  deckId = null;
  mocks.refresh.mockResolvedValue(undefined);
  mocks.cancel.mockResolvedValue(undefined);
  mocks.cancelGuide.mockResolvedValue(undefined);
  mocks.stage.mockImplementation(async (file: File, operationId: string) => ({
    id: operationId, displayName: file.name, purpose: "image", detectedKind: "png",
    mediaType: "image/png", byteSize: file.size, sha256: "d".repeat(64), expiresAt: 9999999999999,
  }));
  mocks.import.mockImplementation(async (input: ImportImageSetInput) => {
    saved = { documentId: id, title: input.title, revision: 1, filterRevision: 1,
      includeInterface: false, prepared: false,
      members: input.members.map((_, index) => member(`source-${index}.png`, index + 1)) };
    return { documentId: id, revision: 1 };
  });
  mocks.amend.mockImplementation(async (input: AmendImageSetInput) => {
    if (!saved) throw new Error("Missing source");
    saved = { ...saved, revision: saved.revision + 1, members: [
      ...saved.members, ...input.append.map((_, index) => member(`added-${index}.png`, saved!.members.length + index + 1)),
    ] };
    return { documentId: id, revision: saved.revision, filterRevision: 1 };
  });
  mocks.details.mockImplementation(async () => saved);
  mocks.prepare.mockImplementation(async () => {
    if (!saved) throw new Error("Missing source");
    saved = { ...saved, prepared: true };
    return { documentId: id, revision: saved.revision, filterRevision: 1 };
  });
  mocks.list.mockImplementation(async () => saved ? [{ id, title: saved.title,
    generatedDeckId: deckId, lifecycleState: "ready", revision: saved.revision,
  } as DocumentSummary] : []);
  mocks.cards.mockImplementation(async () => { deckId = "b".repeat(32); return { createdCardCount: 3 }; });
  mocks.more.mockResolvedValue({ createdCardCount: 2 });
  mocks.guide.mockResolvedValue({ revision: 1 });
  mocks.preview.mockResolvedValue({ revision: 1, width: 80, height: 60, dataUrl: "data:image/png;base64,cGl4ZWxz" });
});

describe("automatic image collections", () => {
  it("accepts multiple files and automatically creates cards and a guide without preparation controls", async () => {
    open();
    const input = screen.getByLabelText("Choose images") as HTMLInputElement;
    expect(input.multiple).toBe(true);
    select([image("second.png"), image("first.png")]);
    await ready();
    expect(mocks.stage).toHaveBeenCalledTimes(2);
    expect(mocks.import).toHaveBeenCalledTimes(1);
    expect(mocks.import.mock.calls[0][0].members).toHaveLength(2);
    expect(mocks.prepare).toHaveBeenCalledWith(id);
    expect(mocks.cards).toHaveBeenCalledWith(id);
    expect(mocks.guide).toHaveBeenCalledWith(id);
    expect(mocks.prepare.mock.invocationCallOrder[0]).toBeLessThan(mocks.cards.mock.invocationCallOrder[0]);
    expect(mocks.cards.mock.invocationCallOrder[0]).toBeLessThan(mocks.guide.mock.invocationCallOrder[0]);
    expect(screen.queryByRole("spinbutton")).toBeNull();
    expect(screen.queryByRole("combobox")).toBeNull();
    expect(screen.queryByText("Content filter")).toBeNull();
    expect(screen.queryByRole("button", { name: /Import image set|Rotate|Retry local OCR|Prepare/ })).toBeNull();
    const list = screen.getByRole("list", { name: "Images in collection" });
    const drop = screen.getByRole("button", { name: "Drop images here or select files" });
    expect(list.compareDocumentPosition(drop) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
    const click = vi.spyOn(input, "click");
    fireEvent.click(screen.getByRole("button", { name: "Add more images" }));
    expect(click).toHaveBeenCalledTimes(1);
  });

  it("accepts dropped and pasted images into the same source while ignoring pasted URLs", async () => {
    open();
    fireEvent.drop(screen.getByRole("button", { name: "Drop images here or select files" }), {
      dataTransfer: { files: [image("dropped.png")], dropEffect: "copy" },
    });
    await ready();
    fireEvent.paste(document, { clipboardData: { files: [image("pasted.png")], items: [] } });
    await waitFor(() => expect(mocks.amend).toHaveBeenCalledTimes(1));
    await ready();
    expect(mocks.import).toHaveBeenCalledTimes(1);
    expect(mocks.cards).toHaveBeenCalledTimes(1);
    expect(mocks.more).toHaveBeenCalledWith(id);
    expect(mocks.guide).toHaveBeenCalledTimes(2);
    fireEvent.paste(document, { clipboardData: { files: [], items: [{ kind: "string", type: "text/plain" }] } });
    expect(mocks.stage).toHaveBeenCalledTimes(2);
  });

  it("retains additions during generation and completes after leaving the upload page", async () => {
    const request = deferred<{ createdCardCount: number }>();
    mocks.cards.mockImplementation(async () => { const result = await request.promise; deckId = "b".repeat(32); return result; });
    open();
    select([image()]);
    await waitFor(() => expect(mocks.cards).toHaveBeenCalledTimes(1));
    select([image("later.png")]);
    expect(mocks.amend).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Back to Documents" }));
    expect(screen.getByText("Another page")).toBeTruthy();
    await act(async () => { request.resolve({ createdCardCount: 3 }); await request.promise; });
    await waitFor(() => expect(mocks.guide).toHaveBeenCalledTimes(1));
    expect(mocks.amend).toHaveBeenCalledTimes(1);
    expect(mocks.more).toHaveBeenCalledTimes(1);
    expect(mocks.cancel).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Open collection" }));
    await ready();
    expect(screen.getAllByRole("button", { name: /^View / })).toHaveLength(2);
  });

  it("retries a failed guide without importing or generating cards again", async () => {
    mocks.guide.mockRejectedValueOnce(new Error("Provider unavailable"));
    open();
    select([image()]);
    await screen.findByRole("button", { name: "Retry processing" });
    fireEvent.click(screen.getByRole("button", { name: "Retry processing" }));
    await ready();
    expect(mocks.import).toHaveBeenCalledTimes(1);
    expect(mocks.cards).toHaveBeenCalledTimes(1);
    expect(mocks.guide).toHaveBeenCalledTimes(2);
  });

  it("preserves upload and source-operation identities across an ambiguous acknowledgement", async () => {
    const publish = mocks.import.getMockImplementation()!;
    mocks.import.mockImplementationOnce(async (input: ImportImageSetInput) => {
      await publish(input);
      throw new Error("Reply interrupted");
    });
    open();
    select([image()]);
    await screen.findByRole("button", { name: "Retry processing" });
    fireEvent.click(screen.getByRole("button", { name: "Retry processing" }));
    await ready();
    expect(mocks.stage).toHaveBeenCalledTimes(1);
    expect(mocks.import).toHaveBeenCalledTimes(2);
    expect(mocks.import.mock.calls[1][0]).toEqual(mocks.import.mock.calls[0][0]);
    expect(mocks.cards).toHaveBeenCalledTimes(1);
  });

  it("keeps failed files removable and continues with the valid selection", async () => {
    const stage = mocks.stage.getMockImplementation()!;
    mocks.stage.mockImplementation(async (file: File, operationId: string) => {
      if (file.name === "broken.png") throw new Error("Image cannot be decoded");
      return stage(file, operationId);
    });
    open();
    select([image("good.png"), image("broken.png")]);
    await screen.findByRole("button", { name: "Remove broken.png" });
    expect(mocks.import).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Remove broken.png" }));
    await ready();
    expect(mocks.import.mock.calls[0][0].members).toHaveLength(1);
    expect(mocks.stage).toHaveBeenCalledTimes(2);
  });

  it("keeps original inspection optional and never treats failed refresh as a failed write", async () => {
    mocks.refresh.mockRejectedValue(new Error("Library read failed"));
    open();
    select([image()]);
    await ready();
    expect(mocks.import).toHaveBeenCalledTimes(1);
    expect(mocks.cards).toHaveBeenCalledTimes(1);
    expect(mocks.guide).toHaveBeenCalledTimes(1);
    expect(mocks.preview).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "View source-0.png" }));
    expect(await screen.findByAltText("Retained original image")).toBeTruthy();
    expect(mocks.preview).toHaveBeenCalledWith({ documentId: id, memberId: member("", 1).id,
      expectedRevision: 1, viewport: null });
  });
});
