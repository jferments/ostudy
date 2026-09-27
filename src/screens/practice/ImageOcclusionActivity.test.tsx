import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { requiredTestValue as required } from "../../test/factories";
import type { ImageOcclusionAttemptView, RevealPracticeQuestionInput } from "../../ipc/questionAttempts";
import type { ImageViewport } from "../../editor/occlusion/geometry";
import { Button } from "../../ui/primitives/Button";
import { ImageOcclusionActivity } from "./ImageOcclusionActivity";

const mocks = vi.hoisted(() => ({ image: vi.fn(), reveal: vi.fn(), exposed: vi.fn(), view: vi.fn() }));
vi.mock("./usePracticeOcclusionImage", () => ({ usePracticeOcclusionImage: (...args: unknown[]) => mocks.image(...args) }));
vi.mock("../../editor/RichTextView", () => ({ RichTextView: ({ document }: { document: unknown }) => <span>{JSON.stringify(document)}</span> }));
vi.mock("../../editor/occlusion/OcclusionImageView", () => ({
  OcclusionImageView: (props: { imageUrl: string; initialViewport: ImageViewport; onViewportChange: (value: ImageViewport) => void }) => {
    mocks.view(props);
    return <section aria-label="Prepared raster viewer"><span>{props.imageUrl}</span>
      <Button onClick={() => props.onViewportChange({ zoom: 2, center: { x: 0.3, y: 0.4 } })}>Mock zoom 2x</Button>
      <Button onClick={() => props.onViewportChange({ zoom: 4, center: { x: 0.3, y: 0.4 } })}>Mock zoom 4x</Button>
    </section>;
  },
}));
const initial: ImageOcclusionAttemptView = {
  instanceId: "3".repeat(32), revision: 1, assisted: false,
  question: { kind: "image_occlusion", prompt: { text: "Identify the highlighted region" },
    targetRectangles: [{ x: 0.1, y: 0.2, width: 0.2, height: 0.2 }] },
  progress: { phase: "answering", savedAnswer: null },
};
function receipt(input: RevealPracticeQuestionInput): ImageOcclusionAttemptView {
  return { ...initial, revision: 2, progress: {
    phase: "revealed", revealId: input.request.revealId, responseTimeMs: input.request.responseTimeMs,
    revealedAtMs: 2000, answer: { text: "ANSWER_FROM_NATIVE" }, explanation: { text: "An authored explanation" },
  } };
}
function Harness() {
  const [attempt, setAttempt] = useState(initial);
  return <ImageOcclusionActivity sessionId={"1".repeat(32)} cardId={"2".repeat(32)}
    attempt={attempt} assets={[]} disabled={false} onReveal={mocks.reveal}
    onAttempt={setAttempt} onRevealed={mocks.exposed} onError={vi.fn()} />;
}
beforeEach(() => {
  for (const mock of Object.values(mocks)) mock.mockReset();
  mocks.image.mockImplementation((_target, revision) => ({
    ready: true, url: `blob:prepared-${revision}`, size: { width: 400, height: 300 }, error: null, loadedAt: Date.now(), retry: vi.fn(),
  }));
  mocks.reveal.mockImplementation(async (input: RevealPracticeQuestionInput) => receipt(input));
  mocks.exposed.mockResolvedValue(undefined);
});

describe("image-occlusion Practice interaction", () => {
  it("starts with a small masked raster and uses Reveal instead of a fabricated objective Check", async () => {
    render(<Harness />);
    expect(required(mocks.image.mock.calls[0]?.[2], "image call argument")).toBe(1024);
    expect(screen.getByText("blob:prepared-1")).toBeTruthy();
    expect(screen.queryByText(/ANSWER_FROM_NATIVE/)).toBeNull();
    expect(screen.queryByRole("button", { name: "Check answer" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Reveal answer" }));
    await waitFor(() => expect(screen.getByText(/ANSWER_FROM_NATIVE/)).toBeTruthy());
    expect(screen.getByText(/self-assessed/)).toBeTruthy();
    expect(screen.getByText("blob:prepared-2")).toBeTruthy();
    expect(mocks.reveal).toHaveBeenCalledTimes(1);
    expect(mocks.exposed).toHaveBeenCalledTimes(1);
  });

  it("requests sharper native rasters automatically as the learner zooms and preserves the viewport", async () => {
    render(<Harness />);
    fireEvent.click(screen.getByRole("button", { name: "Mock zoom 2x" }));
    await waitFor(() => expect(mocks.image.mock.calls.some((call) => call[2] === 2048)).toBe(true));
    expect(mocks.view.mock.calls.at(-1)?.[0].initialViewport).toEqual({ zoom: 2, center: { x: 0.3, y: 0.4 } });
    fireEvent.click(screen.getByRole("button", { name: "Mock zoom 4x" }));
    await waitFor(() => expect(mocks.image.mock.calls.some((call) => call[2] === 4096)).toBe(true));
    fireEvent.click(screen.getByRole("button", { name: "Reveal answer" }));
    await waitFor(() => expect(screen.getByText(/ANSWER_FROM_NATIVE/)).toBeTruthy());
    expect(mocks.view.mock.calls.at(-1)?.[0].initialViewport).toEqual({ zoom: 4, center: { x: 0.3, y: 0.4 } });
  });

  it("does not expose old enlarge or manual sharpness controls", () => {
    render(<Harness />);
    expect(screen.queryByRole("button", { name: "Enlarge image" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Load sharper image" })).toBeNull();
  });

  it("does not reveal while the initial raster is loading or has failed", () => {
    mocks.image.mockReturnValue({ ready: false, url: null, size: null, loadedAt: null, error: "Image unavailable", retry: vi.fn() });
    render(<Harness />);
    expect(screen.getByRole("button", { name: "Reveal answer" })).toHaveProperty("disabled", true);
    fireEvent.click(screen.getByRole("button", { name: "Reveal answer" }));
    expect(mocks.reveal).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Retry image" })).toBeTruthy();
  });

  it("does not expose answer text before a pending native reveal acknowledges success", async () => {
    let finish!: (value: ImageOcclusionAttemptView) => void;
    mocks.reveal.mockImplementationOnce(() => new Promise<ImageOcclusionAttemptView>((resolve) => { finish = resolve; }));
    render(<Harness />);
    fireEvent.click(screen.getByRole("button", { name: "Reveal answer" }));
    expect(screen.queryByText(/ANSWER_FROM_NATIVE/)).toBeNull();
    expect(screen.getByRole("button", { name: "Revealing…" })).toHaveProperty("disabled", true);
    await act(async () => { finish(receipt(required(mocks.reveal.mock.calls[0]?.[0], "reveal input"))); });
    await waitFor(() => expect(screen.getByText(/ANSWER_FROM_NATIVE/)).toBeTruthy());
  });
});
