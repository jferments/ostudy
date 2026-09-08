import type { AiProfilesController } from "../../controllers/aiProfilesController";
import type { AiTaskKind } from "../../ipc/aiTaskDefaults";
import { Button } from "../../ui/primitives/Button";
import styles from "./DefaultAiModelSettings.module.css";

export function DefaultAiModelSettings({ controller }: { controller: AiProfilesController }) {
  const assignments: Array<{ task: AiTaskKind; label: string; value: string | null }> = [
    { task: "flashcard_generation", label: "Flashcard and study-guide generation", value: controller.defaults.flashcardGenerationProfileId },
    { task: "tutor", label: "Interactive tutor", value: controller.defaults.tutorProfileId ?? null },
    { task: "embeddings", label: "Semantic embeddings", value: controller.defaults.embeddingsProfileId ?? null },
  ];
  return (
    <section className={styles.card} aria-labelledby="default-ai-model-title">
      <h2 id="default-ai-model-title">Default AI model</h2>
      {assignments.map(({ task, label, value }) => (
        <label className={styles.field} key={task}>
          <span>{label}</span>
          <select value={value ?? ""} disabled={controller.mutating || Boolean(controller.pendingTaskDefault)}
            onChange={(event) => {
              const profileId = event.currentTarget.value || null;
              if (task === "flashcard_generation") void controller.setFlashcardDefault(profileId);
              else void controller.setTaskDefault(task, profileId);
            }}>
            <option value="">No default selected</option>
            {controller.profiles.filter((profile) => profile.configurationStatus !== "planned"
              && (task !== "embeddings" || ["llama_cpp", "openrouter"].includes(profile.providerKind)))
              .map((profile) => (
                <option key={profile.id} value={profile.id}>
                  {profile.displayName}{profile.modelId ? ` — ${profile.modelId}` : ""}
                  {profile.configurationStatus === "ready" ? "" : " — incomplete"}
                </option>
              ))}
          </select>
        </label>
      ))}
      {controller.pendingTaskDefault ? <p role="alert">
        The assignment outcome needs checking. <Button size="small" disabled={controller.mutating}
          onClick={() => { void controller.retryTaskDefault(); }}>Retry or check assignment</Button>
      </p> : null}
      <p className={styles.note}>
        Each profile stores its own reasoning preference, timeout, privacy policy, and credential
        state; changing a default does not copy settings between models. The generation profile
        also interprets source material during semantic preparation.
      </p>
      <p className={styles.note}>
        Use a separate embedding-model profile for semantic retrieval. No model is installed on
        this device. Changing the embedding model, dimensions, or input policy requires preparing
        the selection again; changing the Tutor model does not rebuild its index.
      </p>
      <p className={styles.timeoutNote}>
        A profile&apos;s request timeout limits initial response and stream inactivity, not the
        total duration of an actively responding generation request.
      </p>
    </section>
  );
}
