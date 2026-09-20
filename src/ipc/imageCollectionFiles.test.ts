import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { stageImageCollectionFile } from "./imageCollectionFiles";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("image collection byte admission", () => {
  beforeEach(() => vi.clearAllMocks());

  it("preserves upload identity and passes bytes, not a path or URL", async () => {
    const input = { operationId: "a".repeat(32), displayName: "scan.png", dataBase64: "cG5n" };
    const result = { id: input.operationId, purpose: "image" };
    vi.mocked(invoke).mockResolvedValue(result);
    expect(await stageImageCollectionFile(input)).toEqual(result);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("stage_image_collection_file", { input });
  });
});
