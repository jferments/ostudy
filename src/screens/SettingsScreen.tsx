import { useEffect, useRef, useState } from "react";

import {
  getAppInfo,
  getErrorMessage,
  saveSettings,
  type AppInfo,
  type AppSettings,
} from "../ipc";

interface SettingsScreenProps {
  settings: AppSettings;
  onPreviewSettings: (settings: AppSettings) => void;
  onSavedSettings: (settings: AppSettings) => void;
  onDirtyChange: (dirty: boolean) => void;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

const DEFAULTS: AppSettings = {
  theme: "dark",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  reduceMotion: false,
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
  const [draft, setDraft] = useState(settings);
  const [savedBaseline, setSavedBaseline] = useState(settings);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const savingRef = useRef(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const dirty = JSON.stringify(draft) !== JSON.stringify(savedBaseline);

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

  useEffect(() => {
    void getAppInfo()
      .then(setAppInfo)
      .catch((error: unknown) => setErrorMessage(getErrorMessage(error)));
  }, []);

  function update(patch: Partial<AppSettings>): void {
    if (savingRef.current) return;
    setDraft((current) => ({ ...current, ...patch }));
  }

  async function persist(): Promise<void> {
    if (savingRef.current || !dirty) return;
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

  return (
    <div className="screen settings-screen" aria-busy={isSaving}>
      <header className="screen-header compact-screen-header">
        <div><h1>Settings</h1></div>
        <div className="header-actions">
          <button type="button" className="secondary-button" disabled={isSaving} onClick={() => setDraft(DEFAULTS)}>
            Restore defaults
          </button>
          <button type="button" className="primary-button" disabled={isSaving || !dirty} onClick={() => void persist()}>
            {isSaving ? "Saving…" : dirty ? "Save settings" : "Saved"}
          </button>
        </div>
      </header>

      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
      {dirty ? <div className="inline-alert info-alert">Previewing unsaved settings.</div> : null}

      <section className="settings-section">
        <div className="section-heading"><h2>Theme</h2></div>
        <div className="theme-grid" role="radiogroup" aria-label="Application theme">
          <ThemeOption name="Light" value="light" selected={draft.theme === "light"} onSelect={() => update({ theme: "light" })} />
          <ThemeOption name="Dark" value="dark" selected={draft.theme === "dark"} onSelect={() => update({ theme: "dark" })} />
          <ThemeOption name="Console" value="console" selected={draft.theme === "console"} onSelect={() => update({ theme: "console" })} />
          <ThemeOption name="Synthwave" value="synthwave" selected={draft.theme === "synthwave"} onSelect={() => update({ theme: "synthwave" })} />
        </div>
      </section>

      <div className="settings-two-column">
        <section className="settings-card">
          <h2>Appearance</h2>
          <FontControl label="Interface font" value={draft.uiFont} onChange={(uiFont) => update({ uiFont })} />
          <FontControl label="Default card font" value={draft.cardFont} onChange={(cardFont) => update({ cardFont })} />
          <RangeControl label="Interface scale" min={80} max={140} step={5} value={draft.uiScale} suffix="%" onChange={(uiScale) => update({ uiScale })} />
          <RangeControl label="Default card text" min={12} max={32} step={1} value={draft.cardBaseSize} suffix=" px" onChange={(cardBaseSize) => update({ cardBaseSize })} />
          <label className="switch-row">
            <span><strong>Reduce motion</strong></span>
            <input type="checkbox" checked={draft.reduceMotion} onChange={(event) => update({ reduceMotion: event.currentTarget.checked })} />
          </label>
        </section>

        <section className="settings-card">
          <h2>Reviews</h2>
          <RangeControl
            label="Desired retention"
            min={80}
            max={97}
            step={1}
            value={Math.round(draft.desiredRetention * 100)}
            suffix="%"
            onChange={(value) => update({ desiredRetention: value / 100 })}
          />
          <div className="daily-limit-grid">
            <NumberControl
              label="New cards per day"
              value={draft.dailyNewLimit}
              onChange={(dailyNewLimit) => update({ dailyNewLimit })}
            />
            <NumberControl
              label="Reviews per day"
              value={draft.dailyReviewLimit}
              onChange={(dailyReviewLimit) => update({ dailyReviewLimit })}
            />
          </div>
          <p className="settings-note">
            Limits bound normal scheduled sessions. A value of 0 pauses that category; explicit extra and filtered sessions remain available.
          </p>
          <p className="settings-note">
            Session-specific options, including shuffling, are chosen on the Practice screen.
          </p>
        </section>

        <section className="settings-card">
          <h2>Confirmations</h2>
          <label className="switch-row">
            <span><strong>Confirm deletions</strong></span>
            <input type="checkbox" checked={draft.confirmDeletions} onChange={(event) => update({ confirmDeletions: event.currentTarget.checked })} />
          </label>
        </section>

        <section className="settings-card">
          <h2>Storage</h2>
          {appInfo ? (
            <dl className="path-list compact-path-list">
              <PathRow label="Application data" value={appInfo.dataDirectory} />
              <PathRow label="Database" value={appInfo.databasePath} />
              <PathRow label="Images" value={appInfo.imageDirectory} />
              <PathRow label="Version" value={appInfo.appVersion} />
              <PathRow label="Schema" value={String(appInfo.databaseSchemaVersion)} />
            </dl>
          ) : (
            <div className="inline-progress">Reading storage paths…</div>
          )}
        </section>
      </div>
    </div>
  );
}

interface ThemeOptionProps {
  name: string;
  value: AppSettings["theme"];
  selected: boolean;
  onSelect: () => void;
}

function ThemeOption({ name, value, selected, onSelect }: ThemeOptionProps) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      className={selected ? `theme-option theme-${value} selected` : `theme-option theme-${value}`}
      onClick={onSelect}
    >
      <span className="theme-preview"><i /><b /><em /></span>
      <span><strong>{name}</strong>{selected ? <small>Selected</small> : null}</span>
    </button>
  );
}

function FontControl({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) {
  const isCustom = value.startsWith("custom:");
  const mode = isCustom ? "custom" : value;
  const custom = isCustom ? value.slice("custom:".length) : "";
  return (
    <div className="setting-control font-control">
      <label>
        <span>{label}</span>
        <select value={mode} onChange={(event) => onChange(event.currentTarget.value === "custom" ? "custom:Arial" : event.currentTarget.value)}>
          <option value="system">System sans-serif</option>
          <option value="humanist">Humanist sans-serif</option>
          <option value="serif">Serif</option>
          <option value="mono">Monospace</option>
          <option value="custom">Installed font…</option>
        </select>
      </label>
      {mode === "custom" ? (
        <label>
          <span>Font family name</span>
          <input value={custom} maxLength={200} onChange={(event) => onChange(`custom:${event.currentTarget.value}`)} placeholder="Atkinson Hyperlegible" />
        </label>
      ) : null}
    </div>
  );
}

function RangeControl({ label, min, max, step, value, suffix, onChange }: {
  label: string; min: number; max: number; step: number; value: number; suffix: string; onChange: (value: number) => void;
}) {
  return (
    <label className="range-control">
      <span><strong>{label}</strong></span>
      <div>
        <input type="range" min={min} max={max} step={step} value={value} onChange={(event) => onChange(Number(event.currentTarget.value))} />
        <output>{value}{suffix}</output>
      </div>
    </label>
  );
}

function NumberControl({ label, value, onChange }: { label: string; value: number; onChange: (value: number) => void }) {
  return (
    <label className="number-control">
      <span><strong>{label}</strong></span>
      <input
        type="number"
        min={0}
        max={10_000}
        step={1}
        value={value}
        onChange={(event) => {
          const parsed = Number(event.currentTarget.value);
          onChange(Number.isFinite(parsed) ? Math.max(0, Math.min(10_000, Math.trunc(parsed))) : 0);
        }}
      />
    </label>
  );
}

function PathRow({ label, value }: { label: string; value: string }) {
  return <div><dt>{label}</dt><dd title={value}>{value}</dd></div>;
}
