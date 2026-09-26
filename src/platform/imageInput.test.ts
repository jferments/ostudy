import { beforeEach, describe, expect, it, vi } from "vitest";
import { stageImageCollectionFileChunk } from "../ipc/imageCollectionFiles";
import {
  dataTransferImageFiles,
  IMAGE_UPLOAD_CHUNK_BYTES,
  readClipboardImageFiles,
  stageBrowserImage,
} from "./imageInput";

vi.mock("../ipc/imageCollectionFiles", () => ({
  stageImageCollectionFileChunk: vi.fn(),
}));

describe("bounded browser image staging", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("never sends the whole image in one IPC payload and finalizes the last chunk", async () => {
    const operationId = "a".repeat(32);
    const bytes = new Uint8Array(IMAGE_UPLOAD_CHUNK_BYTES * 2 + 17);
    for (let index = 0; index < bytes.length; index += 1) bytes[index] = index % 251;
    const file = new File([bytes], "lecture.png", { type: "image/png" });
    const staged = {
      id: operationId,
      purpose: "image" as const,
      detectedKind: "png" as const,
      displayName: "lecture.png",
      mediaType: "image/png",
      byteSize: file.size,
      sha256: "b".repeat(64),
      expiresAt: Date.now() + 60_000,
    };
    vi.mocked(stageImageCollectionFileChunk)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(staged);

    await expect(stageBrowserImage(file, operationId)).resolves.toEqual(staged);

    expect(stageImageCollectionFileChunk).toHaveBeenCalledTimes(3);
    const calls = vi.mocked(stageImageCollectionFileChunk).mock.calls.map(([input]) => input);
    expect(calls.map((input) => input.offset)).toEqual([
      0,
      IMAGE_UPLOAD_CHUNK_BYTES,
      IMAGE_UPLOAD_CHUNK_BYTES * 2,
    ]);
    expect(calls.map((input) => input.finalChunk)).toEqual([false, false, true]);
    expect(calls.every((input) => input.totalBytes === file.size)).toBe(true);
    // Base64 expansion stays bounded to one transfer chunk rather than the whole file.
    expect(Math.max(...calls.map((input) => input.dataBase64.length)))
      .toBeLessThan(IMAGE_UPLOAD_CHUNK_BYTES * 2);
  });

  it("rejects oversized images before any IPC transfer", async () => {
    const file = { size: 25 * 1024 * 1024 + 1, name: "too-large.png" } as File;
    await expect(stageBrowserImage(file, "c".repeat(32))).rejects.toThrow(/no larger than 25 MB/u);
    expect(stageImageCollectionFileChunk).not.toHaveBeenCalled();
  });

  it("accepts item-only cross-application image transfers without fetching strings or URLs", () => {
    const image = new File(["pixels"], "dragged.png", { type: "image/png" });
    const data = {
      files: [] as unknown as FileList,
      items: [
        { kind: "string", type: "text/uri-list", getAsFile: () => null },
        { kind: "file", type: "image/png", getAsFile: () => image },
      ] as unknown as DataTransferItemList,
    };
    expect(dataTransferImageFiles(data)).toEqual([image]);
  });

  it("reads screenshot blobs from the user-initiated clipboard fallback", async () => {
    const reader = {
      read: vi.fn().mockResolvedValue([
        { types: ["text/plain", "image/png"],
          getType: vi.fn().mockResolvedValue(new Blob(["pixels"], { type: "image/png" })) },
        { types: ["text/plain"],
          getType: vi.fn() },
      ]),
    };
    const files = await readClipboardImageFiles(reader);
    expect(files).toHaveLength(1);
    expect(files[0]).toMatchObject({ name: "Clipboard image.png", type: "image/png", size: 6 });
  });
});
