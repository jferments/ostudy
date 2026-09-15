import { render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { Asset } from "../../ipc/assets";
import type { RichDocument } from "../../ipc/common";
import { TiptapRichTextView } from "./TiptapRichTextView";

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
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("does not mount inactive media, then releases active players when the face becomes inactive", async () => {
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

    view.rerender(
      <TiptapRichTextView
        document={mediaDocument()}
        assets={assets}
        mediaPlaybackActive
      />,
    );

    await waitFor(() => {
      expect(view.container.querySelectorAll("audio, video")).toHaveLength(2);
    });
    const audio = view.container.querySelector("audio");
    const video = view.container.querySelector("video");
    expect(audio?.getAttribute("src")).toBe(`https://ostudy-asset.localhost/${audioId}`);
    expect(video?.getAttribute("src")).toBe(`https://ostudy-asset.localhost/${videoId}`);
    expect(audio?.controls).toBe(true);
    expect(video?.controls).toBe(true);
    expect(audio?.autoplay).toBe(false);
    expect(video?.autoplay).toBe(false);
    expect(audio?.preload).toBe("metadata");
    expect(video?.preload).toBe("metadata");
    expect(view.container.textContent).not.toContain("secret-answer");

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
    expect(pauseSpy).toHaveBeenCalledTimes(2);
    expect(loadSpy).toHaveBeenCalledTimes(2);
    expect(view.container.innerHTML).not.toContain("ostudy-asset.localhost");
  });

  it("releases active media when the rich view unmounts", async () => {
    const view = render(
      <TiptapRichTextView document={mediaDocument()} assets={assets} mediaPlaybackActive />,
    );
    await waitFor(() => {
      expect(view.container.querySelectorAll("audio, video")).toHaveLength(2);
    });

    view.unmount();

    expect(pauseSpy).toHaveBeenCalledTimes(2);
    expect(loadSpy).toHaveBeenCalledTimes(2);
  });
});
