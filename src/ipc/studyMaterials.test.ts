import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { beginStudyTargetBundle, getStudyTargetBundle } from "./studyMaterials";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("study material IPC", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockResolvedValue(null);
  });

  it("keeps durable StudyTarget lifecycle behind typed native commands", async () => {
    const target = { kind: "collection" as const, id: "c".repeat(32) };
    await getStudyTargetBundle({ target });
    expect(invoke).toHaveBeenLastCalledWith("get_study_target_bundle", {
      input: { target },
    });

    const input = {
      operationId: "o".repeat(32),
      target,
      outputPolicy: { flashcards: true, studyGuide: true },
      recreateArtifacts: ["flashcards" as const],
    };
    await beginStudyTargetBundle(input);
    expect(invoke).toHaveBeenLastCalledWith("begin_study_target_bundle", { input });
  });
});
