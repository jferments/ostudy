import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { AppSettings } from "../ipc/settings";
import { SettingsScreen } from "./SettingsScreen";

vi.mock("../controllers/aiProfilesController", () => ({
  useAiProfilesController: () => ({ dirty: false, mutating: false }),
}));
vi.mock("./settings/AiLlmSettings", () => ({ AiLlmSettings: () => <div>AI settings</div> }));
vi.mock("./settings/AppearanceSettings", () => ({ AppearanceSettings: () => <div>Appearance settings</div> }));
vi.mock("./settings/DefaultAiModelSettings", () => ({ DefaultAiModelSettings: () => <div>Default model</div> }));
vi.mock("./settings/DiagnosticsSettings", () => ({ DiagnosticsSettings: () => <div>Diagnostics settings</div> }));
vi.mock("./settings/ReviewSchedulingSettings", () => ({ ReviewSchedulingSettings: () => <div>Review settings</div> }));
vi.mock("./settings/StorageBackupSettings", () => ({ StorageBackupSettings: () => <div>Storage settings</div> }));
vi.mock("./TemplatesScreen", () => ({ TemplatesScreen: () => <div aria-label="Card template manager">Template manager</div> }));

const settings: AppSettings = {
  theme: "dark",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  motionPreference: "system",
  materialPreference: "automatic",
  decorativeArtwork: true,
  celebrationsEnabled: true,
  tutorialsEnabled: false,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};

describe("Settings Card templates tool", () => {
  it("opens template management from Settings without adding another Settings tab", () => {
    render(
      <SettingsScreen
        settings={settings}
        onPreviewSettings={vi.fn()}
        onSavedSettings={vi.fn()}
        onDirtyChange={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual([
      "Appearance",
      "Backup/Sync",
      "AI/LLM",
      "Review/Scheduling",
      "Diagnostics",
    ]);
    expect(screen.queryByLabelText("Card template manager")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Card templates" }));

    expect(screen.getByRole("tabpanel", { name: "Card templates" })).toBeTruthy();
    expect(screen.getByLabelText("Card template manager")).toBeTruthy();
    expect(screen.getAllByRole("tab")).toHaveLength(5);
  });
});
