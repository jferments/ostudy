import { RadioGroup } from "../../ui/primitives/FormControls";
import { Panel } from "../../ui/primitives/Panel";
import type { AppSettings, MotionPreference } from "../../ipc/settings";
import { HelpTrigger } from "../../ui/help/HelpProvider";
import { GuidePdfThemePicker } from "../study-guides/GuideDialogs";
import { FontControl, RangeControl, SelectControl, SwitchRow, ThemePreview } from "./SettingsControls";

interface AppearanceSettingsProps {
  draft: AppSettings;
  update: (patch: Partial<AppSettings>) => void;
}

const MOTION_OPTIONS: ReadonlyArray<{ value: MotionPreference; label: string }> = [
  { value: "system", label: "Follow system" },
  { value: "full", label: "Full motion" },
  { value: "reduced", label: "Reduced motion" },
  { value: "none", label: "No nonessential motion" },
];

export function AppearanceSettings({ draft, update }: AppearanceSettingsProps) {
  return (
    <div className="tw:grid tw:min-w-0 tw:gap-5 tw:xl:grid-cols-2">
      <Panel className="tw:col-span-full" title="Theme">
        <RadioGroup label="Application theme" value={draft.theme} presentation="cards"
          options={(["light", "dark", "console", "synthwave"] as const).map((theme) => ({
            value: theme, ariaLabel: theme.charAt(0).toUpperCase() + theme.slice(1),
            label: <ThemePreview theme={theme} />,
          }))} onValueChange={(theme) => update({ theme })} />
      </Panel>

      <Panel className="tw:min-w-0" title="Text">
        <FontControl label="Interface font" value={draft.uiFont} onChange={(uiFont) => update({ uiFont })} />
        <FontControl label="Default card font" value={draft.cardFont} onChange={(cardFont) => update({ cardFont })} />
        <RangeControl label="Interface scale" min={80} max={140} step={5} value={draft.uiScale} suffix="%" onChange={(uiScale) => update({ uiScale })} />
        <RangeControl label="Default card text" min={12} max={32} step={1} value={draft.cardBaseSize} suffix=" px" onChange={(cardBaseSize) => update({ cardBaseSize })} />
      </Panel>

      <Panel className="tw:min-w-0 tw:self-start" title="Motion and art"
        actions={<HelpTrigger topicId="settings.motion" label="Help with motion preferences" />}>
        <SelectControl
          label="Motion"
          value={draft.motionPreference ?? "system"}
          options={MOTION_OPTIONS}
          onChange={(motionPreference) => update({ motionPreference })}
        />
        <div className="tw:grid tw:divide-y tw:divide-border/70">
          <SwitchRow label="Decorative artwork" checked={draft.decorativeArtwork ?? true}
            onChange={(decorativeArtwork) => update({ decorativeArtwork })} />
          <SwitchRow label="Celebrations" checked={draft.celebrationsEnabled ?? true}
            onChange={(celebrationsEnabled) => update({ celebrationsEnabled })} />
          <SwitchRow label="Interactive tutorials" checked={draft.tutorialsEnabled ?? false}
            onChange={(tutorialsEnabled) => update({ tutorialsEnabled })} />
        </div>
      </Panel>

      <Panel className="tw:col-span-full" title="Study guides"
        description="The study guide theme is how a guide looks both when you read it and when you save it as a PDF.">
        <GuidePdfThemePicker value={draft.guidePdfTheme ?? "sage"} onChange={(guidePdfTheme) => update({ guidePdfTheme })} />
        <SwitchRow label="Include my guide and section notes"
          checked={draft.guidePdfIncludeNotes ?? true} onChange={(guidePdfIncludeNotes) => update({ guidePdfIncludeNotes })} />
      </Panel>
    </div>
  );
}
