import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { stageImageCollectionFile, stageImageCollectionFileChunk } from "./imageCollectionFiles";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("image collection byte admission", () => {
  beforeEach(() => vi.clearAllMocks());

  it("sends bounded image chunks through the dedicated native command", async () => {
    const input = {
      operationId: "b".repeat(32),
      displayName: "scan.png",
      totalBytes: 900_000,
      offset: 524_288,
      dataBase64: "cG5n",
      finalChunk: true,
    };
    const result = { id: input.operationId, purpose: "image" };
    vi.mocked(invoke).mockResolvedValue(result);
    expect(await stageImageCollectionFileChunk(input)).toEqual(result);
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
      "stage_image_collection_file_chunk",
      { input },
    );
  });

  it("preserves upload identity and passes bytes, not a path or URL", async () => {
    const input = { operationId: "a".repeat(32), displayName: "scan.png", dataBase64: "cG5n" };
    const result = { id: input.operationId, purpose: "image" };
    vi.mocked(invoke).mockResolvedValue(result);
    expect(await stageImageCollectionFile(input)).toEqual(result);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("stage_image_collection_file", { input });
  });
});
