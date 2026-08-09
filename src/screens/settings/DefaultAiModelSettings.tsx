import type { AiProfilesController } from "../../controllers/aiProfilesController";
import styles from "./DefaultAiModelSettings.module.css";

interface DefaultAiModelSettingsProps {
  controller: AiProfilesController;
}

export function DefaultAiModelSettings({ controller }: DefaultAiModelSettingsProps) {
  return (
    <section className={styles.card} aria-labelledby="default-ai-model-title">
      <h2 id="default-ai-model-title">Default AI model</h2>
      <label className={styles.field}>
        <span>Flashcard generation</span>
        <select
          value={controller.defaults.flashcardGenerationProfileId ?? ""}
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
        This is the model ostudy uses for PDF flashcard generation. There is no automatic provider
        fallback.
      </p>
      <p className={styles.timeoutNote}>
        A profile&apos;s request timeout is an initial-response timeout for long AI jobs. Once the
        provider acknowledges the streamed request, generation can continue until it finishes or
        you stop it.
      </p>
    </section>
  );
}
