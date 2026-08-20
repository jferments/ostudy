import type {
  AppSettings,
  MaterialPreference,
  MotionPreference,
} from "../../ipc/settings";
import { HelpTrigger } from "../../ui/help/HelpProvider";
import { FontControl, RangeControl, SelectControl, ThemeOption } from "./SettingsControls";
import previewStyles from "./AppearancePreview.module.css";
import styles from "./Settings.module.css";

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

const MATERIAL_OPTIONS: ReadonlyArray<{ value: MaterialPreference; label: string }> = [
  { value: "automatic", label: "Automatic" },
  { value: "rich", label: "Rich materials" },
  { value: "simple", label: "Simple opaque materials" },
];

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
        <h2>Scale</h2>
        <RangeControl label="Interface scale" min={80} max={140} step={5} value={draft.uiScale} suffix="%" onChange={(uiScale) => update({ uiScale })} />
        <RangeControl label="Default card text" min={12} max={32} step={1} value={draft.cardBaseSize} suffix=" px" onChange={(cardBaseSize) => update({ cardBaseSize })} />
      </section>

      <section className={styles.settingsCard}>
        <div className={styles.sectionHeading}>
          <h2>Motion and materials</h2>
          <span>
            <HelpTrigger topicId="settings.motion" label="Help with motion preferences" />
            <HelpTrigger topicId="settings.materialEffects" label="Help with material effects" />
          </span>
        </div>
        <SelectControl
          label="Motion"
          value={draft.motionPreference ?? "system"}
          options={MOTION_OPTIONS}
          description="Reduced motion removes spatial transitions, card flips, parallax, reorder interpolation, and particle effects."
          onChange={(motionPreference) => update({ motionPreference })}
        />
        <SelectControl
          label="Material effects"
          value={draft.materialPreference ?? "automatic"}
          options={MATERIAL_OPTIONS}
          description="Automatic uses restrained depth and translucency while respecting operating-system transparency preferences."
          onChange={(materialPreference) => update({ materialPreference })}
        />
      </section>

      <section className={styles.settingsCard}>
        <h2>Artwork and guidance</h2>
        <label className={styles.switchRow}>
          <span>
            <strong>Decorative artwork</strong>
            <small>Show mascot art and illustrated empty states. Card and document images are unaffected.</small>
          </span>
          <input type="checkbox" checked={draft.decorativeArtwork ?? true} onChange={(event) => update({ decorativeArtwork: event.currentTarget.checked })} />
        </label>
        <label className={styles.switchRow}>
          <span>
            <strong>Celebrations</strong>
            <small>Allow one-shot completion effects when full motion is enabled.</small>
          </span>
          <input type="checkbox" checked={draft.celebrationsEnabled ?? true} onChange={(event) => update({ celebrationsEnabled: event.currentTarget.checked })} />
        </label>
        <label className={styles.switchRow}>
          <span>
            <strong>Interactive tutorials</strong>
            <small>Allow tutorials that you explicitly start from Help. They never open automatically.</small>
          </span>
          <input type="checkbox" checked={draft.tutorialsEnabled ?? false} onChange={(event) => update({ tutorialsEnabled: event.currentTarget.checked })} />
        </label>
      </section>

      <section className={styles.fullWidthCard} aria-labelledby="appearance-preview-heading">
        <div className={styles.sectionHeading}>
          <div>
            <h2 id="appearance-preview-heading">Live visual-system preview</h2>
            <p>Representative surfaces, typography, controls, progress, and metrics use the same semantic tokens as the application.</p>
          </div>
        </div>
        <AppearancePreview />
      </section>
    </div>
  );
}

function AppearancePreview() {
  return (
    <div className={previewStyles.preview}>
      <header className={previewStyles.chrome}>
        <span className={previewStyles.brand}>
          <span className={previewStyles.mark} aria-hidden="true">O</span>
          ostudy
        </span>
        <button type="button">Create card</button>
      </header>
      <div className={previewStyles.content}>
        <article className={previewStyles.card}>
          <span className={previewStyles.eyebrow}>Practice</span>
          <h3>How does spaced repetition improve recall?</h3>
          <p>It schedules review near the point where a memory is likely to become difficult to retrieve.</p>
          <div className={previewStyles.progress} aria-label="68 percent complete"><span /></div>
        </article>
        <div className={previewStyles.metrics}>
          <article className={previewStyles.metric}><small>Ready now</small><strong>24</strong></article>
          <article className={previewStyles.metric}><small>Studied this week</small><strong>138</strong></article>
        </div>
      </div>
    </div>
  );
}
