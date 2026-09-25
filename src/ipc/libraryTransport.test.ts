import assert from "node:assert/strict";
import { describe, it } from "vitest";
import { createLibraryTransport, type NativeInvoke } from "./libraryTransport";
import { restoredLibraryMessage } from "../components/libraryRestoreResult";

function transport(handler: (command: string, args: unknown) => unknown) {
  const calls: Array<[string, unknown]> = [];
  const raw: NativeInvoke = async <T>(command: string, args?: Parameters<NativeInvoke>[1]) => {
    calls.push([command, args]);
    return await handler(command, args) as T;
  };
  return { call: createLibraryTransport(raw), calls };
}

describe("library generation transport", () => {
  it("bootstraps once for concurrent requests and preserves caller arguments", async () => {
    const { call, calls } = transport((command) => command === "get_library_session"
      ? { generation: 4, recoveryRequired: false } : "ok");
    const input = { input: { name: "New deck" }, libraryGeneration: 99 };
    await Promise.all([call("create_deck", input), call("get_settings")]);
    assert.equal(calls.filter(([name]) => name === "get_library_session").length, 1);
    assert.equal(input.libraryGeneration, 99);
    assert.deepEqual(calls.find(([name]) => name === "create_deck")?.[1], {
      input: { name: "New deck" }, libraryGeneration: 4,
    });
  });

  it("does not silently retry a stale durable mutation with a new generation", async () => {
    let generation = 0;
    const { call, calls } = transport((command, args) => {
      if (command === "get_library_session") return { generation, recoveryRequired: false };
      if ((args as { libraryGeneration: number }).libraryGeneration !== generation) {
        throw { code: "library_generation_changed" };
      }
      return "ok";
    });
    await call("get_settings");
    generation = 1;
    await call("get_library_session");
    await assert.rejects(call("create_deck", { input: { name: "Old page" } }), {
      code: "library_generation_changed",
    });
    assert.equal(calls.filter(([name]) => name === "create_deck").length, 1);
  });

  it("permits retry of failed bootstrap without retrying a mutation", async () => {
    let attempts = 0;
    const { call, calls } = transport((command) => {
      if (command === "get_library_session") {
        if (attempts++ === 0) throw new Error("startup transport interrupted");
        return { generation: 0, recoveryRequired: false };
      }
      return "ok";
    });
    await assert.rejects(call("create_deck"), /startup transport interrupted/u);
    await call("create_deck");
    assert.equal(calls.filter(([name]) => name === "create_deck").length, 1);
  });

  it("passes plugin traffic through without library arguments", async () => {
    const { call, calls } = transport(() => "ok");
    const args = { event: "restore" };
    await call("plugin:event|listen", args);
    assert.deepEqual(calls, [["plugin:event|listen", args]]);
  });

  it("rejects unsupported binary arguments and recovery-required sessions", async () => {
    const { call, calls } = transport(() => ({ generation: 2, recoveryRequired: true }));
    await assert.rejects(call("create_deck", new Uint8Array([1])), { code: "invalid_library_request" });
    assert.equal(calls.length, 0);
    await assert.rejects(call("create_deck"), { code: "library_recovery_required" });
    assert.equal(calls.filter(([name]) => name === "create_deck").length, 0);
  });

  it("does not report staged-only restoration as successful", () => {
    const result = { operationId: "1".repeat(32), completed: true, restartRequired: false,
      deckCount: 1, cardCount: 189, documentCount: 1, templateCount: 0, reviewCount: 0, assetCount: 0 };
    assert.equal(restoredLibraryMessage(result), "Restored 1 deck, 189 cards, and 1 document. Your library is ready.");
    assert.throws(() => restoredLibraryMessage({ ...result, completed: false, restartRequired: true }));
    assert.throws(() => restoredLibraryMessage({ ...result, cardCount: -1 }));
  });
});
