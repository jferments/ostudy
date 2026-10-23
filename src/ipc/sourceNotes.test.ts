import { invoke } from "./transport";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { writeSourceNotes } from "./sourceNotes";

vi.mock("./transport", () => ({ invoke: vi.fn() }));

describe("source notes IPC", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({});
  });

  it("writes notes from a prompt under one operation", async () => {
    const operationId = "d".repeat(32);
    await writeSourceNotes("The Krebs cycle for my exam", operationId);
    expect(invoke).toHaveBeenCalledWith("write_source_notes", {
      input: { operationId, prompt: "The Krebs cycle for my exam" },
    });
  });
});
