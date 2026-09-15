import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  readMediaAsset: vi.fn(),
}));

vi.mock("../ipc/assets", () => ({
  readMediaAsset: mocks.readMediaAsset,
}));

import {
  createManagedMediaObjectUrl,
  revokeManagedMediaObjectUrl,
} from "./managedPlayback";

const assetId = "a".repeat(32);

let createObjectUrl: ReturnType<typeof vi.fn>;
let revokeObjectUrl: ReturnType<typeof vi.fn>;

beforeEach(() => {
  createObjectUrl = vi.fn(() => "blob:ostudy-media");
  revokeObjectUrl = vi.fn();
  Object.defineProperty(URL, "createObjectURL", {
    configurable: true,
    writable: true,
    value: createObjectUrl,
  });
  Object.defineProperty(URL, "revokeObjectURL", {
    configurable: true,
    writable: true,
    value: revokeObjectUrl,
  });
  mocks.readMediaAsset.mockReset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("managed media playback materialization", () => {
  it("creates a typed Blob from raw managed bytes", async () => {
    mocks.readMediaAsset.mockResolvedValue(Uint8Array.from([1, 2, 3, 4]).buffer);

    await expect(createManagedMediaObjectUrl(assetId, "audio/mpeg"))
      .resolves.toBe("blob:ostudy-media");

    expect(mocks.readMediaAsset).toHaveBeenCalledWith(assetId);
    expect(createObjectUrl).toHaveBeenCalledTimes(1);
    const blob = createObjectUrl.mock.calls[0]?.[0];
    expect(blob).toBeInstanceOf(Blob);
    expect((blob as Blob).type).toBe("audio/mpeg");
    expect((blob as Blob).size).toBe(4);
  });

  it("fails closed before IPC for invalid identities and media types", async () => {
    await expect(createManagedMediaObjectUrl("../secret", "audio/mpeg"))
      .rejects.toThrow(/identifier is invalid/u);
    await expect(createManagedMediaObjectUrl(assetId, "image/png"))
      .rejects.toThrow(/media type is invalid/u);
    expect(mocks.readMediaAsset).not.toHaveBeenCalled();
  });

  it("rejects empty responses and revokes released object URLs", async () => {
    mocks.readMediaAsset.mockResolvedValue(new ArrayBuffer(0));
    await expect(createManagedMediaObjectUrl(assetId, "video/mp4"))
      .rejects.toThrow(/returned no bytes/u);
    expect(createObjectUrl).not.toHaveBeenCalled();

    revokeManagedMediaObjectUrl("blob:released");
    revokeManagedMediaObjectUrl(null);
    expect(revokeObjectUrl).toHaveBeenCalledTimes(1);
    expect(revokeObjectUrl).toHaveBeenCalledWith("blob:released");
  });
});
