import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FrontendDiagnosticEvent } from "./diagnostics";

const nativeInvoke = vi.hoisted(() => vi.fn());
vi.mock("./transport", () => ({ invoke: nativeInvoke }));

let diagnostics: typeof import("./diagnostics");

function sentEvents(): FrontendDiagnosticEvent[] {
  return nativeInvoke.mock.calls.flatMap(([, args]) => {
    const batch = args as { input: { events: FrontendDiagnosticEvent[] } };
    expect(batch.input.events.length).toBeLessThanOrEqual(32);
    return batch.input.events;
  });
}

beforeEach(async () => {
  vi.useFakeTimers();
  vi.resetModules();
  nativeInvoke.mockReset().mockResolvedValue(undefined);
  diagnostics = await import("./diagnostics");
});

afterEach(async () => {
  await diagnostics.flushFrontendDiagnostics();
  await vi.runAllTimersAsync();
  vi.useRealTimers();
});

describe("bounded caught-failure diagnostics", () => {
  it("retains a bounded queue behind a stalled native call and reports the drop count", async () => {
    let release: () => void = () => { throw new Error("the transport was not started"); };
    nativeInvoke.mockImplementationOnce(() => new Promise<void>((resolve) => { release = resolve; }));
    diagnostics.queueFrontendDiagnostic({ level: "error", target: "frontend.test", event: "first_failure", message: "First failure." });
    for (let index = 0; index < 1000; index += 1) {
      diagnostics.queueFrontendDiagnostic({ level: "warn", target: "frontend.test", event: "later_failure", message: "Later failure." });
    }
    expect(nativeInvoke).toHaveBeenCalledTimes(1);
    release();
    await vi.runAllTimersAsync();
    const events = sentEvents();
    expect(events).toHaveLength(130); // first in flight + 128 retained + one drop summary
    expect(events.filter((event) => event.event === "later_failure")).toHaveLength(128);
    expect(events.find((event) => event.event === "frontend_diagnostics_dropped")?.fields).toEqual({ count: 872 });
    await diagnostics.flushFrontendDiagnostics();
    expect(sentEvents()).toHaveLength(130);
  });

  it("does not recursively report or replay failed diagnostic batches", async () => {
    nativeInvoke.mockRejectedValue(new Error("Unavailable bridge"));
    for (let index = 0; index < 65; index += 1) {
      diagnostics.queueFrontendDiagnostic({ level: "warn", target: "frontend.test", event: "listener_failure", message: "Listener failed." });
    }
    await vi.runAllTimersAsync();
    await expect(diagnostics.flushFrontendDiagnostics()).resolves.toBeUndefined();
    expect(sentEvents()).toHaveLength(65);
    expect(nativeInvoke).toHaveBeenCalledTimes(3);
    await vi.runAllTimersAsync();
    expect(nativeInvoke).toHaveBeenCalledTimes(3);
  });

  it("reports static context and allowlisted codes without captured private content", async () => {
    const secret = "private-api-key-and-source-document";
    const error = new Error(secret);
    error.stack = `stack including ${secret}`;
    diagnostics.reportFrontendFailure("android_back_listener_failed", error);
    diagnostics.reportFrontendFailure("incoming_share_list_failed", { code: secret, displayName: secret });
    diagnostics.reportFrontendFailure("clipboard_image_read_failed", new DOMException(secret, "NotAllowedError"));
    await diagnostics.flushFrontendDiagnostics();
    expect(sentEvents().map((event) => event.fields)).toEqual([
      { code: "unexpected" }, { code: "unexpected" }, { code: "permission_denied" },
    ]);
    expect(sentEvents().map((event) => event.event)).toEqual([
      "android_back_listener_failed", "incoming_share_list_failed", "clipboard_image_read_failed",
    ]);
    expect(JSON.stringify(nativeInvoke.mock.calls)).not.toContain(secret);
    expect(sentEvents().every((event) => event.stack === undefined)).toBe(true);
  });

  it("does not throw while inspecting an unexpected diagnostic value", async () => {
    const fields = { get source() { throw new Error("private getter failure"); } };
    expect(() => diagnostics.queueFrontendDiagnostic({
      level: "warn", target: "frontend.test", event: "invalid_fields", message: "Failed.", fields,
    })).not.toThrow();
    await diagnostics.flushFrontendDiagnostics();
    expect(sentEvents()).toEqual([expect.objectContaining({
      event: "frontend_diagnostics_dropped", fields: { count: 1 },
    })]);
    expect(JSON.stringify(nativeInvoke.mock.calls)).not.toContain("private getter");
  });
});
