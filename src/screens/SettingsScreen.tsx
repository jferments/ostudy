import { useEffect, useRef, useState, type ReactNode } from "react";

import type { ShowToast } from "../components/Toast";
import { useAiProfilesController } from "../controllers/aiProfilesController";
import { getErrorMessage } from "../ipc/common";
import { saveSettings, type AppSettings } from "../ipc/settings";
import { registerPlatformBackHandler } from "../platform/backNavigation";
import { DEFAULT_FONT_PRESET } from "../ui/appearance/appearanceTypes";
import { Icon, type IconName } from "../ui/icons/Icon";
import { IconButton } from "../ui/icons/IconButton";
import { useLayoutMode } from "../ui/layout/LayoutModeProvider";
import { PageScaffold } from "../ui/patterns/WorkspacePatterns";
import { Button } from "../ui/primitives/Button";
import { Card } from "../ui/primitives/Card";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { ListRow, ListRowMedia, ListRows } from "../ui/primitives/ListRow";
import { PaneTabs } from "../ui/primitives/Tabs";
import { TemplatesScreen } from "./TemplatesScreen";
import { AiLlmSettings } from "./settings/AiLlmSettings";
import type { AiSetupStart } from "./settings/AiSetupWizard";
import { BuiltInTemplates } from "./settings/BuiltInTemplates";
import { AppearanceSettings } from "./settings/AppearanceSettings";
import { DefaultAiModelSettings } from "./settings/DefaultAiModelSettings";
import { LessonVoiceSettings } from "./settings/LessonVoiceSettings";
import { DiagnosticsSettings } from "./settings/DiagnosticsSettings";
import { ReviewSchedulingSettings } from "./settings/ReviewSchedulingSettings";
import { StorageBackupSettings } from "./settings/StorageBackupSettings";


type SettingsTab =
  | "storage"
  | "appearance"
  | "ai"
  | "review"
  | "templates"
  | "diagnostics";

interface SettingsScreenProps {
  settings: AppSettings;
  initialSection?: SettingsTab;
  /** Opens Settings → AI with the setup wizard; consumed once shown. */
  aiSetupRequest?: { requestId: number; service: "openrouter" | null } | null;
  onAiSetupConsumed?: () => void;
  onPreviewSettings: (settings: AppSettings) => void;
  onSavedSettings: (settings: AppSettings) => void;
  onDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
}

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
  newDayStartHour: 4,
};

/** Study first, then services and data; Diagnostics sits apart at the end. */
const SECTIONS: ReadonlyArray<{ value: SettingsTab; label: string; icon: IconName }> = [
  { value: "appearance", label: "Appearance", icon: "view" },
  { value: "review", label: "Review", icon: "schedule" },
  { value: "templates", label: "Card templates", icon: "template" },
  { value: "ai", label: "AI", icon: "sparkles" },
  { value: "storage", label: "Backup and sync", icon: "archive" },
  { value: "diagnostics", label: "Diagnostics", icon: "information" },
];

const sectionLabel = (value: SettingsTab) => SECTIONS.find((section) => section.value === value)?.label ?? "Settings";

