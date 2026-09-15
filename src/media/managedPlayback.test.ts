import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  readMediaAsset: vi.fn(),
}));

vi.mock("../ipc/assets", () => ({
  readMediaAsset: mocks.readMediaAsset,
}));

import {
  createManagedMediaObjectUrl,
  playbackMimeType,
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

function bytes(...values: number[]): ArrayBuffer {
  return Uint8Array.from(values).buffer;
}

function ascii(value: string): number[] {
  return [...value].map((character) => character.charCodeAt(0));
}

function isoBmffFixture(): ArrayBuffer {
  return bytes(
    0, 0, 0, 24,
    ...ascii("ftyp"),
    ...ascii("isom"),
    0, 0, 0, 0,
    ...ascii("isom"),
    ...ascii("mp42"),
  );
}

describe("managed media playback materialization", () => {
  it("creates an explicitly typed Blob from raw managed bytes", async () => {
    mocks.readMediaAsset.mockResolvedValue(bytes(0xff, 0xfb, 0x90, 0x00));

    await expect(createManagedMediaObjectUrl(assetId, "audio"))
      .resolves.toBe("blob:ostudy-media");

    expect(mocks.readMediaAsset).toHaveBeenCalledWith(assetId);
    expect(createObjectUrl).toHaveBeenCalledTimes(1);
    const blob = createObjectUrl.mock.calls[0]?.[0];
    expect(blob).toBeInstanceOf(Blob);
    expect((blob as Blob).type).toBe("audio/mpeg");
    expect((blob as Blob).size).toBe(4);
  });

  it("derives every admitted playback container family", () => {
    expect(playbackMimeType(bytes(...ascii("RIFF"), 4, 0, 0, 0, ...ascii("WAVE")), "audio"))
      .toBe("audio/wav");
    expect(playbackMimeType(bytes(...ascii("OggS"), 0, 0, 0, 0), "audio"))
      .toBe("audio/ogg");
    expect(playbackMimeType(bytes(...ascii("ID3"), 4, 0, 0, 0), "audio"))
      .toBe("audio/mpeg");
    expect(playbackMimeType(bytes(0xff, 0xf1, 0x50, 0x80, 0, 0, 0), "audio"))
      .toBe("audio/aac");
    expect(playbackMimeType(bytes(0xff, 0xfb, 0x90, 0x00), "audio"))
      .toBe("audio/mpeg");
    expect(playbackMimeType(isoBmffFixture(), "audio")).toBe("audio/mp4");
    expect(playbackMimeType(isoBmffFixture(), "video")).toBe("video/mp4");
  });

  it("fails closed for invalid identities, empty reads, and kind mismatches", async () => {
    await expect(createManagedMediaObjectUrl("../secret", "audio"))
      .rejects.toThrow(/identifier is invalid/u);
    expect(mocks.readMediaAsset).not.toHaveBeenCalled();

    mocks.readMediaAsset.mockResolvedValue(new ArrayBuffer(0));
    await expect(createManagedMediaObjectUrl(assetId, "video"))
      .rejects.toThrow(/returned no bytes/u);

    mocks.readMediaAsset.mockResolvedValue(bytes(...ascii("RIFF"), 4, 0, 0, 0, ...ascii("WAVE")));
    await expect(createManagedMediaObjectUrl(assetId, "video"))
      .rejects.toThrow(/video container is not supported/u);
    expect(createObjectUrl).not.toHaveBeenCalled();
  });

  it("revokes released object URLs", () => {
    revokeManagedMediaObjectUrl("blob:released");
    revokeManagedMediaObjectUrl(null);
    expect(revokeObjectUrl).toHaveBeenCalledTimes(1);
    expect(revokeObjectUrl).toHaveBeenCalledWith("blob:released");
  });
});
