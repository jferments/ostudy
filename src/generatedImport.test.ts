import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { importGeneratedFlashcards } from "./generatedImport";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("generated flashcard import IPC", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("uses the dedicated content-only import command and source path", async () => {
    const result = {
      deckId: "a".repeat(32),
      createdNoteCount: 3,
      createdCardCount: 4,
      skippedDuplicateCount: 1,
    };
    vi.mocked(invoke).mockResolvedValue(result);

    await expect(importGeneratedFlashcards("/tmp/cards.ostudy.json")).resolves.toEqual(
      result,
    );
    expect(invoke).toHaveBeenCalledWith("import_generated_flashcards", {
      sourcePath: "/tmp/cards.ostudy.json",
    });
  });

  it("does not translate native validation errors", async () => {
    const error = { code: "invalid_flashcard_import", message: "Invalid cards." };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(importGeneratedFlashcards("/tmp/invalid.json")).rejects.toBe(error);
  });
});
