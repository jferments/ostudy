import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../ipc";
import { makeSettings } from "../test/factories";
import { SettingsScreen } from "./SettingsScreen";

vi.mock("../ipc", () => ({
  getAppInfo: vi.fn(),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  saveSettings: vi.fn(),
}));

function renderSettings(
  settings = makeSettings(),
  overrides: Partial<React.ComponentProps<typeof SettingsScreen>> = {},
) {
  const props: React.ComponentProps<typeof SettingsScreen> = {
    settings,
    onPreviewSettings: vi.fn(),
    onSavedSettings: vi.fn(),
    onDirtyChange: vi.fn(),
    showToast: vi.fn(),
    ...overrides,
  };
  render(<SettingsScreen {...props} />);
  return props;
}

describe("SettingsScreen extended behavior", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.getAppInfo).mockResolvedValue({
      appVersion: "0.1.0",
      dataDirectory: "/tmp/ostudy/data",
      databasePath: "/tmp/ostudy/data/library.sqlite3",
      imageDirectory: "/tmp/ostudy/data/assets/images",
      databaseSchemaVersion: 10,
    });
    vi.mocked(ipc.saveSettings).mockImplementation(async (settings) => settings);
  });

  it("restores every product default as an unsaved preview", async () => {
    const props = renderSettings(
      makeSettings({
        theme: "synthwave",
        uiFont: "serif",
        cardFont: "mono",
        uiScale: 135,
        cardBaseSize: 30,
        reduceMotion: true,
        confirmDeletions: false,
        practiceShuffle: false,
        desiredRetention: 0.97,
        dailyNewLimit: 99,
        dailyReviewLimit: 999,
      }),
    );

    await screen.findByText("/tmp/ostudy/data");
    fireEvent.click(screen.getByRole("button", { name: "Restore defaults" }));

    await waitFor(() => {
      expect(props.onPreviewSettings).toHaveBeenLastCalledWith(makeSettings());
      expect(props.onDirtyChange).toHaveBeenLastCalledWith(true);
    });
    expect(screen.getByText("Previewing unsaved settings.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Save settings" })).toBeTruthy();
  });

  it("converts the visible retention percentage to a bounded decimal", async () => {
    const props = renderSettings();
    await screen.findByText("/tmp/ostudy/data");

    fireEvent.change(
      screen.getByRole("slider", { name: /Desired retention/u }),
      { target: { value: "94" } },
    );

    await waitFor(() => {
      expect(props.onPreviewSettings).toHaveBeenLastCalledWith(
        expect.objectContaining({ desiredRetention: 0.94 }),
      );
    });
  });

  it("previews an installed custom font without treating its name as CSS", async () => {
    const props = renderSettings();
    await screen.findByText("/tmp/ostudy/data");

    fireEvent.change(screen.getByRole("combobox", { name: "Interface font" }), {
      target: { value: "custom" },
    });
    fireEvent.change(screen.getByRole("textbox", { name: "Font family name" }), {
      target: { value: "Atkinson Hyperlegible" },
    });

    await waitFor(() => {
      expect(props.onPreviewSettings).toHaveBeenLastCalledWith(
        expect.objectContaining({ uiFont: "custom:Atkinson Hyperlegible" }),
      );
    });
  });

  it("keeps an unsaved draft visible when native persistence fails", async () => {
    vi.mocked(ipc.saveSettings).mockRejectedValue(new Error("Settings database is busy."));
    const props = renderSettings();
    await screen.findByText("/tmp/ostudy/data");

    fireEvent.change(screen.getByRole("spinbutton", { name: "New cards per day" }), {
      target: { value: "12" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save settings" }));

    expect(await screen.findByText("Settings database is busy.")).toBeTruthy();
    expect(props.onSavedSettings).not.toHaveBeenCalled();
    expect(props.showToast).not.toHaveBeenCalled();
    expect(
      (screen.getByRole("spinbutton", { name: "New cards per day" }) as HTMLInputElement)
        .value,
    ).toBe("12");
    expect(screen.getByRole("button", { name: "Save settings" })).toBeTruthy();
  });

  it("reports an app-info failure without preventing settings editing", async () => {
    vi.mocked(ipc.getAppInfo).mockRejectedValue(new Error("Storage paths unavailable."));
    const props = renderSettings();

    expect(await screen.findByText("Storage paths unavailable.")).toBeTruthy();
    fireEvent.click(screen.getByRole("radio", { name: /Light/u }));
    await waitFor(() => {
      expect(props.onPreviewSettings).toHaveBeenLastCalledWith(
        expect.objectContaining({ theme: "light" }),
      );
    });
  });
});
