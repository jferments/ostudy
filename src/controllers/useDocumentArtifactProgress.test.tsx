import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { listLatestDocumentStudyGuideJobs } from "../ipc/documentArtifactProgress";
import type { DocumentArtifactJob } from "../ipc/studyGuides";
import { useDocumentArtifactProgress } from "./useDocumentArtifactProgress";

vi.mock("../ipc/documentArtifactProgress", () => ({ listLatestDocumentStudyGuideJobs: vi.fn() }));
const read = vi.mocked(listLatestDocumentStudyGuideJobs);
const job = { id: "job", documentId: "document", state: "generating", totalUnits: 10, completedUnits: 4 } as DocumentArtifactJob;
beforeEach(() => { vi.resetAllMocks(); vi.useFakeTimers(); });
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe("durable artifact observation", () => {
  it("keeps polling a guide after document processing ends and stops after publication", async () => {
    read.mockResolvedValueOnce([job]).mockResolvedValueOnce([{ ...job, state: "succeeded", completedUnits: 10 }]);
    const { result } = renderHook(() => useDocumentArtifactProgress(false));
    await act(async () => { await Promise.resolve(); });
    expect(result.current.jobs.document.completedUnits).toBe(4);
    await act(async () => { await vi.advanceTimersByTimeAsync(1_000); });
    expect(result.current.jobs.document.state).toBe("succeeded");
    await act(async () => { await vi.advanceTimersByTimeAsync(5_000); });
    expect(read).toHaveBeenCalledTimes(2);
  });

  it("a failed read retains the last successful job and does not claim success", async () => {
    read.mockResolvedValueOnce([job]).mockRejectedValueOnce(new Error("temporary read failure")).mockResolvedValue([job]);
    const { result } = renderHook(() => useDocumentArtifactProgress(false));
    await act(async () => { await Promise.resolve(); });
    await act(async () => { await vi.advanceTimersByTimeAsync(1_000); });
    expect(result.current.jobs.document).toEqual(job);
    expect(result.current.readFailed).toBe(true);
    await act(async () => { await vi.advanceTimersByTimeAsync(1_000); });
    expect(result.current.readFailed).toBe(false);
  });

  it("does not overlap reads or publish late results after navigation", async () => {
    let finish!: (rows: DocumentArtifactJob[]) => void;
    read.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const { result, unmount } = renderHook(() => useDocumentArtifactProgress(true));
    await act(async () => { await vi.advanceTimersByTimeAsync(5_000); });
    expect(read).toHaveBeenCalledTimes(1);
    unmount();
    await act(async () => { finish([job]); await Promise.resolve(); });
    expect(result.current.jobs).toEqual({});
    await act(async () => { await vi.advanceTimersByTimeAsync(5_000); });
    expect(read).toHaveBeenCalledTimes(1);
  });
});
