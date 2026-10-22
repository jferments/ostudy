import { invoke } from "./transport";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { applyMaterialInkChanges, getMaterialInk } from "./materialInk";

vi.mock("./transport", () => ({ invoke: vi.fn() }));

const lessonId = "a".repeat(32);
const documentId = "d".repeat(32);

describe("material ink IPC", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({ marks: [] });
  });

  it("reads a lesson's marks", async () => {
    expect(await getMaterialInk({ kind: "lesson", lessonId })).toEqual([]);
    expect(invoke).toHaveBeenCalledWith("get_material_ink", { input: { material: { kind: "lesson", lessonId } } });
  });

  it("saves changes to a guide's page marks", async () => {
    const changes = [{ op: "delete" as const, id: "c".repeat(32) }];
    await applyMaterialInkChanges({ kind: "studyGuide", documentId }, changes);
    expect(invoke).toHaveBeenCalledWith("apply_material_ink_changes", {
      input: { material: { kind: "studyGuide", documentId }, changes },
    });
  });
});
