import { useState } from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DocumentJobsProvider } from "../../controllers/DocumentJobsProvider";
import type { DocumentSummary } from "../../ipc/documents";
import { MAX_GENERATION_INSTRUCTIONS_BYTES } from "../../ipc/generationInstructions";
import type { AmendImageSetInput, ImageSetDetails, ImportImageSetInput } from "../../ipc/imageSets";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { ImageSetWorkspace } from "./ImageSetWorkspace";

import { requiredTestValue as required } from "../../test/factories";
const mocks = vi.hoisted(() => ({
  stage: vi.fn(), import: vi.fn(), amend: vi.fn(), details: vi.fn(), preview: vi.fn(),
  prepare: vi.fn(), cards: vi.fn(), more: vi.fn(), guide: vi.fn(), list: vi.fn(), guideJobs: vi.fn(),
  cancel: vi.fn(), cancelGuide: vi.fn(), toast: vi.fn(), refresh: vi.fn(), clipboardRead: vi.fn(),
}));
vi.mock("../../platform/imageInput", async (original) => ({
  ...await original<typeof import("../../platform/imageInput")>(),
  stageBrowserImage: mocks.stage, readClipboardImageFiles: mocks.clipboardRead,
}));
vi.mock("../../ipc/imageSets", () => ({
  importImageSet: mocks.import, amendImageSet: mocks.amend, getImageSet: mocks.details,
  getImageSetPreview: mocks.preview, prepareImageSet: mocks.prepare,
}));
vi.mock("../../ipc/documents", () => ({
  listDocuments: mocks.list, getDocumentGenerationProgress: vi.fn().mockResolvedValue(null),
  startDocumentFlashcardGeneration: vi.fn(),
  retryDocumentFlashcardGeneration: mocks.cards,
  generateMoreDocumentFlashcards: mocks.more,
  cancelDocumentFlashcardGeneration: mocks.cancel, deleteDocument: vi.fn(),
  importDocument: vi.fn(), openDocumentSource: vi.fn(),
}));
vi.mock("../../ipc/studyGuides", () => ({
  generateDocumentStudyGuide: mocks.guide,
  cancelDocumentStudyGuideGeneration: mocks.cancelGuide,
  getDocumentLearningPlan: vi.fn().mockResolvedValue(null),
}));
vi.mock("../../ipc/documentArtifactProgress", () => ({ listLatestDocumentStudyGuideJobs: mocks.guideJobs }));

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
function pasteArea() { return screen.getByRole("textbox", { name: "Paste images here" }); }
function instructionField() {
  return screen.getByRole("textbox", { name: "Flashcard instructions (optional)" }) as HTMLTextAreaElement;
}
function guideCards(value: string) { fireEvent.change(instructionField(), { target: { value } }); }
async function ready() {
  await waitFor(() => expect(screen.getByRole("status").textContent).toContain("Study materials are ready"));
}

