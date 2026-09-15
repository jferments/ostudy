import { render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { Asset } from "../../ipc/assets";
import type { RichDocument } from "../../ipc/common";
import { TiptapRichTextView } from "./TiptapRichTextView";

const mediaMocks = vi.hoisted(() => ({
  createObjectUrl: vi.fn(async (id: string, kind: "audio" | "video") => `blob:ostudy-${kind}-${id}`),
  revokeObjectUrl: vi.fn(),
}));

vi.mock("../../media/managedPlayback", () => ({
  createManagedMediaObjectUrl: mediaMocks.createObjectUrl,
  revokeManagedMediaObjectUrl: mediaMocks.revokeObjectUrl,
}));

vi.mock("../../platform/assets", () => ({
  managedAssetUrl: (id: string) => `https://ostudy-asset.localhost/${id}`,
}));

const audioId = "a".repeat(32);
const videoId = "b".repeat(32);

function mediaDocument(): RichDocument {
  return {
    type: "doc",
    attrs: { schemaVersion: 3 },
    content: [
      { type: "audio", attrs: { assetId: audioId, src: `study-asset://${audioId}` } },
      { type: "video", attrs: { assetId: videoId, src: `study-asset://${videoId}` } },
    ],
  };
}

const assets: Asset[] = [
  {
    id: audioId,
    mimeType: "audio/mpeg",
    originalFilename: "secret-answer.mp3",
    byteSize: 100,
    createdAt: 1,
  },
  {
    id: videoId,
    mimeType: "video/mp4",
    originalFilename: "secret-answer.mp4",
    byteSize: 200,
    createdAt: 2,
  },
];

describe("TiptapRichTextView managed-media lifecycle", () => {
  let pauseSpy: ReturnType<typeof vi.spyOn>;
  let loadSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    pauseSpy = vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => undefined);
    loadSpy = vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => undefined);
    mediaMocks.createObjectUrl.mockClear();
    mediaMocks.revokeObjectUrl.mockClear();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("does not load inactive media, then materializes and releases the active face", async () => {
    const view = render(
      <TiptapRichTextView
        document={mediaDocument()}
        assets={assets}
        mediaPlaybackActive={false}
      />,
    );

    expect(view.container.querySelector("audio, video")).toBeNull();
    expect(view.container.textContent).toContain("[Audio]");
    expect(view.container.textContent).toContain("[Video]");
    expect(view.container.textContent).not.toContain("secret-answer");
    expect(view.container.innerHTML).not.toContain("ostudy-asset.localhost");
    expect(mediaMocks.createObjectUrl).not.toHaveBeenCalled();

    view.rerender(
      <TiptapRichTextView
        document={mediaDocument()}
        assets={assets}
        mediaPlaybackActive
      />,
    );

    await waitFor(() => {
      expect(view.container.querySelectorAll("audio, video")).toHaveLength(2);
      expect(view.container.querySelector("audio")?.getAttribute("src"))
        .toBe(`blob:ostudy-audio-${audioId}`);
      expect(view.container.querySelector("video")?.getAttribute("src"))
        .toBe(`blob:ostudy-video-${videoId}`);
    });
    const audio = view.container.querySelector("audio");
    const video = view.container.querySelector("video");
    expect(audio?.controls).toBe(true);
    expect(video?.controls).toBe(true);
    expect(audio?.autoplay).toBe(false);
    expect(video?.autoplay).toBe(false);
    expect(audio?.preload).toBe("metadata");
    expect(video?.preload).toBe("metadata");
    expect(view.container.textContent).not.toContain("secret-answer");
    expect(view.container.innerHTML).not.toContain("ostudy-asset.localhost");
    expect(mediaMocks.createObjectUrl).toHaveBeenCalledWith(audioId, "audio");
    expect(mediaMocks.createObjectUrl).toHaveBeenCalledWith(videoId, "video");

    view.rerender(
      <TiptapRichTextView
        document={mediaDocument()}
        assets={assets}
        mediaPlaybackActive={false}
      />,
    );

    await waitFor(() => {
      expect(view.container.querySelector("audio, video")).toBeNull();
    });
    expect(pauseSpy.mock.calls.length).toBeGreaterThanOrEqual(2);
    expect(loadSpy.mock.calls.length).toBeGreaterThanOrEqual(2);
    expect(mediaMocks.revokeObjectUrl).toHaveBeenCalledWith(`blob:ostudy-audio-${audioId}`);
    expect(mediaMocks.revokeObjectUrl).toHaveBeenCalledWith(`blob:ostudy-video-${videoId}`);
    expect(view.container.innerHTML).not.toContain("ostudy-asset.localhost");
  });

  it("releases every captured active player and object URL when the rich view unmounts", async () => {
    const view = render(
      <TiptapRichTextView document={mediaDocument()} assets={assets} mediaPlaybackActive />,
    );
    await waitFor(() => {
      expect(view.container.querySelectorAll("audio, video")).toHaveLength(2);
      expect(view.container.querySelector("audio")?.getAttribute("src"))
        .toBe(`blob:ostudy-audio-${audioId}`);
      expect(view.container.querySelector("video")?.getAttribute("src"))
        .toBe(`blob:ostudy-video-${videoId}`);
    });
    const players = [...view.container.querySelectorAll<HTMLMediaElement>("audio, video")];
    pauseSpy.mockClear();
    loadSpy.mockClear();
    mediaMocks.revokeObjectUrl.mockClear();

    view.unmount();

    expect(pauseSpy.mock.calls.length).toBeGreaterThanOrEqual(players.length);
    expect(loadSpy.mock.calls.length).toBeGreaterThanOrEqual(players.length);
    for (const player of players) expect(player.hasAttribute("src")).toBe(false);
    expect(mediaMocks.revokeObjectUrl).toHaveBeenCalledTimes(players.length);
  });
});
