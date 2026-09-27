import { beforeEach, describe, expect, it, vi } from "vitest";

const nativeInvoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: nativeInvoke,
}));

beforeEach(() => {
  nativeInvoke.mockReset();
  vi.resetModules();
});

describe("explicit application IPC transport", () => {
  it("routes a real domain gateway through one captured library generation", async () => {
    nativeInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_library_session") {
        return { generation: 7, ready: true, restoring: false, recoveryRequired: false, lastRestore: null };
      }
      if (command === "create_deck") {
        return {
          id: "a".repeat(32),
          name: "Transport proof",
          description: "",
          sourceNoteCount: 0,
          generatedCardCount: 0,
          studyableDueCount: 0,
          archivedAt: null,
          createdAt: 1,
          updatedAt: 1,
          revision: 1,
        };
      }
      throw new Error(`Unexpected command: ${command} ${JSON.stringify(args)}`);
    });

    const { createDeck } = await import("./decks");
    await createDeck({ name: "Transport proof" });

    expect(nativeInvoke).toHaveBeenNthCalledWith(1, "get_library_session");
    expect(nativeInvoke).toHaveBeenNthCalledWith(2, "create_deck", {
      input: { name: "Transport proof" },
      libraryGeneration: 7,
    }, undefined);
  });

  it("does not refresh the captured generation to replay a stale mutation", async () => {
    let liveGeneration = 3;
    nativeInvoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_library_session") {
        return { generation: liveGeneration, ready: true, restoring: false, recoveryRequired: false, lastRestore: null };
      }
      const requestGeneration = (args as { libraryGeneration?: number } | undefined)?.libraryGeneration;
      if (requestGeneration !== liveGeneration) throw { code: "library_generation_changed" };
      if (command === "get_settings") return {};
      if (command === "create_deck") return {};
      throw new Error(`Unexpected command: ${command}`);
    });

    const { getSettings } = await import("./settings");
    const { createDeck } = await import("./decks");
    await getSettings();
    liveGeneration = 4;

    await expect(createDeck({ name: "Old page" })).rejects.toMatchObject({
      code: "library_generation_changed",
    });
    expect(nativeInvoke.mock.calls.filter(([command]) => command === "get_library_session")).toHaveLength(1);
    expect(nativeInvoke.mock.calls.filter(([command]) => command === "create_deck")).toHaveLength(1);
  });

  it("lets the library-session gateway bypass generation injection through the same transport", async () => {
    nativeInvoke.mockResolvedValue({
      generation: 9,
      ready: true,
      restoring: false,
      recoveryRequired: false,
      lastRestore: null,
    });
    const { getLibrarySession } = await import("./librarySession");
    await expect(getLibrarySession()).resolves.toMatchObject({ generation: 9 });
    expect(nativeInvoke).toHaveBeenCalledExactlyOnceWith("get_library_session", undefined, undefined);
  });
});
