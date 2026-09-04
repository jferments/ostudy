import { useEffect, useRef, useState } from "react";

import type { ShowToast } from "../components/Toast";
import { useAiProfilesController } from "../controllers/aiProfilesController";
import { getErrorMessage } from "../ipc/common";
import { saveSettings, type AppSettings } from "../ipc/settings";
import { DEFAULT_FONT_PRESET } from "../ui/appearance/appearanceTypes";
import { Icon } from "../ui/icons/Icon";
import { PageHeader } from "../ui/patterns/WorkspacePatterns";
import { Tabs, type TabDefinition } from "../ui/primitives/Tabs";
import { Button } from "../ui/primitives/Button";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { ownedClassNames, sharedClassMaps } from "../ui/styles/classOwnership";
import { TemplatesScreen } from "./TemplatesScreen";
import { AiLlmSettings } from "./settings/AiLlmSettings";
import { AppearanceSettings } from "./settings/AppearanceSettings";
import { DefaultAiModelSettings } from "./settings/DefaultAiModelSettings";
import { DiagnosticsSettings } from "./settings/DiagnosticsSettings";
import { ReviewSchedulingSettings } from "./settings/ReviewSchedulingSettings";
import { StorageBackupSettings } from "./settings/StorageBackupSettings";
import styles from "./SettingsScreen.module.css";

const ownedStyleMaps = sharedClassMaps;

interface SettingsScreenProps {
  settings: AppSettings;
  onPreviewSettings: (settings: AppSettings) => void;
  onSavedSettings: (settings: AppSettings) => void;
  onDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
}

type SettingsTab =
  | "storage"
  | "appearance"
  | "ai"
  | "review"
  | "templates"
  | "diagnostics";

const DEFAULTS: AppSettings = {
  theme: "dark",
  uiFont: DEFAULT_FONT_PRESET,
  cardFont: DEFAULT_FONT_PRESET,
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
  const [templateDirty, setTemplateDirty] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const savingRef = useRef(false);
  const aiController = useAiProfilesController(showToast);
  const preferencesDirty = JSON.stringify(draft) !== JSON.stringify(savedBaseline);
  const dirty = preferencesDirty || aiController.dirty || templateDirty;

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
          defaultBlock={<DefaultAiModelSettings controller={aiController} />}
        />
      ),
    },
    {
      value: "review",
      label: "Review/Scheduling",
      panel: <ReviewSchedulingSettings draft={draft} update={update} />,
    },
    {
      value: "templates",
      label: "Card templates",
      hiddenFromTabList: true,
      panel: (
        <TemplatesScreen
          onDirtyChange={setTemplateDirty}
          showToast={showToast}
        />
      ),
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
        : "safety";
  const canRestoreDefaults =
    activeTab === "appearance" || activeTab === "review" || activeTab === "storage";

  return (
    <div
      className={ownedClassNames(ownedStyleMaps, "screen")}
      aria-busy={isSaving || aiController.mutating}
    >
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

      <div className={styles.shortcutRow} aria-label="Settings tools">
        <Button
          type="button"
          variant={activeTab === "templates" ? "primary" : "secondary"}
          onClick={() => setActiveTab("templates")}
        >
          <Icon name="template" size={18} /> Card templates
        </Button>
      </div>

      <Tabs
        value={activeTab}
        onValueChange={setActiveTab}
        tabs={tabs}
        ariaLabel="Settings sections"
      />
    </div>
  );
}
