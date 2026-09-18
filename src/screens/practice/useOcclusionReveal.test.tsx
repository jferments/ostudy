import { act, renderHook, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ImageOcclusionAttemptView, RevealPracticeQuestionInput } from "../../ipc/questionAttempts";
import { useOcclusionReveal } from "./useOcclusionReveal";

const mocks = vi.hoisted(() => ({ reload: vi.fn(), reveal: vi.fn(), exposed: vi.fn(), accepted: vi.fn() }));
vi.mock("../../ipc/questionAttempts", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../ipc/questionAttempts")>(),
  getPracticeQuestionAttempt: mocks.reload,
}));
const target = { sessionId: "1".repeat(32), cardId: "2".repeat(32), instanceId: "3".repeat(32) };
function answering(revision = 1): ImageOcclusionAttemptView {
  return {
    instanceId: target.instanceId, revision, assisted: revision === 2,
    question: { kind: "image_occlusion", prompt: { type: "doc" }, targetRectangles: [] },
    progress: { phase: "answering", savedAnswer: null },
  };
}
function revealed(): ImageOcclusionAttemptView {
  return { ...answering(), revision: 2, progress: {
    phase: "revealed", revealId: "4".repeat(32), responseTimeMs: 234, revealedAtMs: 5000,
    answer: { type: "doc" }, explanation: null,
  } };
}
function receipt(input: RevealPracticeQuestionInput): ImageOcclusionAttemptView {
  const view = revealed();
  if (view.progress.phase !== "revealed") throw new Error("Expected reveal fixture");
  return { ...view, revision: input.request.expectedRevision + 1,
    assisted: input.request.expectedRevision === 2,
    progress: { ...view.progress, revealId: input.request.revealId, responseTimeMs: input.request.responseTimeMs } };
}
function harness(initial = answering()) {
  const [attempt, setAttempt] = useState(initial);
  const interaction = useOcclusionReveal({
    target, attempt, disabled: false, imageReady: true, loadedAt: 1000,
    onReveal: mocks.reveal,
    onAttempt: (view) => { setAttempt(view); mocks.accepted(view); }, onRevealed: mocks.exposed,
  });
  return { interaction, attempt };
}
beforeEach(() => {
  for (const mock of Object.values(mocks)) mock.mockReset();
  mocks.reveal.mockImplementation(async (input: RevealPracticeQuestionInput) => receipt(input));
  mocks.reload.mockResolvedValue({ attempt: answering(), assets: [] });
  mocks.exposed.mockResolvedValue(undefined);
  vi.spyOn(Date, "now").mockReturnValue(1234);
});
afterEach(() => { vi.restoreAllMocks(); });