beforeEach(() => {
  vi.resetAllMocks();
  saved = null;
  deckId = null;
  mocks.refresh.mockResolvedValue(undefined);
  mocks.clipboardRead.mockResolvedValue([]);
  mocks.cancel.mockResolvedValue(undefined);
  mocks.cancelGuide.mockResolvedValue(undefined);
  mocks.guideJobs.mockResolvedValue([{ documentId: id, state: "succeeded", safeErrorMessage: "", safeErrorCode: "" }]);
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
  it("does not leak an unrelated document-provider error into image upload", async () => {
    mocks.list.mockRejectedValueOnce(new Error(
      "The provider ended the streamed document request with finish_reason=content_filter.",
    ));
    open();
    await waitFor(() => expect(mocks.list).toHaveBeenCalled());
    expect(screen.queryByText(/finish_reason=content_filter/u)).toBeNull();
    expect(screen.getByRole("button", { name: "Select images" })).toBeTruthy();
  });

  it("accepts multiple files and uses native paired artifact generation without preparation controls", async () => {
    open();
    const input = screen.getByLabelText("Choose images") as HTMLInputElement;
    expect(input.multiple).toBe(true);
    select([image("second.png"), image("first.png")]);
    await ready();
    expect(mocks.stage).toHaveBeenCalledTimes(2);
    expect(mocks.import).toHaveBeenCalledTimes(1);
    expect(required(mocks.import.mock.calls[0]?.[0], "import call argument").members).toHaveLength(2);
    expect(mocks.prepare).toHaveBeenCalledWith(id);
    expect(mocks.cards).toHaveBeenCalledWith(id, "");
    expect(mocks.guide).not.toHaveBeenCalled();
    expect(mocks.guideJobs).toHaveBeenCalledTimes(1);
    expect(required(mocks.prepare.mock.invocationCallOrder[0], "prepare invocation order")).toBeLessThan(required(mocks.cards.mock.invocationCallOrder[0], "cards invocation order"));
    expect(screen.queryByRole("spinbutton")).toBeNull();
    expect(screen.queryByRole("combobox")).toBeNull();
    expect(screen.queryByText("Content filter")).toBeNull();
    expect(screen.queryByRole("button", { name: /Import image set|Rotate|Retry local OCR|Prepare/ })).toBeNull();
    const list = screen.getByRole("list", { name: "Images in collection" });
    expect(list.compareDocumentPosition(pasteArea()) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
    const click = vi.spyOn(input, "click");
    fireEvent.click(screen.getByRole("button", { name: "Add more images" }));
    expect(click).toHaveBeenCalledTimes(1);
  });

  it("accepts dropped and pasted images into the same source while ignoring pasted URLs", async () => {
    open();
    const dropped = image("dropped.png");
    fireEvent.drop(pasteArea(), {
      dataTransfer: { files: [], items: [
        { kind: "file", type: "image/png", getAsFile: () => dropped },
      ], dropEffect: "copy" },
    });
    await ready();
    fireEvent.paste(pasteArea(), { clipboardData: { files: [image("pasted.png")], items: [] } });
    await waitFor(() => expect(mocks.amend).toHaveBeenCalledTimes(1));
    await ready();
    expect(mocks.import).toHaveBeenCalledTimes(1);
    expect(mocks.cards).toHaveBeenCalledTimes(1);
    expect(mocks.more).toHaveBeenCalledWith(id, "");
    expect(mocks.guide).toHaveBeenCalledTimes(1);
    fireEvent.paste(pasteArea(), { clipboardData: { files: [], items: [{ kind: "string", type: "text/plain" }] } });
    expect(mocks.stage).toHaveBeenCalledTimes(2);
  });

  it("reads the clipboard during Ctrl+V when the native event carries no image payload", async () => {
    mocks.clipboardRead.mockResolvedValueOnce([image("clipboard-fallback.png")]);
    open();
    fireEvent.keyDown(pasteArea(), { key: "v", ctrlKey: true });
    expect(mocks.clipboardRead).toHaveBeenCalledTimes(1);
    await ready();
    expect(mocks.stage).toHaveBeenCalledWith(expect.objectContaining({ name: "clipboard-fallback.png" }), expect.any(String));
    expect(mocks.import).toHaveBeenCalledTimes(1);
  });

  it("keeps an in-flight clipboard read when the first upload acquires its durable document ID", async () => {
    const clipboard = deferred<File[]>();
    mocks.clipboardRead.mockReturnValueOnce(clipboard.promise);
    open();
    select([image("first.png")]);
    fireEvent.keyDown(pasteArea(), { key: "v", ctrlKey: true });
    await ready();
    expect(mocks.import).toHaveBeenCalledTimes(1);
    await act(async () => { clipboard.resolve([image("second.png")]); await clipboard.promise; });
    await waitFor(() => expect(mocks.amend).toHaveBeenCalledTimes(1));
    await ready();
    expect(mocks.import).toHaveBeenCalledTimes(1);
    expect(mocks.stage.mock.calls.map((call) => call[0].name)).toEqual(["first.png", "second.png"]);
    expect(mocks.more).toHaveBeenCalledTimes(1);
  });

  it("offers explicit Paste and exposes a safe acquisition failure instead of silently doing nothing", async () => {
    open();
    fireEvent.click(screen.getByRole("button", { name: "Paste images" }));
    expect(mocks.clipboardRead).toHaveBeenCalledTimes(1);
    expect(await screen.findByText(/No image was available to paste/)).toBeTruthy();
    expect(mocks.stage).not.toHaveBeenCalled();
    mocks.clipboardRead.mockResolvedValueOnce([image("explicit.png")]);
    fireEvent.click(screen.getByRole("button", { name: "Paste images" }));
    await ready();
    expect(screen.queryByText(/No image was available to paste/)).toBeNull();
    expect(mocks.stage).toHaveBeenCalledTimes(1);
  });

  it("keeps staging alive after leaving the image workspace", async () => {
    const staging = deferred<{
      id: string; displayName: string; purpose: "image"; detectedKind: "png";
      mediaType: string; byteSize: number; sha256: string; expiresAt: number;
    }>();
    mocks.stage.mockReturnValueOnce(staging.promise);
    open();
    select([image("slow.png")]);
    await waitFor(() => expect(mocks.stage).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole("button", { name: "Back to Documents" }));
    expect(screen.getByText("Another page")).toBeTruthy();
    await act(async () => {
      staging.resolve({
        id: required(mocks.stage.mock.calls[0]?.[1], "stage call argument"), displayName: "slow.png", purpose: "image", detectedKind: "png",
        mediaType: "image/png", byteSize: 6, sha256: "d".repeat(64), expiresAt: 9999999999999,
      });
      await staging.promise;
    });
    await waitFor(() => expect(mocks.import).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(mocks.cards).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(mocks.guideJobs).toHaveBeenCalledTimes(1));
    expect(mocks.guide).not.toHaveBeenCalled();
    expect(mocks.cancel).not.toHaveBeenCalled();
    expect(mocks.cancelGuide).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Open collection" }));
    await ready();
    expect(screen.getByRole("button", { name: "View source-0.png" })).toBeTruthy();
  });

  it("retains additions and captured guidance during generation after leaving the upload page", async () => {
    const request = deferred<{ createdCardCount: number }>();
    mocks.cards.mockImplementation(async () => { const result = await request.promise; deckId = "b".repeat(32); return result; });
    open();
    guideCards("Nursing assessment tests");
    select([image()]);
    await waitFor(() => expect(mocks.cards).toHaveBeenCalledTimes(1));
    expect(mocks.cards).toHaveBeenCalledWith(id, "Nursing assessment tests");
    expect(instructionField().disabled).toBe(true);
    select([image("later.png")]);
    expect(mocks.amend).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Back to Documents" }));
    expect(screen.getByText("Another page")).toBeTruthy();
    await act(async () => { request.resolve({ createdCardCount: 3 }); await request.promise; });
    await waitFor(() => expect(mocks.guide).toHaveBeenCalledTimes(1));
    expect(mocks.amend).toHaveBeenCalledTimes(1);
    expect(mocks.more).toHaveBeenCalledTimes(1);
    expect(mocks.more).toHaveBeenCalledWith(id, "Nursing assessment tests");
    expect(mocks.cancel).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Open collection" }));
    await ready();
    expect(screen.getAllByRole("button", { name: /^View / })).toHaveLength(2);
    expect(instructionField().value).toBe("Nursing assessment tests");
  });

  it("retries a failed native guide without importing or generating guided cards again", async () => {
    mocks.guideJobs.mockResolvedValueOnce([{
      documentId: id, state: "failed", safeErrorCode: "ai_provider_unavailable", safeErrorMessage: "Provider unavailable",
    }]);
    open();
    guideCards("Nursing assessment tests");
    select([image()]);
    await screen.findByRole("button", { name: "Retry processing" });
    expect(mocks.guide).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Retry processing" }));
    await ready();
    expect(mocks.import).toHaveBeenCalledTimes(1);
    expect(mocks.cards).toHaveBeenCalledTimes(1);
    expect(mocks.cards).toHaveBeenCalledWith(id, "Nursing assessment tests");
    expect(mocks.more).not.toHaveBeenCalled();
    expect(mocks.guide).toHaveBeenCalledTimes(1);
  });

  it("retries only the status read after native artifacts committed but their acknowledgement read failed", async () => {
    mocks.guideJobs.mockRejectedValueOnce(new Error("Status read unavailable"));
    open();
    select([image()]);
    await screen.findByRole("button", { name: "Retry processing" });
    fireEvent.click(screen.getByRole("button", { name: "Retry processing" }));
    await ready();
    expect(mocks.guideJobs).toHaveBeenCalledTimes(2);
    expect(mocks.import).toHaveBeenCalledTimes(1);
    expect(mocks.cards).toHaveBeenCalledTimes(1);
    expect(mocks.guide).not.toHaveBeenCalled();
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
    expect(required(mocks.import.mock.calls[1]?.[0], "import call argument")).toEqual(required(mocks.import.mock.calls[0]?.[0], "import call argument"));
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
    expect(required(mocks.import.mock.calls[0]?.[0], "import call argument").members).toHaveLength(1);
    expect(mocks.stage).toHaveBeenCalledTimes(2);
  });

  it("keeps original inspection optional and never treats failed refresh as a failed write", async () => {
    mocks.refresh.mockRejectedValue(new Error("Library read failed"));
    open();
    select([image()]);
    await ready();
    expect(mocks.import).toHaveBeenCalledTimes(1);
    expect(mocks.cards).toHaveBeenCalledTimes(1);
    expect(mocks.guide).not.toHaveBeenCalled();
    expect(mocks.preview).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button", { name: "View source-0.png" }));
    const preview = await screen.findByAltText("Retained original image");
    expect(preview).toBeTruthy();
    expect(screen.getByRole("button", { name: "Hide source-0.png" })).toBeTruthy();
    expect(preview.compareDocumentPosition(pasteArea()) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
    expect(mocks.preview).toHaveBeenCalledWith({ documentId: id, memberId: member("", 1).id,
      expectedRevision: 1, viewport: null });
  });

  it("shows retained-original preview failures next to the selected image", async () => {
    mocks.preview.mockRejectedValueOnce(new Error("Retained preview unavailable"));
    open();
    select([image()]);
    await ready();
    fireEvent.click(await screen.findByRole("button", { name: "View source-0.png" }));
    expect(await screen.findByText("Retained preview unavailable")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Hide source-0.png" })).toBeTruthy();
  });

  it("applies guidance to chooser, dropped, and explicitly pasted screenshots without changing source requests", async () => {
    open();
    guideCards("  Nursing cranial nerve assessments  ");
    select([image("first.png")]);
    await ready();
    expect(mocks.cards).toHaveBeenCalledWith(id, "Nursing cranial nerve assessments");
    fireEvent.drop(pasteArea(), { dataTransfer: { files: [image("second.png")], items: [], dropEffect: "copy" } });
    await waitFor(() => expect(mocks.amend).toHaveBeenCalledTimes(1));
    await ready();
    expect(mocks.more).toHaveBeenCalledWith(id, "Nursing cranial nerve assessments");
    guideCards("Arterial anatomy");
    mocks.clipboardRead.mockResolvedValueOnce([image("third.png")]);
    fireEvent.click(screen.getByRole("button", { name: "Paste images" }));
    await waitFor(() => expect(mocks.amend).toHaveBeenCalledTimes(2));
    await ready();
    expect(mocks.more).toHaveBeenLastCalledWith(id, "Arterial anatomy");
    expect(mocks.import).toHaveBeenCalledTimes(1);
    expect(mocks.stage.mock.calls.map((call) => call[0].name)).toEqual(["first.png", "second.png", "third.png"]);
    expect(mocks.import.mock.calls[0]?.[0]).not.toHaveProperty("generationInstructions");
    expect(mocks.amend.mock.calls[1]?.[0]).not.toHaveProperty("generationInstructions");
  });

  it("distinguishes an explicitly changed prompt from a guide-only retry without reimporting images", async () => {
    mocks.guideJobs.mockResolvedValueOnce([{
      documentId: id, state: "failed", safeErrorCode: "ai_provider_unavailable", safeErrorMessage: "Provider unavailable",
    }]);
    open();
    guideCards("Nursing assessment tests");
    select([image()]);
    await screen.findByRole("button", { name: "Retry processing" });
    guideCards("Mechanisms of nerve injury");
    fireEvent.click(screen.getByRole("button", { name: "Generate with new instructions" }));
    await ready();
    expect(mocks.cards).toHaveBeenCalledTimes(1);
    expect(mocks.more).toHaveBeenCalledTimes(1);
    expect(mocks.more).toHaveBeenCalledWith(id, "Mechanisms of nerve injury");
    expect(mocks.import).toHaveBeenCalledTimes(1);
    expect(mocks.amend).not.toHaveBeenCalled();
    expect(mocks.guide).toHaveBeenCalledTimes(1);
  });

  it("rejects invalid guidance before staging images even when file input bypasses the disabled picker button", async () => {
    open();
    guideCards("x".repeat(MAX_GENERATION_INSTRUCTIONS_BYTES + 1));
    expect(instructionField().getAttribute("aria-invalid")).toBe("true");
    expect((screen.getByRole("button", { name: "Select images" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Paste images" }) as HTMLButtonElement).disabled).toBe(true);
    select([image()]);
    await waitFor(() => expect(mocks.toast).toHaveBeenCalledWith("error", expect.stringContaining("16 KiB")));
    expect(mocks.stage).not.toHaveBeenCalled();
    expect(mocks.import).not.toHaveBeenCalled();
    expect(mocks.cards).not.toHaveBeenCalled();
    expect(mocks.more).not.toHaveBeenCalled();
  });

  it("leaves ordinary text paste in the guidance textarea to the browser", async () => {
    open();
    const field = instructionField();
    field.focus();
    fireEvent.keyDown(field, { key: "v", ctrlKey: true });
    fireEvent.paste(field, { clipboardData: {
      files: [], items: [{ kind: "string", type: "text/plain" }], getData: () => "Nursing assessments",
    } });
    await act(async () => { await Promise.resolve(); });
    expect(mocks.clipboardRead).not.toHaveBeenCalled();
    expect(mocks.stage).not.toHaveBeenCalled();
    expect(mocks.import).not.toHaveBeenCalled();
  });
});
