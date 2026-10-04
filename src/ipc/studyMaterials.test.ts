import { invoke } from "./transport";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  beginStudyTargetBundle,
  composeStudyTargetBundle,
  getStudyTargetBundle,
} from "./studyMaterials";

vi.mock("./transport", () => ({ invoke: vi.fn() }));

describe("study material IPC", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockResolvedValue(null);
  });

  it("forwards optional per-run collection flashcard guidance without changing blank callers", async () => {
    const target = { kind: "collection" as const, id: "c".repeat(32) };
    const input = {
      operationId: "g".repeat(32),
      target,
      generationInstructions: "Focus on nursing cranial nerve assessments",
      outputPolicy: { flashcards: true, studyGuide: true },
      recreateArtifacts: [],
    };
    await beginStudyTargetBundle(input);
    expect(invoke).toHaveBeenLastCalledWith("begin_study_target_bundle", { input });
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

    const composeInput = { operationId: input.operationId };
    await composeStudyTargetBundle(composeInput);
    expect(invoke).toHaveBeenLastCalledWith("compose_study_target_bundle", {
      input: composeInput,
    });
  });
});
