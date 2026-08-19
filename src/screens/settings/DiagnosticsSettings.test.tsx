import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as diagnostics from "../../ipc/diagnostics";
import { DiagnosticsSettings } from "./DiagnosticsSettings";

vi.mock("../../ipc/diagnostics", () => ({
  getDiagnosticConfiguration: vi.fn(),
  setDiagnosticFilter: vi.fn(),
  flushNativeDiagnostics: vi.fn(),
}));

const configuration: diagnostics.DiagnosticConfiguration = {
  filter: "info",
  logPath: "/tmp/ostudy/diagnostics/ostudy.jsonl",
  droppedEvents: 0,
  queueCapacity: 2048,
  maximumFileBytes: 2 * 1024 * 1024,
};

describe("DiagnosticsSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(diagnostics.getDiagnosticConfiguration).mockResolvedValue(configuration);
    vi.mocked(diagnostics.setDiagnosticFilter).mockImplementation(async (filter) => ({
      ...configuration,
      filter,
    }));
    vi.mocked(diagnostics.flushNativeDiagnostics).mockResolvedValue(true);
  });

  it("enables an automatically expiring subsystem filter", async () => {
    const showToast = vi.fn();
    render(<DiagnosticsSettings showToast={showToast} />);
    expect(await screen.findByText("/tmp/ostudy/diagnostics/ostudy.jsonl")).toBeTruthy();

    fireEvent.change(screen.getByLabelText("Subsystem"), { target: { value: "qr" } });
    fireEvent.change(screen.getByLabelText("Detail level"), { target: { value: "trace" } });
    fireEvent.change(screen.getByLabelText("Automatic expiry"), { target: { value: "300" } });
    fireEvent.click(screen.getByRole("button", { name: "Enable temporary diagnostics" }));

    await waitFor(() => {
      expect(diagnostics.setDiagnosticFilter).toHaveBeenCalledWith(
        "warn,ostudy.qr=trace",
        300,
      );
      expect(showToast).toHaveBeenCalledWith("success", "Temporary diagnostics enabled.");
    });
  });

  it("restores normal diagnostics and flushes the bounded writer", async () => {
    const showToast = vi.fn();
    render(<DiagnosticsSettings showToast={showToast} />);
    await screen.findByText("info");

    fireEvent.click(screen.getByRole("button", { name: "Restore normal diagnostics" }));
    await waitFor(() => {
      expect(diagnostics.setDiagnosticFilter).toHaveBeenCalledWith("info");
    });

    fireEvent.click(screen.getByRole("button", { name: "Flush diagnostic log" }));
    await waitFor(() => {
      expect(diagnostics.flushNativeDiagnostics).toHaveBeenCalledTimes(1);
      expect(showToast).toHaveBeenCalledWith("success", "Diagnostic log flushed.");
    });
  });

  it("states the local-only content-redaction contract", async () => {
    render(<DiagnosticsSettings showToast={vi.fn()} />);
    expect(await screen.findByText(/never uploads diagnostics automatically/u)).toBeTruthy();
    expect(screen.getByText(/prompts, model responses/u)).toBeTruthy();
    expect(screen.getByText(/current and previous bounded log files/u)).toBeTruthy();
  });
});
