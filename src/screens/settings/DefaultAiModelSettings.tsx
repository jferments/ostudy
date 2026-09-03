import type { AiProfilesController } from "../../controllers/aiProfilesController";
import styles from "./DefaultAiModelSettings.module.css";

interface DefaultAiModelSettingsProps {
  controller: AiProfilesController;
}

export function DefaultAiModelSettings({ controller }: DefaultAiModelSettingsProps) {
  const defaultProfileId = controller.defaults.flashcardGenerationProfileId;

  return (
    <section className={styles.card} aria-labelledby="default-ai-model-title">
      <h2 id="default-ai-model-title">Default AI model</h2>
      <label className={styles.field}>
        <span>Flashcard and study-guide generation</span>
        <select
          value={defaultProfileId ?? ""}
          disabled={controller.mutating}
          onChange={(event) =>
            void controller.setFlashcardDefault(event.currentTarget.value || null)
          }
        >
          <option value="">No default selected</option>
          {controller.profiles
            .filter((profile) => profile.configurationStatus !== "planned")
            .map((profile) => (
              <option key={profile.id} value={profile.id}>
                {profile.displayName}
                {profile.modelId ? ` — ${profile.modelId}` : ""}
                {profile.configurationStatus === "ready" ? "" : " — incomplete"}
              </option>
            ))}
        </select>
      </label>
      <p className={styles.note}>
        This assignment selects the profile used for document generation. Each profile stores its
        own reasoning preference, timeout, privacy policy, and credential state; changing the
        default does not copy settings between models.
      </p>
      <p className={styles.timeoutNote}>
        A profile&apos;s request timeout governs the initial provider response. Once a streamed
        request is acknowledged, generation can continue until it finishes or you stop it.
      </p>
    </section>
  );
}
