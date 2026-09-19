import { invoke } from "@tauri-apps/api/core";
import { beforeEach, expect, it, vi } from "vitest";

import { inspectImageSet, type ImageSetInspection } from "./imageSets";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

beforeEach(() => {
  // A returned mock is a teardown function to Vitest; resetting must return void.
  vi.mocked(invoke).mockReset();
});

it("passes stable selection identities in the chosen order to read-only native inspection", async () => {
  const ids = Object.freeze(["b".repeat(32), "a".repeat(32)]);
  const result: ImageSetInspection = {
    maximumCount: 128,
    maximumSetBytes: 512 * 1024 * 1024,
    maximumSetPixels: 256_000_000,
    members: ids.map((id) => ({
      id,
      image: {
        originalFilename: "same.png",
        mediaType: "image/png",
        byteSize: 100,
        sha256: "c".repeat(64),
        width: 32,
        height: 16,
        pixelSha256: "d".repeat(64),
        pixelContract: "image-0.25.10:exif-upright-rgba8:v1",
      },
      error: null,
    })),
    canImport: true,
    decodedImages: 1,
    reusedImages: 1,
  };
  vi.mocked(invoke).mockResolvedValue(result);
  expect(await inspectImageSet(ids)).toBe(result);
  expect(invoke).toHaveBeenCalledTimes(1);
  expect(invoke).toHaveBeenCalledWith("inspect_image_set", {
    input: { stagedFileIds: [ids[0], ids[1]] },
  });
  expect(ids).toEqual(["b".repeat(32), "a".repeat(32)]);
});

it("preserves per-selection errors and successful siblings without claiming import succeeded", async () => {
  const result = {
    maximumCount: 128,
    maximumSetBytes: 512 * 1024 * 1024,
    maximumSetPixels: 256_000_000,
    members: [{ id: "a".repeat(32), image: null, error: { code: "invalid_image_set", message: "Invalid image" } }],
    canImport: false,
    decodedImages: 0,
    reusedImages: 0,
  };
  vi.mocked(invoke).mockResolvedValue(result);
  expect(await inspectImageSet(["a".repeat(32)])).toBe(result);
  expect(invoke).toHaveBeenCalledTimes(1);
});

it("does not retry or consume files after native refusal", async () => {
  const failure = { code: "invalid_image_set", message: "The selection is too large." };
  vi.mocked(invoke).mockRejectedValue(failure);
  await expect(inspectImageSet(["a".repeat(32)])).rejects.toBe(failure);
  expect(invoke).toHaveBeenCalledTimes(1);
});