export function SettingsScreen({
  settings,
  initialSection = "appearance",
  aiSetupRequest = null,
  onAiSetupConsumed,
  onPreviewSettings,
  onSavedSettings,
  onDirtyChange,
  showToast,
}: SettingsScreenProps) {
  const { layoutMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const [activeTab, setActiveTab] = useState<SettingsTab>(initialSection);
  // A phone shows the list of sections until one is chosen; a direct link opens its section.
  const [compactOpen, setCompactOpen] = useState(initialSection !== "appearance");
  // Sections mount when first opened and then stay mounted, so their unsaved drafts survive.
  const [visited, setVisited] = useState<ReadonlySet<SettingsTab>>(() => new Set([initialSection]));
  const [draft, setDraft] = useState(settings);
  const [savedBaseline, setSavedBaseline] = useState(settings);
  const [templateDirty, setTemplateDirty] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const savingRef = useRef(false);
  const aiController = useAiProfilesController(showToast);
  const [aiSetup, setAiSetup] = useState<AiSetupStart | null>(null);
  const preferencesDirty = JSON.stringify(draft) !== JSON.stringify(savedBaseline);
  const dirty = preferencesDirty || aiController.dirty || templateDirty;
  const showingSection = !compact || compactOpen;

  useEffect(() => {
    setActiveTab(initialSection);
    setCompactOpen(initialSection !== "appearance");
  }, [initialSection]);

  useEffect(() => {
    if (!aiSetupRequest) return;
    setActiveTab("ai");
    setCompactOpen(true);
    setAiSetup(aiSetupRequest.service ? { service: aiSetupRequest.service } : {});
    onAiSetupConsumed?.();
  }, [aiSetupRequest, onAiSetupConsumed]);

  useEffect(() => {
    if (!visited.has(activeTab)) setVisited((previous) => new Set(previous).add(activeTab));
  }, [activeTab, visited]);

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

  // Android Back steps from a phone's section to the list of sections before leaving Settings.
  useEffect(() => {
    if (!compact || !compactOpen) return;
    return registerPlatformBackHandler(() => setCompactOpen(false));
  }, [compact, compactOpen]);

  function update(patch: Partial<AppSettings>): void {
    if (savingRef.current) return;
    setDraft((current) => ({ ...current, ...patch }));
  }

  function openSection(value: SettingsTab): void {
    setActiveTab(value);
    setCompactOpen(true);
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
        newDayStartHour: DEFAULTS.newDayStartHour,
      });
    }
    if (activeTab === "storage") update({ confirmDeletions: DEFAULTS.confirmDeletions });
  }

  function discard(): void {
    if (savingRef.current) return;
    setDraft(savedBaseline);
    setErrorMessage(null);
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

  // A new object each time, so the wizard opens again after it was closed.
  const addOpenRouter = () => setAiSetup({ service: "openrouter" });

  function panel(value: SettingsTab): ReactNode {
    switch (value) {
      case "appearance":
        return <AppearanceSettings draft={draft} update={update} />;
      case "review":
        return <ReviewSchedulingSettings draft={draft} update={update} />;
      case "templates":
        return <TemplatesScreen builtIn={<BuiltInTemplates />} onDirtyChange={setTemplateDirty} showToast={showToast} />;
      case "ai":
        return (
          <AiLlmSettings
            controller={aiController}
            setupRequest={aiSetup}
            defaultBlock={
              <DefaultAiModelSettings
                controller={aiController}
                onAddOpenRouter={addOpenRouter}
                extraRows={<LessonVoiceSettings connectionsKey={aiController.profiles} onAddOpenRouter={addOpenRouter} />}
              />
            }
          />
        );
      case "storage":
        return <StorageBackupSettings draft={draft} update={update} showToast={showToast} />;
      case "diagnostics":
        return <DiagnosticsSettings showToast={showToast} />;
    }
  }

  const canRestoreDefaults = activeTab === "appearance" || activeTab === "review" || activeTab === "storage";

  const sectionHeader = (
    <header className="tw:flex tw:min-h-16 tw:min-w-0 tw:items-center tw:gap-2 tw:border-0 tw:border-b tw:border-solid tw:border-border/70 tw:px-8 tw:py-2 tw:compact:min-h-14 tw:compact:px-2">
      {compact ? (
        <IconButton icon="back" label="Back to Settings" onClick={() => setCompactOpen(false)} />
      ) : null}
      {compact ? (
        <h1 className="tw:m-0 tw:min-w-0 tw:flex-1 tw:truncate tw:text-section tw:font-semibold">{sectionLabel(activeTab)}</h1>
      ) : (
        <h2 className="tw:m-0 tw:min-w-0 tw:flex-1 tw:truncate tw:text-section tw:font-semibold">{sectionLabel(activeTab)}</h2>
      )}
      {canRestoreDefaults ? (
        <Button type="button" variant="ghost" size="small" disabled={isSaving} onClick={restoreCurrentDefaults}>
          Restore defaults
        </Button>
      ) : null}
    </header>
  );

  const saveBar = preferencesDirty ? (
    <div
      role="region"
      aria-label="Unsaved changes"
      className="tw:sticky tw:bottom-4 tw:z-(--ostudy-layer-sticky) tw:mt-6 tw:flex tw:max-w-3xl tw:items-center tw:gap-3 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:py-2 tw:pr-2 tw:pl-5 tw:shadow-lg tw:compact:bottom-2 tw:compact:pl-4"
    >
      <span aria-hidden="true" className="tw:size-2 tw:shrink-0 tw:rounded-full tw:bg-primary tw:compact:hidden" />
      <span className="tw:min-w-0 tw:flex-1 tw:text-body tw:font-medium">Unsaved changes</span>
      <Button type="button" variant="secondary" disabled={isSaving} onClick={discard}>Discard</Button>
      <Button type="button" variant="primary" disabled={isSaving} onClick={() => void persist()}>
        {isSaving ? "Saving…" : "Save"}
      </Button>
    </div>
  ) : null;

  const body = (children: ReactNode) => (
    <div className="tw:min-h-0 tw:min-w-0 tw:flex-1 tw:overflow-y-auto tw:overscroll-contain tw:px-8 tw:pt-6 tw:pb-10 tw:compact:px-4 tw:compact:pt-4 tw:compact:pb-6">
      <div className="tw:grid tw:max-w-5xl tw:min-w-0 tw:content-start tw:gap-5">
        {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
        {children}
      </div>
      {saveBar}
    </div>
  );

  if (compact) {
    return (
      <PageScaffold
        width="full"
        inset={false}
        className="tw:flex tw:h-full tw:flex-col"
        aria-busy={isSaving || aiController.mutating}
        data-testid="settings-screen"
      >
        {showingSection ? sectionHeader : null}
        {!showingSection ? body(
          <>
            <h1 className="tw:m-0 tw:text-title tw:font-semibold tw:tracking-tight">Settings</h1>
            <nav aria-label="Settings sections">
              <Card className="tw:py-0">
                <ListRows>
                  {SECTIONS.map((section) => (
                    <ListRow
                      key={section.value}
                      title={section.label}
                      leading={<ListRowMedia icon={section.icon} className="tw:bg-primary/10 tw:text-primary" />}
                      chevron
                      onActivate={() => openSection(section.value)}
                    />
                  ))}
                </ListRows>
              </Card>
            </nav>
          </>,
        ) : null}
        {/* Opened sections stay mounted while the list or another section shows, keeping drafts. */}
        {SECTIONS.filter((section) => visited.has(section.value)).map((section) => {
          const active = showingSection && section.value === activeTab;
          return (
            <section
              key={section.value}
              aria-label={section.label}
              hidden={!active}
              className={active ? "tw:flex tw:min-h-0 tw:flex-1 tw:flex-col" : undefined}
            >
              {body(panel(section.value))}
            </section>
          );
        })}
      </PageScaffold>
    );
  }

  return (
    <PageScaffold
      width="full"
      inset={false}
      className="tw:h-full"
      aria-busy={isSaving || aiController.mutating}
      data-testid="settings-screen"
    >
      <PaneTabs
        value={activeTab}
        onValueChange={setActiveTab}
        ariaLabel="Settings sections"
        heading={<h1 className="tw:mx-3 tw:mt-2 tw:mb-4 tw:text-section tw:font-semibold tw:tracking-tight">Settings</h1>}
        header={sectionHeader}
        tabs={SECTIONS.map((section) => ({
          value: section.value,
          label: section.label,
          icon: <Icon name={section.icon} size={20} />,
          separated: section.value === "diagnostics",
          panel: visited.has(section.value) ? body(panel(section.value)) : null,
        }))}
      />
    </PageScaffold>
  );
}
