import type { AppSettings } from "../../ipc/settings";
import styles from "./Settings.module.css";

interface ThemeOptionProps {
  name: string;
  value: AppSettings["theme"];
  selected: boolean;
  onSelect: () => void;
}

export function ThemeOption({ name, value, selected, onSelect }: ThemeOptionProps) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      className={styles.themeOption}
      data-theme-option={value}
      data-selected={selected}
      onClick={onSelect}
    >
      <span className={styles.themePreview} aria-hidden="true">
        <i />
        <b />
        <em />
      </span>
      <span>
        <strong>{name}</strong>
        {selected ? <small>Selected</small> : null}
      </span>
    </button>
  );
}

export function FontControl({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  const isCustom = value.startsWith("custom:");
  const mode = isCustom ? "custom" : value;
  const custom = isCustom ? value.slice("custom:".length) : "";
  return (
    <div className={styles.settingControl}>
      <label>
        <span>{label}</span>
        <select
          value={mode}
          onChange={(event) =>
            onChange(
              event.currentTarget.value === "custom"
                ? "custom:Arial"
                : event.currentTarget.value,
            )
          }
        >
          <option value="system">System sans-serif</option>
          <option value="atkinson">Atkinson Hyperlegible</option>
          <option value="plex">IBM Plex Sans</option>
          <option value="humanist">Humanist sans-serif</option>
          <option value="serif">Serif</option>
          <option value="mono">Monospace</option>
          <option value="custom">Installed font…</option>
        </select>
      </label>
      {mode === "custom" ? (
        <label>
          <span>Font family name</span>
          <input
            value={custom}
            maxLength={200}
            onChange={(event) => onChange(`custom:${event.currentTarget.value}`)}
            placeholder="Atkinson Hyperlegible"
          />
        </label>
      ) : null}
    </div>
  );
}

export function SelectControl<T extends string>({
  label,
  value,
  options,
  description,
  onChange,
}: {
  label: string;
  value: T;
  options: ReadonlyArray<{ value: T; label: string }>;
  description?: string;
  onChange: (value: T) => void;
}) {
  return (
    <label className={styles.stackedField}>
      <span>{label}</span>
      <select value={value} onChange={(event) => onChange(event.currentTarget.value as T)}>
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
      {description ? <small className={styles.fieldHelp}>{description}</small> : null}
    </label>
  );
}

export function RangeControl({
  label,
  min,
  max,
  step,
  value,
  suffix,
  onChange,
}: {
  label: string;
  min: number;
  max: number;
  step: number;
  value: number;
  suffix: string;
  onChange: (value: number) => void;
}) {
  return (
    <label className={styles.rangeControl}>
      <span>
        <strong>{label}</strong>
      </span>
      <div>
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(event) => onChange(Number(event.currentTarget.value))}
        />
        <output>
          {value}
          {suffix}
        </output>
      </div>
    </label>
  );
}

export function NumberControl({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className={styles.numberControl}>
      <span>
        <strong>{label}</strong>
      </span>
      <input
        type="number"
        min={0}
        max={10_000}
        step={1}
        value={value}
        onChange={(event) => {
          const parsed = Number(event.currentTarget.value);
          onChange(
            Number.isFinite(parsed) ? Math.max(0, Math.min(10_000, Math.trunc(parsed))) : 0,
          );
        }}
      />
    </label>
  );
}

export function PathRow({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd title={value}>{value}</dd>
    </div>
  );
}
