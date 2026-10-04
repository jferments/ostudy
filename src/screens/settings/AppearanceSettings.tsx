import { Checkbox, RadioGroup } from "../../ui/primitives/FormControls";
import { Badge } from "../../ui/primitives/Badge";
import { Progress } from "../../ui/primitives/Progress";
import { Button } from "../../ui/primitives/Button";
import { Panel } from "../../ui/primitives/Panel";
import { StatTile } from "../../ui/primitives/StatTile";
import type { AppSettings, MotionPreference } from "../../ipc/settings";
import { HelpTrigger } from "../../ui/help/HelpProvider";
import { FontControl, RangeControl, SelectControl, ThemePreview } from "./SettingsControls";

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
    <div className="tw:grid tw:min-w-0 tw:gap-6 tw:xl:grid-cols-2">
      <Panel className="tw:col-span-full">
        <div className="tw:flex tw:flex-wrap tw:items-start tw:justify-between tw:gap-4 tw:[&_h2]:m-0 tw:[&_h2]:text-section tw:[&_h2]:font-semibold tw:[&_p]:mt-2 tw:[&_p]:mb-0 tw:[&_p]:text-label tw:[&_p]:font-normal tw:[&_p]:leading-relaxed tw:[&_p]:text-muted-foreground">
          <div>
            <h2 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight">Theme</h2>
            <p>Theme and typography changes preview immediately throughout ostudy.</p>
          </div>
        </div>
        <RadioGroup label="Application theme" value={draft.theme} presentation="cards"
          options={(["light", "dark", "console", "synthwave"] as const).map((theme) => ({
            value: theme, ariaLabel: theme.charAt(0).toUpperCase() + theme.slice(1),
            label: <ThemePreview theme={theme} />,
          }))} onValueChange={(theme) => update({ theme })} />
      </Panel>

      <Panel className="tw:min-w-0">
        <h2 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight">Typography</h2>
        <FontControl label="Interface font" value={draft.uiFont} onChange={(uiFont) => update({ uiFont })} />
        <FontControl label="Default card font" value={draft.cardFont} onChange={(cardFont) => update({ cardFont })} />
      </Panel>

      <Panel className="tw:min-w-0">
        <h2 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight">Scale</h2>
        <RangeControl label="Interface scale" min={80} max={140} step={5} value={draft.uiScale} suffix="%" onChange={(uiScale) => update({ uiScale })} />
        <RangeControl label="Default card text" min={12} max={32} step={1} value={draft.cardBaseSize} suffix=" px" onChange={(cardBaseSize) => update({ cardBaseSize })} />
      </Panel>

      <Panel className="tw:min-w-0">
        <div className="tw:flex tw:flex-wrap tw:items-start tw:justify-between tw:gap-4 tw:[&_h2]:m-0 tw:[&_h2]:text-section tw:[&_h2]:font-semibold tw:[&_p]:mt-2 tw:[&_p]:mb-0 tw:[&_p]:text-label tw:[&_p]:font-normal tw:[&_p]:leading-relaxed tw:[&_p]:text-muted-foreground">
          <h2 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight">Motion</h2>
          <HelpTrigger topicId="settings.motion" label="Help with motion preferences" />
        </div>
        <SelectControl
          label="Motion"
          value={draft.motionPreference ?? "system"}
          options={MOTION_OPTIONS}
          description="Reduced motion removes spatial transitions, card flips, parallax, reorder interpolation, and particle effects."
          onChange={(motionPreference) => update({ motionPreference })}
        />
      </Panel>

      <Panel className="tw:min-w-0">
        <h2 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight">Artwork and guidance</h2>
        <label className="tw:flex tw:min-h-11 tw:items-start tw:justify-between tw:gap-6 tw:text-label tw:[&>span:first-child]:grid tw:[&>span:first-child]:gap-1 tw:[&_small]:text-muted-foreground tw:[&_small]:font-normal tw:[&_small]:leading-relaxed">
          <span>
            <strong>Decorative artwork</strong>
            <small>Show mascot art and illustrated empty states. Card and document images are unaffected.</small>
          </span>
          <Checkbox checked={draft.decorativeArtwork ?? true} onCheckedChange={(checked) => update({ decorativeArtwork: checked })} />
        </label>
        <label className="tw:flex tw:min-h-11 tw:items-start tw:justify-between tw:gap-6 tw:text-label tw:[&>span:first-child]:grid tw:[&>span:first-child]:gap-1 tw:[&_small]:text-muted-foreground tw:[&_small]:font-normal tw:[&_small]:leading-relaxed">
          <span>
            <strong>Celebrations</strong>
            <small>Allow one-shot completion effects when full motion is enabled.</small>
          </span>
          <Checkbox checked={draft.celebrationsEnabled ?? true} onCheckedChange={(checked) => update({ celebrationsEnabled: checked })} />
        </label>
        <label className="tw:flex tw:min-h-11 tw:items-start tw:justify-between tw:gap-6 tw:text-label tw:[&>span:first-child]:grid tw:[&>span:first-child]:gap-1 tw:[&_small]:text-muted-foreground tw:[&_small]:font-normal tw:[&_small]:leading-relaxed">
          <span>
            <strong>Interactive tutorials</strong>
            <small>Allow tutorials that you explicitly start from Help. They never open automatically.</small>
          </span>
          <Checkbox checked={draft.tutorialsEnabled ?? false} onCheckedChange={(checked) => update({ tutorialsEnabled: checked })} />
        </label>
      </Panel>

      <Panel className="tw:col-span-full" aria-labelledby="appearance-preview-heading">
        <div className="tw:flex tw:flex-wrap tw:items-start tw:justify-between tw:gap-4 tw:[&_h2]:m-0 tw:[&_h2]:text-section tw:[&_h2]:font-semibold tw:[&_p]:mt-2 tw:[&_p]:mb-0 tw:[&_p]:text-label tw:[&_p]:font-normal tw:[&_p]:leading-relaxed tw:[&_p]:text-muted-foreground">
          <div>
            <h2 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight" id="appearance-preview-heading">Live visual-system preview</h2>
            <p>Representative surfaces, typography, controls, progress, and metrics use the same semantic tokens as the application.</p>
          </div>
        </div>
        <AppearancePreview />
      </Panel>
    </div>
  );
}

function AppearancePreview() {
  return <div className="tw:grid tw:items-start tw:gap-4 tw:lg:grid-cols-[2fr_1fr]">
    <Panel title="How does spaced repetition improve recall?" description="Practice · a preview of your chosen appearance">
      <p className="tw:m-0 tw:text-body tw:leading-relaxed">It schedules review near the point where a memory is likely to become difficult to retrieve.</p>
      <Progress value={68} max={100} label="68 percent complete" />
      <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-3">
        <Badge tone="success">Ready to review</Badge>
        <Button type="button" variant="primary" disabled>Create card</Button>
      </div>
    </Panel>
    <div className="tw:grid tw:gap-4 tw:sm:grid-cols-2 tw:lg:grid-cols-1">
      <StatTile label="Ready now" value={24} />
      <StatTile label="Studied this week" value={138} delta={{ label: "12 cards", direction: "up", tone: "positive" }} />
    </div>
  </div>;
}
