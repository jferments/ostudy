import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { createOperationId } from "../../ipc/common";
import * as studyMaterials from "../../ipc/studyMaterials";
import { useStudyTargetBundleController } from "./useStudyTargetBundleController";

vi.mock("../../ipc/studyMaterials", () => ({
  beginStudyTargetBundle: vi.fn(),
  composeStudyTargetBundle: vi.fn(),
  cancelStudyTargetBundle: vi.fn(),
  getStudyTargetBundle: vi.fn(),
}));
vi.mock("../../ipc/common", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../ipc/common")>();
  return { ...actual, createOperationId: vi.fn(() => "o".repeat(32)) };
});

const target = { kind: "collection" as const, id: "c".repeat(32) };
const queued: studyMaterials.StudyTargetBundleStatus = {
  targetId: "t".repeat(32),
  target,
  title: "Week 1",
  revision: 2,
  sourceSnapshotId: "s".repeat(64),
  sourceChanged: false,
  deckId: null,
  guideId: null,
  guideRevision: null,
  latestOperationId: "o".repeat(32),
  latestBatchState: "queued",
  flashcards: {
    kind: "flashcards",
    state: "pending",
    artifactId: null,
    totalUnits: 0,
    completedUnits: 0,
    safeErrorCode: "",
    safeErrorMessage: "",
  },
  studyGuide: {
    kind: "study_guide",
    state: "pending",
    artifactId: null,
    totalUnits: 0,
    completedUnits: 0,
    safeErrorCode: "",
    safeErrorMessage: "",
  },
};
const complete: studyMaterials.StudyTargetBundleStatus = {
  ...queued,
  deckId: "d".repeat(32),
  guideId: "g".repeat(32),
  guideRevision: 1,
  latestBatchState: "complete",
  flashcards: { ...queued.flashcards, state: "succeeded", artifactId: "d".repeat(32) },
  studyGuide: { ...queued.studyGuide, state: "succeeded", artifactId: "g".repeat(32) },
};

