import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as diagnostics from "../../ipc/diagnostics";
import * as settings from "../../ipc/settings";
import { DiagnosticsSettings } from "./DiagnosticsSettings";

vi.mock("../../ipc/diagnostics", () => ({
  getDiagnosticConfiguration: vi.fn(),
  setDiagnosticFilter: vi.fn(),
  flushNativeDiagnostics: vi.fn(),
}));
vi.mock("../../ipc/settings", () => ({ getAppInfo: vi.fn() }));

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
    vi.mocked(settings.getAppInfo).mockResolvedValue({
      appVersion: "0.1.0",
      dataDirectory: "/tmp/ostudy/data",
      databasePath: "/tmp/ostudy/data/library.sqlite3",
      imageDirectory: "/tmp/ostudy/data/assets/images",
      diagnosticLogPath: "/tmp/ostudy/diagnostics/ostudy.jsonl",
      databaseSchemaVersion: 3,
    });
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
    expect(screen.getByText("0.1.0")).toBeTruthy();
    expect(screen.getByText("3")).toBeTruthy();

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

  it("keeps the diagnostics screen focused on diagnostic controls rather than policy documentation", async () => {
    render(<DiagnosticsSettings showToast={vi.fn()} />);
    await screen.findByText("/tmp/ostudy/diagnostics/ostudy.jsonl");
    expect(screen.queryByRole("heading", { name: "Privacy and storage limits" })).toBeNull();
    expect(screen.queryByText(/never uploads diagnostics automatically/u)).toBeNull();
  });
});
