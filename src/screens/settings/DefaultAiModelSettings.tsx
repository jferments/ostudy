import type { AiProfilesController } from "../../controllers/aiProfilesController";
import type { AiReasoningEffort } from "../../ipc/aiProfiles";
import type { AppSettings } from "../../ipc/settings";
import styles from "./DefaultAiModelSettings.module.css";

interface DefaultAiModelSettingsProps {
  controller: AiProfilesController;
  settings: AppSettings;
  update: (patch: Partial<AppSettings>) => void;
}

const ALL_REASONING_EFFORTS: AiReasoningEffort[] = [
  "none",
  "minimal",
  "low",
  "medium",
  "high",
  "xhigh",
  "max",
];

export function DefaultAiModelSettings({
  controller,
  settings,
  update,
}: DefaultAiModelSettingsProps) {
  const defaultProfileId = controller.defaults.flashcardGenerationProfileId;
  const defaultProfile = controller.profiles.find((profile) => profile.id === defaultProfileId) ?? null;
  const selectedDefaultModel =
    controller.selectedProfileId === defaultProfileId
      ? controller.models.find((model) => model.id === defaultProfile?.modelId) ?? null
      : null;
  const reasoning = selectedDefaultModel?.reasoning ?? null;
  const advertisedEfforts = reasoning?.supportedEfforts ?? ALL_REASONING_EFFORTS;
  const allowedEfforts = advertisedEfforts.filter(
    (effort) => !(reasoning?.mandatory && effort === "none"),
  );
  const selectedEffort = settings.aiReasoningEffort ?? "low";
  const visibleEfforts = allowedEfforts.includes(selectedEffort as AiReasoningEffort)
    ? allowedEfforts
    : selectedEffort === "automatic"
      ? allowedEfforts
      : [selectedEffort as AiReasoningEffort, ...allowedEfforts];

  return (
    <section className={styles.card} aria-labelledby="default-ai-model-title">
      <h2 id="default-ai-model-title">Default AI model</h2>
      <label className={styles.field}>
        <span>Flashcard generation</span>
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
        This is the model ostudy uses for document flashcard generation. There is no automatic
        cross-model fallback.
      </p>

      <label className={`${styles.field} ${styles.reasoningField}`}>
        <span>Document reasoning effort</span>
        <select
          aria-label="Document reasoning effort"
          value={selectedEffort}
          onChange={(event) =>
            update({
              aiReasoningEffort: event.currentTarget.value as AppSettings["aiReasoningEffort"],
            })
          }
        >
          <option value="automatic">
            Automatic{reasoning?.defaultEffort ? ` — provider default ${reasoning.defaultEffort}` : ""}
          </option>
          {visibleEfforts.map((effort) => (
            <option key={effort} value={effort}>
              {reasoningLabel(effort)}
              {effort === "low" ? " — ostudy default" : ""}
            </option>
          ))}
        </select>
      </label>
      <p className={styles.note}>
        Low is the ostudy default for bounded textbook interpretation: it reduces latency and
        reasoning-token spend while strict schemas, source windows, and validation constrain the
        output. Choose Automatic to use the provider/model default instead.
      </p>
      {defaultProfile?.providerKind === "openrouter" ? (
        <p className={styles.capabilityNote}>
          {selectedDefaultModel?.contextWindowTokens
            ? `Selected model context: ${formatTokenCapacity(selectedDefaultModel.contextWindowTokens)}. `
            : "Refresh the selected OpenRouter profile's model list to show its advertised context size. "}
          {reasoning
            ? reasoning.mandatory
              ? `Reasoning is mandatory for this model. Advertised efforts: ${allowedEfforts.map(reasoningLabel).join(", ") || "provider-managed"}.`
              : `Advertised reasoning efforts: ${allowedEfforts.map(reasoningLabel).join(", ") || "provider-managed"}.`
            : "This model did not advertise adjustable reasoning metadata in the current catalog response."}
        </p>
      ) : null}

      <p className={styles.timeoutNote}>
        A profile&apos;s request timeout is an initial-response timeout for long AI jobs. Once the
        provider acknowledges the streamed request, generation can continue until it finishes or
        you stop it.
      </p>
    </section>
  );
}

function reasoningLabel(value: string): string {
  switch (value) {
    case "none":
      return "None";
    case "minimal":
      return "Minimal";
    case "low":
      return "Low";
    case "medium":
      return "Medium";
    case "high":
      return "High";
    case "xhigh":
      return "Extra high";
    case "max":
      return "Maximum";
    default:
      return value;
  }
}

function formatTokenCapacity(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M tokens`;
  if (value >= 1_000) return `${Math.round(value / 1_000)}k tokens`;
  return `${value.toLocaleString()} tokens`;
}