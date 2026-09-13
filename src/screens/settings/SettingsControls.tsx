import type { AppSettings } from "../../ipc/settings";
import { FONT_PRESET_OPTIONS } from "../../ui/appearance/appearanceTypes";
import { FormField } from "../../ui/primitives/FormField";
import { Slider } from "../../ui/primitives/FormControls";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { TextField } from "../../ui/primitives/TextField";

/** A miniature theme illustration, not another interactive control or palette. */
export function ThemePreview({ theme }: { theme: AppSettings["theme"] }) {
  const names = { light: "Light", dark: "Dark", console: "Console", synthwave: "Synthwave" };
  const colors = {
    light: ["var(--ostudy-theme-preview-light-canvas)", "var(--ostudy-theme-preview-light-sidebar)", "var(--ostudy-theme-preview-light-card)"],
    dark: ["var(--ostudy-theme-preview-dark-canvas)", "var(--ostudy-theme-preview-dark-sidebar)", "var(--ostudy-theme-preview-dark-card)"],
    console: ["var(--ostudy-theme-preview-console-canvas)", "var(--ostudy-theme-preview-console-chrome)", "var(--ostudy-theme-preview-console-card)"],
    synthwave: ["var(--ostudy-theme-preview-synthwave-canvas)", "var(--ostudy-theme-preview-synthwave-sidebar)", "var(--ostudy-theme-preview-synthwave-card)"],
  };
  const [canvas, sidebar, card] = colors[theme];
  return <span className="tw:grid tw:w-full tw:gap-2">
    <span aria-hidden="true" className="tw:flex tw:h-16 tw:w-full tw:overflow-hidden tw:rounded-lg tw:ring-1 tw:ring-border" style={{ background: canvas }}>
      <span className="tw:w-1/4" style={{ background: sidebar }} />
      <span className="tw:m-2.5 tw:flex-1 tw:rounded-md" style={{ background: card }} />
    </span>
    <strong className="tw:text-sm tw:font-medium">{names[theme]}</strong>
  </span>;
}

export function FontControl({ label, value, onChange }: {
  label: string; value: string; onChange: (value: string) => void;
}) {
  const isCustom = value.startsWith("custom:");
  const mode = isCustom ? "custom" : value;
  const custom = isCustom ? value.slice("custom:".length) : "";
  const bundled = FONT_PRESET_OPTIONS.filter((option) => option.group === "bundled");
  const system = FONT_PRESET_OPTIONS.filter((option) => option.group === "system");
  return <div className="tw:grid tw:gap-3">
    <FormField label={label}>
      <NativeSelect value={mode} onChange={(event) => onChange(event.currentTarget.value === "custom" ? "custom:Arial" : event.currentTarget.value)}>
        <optgroup label="Bundled fonts">{bundled.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</optgroup>
        <optgroup label="System and accessibility fonts">{system.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</optgroup>
        <option value="custom">Installed font…</option>
      </NativeSelect>
    </FormField>
    {mode === "custom" ? <FormField label="Font family name"><TextField value={custom} maxLength={200} onChange={(event) => onChange(`custom:${event.currentTarget.value}`)} placeholder="Atkinson Hyperlegible" /></FormField> : null}
  </div>;
}

export function SelectControl<T extends string>({ label, value, options, description, onChange }: {
  label: string; value: T; options: ReadonlyArray<{ value: T; label: string }>;
  description?: string; onChange: (value: T) => void;
}) {
  return <FormField label={label} description={description}>
    <NativeSelect value={value} onChange={(event) => {
      const option = options.find((item) => item.value === event.currentTarget.value);
      if (option) onChange(option.value);
    }}>{options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</NativeSelect>
  </FormField>;
}

export function RangeControl({ label, min, max, step, value, suffix, onChange }: {
  label: string; min: number; max: number; step: number; value: number; suffix: string;
  onChange: (value: number) => void;
}) {
  return <div className="tw:grid tw:gap-1.5">
    <div className="tw:flex tw:items-center tw:justify-between tw:gap-4 tw:text-sm">
      <span className="tw:font-medium">{label}</span>
      <output className="tw:rounded-md tw:bg-muted tw:px-2 tw:py-0.5 tw:text-xs tw:font-medium tw:tabular-nums">{value}{suffix}</output>
    </div>
    <Slider label={label} min={min} max={max} step={step} value={value} onValueChange={onChange} />
  </div>;
}

export function NumberControl({ label, value, onChange }: {
  label: string; value: number; onChange: (value: number) => void;
}) {
  return <FormField label={label}>
    <TextField type="number" min={0} max={10_000} step={1} value={value} onChange={(event) => {
      const parsed = Number(event.currentTarget.value);
      onChange(Number.isFinite(parsed) ? Math.max(0, Math.min(10_000, Math.trunc(parsed))) : 0);
    }} />
  </FormField>;
}

export function PathRow({ label, value }: { label: string; value: string }) {
  return <div className="tw:grid tw:min-w-0 tw:gap-0.5"><dt className="tw:text-xs tw:text-muted-foreground">{label}</dt><dd className="tw:m-0 tw:break-all tw:text-sm" title={value}>{value}</dd></div>;
}
