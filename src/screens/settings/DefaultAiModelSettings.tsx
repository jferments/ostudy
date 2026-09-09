import { useEffect, useMemo, useState } from "react";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import {
  listAiProviderModels,
  testAiProviderProfile,
  type AiProviderModel,
  type AiReasoningEffort,
} from "../../ipc/aiProfiles";
import type { AiTaskKind } from "../../ipc/aiTaskDefaults";
import {
  getEmbeddingOptions,
  listEmbeddingModels,
  saveEmbeddingOptions,
  testEmbeddingProfile,
  type EmbeddingInputPolicy,
  type EmbeddingOptions,
} from "../../ipc/embeddings";
import { getErrorMessage } from "../../ipc/common";
import { Button } from "../../ui/primitives/Button";
import styles from "./DefaultAiModelSettings.module.css";

interface Assignment {
  profileId: string | null;
  modelId: string | null;
  reasoningEffort: AiReasoningEffort;
}

interface ModelChoice {
  id: string;
  label: string;
  reasoning?: AiProviderModel["reasoning"];
}

const FALLBACK_REASONING: AiReasoningEffort[] = [
  "automatic",
  "none",
  "minimal",
  "low",
  "medium",
  "high",
  "xhigh",
  "max",
];

export function DefaultAiModelSettings({ controller }: { controller: AiProfilesController }) {
  return (
    <section className={styles.card} aria-labelledby="default-ai-model-title">
      <h2 id="default-ai-model-title">Default AI models</h2>
      <TaskModelRow
        controller={controller}
        task="flashcard_generation"
        label="Flashcard and study-guide generation"
        assignment={assignmentFor(controller, "flashcard_generation")}
      />
      <TaskModelRow
        controller={controller}
        task="tutor"
        label="Interactive tutor"
        assignment={assignmentFor(controller, "tutor")}
      />
      <TaskModelRow
        controller={controller}
        task="embeddings"
        label="Semantic embeddings"
        assignment={assignmentFor(controller, "embeddings")}
      />
      {controller.pendingTaskDefault ? (
        <p role="alert" className={styles.inlineStatus}>
          The assignment outcome needs checking.{" "}
          <Button
            size="small"
            disabled={controller.mutating}
            onClick={() => { void controller.retryTaskDefault(); }}
          >
            Retry or check assignment
          </Button>
        </p>
      ) : null}
    </section>
  );
}

