import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  clearUiCheckpoint,
  getUiCheckpoint,
  saveUiCheckpoint,
  type SaveUiCheckpointInput,
  type UiCheckpoint,
} from "./uiCheckpoint";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("UI checkpoint IPC", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it("uses the unchanged get command and returns the canonical v2 shape", async () => {
    const checkpoint: UiCheckpoint = {
      schemaVersion: 2,
      updatedAt: 10,
      section: "study-guides",
      flashcardsWorkspace: null,
      deckId: null,
      editorDraftId: null,
      practice: {
        sessionId: "s".repeat(32),
        position: 3,
        revealed: true,
        returnTarget: {
          section: "study-guides",
          flashcardsWorkspace: null,
          deckId: null,
        },
      },
      analytics: null,
    };
    vi.mocked(invoke).mockResolvedValue(checkpoint);

    await expect(getUiCheckpoint()).resolves.toEqual(checkpoint);
    expect(invoke).toHaveBeenCalledWith("get_ui_checkpoint");
  });

  it("passes visible destination and resumable Practice data to the unchanged save command", async () => {
    const input: SaveUiCheckpointInput = {
      section: "decks",
      flashcardsWorkspace: "search",
      practice: {
        sessionId: "p".repeat(32),
        position: 8,
        revealed: false,
        returnTarget: {
          section: "decks",
          flashcardsWorkspace: "search",
          deckId: null,
        },
      },
    };
    vi.mocked(invoke).mockResolvedValue({});

    await saveUiCheckpoint(input);

    expect(invoke).toHaveBeenCalledWith("save_ui_checkpoint", { input });
  });

  it("uses the unchanged clear command", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await clearUiCheckpoint();

    expect(invoke).toHaveBeenCalledWith("clear_ui_checkpoint");
  });
});
