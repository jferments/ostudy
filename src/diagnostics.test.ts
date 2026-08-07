import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { recordWebviewDiagnostic } from "./diagnostics";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("recordWebviewDiagnostic", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  it("sends bounded structured diagnostics rather than arbitrary objects", async () => {
    const error = new Error("x".repeat(5_000));
    error.stack = "s".repeat(13_000);

    await recordWebviewDiagnostic("webview_render_error", error);

    expect(invoke).toHaveBeenCalledWith("record_webview_diagnostic", {
      input: {
        event: "webview_render_error",
        message: expect.stringMatching(/truncated 1000 characters/u),
        stack: expect.stringMatching(/truncated 1000 characters/u),
      },
    });
  });

  it("absorbs diagnostic transport failures", async () => {
    vi.mocked(invoke).mockRejectedValue(new Error("native unavailable"));
    await expect(recordWebviewDiagnostic("not valid", { private: "payload" })).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith(
      "record_webview_diagnostic",
      expect.objectContaining({
        input: expect.objectContaining({ event: "webview_error", message: "Unhandled WebView error" }),
      }),
    );
  });
});