function TaskModelRow({
  controller,
  task,
  label,
  assignment,
}: {
  controller: AiProfilesController;
  task: AiTaskKind;
  label: string;
  assignment: Assignment;
}) {
  const eligibleProfiles = useMemo(
    () => controller.profiles.filter((profile) =>
      profile.configurationStatus !== "planned"
      && (task !== "embeddings" || ["llama_cpp", "openrouter"].includes(profile.providerKind))),
    [controller.profiles, task],
  );
  const [profileId, setProfileId] = useState(assignment.profileId ?? "");
  const [modelId, setModelId] = useState(assignment.modelId ?? "");
  const [reasoning, setReasoning] = useState<AiReasoningEffort>(assignment.reasoningEffort);
  const [models, setModels] = useState<ModelChoice[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);
  const [testing, setTesting] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setProfileId(assignment.profileId ?? "");
    setModelId(assignment.modelId ?? "");
    setReasoning(assignment.reasoningEffort);
  }, [assignment.modelId, assignment.profileId, assignment.reasoningEffort]);

  useEffect(() => {
    let canceled = false;
    setModels([]);
    setStatus(null);
    setError(null);
    if (!profileId) return () => { canceled = true; };
    const profile = eligibleProfiles.find((candidate) => candidate.id === profileId);
    if (!profile || profile.configurationStatus !== "ready") return () => { canceled = true; };
    setLoadingModels(true);
    const load = task === "embeddings"
      ? listEmbeddingModels(profile.id).then((items) =>
          items.map((item) => ({ id: item.id, label: item.name || item.id })))
      : listAiProviderModels(profile.id, profile.revision).then((result) =>
          result.models.map((item) => ({
            id: item.id,
            label: item.displayName || item.id,
            reasoning: item.reasoning,
          })));
    void load.then((items) => {
      if (!canceled) setModels(items);
    }).catch((failure: unknown) => {
      if (!canceled) setError(getErrorMessage(failure));
    }).finally(() => {
      if (!canceled) setLoadingModels(false);
    });
    return () => { canceled = true; };
  }, [eligibleProfiles, profileId, task]);

  const selectedProfile = eligibleProfiles.find((profile) => profile.id === profileId) ?? null;
  const selectedModel = models.find((model) => model.id === modelId) ?? null;
  const reasoningOptions = reasoningChoices(selectedModel?.reasoning ?? null, reasoning);
  const locked = controller.mutating || Boolean(controller.pendingTaskDefault);

  async function assign(nextModel = modelId, nextReasoning = reasoning): Promise<void> {
    if (!profileId || !nextModel) return;
    setStatus(null);
    setError(null);
    await controller.setTaskDefault(
      task,
      profileId,
      nextModel,
      task === "embeddings" ? "none" : nextReasoning,
    );
  }

  async function testModel(): Promise<void> {
    if (!selectedProfile || !modelId || testing) return;
    setTesting(true);
    setStatus(null);
    setError(null);
    try {
      if (task === "embeddings") {
        const result = await testEmbeddingProfile(selectedProfile.id, modelId);
        setStatus(`Verified ${result.dimensions}-dimension embeddings.`);
      } else {
        const result = await testAiProviderProfile(
          selectedProfile.id,
          selectedProfile.revision,
          modelId,
          reasoning,
        );
        setStatus(
          result.overallStatus === "verified"
            ? "Model verified."
            : result.overallStatus === "partial"
              ? "Model verified with partial capability support."
              : "Model verification failed.",
        );
      }
    } catch (failure: unknown) {
      setError(getErrorMessage(failure));
    } finally {
      setTesting(false);
    }
  }

  return (
    <div className={styles.assignment} data-task={task}>
      <strong className={styles.assignmentTitle}>{label}</strong>
      <div className={styles.assignmentGrid}>
        <label className={styles.field}>
          <span>Provider</span>
          <select
            aria-label={`${label} provider`}
            value={profileId}
            disabled={locked}
            onChange={(event) => {
              const next = event.currentTarget.value;
              setProfileId(next);
              setModelId("");
              setReasoning("automatic");
              if (!next) void controller.setTaskDefault(task, null, null, null);
            }}
          >
            <option value="">No provider selected</option>
            {eligibleProfiles.map((profile) => (
              <option key={profile.id} value={profile.id}>
                {profile.displayName} — {profile.providerLabel}
                {profile.configurationStatus === "ready" ? "" : " — incomplete"}
              </option>
            ))}
          </select>
        </label>

        <label className={styles.field}>
          <span>Model</span>
          <select
            aria-label={`${label} model`}
            value={modelId}
            disabled={locked || !profileId || loadingModels || selectedProfile?.configurationStatus !== "ready"}
            onChange={(event) => {
              const nextModel = event.currentTarget.value;
              const model = models.find((candidate) => candidate.id === nextModel) ?? null;
              const nextReasoning = task === "embeddings"
                ? "none"
                : compatibleReasoning(reasoning, model?.reasoning ?? null);
              setModelId(nextModel);
              setReasoning(nextReasoning);
              if (nextModel) void assign(nextModel, nextReasoning);
            }}
          >
            <option value="">
              {!profileId
                ? "Select a provider first"
                : loadingModels
                  ? "Loading models…"
                  : models.length === 0
                    ? "No models reported"
                    : "Select a model"}
            </option>
            {modelId && !models.some((model) => model.id === modelId) ? (
              <option value={modelId}>{modelId} — saved selection</option>
            ) : null}
            {models.map((model) => (
              <option key={model.id} value={model.id}>{model.label}</option>
            ))}
          </select>
        </label>

        {task !== "embeddings" ? (
          <label className={styles.field}>
            <span>Reasoning</span>
            <select
              aria-label={`${label} reasoning`}
              value={reasoning}
              disabled={locked || !modelId}
              onChange={(event) => {
                const next = event.currentTarget.value as AiReasoningEffort;
                setReasoning(next);
                void assign(modelId, next);
              }}
            >
              {reasoningOptions.map((effort) => (
                <option key={effort} value={effort}>{reasoningLabel(effort)}</option>
              ))}
            </select>
          </label>
        ) : null}

        <div className={styles.testCell}>
          <Button
            size="small"
            variant="secondary"
            disabled={!modelId || testing || locked}
            onClick={() => { void testModel(); }}
          >
            {testing ? "Testing…" : "Test model"}
          </Button>
        </div>
      </div>

      {task === "embeddings" && profileId && modelId ? (
        <EmbeddingModelOptions profileId={profileId} modelId={modelId} disabled={locked || testing} />
      ) : null}
      {status ? <p className={styles.success} role="status">{status}</p> : null}
      {error ? <p className={styles.error} role="alert">{error}</p> : null}
    </div>
  );
}