describe("study target bundle controller", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(createOperationId).mockReturnValue("o".repeat(32));
    vi.mocked(studyMaterials.getStudyTargetBundle).mockResolvedValue(null);
    vi.mocked(studyMaterials.beginStudyTargetBundle).mockResolvedValue(queued);
    vi.mocked(studyMaterials.composeStudyTargetBundle).mockResolvedValue(complete);
    vi.mocked(studyMaterials.cancelStudyTargetBundle).mockResolvedValue(undefined);
  });

  it("publishes the durable queued/running status while composition remains in flight", async () => {
    let finish!: (value: studyMaterials.StudyTargetBundleStatus) => void;
    vi.mocked(studyMaterials.composeStudyTargetBundle).mockReturnValueOnce(
      new Promise((resolve) => { finish = resolve; }),
    );
    const { result } = renderHook(() => useStudyTargetBundleController({ target }));
    await waitFor(() => expect(studyMaterials.getStudyTargetBundle).toHaveBeenCalledTimes(1));

    let running!: Promise<boolean>;
    await act(async () => {
      running = result.current.run();
      await Promise.resolve();
    });
    await waitFor(() => expect(result.current.status?.latestBatchState).toBe("queued"));
    expect(result.current.busy).toBe(true);

    await act(async () => {
      finish(complete);
      await running;
    });
    expect(result.current.busy).toBe(false);
    expect(result.current.status?.latestBatchState).toBe("complete");
  });

  it("cancels the exact running operation and keeps completed durable work reusable", async () => {
    let finish!: (value: studyMaterials.StudyTargetBundleStatus) => void;
    vi.mocked(studyMaterials.composeStudyTargetBundle).mockReturnValueOnce(
      new Promise((resolve) => { finish = resolve; }),
    );
    const { result } = renderHook(() => useStudyTargetBundleController({ target }));
    await waitFor(() => expect(studyMaterials.getStudyTargetBundle).toHaveBeenCalledTimes(1));

    let running!: Promise<boolean>;
    await act(async () => {
      running = result.current.run();
      await Promise.resolve();
    });
    await waitFor(() => expect(result.current.busy).toBe(true));

    await act(async () => {
      await result.current.stop();
    });
    expect(studyMaterials.cancelStudyTargetBundle).toHaveBeenCalledWith("o".repeat(32));
    expect(result.current.canceling).toBe(true);

    const canceled = {
      ...queued,
      latestBatchState: "canceled",
      flashcards: { ...queued.flashcards, state: "canceled" },
      studyGuide: { ...queued.studyGuide, state: "canceled" },
    };
    await act(async () => {
      finish(canceled);
      await running;
    });
    expect(result.current.busy).toBe(false);
    expect(result.current.canceling).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("reuses the same durable operation after an ambiguous compose response", async () => {
    const { result } = renderHook(() => useStudyTargetBundleController({ target }));
    await waitFor(() => expect(studyMaterials.getStudyTargetBundle).toHaveBeenCalledTimes(1));

    vi.mocked(studyMaterials.composeStudyTargetBundle)
      .mockRejectedValueOnce(new Error("response lost"))
      .mockResolvedValueOnce(complete);
    vi.mocked(studyMaterials.getStudyTargetBundle)
      .mockResolvedValueOnce(queued);

    await act(async () => {
      expect(await result.current.run()).toBe(false);
    });
    await act(async () => {
      expect(await result.current.run()).toBe(true);
    });

    expect(createOperationId).toHaveBeenCalledTimes(1);
    expect(studyMaterials.beginStudyTargetBundle).toHaveBeenCalledTimes(2);
    expect(studyMaterials.beginStudyTargetBundle).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({ operationId: "o".repeat(32), target }),
    );
    expect(studyMaterials.beginStudyTargetBundle).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({ operationId: "o".repeat(32), target }),
    );
    expect(result.current.status?.latestBatchState).toBe("complete");
  });

  it("requests both artifacts without flashcard instructions, which collections no longer use", async () => {
    const { result } = renderHook(() => useStudyTargetBundleController({ target }));
    await waitFor(() => expect(studyMaterials.getStudyTargetBundle).toHaveBeenCalledTimes(1));
    vi.mocked(studyMaterials.composeStudyTargetBundle).mockResolvedValueOnce(complete);

    await act(async () => {
      expect(await result.current.run()).toBe(true);
    });

    expect(studyMaterials.beginStudyTargetBundle).toHaveBeenCalledWith({
      operationId: "o".repeat(32),
      target,
      outputPolicy: { flashcards: true, studyGuide: true },
      recreateArtifacts: [],
    });
  });

  it("makes only the chosen output and recreates only a chosen suppressed one", async () => {
    vi.mocked(studyMaterials.getStudyTargetBundle).mockResolvedValue({
      ...complete,
      flashcards: { ...complete.flashcards, state: "suppressed" },
      studyGuide: { ...complete.studyGuide, state: "suppressed" },
    });
    const { result } = renderHook(() => useStudyTargetBundleController({ target }));
    await waitFor(() => expect(result.current.status).not.toBeNull());

    await act(async () => {
      expect(await result.current.run({ flashcards: false, studyGuide: true })).toBe(true);
    });

    expect(studyMaterials.beginStudyTargetBundle).toHaveBeenCalledWith({
      operationId: "o".repeat(32),
      target,
      outputPolicy: { flashcards: false, studyGuide: true },
      recreateArtifacts: ["study_guide"],
    });
  });

  it("accepts a terminal durable receipt after a lost response without repeating the write", async () => {
    const changed = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useStudyTargetBundleController({ target, changed }));
    await waitFor(() => expect(studyMaterials.getStudyTargetBundle).toHaveBeenCalledTimes(1));

    vi.mocked(studyMaterials.composeStudyTargetBundle).mockRejectedValueOnce(new Error("response lost"));
    vi.mocked(studyMaterials.getStudyTargetBundle).mockResolvedValueOnce(complete);

    await act(async () => {
      expect(await result.current.run()).toBe(true);
    });

    expect(studyMaterials.beginStudyTargetBundle).toHaveBeenCalledTimes(1);
    expect(studyMaterials.composeStudyTargetBundle).toHaveBeenCalledTimes(1);
    expect(changed).toHaveBeenCalledTimes(1);
    expect(result.current.status?.deckId).toBe("d".repeat(32));
  });
});
