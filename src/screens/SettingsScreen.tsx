import { useEffect, useRef, useState } from "react";

import type { ShowToast } from "../components/Toast";
import { useAiProfilesController } from "../controllers/aiProfilesController";
import { getErrorMessage } from "../ipc/common";
import { saveSettings, type AppSettings } from "../ipc/settings";
import { PageHeader } from "../ui/patterns/WorkspacePatterns";
import { Tabs, type TabDefinition } from "../ui/primitives/Tabs";
import { AiLlmSettings } from "./settings/AiLlmSettings";
import { AppearanceSettings } from "./settings/AppearanceSettings";
import { DefaultAiModelSettings } from "./settings/DefaultAiModelSettings";
import { DiagnosticsSettings } from "./settings/DiagnosticsSettings";
import { ReviewSchedulingSettings } from "./settings/ReviewSchedulingSettings";
import { StorageBackupSettings } from "./settings/StorageBackupSettings";
import { Button } from "../ui/primitives/Button";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { ownedClassNames, sharedClassMaps } from "../ui/styles/classOwnership";

const ownedStyleMaps = sharedClassMaps;


interface SettingsScreenProps {
  settings: AppSettings;
  onPreviewSettings: (settings: AppSettings) => void;
  onSavedSettings: (settings: AppSettings) => void;
  onDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
}

type SettingsTab = "storage" | "appearance" | "ai" | "review" | "diagnostics";

const DEFAULTS: AppSettings = {
  theme: "dark",
  uiFont: "plex",
  cardFont: "plex",
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
  aiReasoningEffort: "low",
};

export function SettingsScreen({
  settings,
  onPreviewSettings,
  onSavedSettings,
  onDirtyChange,
  showToast,
}: SettingsScreenProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("appearance");
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
    if (activeTab === "ai") update({ aiReasoningEffort: DEFAULTS.aiReasoningEffort });
  }

  async function persist(): Promise<void> {
    if (savingRef.current || !preferencesDirty) return;
    savingRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);
    try {
      const saved = await saveSettings({
        ...draft,
        aiReasoningEffort: draft.aiReasoningEffort ?? "low",
      });
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
      value: "appearance",
      label: "Appearance",
      panel: <AppearanceSettings draft={draft} update={update} />,
    },
    {
      value: "storage",
      label: "Backup/Sync",
      panel: <StorageBackupSettings draft={draft} update={update} showToast={showToast} />,
    },
    {
      value: "ai",
      label: "AI/LLM",
      panel: (
        <AiLlmSettings
          controller={aiController}
          sidebarTop={
            <DefaultAiModelSettings controller={aiController} settings={draft} update={update} />
          }
        />
      ),
    },
    {
      value: "review",
      label: "Review/Scheduling",
      panel: <ReviewSchedulingSettings draft={draft} update={update} />,
    },
    {
      value: "diagnostics",
      label: "Diagnostics",
      panel: <DiagnosticsSettings showToast={showToast} />,
    },
  ];

  const defaultsLabel =
    activeTab === "appearance"
      ? "appearance"
      : activeTab === "review"
        ? "scheduling"
        : activeTab === "ai"
          ? "AI"
          : "safety";
  const canRestoreDefaults =
    activeTab === "appearance" ||
    activeTab === "review" ||
    activeTab === "storage" ||
    activeTab === "ai";

  return (
    <div className={ownedClassNames(ownedStyleMaps, "screen")} aria-busy={isSaving || aiController.mutating}>
      <PageHeader
        title="Settings"
        icon="settings"
        compact
        actions={
          <div className={ownedClassNames(ownedStyleMaps, "header-actions")}>
            {canRestoreDefaults ? (
              <Button
                type="button"
                variant="secondary"
                disabled={isSaving}
                onClick={restoreCurrentDefaults}
              >
                Restore {defaultsLabel} defaults
              </Button>
            ) : null}
            {preferencesDirty ? (
              <Button
                type="button"
                variant="primary"
                disabled={isSaving}
                onClick={() => void persist()}
              >
                {isSaving ? "Saving…" : "Save preferences"}
              </Button>
            ) : null}
          </div>
        }
      />

      {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
      {preferencesDirty ? (
        <InlineNotice kind="info">Previewing unsaved preferences.</InlineNotice>
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