function EmbeddingModelOptions({
  profileId,
  modelId,
  disabled,
}: {
  profileId: string;
  modelId: string;
  disabled: boolean;
}) {
  const [saved, setSaved] = useState<EmbeddingOptions | null>(null);
  const [dimensions, setDimensions] = useState("");
  const [policy, setPolicy] = useState<EmbeddingInputPolicy>("automatic");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let canceled = false;
    setSaved(null);
    setError(null);
    void getEmbeddingOptions(profileId, modelId)
      .then((value) => {
        if (canceled) return;
        setSaved(value);
        setDimensions(value.dimensions?.toString() ?? "");
        setPolicy(value.inputPolicy);
      })
      .catch((failure: unknown) => {
        if (!canceled) setError(getErrorMessage(failure));
      });
    return () => { canceled = true; };
  }, [modelId, profileId]);

  const parsedDimensions = dimensions.trim() === "" ? null : Number(dimensions);
  const validDimensions = parsedDimensions === null
    || (Number.isInteger(parsedDimensions) && parsedDimensions >= 32 && parsedDimensions <= 4096);
  const dirty = Boolean(saved)
    && (saved?.dimensions !== parsedDimensions || saved?.inputPolicy !== policy);

  async function save(): Promise<void> {
    if (!saved || !validDimensions || busy) return;
    setBusy(true);
    setError(null);
    try {
      const value = await saveEmbeddingOptions({
        profileId,
        modelId,
        expectedRevision: saved.revision,
        dimensions: parsedDimensions,
        inputPolicy: policy,
      });
      setSaved(value);
    } catch (failure: unknown) {
      setError(getErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }

  return (
    <details className={styles.embeddingOptions}>
      <summary>Embedding options</summary>
      <div className={styles.embeddingGrid}>
        <label className={styles.field}>
          <span>Dimensions</span>
          <input
            inputMode="numeric"
            placeholder="Provider default"
            value={dimensions}
            disabled={disabled || busy}
            onChange={(event) => setDimensions(event.currentTarget.value)}
          />
        </label>
        <label className={styles.field}>
          <span>Input policy</span>
          <select
            value={policy}
            disabled={disabled || busy}
            onChange={(event) => setPolicy(event.currentTarget.value as EmbeddingInputPolicy)}
          >
            <option value="automatic">Automatic</option>
            <option value="symmetric">Symmetric</option>
            <option value="qwen_retrieval">Qwen retrieval</option>
            <option value="search_types">Search query/document types</option>
          </select>
        </label>
        <div className={styles.testCell}>
          <Button
            size="small"
            variant="secondary"
            disabled={disabled || busy || !dirty || !validDimensions}
            onClick={() => { void save(); }}
          >
            {busy ? "Saving…" : "Save options"}
          </Button>
        </div>
      </div>
      {!validDimensions ? <p className={styles.error}>Use 32–4096 dimensions or leave blank.</p> : null}
      {error ? <p className={styles.error} role="alert">{error}</p> : null}
    </details>
  );
}

function assignmentFor(controller: AiProfilesController, task: AiTaskKind): Assignment {
  switch (task) {
    case "flashcard_generation":
      return {
        profileId: controller.defaults.flashcardGenerationProfileId,
        modelId: controller.defaults.flashcardGenerationModelId ?? null,
        reasoningEffort: controller.defaults.flashcardGenerationReasoningEffort ?? "automatic",
      };
    case "tutor":
      return {
        profileId: controller.defaults.tutorProfileId ?? null,
        modelId: controller.defaults.tutorModelId ?? null,
        reasoningEffort: controller.defaults.tutorReasoningEffort ?? "automatic",
      };
    case "embeddings":
      return {
        profileId: controller.defaults.embeddingsProfileId ?? null,
        modelId: controller.defaults.embeddingsModelId ?? null,
        reasoningEffort: "none",
      };
  }
}

function reasoningChoices(
  metadata: AiProviderModel["reasoning"] | null,
  current: AiReasoningEffort,
): AiReasoningEffort[] {
  const supported = metadata?.supportedEfforts;
  const choices: AiReasoningEffort[] = ["automatic"];
  if (!metadata?.mandatory) choices.push("none");
  if (supported) choices.push(...supported);
  else choices.push("minimal", "low", "medium", "high", "xhigh", "max");
  if (!choices.includes(current)) choices.push(current);
  return [...new Set(choices)];
}

function compatibleReasoning(
  current: AiReasoningEffort,
  metadata: AiProviderModel["reasoning"] | null,
): AiReasoningEffort {
  if (current === "automatic") return current;
  if (metadata?.mandatory && current === "none") return "automatic";
  if (metadata?.supportedEfforts && !metadata.supportedEfforts.includes(current as Exclude<AiReasoningEffort, "automatic">)) {
    return "automatic";
  }
  return current;
}

function reasoningLabel(value: AiReasoningEffort): string {
  if (value === "automatic") return "Automatic";
  if (value === "none") return "Off";
  return value === "xhigh" ? "Extra high" : `${value[0]?.toUpperCase() ?? ""}${value.slice(1)}`;
}
