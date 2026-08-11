import type { AppSettings } from "../../ipc/settings";
import { FontControl, RangeControl, ThemeOption } from "./SettingsControls";
import styles from "./Settings.module.css";

interface AppearanceSettingsProps {
  draft: AppSettings;
  update: (patch: Partial<AppSettings>) => void;
}

export function AppearanceSettings({ draft, update }: AppearanceSettingsProps) {
  return (
    <div className={styles.panelGrid}>
      <section className={styles.fullWidthCard}>
        <div className={styles.sectionHeading}>
          <div>
            <h2>Theme</h2>
            <p>Theme and typography changes preview immediately throughout ostudy.</p>
          </div>
        </div>
        <div className={styles.themeGrid} role="radiogroup" aria-label="Application theme">
          <ThemeOption name="Light" value="light" selected={draft.theme === "light"} onSelect={() => update({ theme: "light" })} />
          <ThemeOption name="Dark" value="dark" selected={draft.theme === "dark"} onSelect={() => update({ theme: "dark" })} />
          <ThemeOption name="Console" value="console" selected={draft.theme === "console"} onSelect={() => update({ theme: "console" })} />
          <ThemeOption name="Synthwave" value="synthwave" selected={draft.theme === "synthwave"} onSelect={() => update({ theme: "synthwave" })} />
        </div>
      </section>

      <section className={styles.settingsCard}>
        <h2>Typography</h2>
        <FontControl label="Interface font" value={draft.uiFont} onChange={(uiFont) => update({ uiFont })} />
        <FontControl label="Default card font" value={draft.cardFont} onChange={(cardFont) => update({ cardFont })} />
      </section>

      <section className={styles.settingsCard}>
        <h2>Scale and motion</h2>
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
        <label className={styles.switchRow}>
          <span>
            <strong>Animations and transitions</strong>
            <small>Enable card flips and other nonessential motion effects.</small>
          </span>
          <input
            type="checkbox"
            checked={draft.animationsEnabled}
            onChange={(event) => update({ animationsEnabled: event.currentTarget.checked })}
          />
        </label>
        <label className={styles.switchRow}>
          <span>
            <strong>Reduce motion</strong>
            <small>Minimize motion even when animations are enabled.</small>
          </span>
          <input
            type="checkbox"
            checked={draft.reduceMotion}
            onChange={(event) => update({ reduceMotion: event.currentTarget.checked })}
          />
        </label>
      </section>
    </div>
  );
}