describe("self-assessed reveal mutation ownership", () => {
  it("holds one request across duplicate clicks while the native mutation is pending", async () => {
    let finish!: (view: ImageOcclusionAttemptView) => void;
    mocks.reveal.mockImplementationOnce(() => new Promise<ImageOcclusionAttemptView>((resolve) => { finish = resolve; }));
    const { result } = renderHook(() => harness());
    act(() => { void result.current.interaction.reveal(); void result.current.interaction.reveal(); });
    expect(mocks.reveal).toHaveBeenCalledTimes(1);
    expect(result.current.interaction.busy).toBe(true);
    await act(async () => { finish(receipt(mocks.reveal.mock.calls[0][0])); });
    await waitFor(() => expect(result.current.attempt.progress.phase).toBe("revealed"));
    expect(mocks.exposed).toHaveBeenCalledTimes(1);
    expect(result.current.interaction.busy).toBe(false);
  });

  it("a lost acknowledgement retries the exact identity, revision and first response time", async () => {
    mocks.reveal.mockRejectedValueOnce(new Error("Acknowledgement lost"));
    const { result } = renderHook(() => harness());
    await act(async () => { await result.current.interaction.reveal(); });
    expect(result.current.interaction.error).toMatch(/same request/);
    const first = structuredClone(mocks.reveal.mock.calls[0][0]);
    expect(first.request.responseTimeMs).toBe(234);
    expect(first.request.expectedRevision).toBe(1);
    vi.mocked(Date.now).mockReturnValue(9000);
    await act(async () => { await result.current.interaction.reveal(); });
    expect(mocks.reveal.mock.calls[1][0]).toEqual(first);
    expect(result.current.attempt.progress.phase).toBe("revealed");
  });

  it("unchanged answering state after reload does not authorize a new retry identity", async () => {
    mocks.reveal.mockRejectedValueOnce(new Error("Unknown result"));
    const { result } = renderHook(() => harness());
    await act(async () => { await result.current.interaction.reveal(); });
    const first = structuredClone(mocks.reveal.mock.calls[0][0]);
    await act(async () => { await result.current.interaction.reload(); });
    vi.mocked(Date.now).mockReturnValue(9000);
    await act(async () => { await result.current.interaction.reveal(); });
    expect(mocks.reveal.mock.calls[1][0]).toEqual(first);
  });

  it("newer native assistance evidence admits a new request rather than replaying an invalid revision", async () => {
    mocks.reveal.mockRejectedValueOnce(new Error("Unknown result"));
    mocks.reload.mockResolvedValueOnce({ attempt: answering(2), assets: [] });
    const { result } = renderHook(() => harness());
    await act(async () => { await result.current.interaction.reveal(); });
    const first = structuredClone(mocks.reveal.mock.calls[0][0]);
    await act(async () => { await result.current.interaction.reload(); });
    await act(async () => { await result.current.interaction.reveal(); });
    const next = mocks.reveal.mock.calls[1][0];
    expect(next.request.expectedRevision).toBe(2);
    expect(next.request.revealId).not.toBe(first.request.revealId);
  });

  it("older native evidence cannot roll back an attempt or discard its ambiguous request", async () => {
    mocks.reveal.mockRejectedValueOnce(new Error("Unknown result"));
    const { result } = renderHook(() => harness(answering(2)));
    await act(async () => { await result.current.interaction.reveal(); });
    const first = structuredClone(mocks.reveal.mock.calls[0][0]);
    await act(async () => { await result.current.interaction.reload(); });
    expect(result.current.attempt.revision).toBe(2);
    await act(async () => { await result.current.interaction.reveal(); });
    expect(mocks.reveal.mock.calls[1][0]).toEqual(first);
  });

  it("reloading a committed reveal restores feedback without another native reveal", async () => {
    mocks.reveal.mockRejectedValueOnce(new Error("Acknowledgement lost"));
    mocks.reload.mockResolvedValueOnce({ attempt: revealed(), assets: [] });
    const { result } = renderHook(() => harness());
    await act(async () => { await result.current.interaction.reveal(); });
    await act(async () => { await result.current.interaction.reload(); });
    expect(result.current.attempt.progress.phase).toBe("revealed");
    await act(async () => { await result.current.interaction.reveal(); });
    expect(mocks.reveal).toHaveBeenCalledTimes(1);
    expect(mocks.exposed).toHaveBeenCalledTimes(1);
  });

  it("a feedback-refresh failure does not turn a committed reveal into a second mutation", async () => {
    mocks.exposed.mockRejectedValueOnce(new Error("Interval refresh failed"));
    const { result } = renderHook(() => harness());
    await act(async () => { await result.current.interaction.reveal(); });
    expect(result.current.attempt.progress.phase).toBe("revealed");
    expect(result.current.interaction.error).toMatch(/saved answer state/);
    expect(result.current.interaction.error).not.toMatch(/Retry Reveal/);
    await act(async () => { await result.current.interaction.reveal(); });
    expect(mocks.reveal).toHaveBeenCalledTimes(1);
  });

  it("a response for a different captured question is rejected without losing the original retry identity", async () => {
    mocks.reveal.mockResolvedValueOnce({ ...revealed(), instanceId: "f".repeat(32) });
    const { result } = renderHook(() => harness());
    await act(async () => { await result.current.interaction.reveal(); });
    const first = structuredClone(mocks.reveal.mock.calls[0][0]);
    expect(mocks.accepted).not.toHaveBeenCalled();
    expect(result.current.attempt.progress.phase).toBe("answering");
    await act(async () => { await result.current.interaction.reveal(); });
    expect(mocks.reveal.mock.calls[1][0]).toEqual(first);
  });
});
