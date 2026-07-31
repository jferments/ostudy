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
  newCardsPerDay: 20,
  reviewsPerDay: 200,
  strugglingCardThreshold: 8,
  autoSuspendStrugglingCards: false,
  buryRelatedCards: true,
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
        <div>
          <h1>Settings</h1>
        </div>
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
        <div className="section-heading">
          <h2>Theme</h2>
        </div>
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
          <RangeControl
            label="Interface scale"
            min={80}
            max={140}
            step={5}
            value={draft.uiScale}
            suffix="%"
            onChange={(uiScale) => update({ uiScale })}
          />
          <RangeControl
            label="Default card text"
            min={12}
            max={32}
            step={1}
            value={draft.cardBaseSize}
            suffix=" px"
            onChange={(cardBaseSize) => update({ cardBaseSize })}
          />
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
          <NumberControl
            label="New cards per day"
            description="Maximum unseen cards added to a practice day. Enter 0 to pause new cards."
            min={0}
            max={999}
            value={draft.newCardsPerDay}
            onChange={(newCardsPerDay) => update({ newCardsPerDay })}
          />
          <NumberControl
            label="Reviews per day"
            description="Maximum previously reviewed cards in a practice day. Enter 0 to pause reviews."
            min={0}
            max={5000}
            value={draft.reviewsPerDay}
            onChange={(reviewsPerDay) => update({ reviewsPerDay })}
          />
          <NumberControl
            label="Mark as struggling after"
            description="Number of wrong answers before a card appears in the Struggling filter."
            min={2}
            max={50}
            value={draft.strugglingCardThreshold}
            suffix=" wrong answers"
            onChange={(strugglingCardThreshold) => update({ strugglingCardThreshold })}
          />
          <label className="switch-row">
            <span><strong>Automatically suspend struggling cards</strong><small>Pause a card when it crosses the wrong-answer threshold.</small></span>
            <input type="checkbox" checked={draft.autoSuspendStrugglingCards} onChange={(event) => update({ autoSuspendStrugglingCards: event.currentTarget.checked })} />
          </label>
          <label className="switch-row">
            <span><strong>Bury related cards</strong><small>Hide other cards generated from the same note until the next local day.</small></span>
            <input type="checkbox" checked={draft.buryRelatedCards} onChange={(event) => update({ buryRelatedCards: event.currentTarget.checked })} />
          </label>
          <label className="switch-row">
            <span><strong>Shuffle cards</strong></span>
            <input type="checkbox" checked={draft.practiceShuffle} onChange={(event) => update({ practiceShuffle: event.currentTarget.checked })} />
          </label>
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

interface FontControlProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
}

function FontControl({ label, value, onChange }: FontControlProps) {
  const isCustom = value.startsWith("custom:");
  const mode = isCustom ? "custom" : value;
  const custom = isCustom ? value.slice("custom:".length) : "";

  return (
    <div className="setting-control font-control">
      <label>
        <span>{label}</span>
        <select
          value={mode}
          onChange={(event) => {
            const next = event.currentTarget.value;
            onChange(next === "custom" ? "custom:Arial" : next);
          }}
        >
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

interface RangeControlProps {
  label: string;
  min: number;
  max: number;
  step: number;
  value: number;
  suffix: string;
  onChange: (value: number) => void;
}

function RangeControl({ label, min, max, step, value, suffix, onChange }: RangeControlProps) {
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

interface NumberControlProps {
  label: string;
  description: string;
  min: number;
  max: number;
  value: number;
  suffix?: string;
  onChange: (value: number) => void;
}

function NumberControl({ label, description, min, max, value, suffix = "", onChange }: NumberControlProps) {
  return (
    <label className="number-control">
      <span><strong>{label}</strong><small>{description}</small></span>
      <span className="number-control-input">
        <input
          type="number"
          min={min}
          max={max}
          value={value}
          onChange={(event) => {
            const parsed = Number(event.currentTarget.value);
            if (Number.isFinite(parsed)) onChange(Math.min(max, Math.max(min, Math.round(parsed))));
          }}
        />
        {suffix ? <small>{suffix}</small> : null}
      </span>
    </label>
  );
}

function PathRow({ label, value }: { label: string; value: string }) {
  return <div><dt>{label}</dt><dd title={value}>{value}</dd></div>;
}
