import { invoke } from "@tauri-apps/api/core";
import { beforeEach, expect, it, vi } from "vitest";
import { changeTutorMemory, getTutorMemory, type ChangeTutorMemoryInput } from "./tutorMemory";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => { vi.mocked(invoke).mockReset(); vi.mocked(invoke).mockResolvedValue(null); });
it("uses narrow local native commands and preserves reset operation identity", async () => {
  const input: ChangeTutorMemoryInput = { operationId: "a".repeat(32), conversationId: "b".repeat(32), expectedRevision: 5, action: "forget_earlier" };
  await getTutorMemory(input.conversationId);
  await changeTutorMemory(input);
  await changeTutorMemory(input);
  expect(vi.mocked(invoke).mock.calls).toEqual([
    ["get_tutor_memory", { conversationId: input.conversationId }],
    ["change_tutor_memory", { input }], ["change_tutor_memory", { input }],
  ]);
});
