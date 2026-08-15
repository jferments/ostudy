import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  createDeckFolder,
  deleteDeckFolder,
  getDeckLibraryOrganization,
  moveDeckFolder,
  moveDeckInLibrary,
  updateDeckFolder,
} from "./deckFolders";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const organization = { revision: 1, folders: [], placements: [] };

describe("deck folder IPC", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockResolvedValue(organization);
  });

  it("uses explicit typed command names and payloads", async () => {
    await getDeckLibraryOrganization();
    expect(invoke).toHaveBeenLastCalledWith("get_deck_library_organization");

    const createInput = { name: "Nursing", expectedOrganizationRevision: 1 };
    await createDeckFolder(createInput);
    expect(invoke).toHaveBeenLastCalledWith("create_deck_folder", { input: createInput });

    const updateInput = {
      folderId: "f".repeat(32),
      name: "Clinical nursing",
      revision: 1,
      expectedOrganizationRevision: 2,
    };
    await updateDeckFolder(updateInput);
    expect(invoke).toHaveBeenLastCalledWith("update_deck_folder", { input: updateInput });

    const deleteInput = {
      folderId: "f".repeat(32),
      revision: 2,
      expectedOrganizationRevision: 3,
    };
    await deleteDeckFolder(deleteInput);
    expect(invoke).toHaveBeenLastCalledWith("delete_deck_folder", { input: deleteInput });

    const moveDeckInput = {
      deckId: "d".repeat(32),
      targetFolderId: "f".repeat(32),
      targetIndex: 0,
      expectedOrganizationRevision: 4,
    };
    await moveDeckInLibrary(moveDeckInput);
    expect(invoke).toHaveBeenLastCalledWith("move_deck_in_library", { input: moveDeckInput });

    const moveFolderInput = {
      folderId: "f".repeat(32),
      targetIndex: 1,
      expectedOrganizationRevision: 5,
    };
    await moveDeckFolder(moveFolderInput);
    expect(invoke).toHaveBeenLastCalledWith("move_deck_folder", { input: moveFolderInput });
  });
});
