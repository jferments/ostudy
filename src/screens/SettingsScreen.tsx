import { useEffect, useRef, useState } from "react";

import type { ShowToast } from "../components/Toast";
import { useAiProfilesController } from "../controllers/aiProfilesController";
import { getErrorMessage } from "../ipc/common";
import { saveSettings, type AppSettings } from "../ipc/settings";
import { Tabs, type TabDefinition } from "../ui/primitives/Tabs";
import { AiLlmSettings } from "./settings/AiLlmSettings";
import { AppearanceSettings } from "./settings/AppearanceSettings";
import { DefaultAiModelSettings } from "./settings/DefaultAiModelSettings";
import { ReviewSchedulingSettings } from "./settings/ReviewSchedulingSettings";
import { StorageBackupSettings } from "./settings/StorageBackupSettings";

interface SettingsScreenProps {
  settings: AppSettings;
  onPreviewSettings: (settings: AppSettings) => void;
  onSavedSettings: (settings: AppSettings) => void;
  onDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
}

type SettingsTab = "storage" | "appearance" | "ai" | "review";

const DEFAULTS: AppSettings = {
  theme: "dark",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  motionPreference: "system",
  materialPreference: "automatic",
  decorativeArtwork: true,
  celebrationsEnabled: true,
  tutorialsEnabled: true,
  deckViewDensity: "comfortable",
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};

export function SettingsScreen({
  settings,
  onPreviewSettings,
  onSavedSettings,
  onDirtyChange,
  showToast,
}: SettingsScreenProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("storage");
  const [draft, setDraft] = useState(settings);
  const [savedBaseline, setSavedBaseline] = useState(settings);
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const savingRef = useRef(false);
  const aiController = useAiProfilesController(showToast);
  const preferencesDirty = JSON.stringify(draft) !== JSON.stringify(savedBaseline);
  const dirty = preferencesDirty || aiController.dirty;

  useEffect(() => {
    setDraft(settings);
    setSavedBaseline(settings);
  }, [settings]);

  useEffect(() => {
    onPreviewSettings(draft);
  }, [draft, onPreviewSettings]);

  useEffect(() => {
    onDirtyChange(dirty);
    return () => onDirtyChange(false);
  }, [dirty, onDirtyChange]);

  function update(patch: Partial<AppSettings>): void {
    if (savingRef.current) return;
    setDraft((current) => ({ ...current, ...patch }));
  }

  function restoreCurrentDefaults(): void {
    if (activeTab === "appearance") {
      update({
        theme: DEFAULTS.theme,
        uiFont: DEFAULTS.uiFont,
        cardFont: DEFAULTS.cardFont,
        uiScale: DEFAULTS.uiScale,
        cardBaseSize: DEFAULTS.cardBaseSize,
        motionPreference: DEFAULTS.motionPreference,
        materialPreference: DEFAULTS.materialPreference,
        decorativeArtwork: DEFAULTS.decorativeArtwork,
        celebrationsEnabled: DEFAULTS.celebrationsEnabled,
        tutorialsEnabled: DEFAULTS.tutorialsEnabled,
        deckViewDensity: DEFAULTS.deckViewDensity,
      });
    }
    if (activeTab === "review") {
      update({
        desiredRetention: DEFAULTS.desiredRetention,
        dailyNewLimit: DEFAULTS.dailyNewLimit,
        dailyReviewLimit: DEFAULTS.dailyReviewLimit,
      });
    }
    if (activeTab === "storage") update({ confirmDeletions: DEFAULTS.confirmDeletions });
  }

  async function persist(): Promise<void> {
    if (savingRef.current || !preferencesDirty) return;
    savingRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);
    try {
      const saved = await saveSettings(draft);
      onSavedSettings(saved);
      setDraft(saved);
      setSavedBaseline(saved);
      showToast("success", "Settings saved.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      savingRef.current = false;
      setIsSaving(false);
    }
  }

  const tabs: ReadonlyArray<TabDefinition<SettingsTab>> = [
    {
      value: "storage",
      label: "Storage/Backup",
      panel: <StorageBackupSettings draft={draft} update={update} showToast={showToast} />,
    },
    {
      value: "appearance",
      label: "Appearance",
      panel: <AppearanceSettings draft={draft} update={update} />,
    },
    {
      value: "ai",
      label: "AI/LLM",
      panel: (
        <AiLlmSettings
          controller={aiController}
          sidebarTop={<DefaultAiModelSettings controller={aiController} />}
        />
      ),
    },
    {
      value: "review",
      label: "Review/Scheduling",
      panel: <ReviewSchedulingSettings draft={draft} update={update} />,
    },
  ];

  const defaultsLabel =
    activeTab === "appearance"
      ? "appearance"
      : activeTab === "review"
        ? "scheduling"
        : "safety";

  return (
    <div className="screen settings-screen" aria-busy={isSaving || aiController.mutating}>
      <header className="screen-header compact-screen-header">
        <div>
          <h1>Settings</h1>
          <p>Configure local study behavior, storage, and explicit AI provider connections.</p>
        </div>
        <div className="header-actions">
          {activeTab !== "ai" ? (
            <button
              type="button"
              className="secondary-button"
              disabled={isSaving}
              onClick={restoreCurrentDefaults}
            >
              Restore {defaultsLabel} defaults
            </button>
          ) : null}
          {preferencesDirty ? (
            <button
              type="button"
              className="primary-button"
              disabled={isSaving}
              onClick={() => void persist()}
            >
              {isSaving ? "Saving…" : "Save preferences"}
            </button>
          ) : null}
        </div>
      </header>

      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
      {preferencesDirty ? (
        <div className="inline-alert info-alert">Previewing unsaved preferences.</div>
      ) : null}

      <Tabs
        value={activeTab}
        onValueChange={setActiveTab}
        tabs={tabs}
        ariaLabel="Settings sections"
      />
    </div>
  );
}
