import { Panel } from "../../ui/primitives/Panel";
import { Disclosure } from "../../ui/primitives/FormControls";
import { useEffect, useMemo, useState } from "react";

import {
  defaultTaskReasoning,
  type AiProfilesController,
} from "../../controllers/aiProfilesController";
import {
  listAiProviderModels,
  testAiProviderProfile,
  type AiProviderModel,
  type AiProviderProbeResult,
  type AiReasoningEffort,
} from "../../ipc/aiProfiles";
import { getErrorMessage } from "../../ipc/common";
import type { AiTaskKind } from "../../ipc/aiTaskDefaults";
import {
  getEmbeddingOptions,
  listEmbeddingModels,
  saveEmbeddingOptions,
  testEmbeddingProfile,
  type EmbeddingInputPolicy,
  type EmbeddingOptions,
} from "../../ipc/embeddings";
import { Icon } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { TextField } from "../../ui/primitives/TextField";
import {
  compatibleReasoning,
  reasoningChoices,
  reasoningLabel,
  reasoningValidationMessage,
} from "./reasoningOptions";

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

export function DefaultAiModelSettings({ controller }: { controller: AiProfilesController }) {
  return (
    <Panel title="Default AI models" description="Choose separate models for generation, tutoring, and semantic search.">
      <div className="tw:grid tw:min-w-0 tw:gap-4 tw:xl:grid-cols-3">
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
      </div>
      {controller.pendingTaskDefault ? (
        <p role="alert" className="tw:m-0 tw:text-label tw:font-normal tw:leading-relaxed tw:text-muted-foreground">
          The assignment outcome needs checking.{" "}
          <Button
            size="small"
            disabled={controller.mutating}
            onClick={() => {
              void controller.retryTaskDefault();
            }}
          >
            Retry or check assignment
          </Button>
        </p>
      ) : null}
    </Panel>
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
    () =>
      controller.profiles.filter(
        (profile) =>
          profile.configurationStatus !== "planned" &&
          (task !== "embeddings" || ["llama_cpp", "openrouter"].includes(profile.providerKind)),
      ),
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
    if (!profileId)
      return () => {
        canceled = true;
      };
    const profile = eligibleProfiles.find((candidate) => candidate.id === profileId);
    if (!profile || profile.configurationStatus !== "ready")
      return () => {
        canceled = true;
      };
    setLoadingModels(true);
    const load =
      task === "embeddings"
        ? listEmbeddingModels(profile.id).then((items) =>
            items.map((item) => ({ id: item.id, label: item.name || item.id })),
          )
        : listAiProviderModels(profile.id, profile.revision).then((result) =>
            result.models.map((item) => ({
              id: item.id,
              label: item.displayName || item.id,
              reasoning: item.reasoning,
            })),
          );
    void load
      .then((items) => {
        if (!canceled) setModels(items);
      })
      .catch((failure: unknown) => {
        if (!canceled) setError(getErrorMessage(failure));
      })
      .finally(() => {
        if (!canceled) setLoadingModels(false);
      });
    return () => {
      canceled = true;
    };
  }, [eligibleProfiles, profileId, task]);

  const selectedProfile = eligibleProfiles.find((profile) => profile.id === profileId) ?? null;
  const selectedModel = models.find((model) => model.id === modelId) ?? null;
  const reasoningOptions = reasoningChoices(selectedModel?.reasoning ?? null);
  const reasoningValidationError =
    task === "embeddings" || !selectedModel
      ? null
      : reasoningValidationMessage(
          reasoning,
          selectedModel.reasoning ?? null,
          selectedModel.label,
        );
  const locked = controller.mutating || Boolean(controller.pendingTaskDefault);

  async function assign(nextModel = modelId, nextReasoning = reasoning): Promise<void> {
    if (!profileId || !nextModel) return;
    const nextSelectedModel = models.find((model) => model.id === nextModel) ?? null;
    const nextReasoningError =
      task === "embeddings" || !nextSelectedModel
        ? null
        : reasoningValidationMessage(
            nextReasoning,
            nextSelectedModel.reasoning ?? null,
            nextSelectedModel.label,
          );
    setStatus(null);
    setError(nextReasoningError);
    if (nextReasoningError) return;
    await controller.setTaskDefault(
      task,
      profileId,
      nextModel,
      task === "embeddings" ? "none" : nextReasoning,
    );
  }

  async function testModel(): Promise<void> {
    if (!selectedProfile || !modelId || testing || reasoningValidationError) return;
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
        const probeError = modelProbeError(result, task);
        if (probeError) {
          setError(probeError);
        } else if (task === "flashcard_generation") {
          setStatus(
            result.overallStatus === "partial"
              ? "Model verified for document learning tools with partial optional capability support."
              : "Model verified for document learning tools.",
          );
        } else {
          setStatus(
            result.overallStatus === "partial"
              ? "Model verified with partial optional capability support."
              : "Model verified.",
          );
        }
      }
    } catch (failure: unknown) {
      setError(getErrorMessage(failure));
    } finally {
      setTesting(false);
    }
  }

  return (
    <div className="tw:grid tw:min-w-0 tw:content-start tw:gap-4 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-background tw:p-5" data-task={task}>
      <strong className="tw:text-body tw:font-semibold tw:tracking-tight">{label}</strong>
      <div className="tw:grid tw:min-w-0 tw:gap-4">
        <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground">
          <span>Provider</span>
          <NativeSelect
            aria-label={`${label} provider`}
            value={profileId}
            disabled={locked}
            onChange={(event) => {
              const next = event.currentTarget.value;
              setProfileId(next);
              setModelId("");
              setReasoning(defaultTaskReasoning(task));
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
          </NativeSelect>
        </label>

        <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground">
          <span>Model</span>
          <NativeSelect
            aria-label={`${label} model`}
            value={modelId}
            disabled={
              locked ||
              !profileId ||
              loadingModels ||
              selectedProfile?.configurationStatus !== "ready"
            }
            onChange={(event) => {
              const nextModel = event.currentTarget.value;
              const model = models.find((candidate) => candidate.id === nextModel) ?? null;
              const nextReasoning =
                task === "embeddings"
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
              <option key={model.id} value={model.id}>
                {model.label}
              </option>
            ))}
          </NativeSelect>
        </label>

        {task !== "embeddings" ? (
          <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground">
            <span>Reasoning</span>
            <NativeSelect
              aria-label={`${label} reasoning`}
              value={reasoning}
              disabled={locked || !modelId}
              onChange={(event) => {
                const next = event.currentTarget.value as AiReasoningEffort;
                setReasoning(next);
                void assign(modelId, next);
              }}
            >
              {reasoningValidationError ? (
                <option value={reasoning} disabled>
                  {reasoningLabel(reasoning)} — unsupported saved selection
                </option>
              ) : null}
              {reasoningOptions.map((effort) => (
                <option key={effort} value={effort}>
                  {reasoningLabel(effort)}
                </option>
              ))}
            </NativeSelect>
          </label>
        ) : null}

        <div className="tw:flex tw:items-center tw:gap-2">
          <Button
            size="small"
            variant="secondary"
            disabled={!modelId || testing || locked || Boolean(reasoningValidationError)}
            onClick={() => {
              void testModel();
            }}
          >
            {testing ? "Testing…" : "Test model"}
          </Button>
          {status ? (
            <Icon
              name="check"
              size={20}
              label="Model test succeeded"
              className="tw:size-5 tw:shrink-0 tw:text-success"
            />
          ) : null}
        </div>
      </div>

      {task === "embeddings" && profileId && modelId ? (
        <EmbeddingModelOptions profileId={profileId} modelId={modelId} disabled={locked || testing} />
      ) : null}
      {reasoningValidationError ? (
        <p className="tw:m-0 tw:text-label tw:leading-relaxed tw:text-destructive" role="alert">
          {reasoningValidationError}
        </p>
      ) : null}
      {status ? (
        <p className="tw:m-0 tw:text-label tw:font-normal tw:leading-relaxed tw:text-muted-foreground" role="status">
          {status}
        </p>
      ) : null}
      {error ? (
        <p className="tw:m-0 tw:text-label tw:leading-relaxed tw:text-destructive" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}

function modelProbeError(result: AiProviderProbeResult, task: AiTaskKind): string | null {
  if (!result.persisted || result.stale) {
    return "Model verification completed, but the result is not current for this provider connection. Test the model again before using it.";
  }
  if (result.overallStatus === "failed") {
    return probeFailureMessage(result, "Model verification failed.");
  }
  if (task === "flashcard_generation") {
    const schemaCapability = result.capabilities.find(
      (capability) => capability.key === "document_generation_schema",
    );
    if (schemaCapability?.status !== "supported") {
      return probeFailureMessage(
        result,
        "The model connected, but it did not pass ostudy's document-generation structured-output contract.",
        "document_generation_schema",
      );
    }
  }
  return null;
}

function probeFailureMessage(
  result: AiProviderProbeResult,
  fallback: string,
  preferredPhase?: string,
): string {
  const phase =
    (preferredPhase
      ? result.phases.find((candidate) => candidate.phase === preferredPhase && candidate.status === "failed")
      : null) ?? result.phases.find((candidate) => candidate.status === "failed");
  if (!phase) return fallback;
  const detail = phase.error?.message || phase.message;
  return detail ? `${fallback} ${phase.label}: ${detail}` : fallback;
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
    return () => {
      canceled = true;
    };
  }, [modelId, profileId]);

  const parsedDimensions = dimensions.trim() === "" ? null : Number(dimensions);
  const validDimensions =
    parsedDimensions === null ||
    (Number.isInteger(parsedDimensions) && parsedDimensions >= 32 && parsedDimensions <= 4096);
  const dirty =
    Boolean(saved) && (saved?.dimensions !== parsedDimensions || saved?.inputPolicy !== policy);

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
    <Disclosure title="Embedding options" className="tw:mt-1">
      <div className="tw:mt-4 tw:grid tw:min-w-0 tw:gap-4">
        <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground">
          <span>Dimensions</span>
          <TextField
            inputMode="numeric"
            placeholder="Provider default"
            value={dimensions}
            disabled={disabled || busy}
            onChange={(event) => setDimensions(event.currentTarget.value)}
          />
        </label>
        <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground">
          <span>Input policy</span>
          <NativeSelect
            value={policy}
            disabled={disabled || busy}
            onChange={(event) => setPolicy(event.currentTarget.value as EmbeddingInputPolicy)}
          >
            <option value="automatic">Automatic</option>
            <option value="symmetric">Symmetric</option>
            <option value="qwen_retrieval">Qwen retrieval</option>
            <option value="search_types">Search query/document types</option>
          </NativeSelect>
        </label>
        <div className="tw:flex tw:items-center tw:gap-2">
          <Button
            size="small"
            variant="secondary"
            disabled={disabled || busy || !dirty || !validDimensions}
            onClick={() => {
              void save();
            }}
          >
            {busy ? "Saving…" : "Save options"}
          </Button>
        </div>
      </div>
      {!validDimensions ? <p className="tw:m-0 tw:text-label tw:leading-relaxed tw:text-destructive">Use 32–4096 dimensions or leave blank.</p> : null}
      {error ? (
        <p className="tw:m-0 tw:text-label tw:leading-relaxed tw:text-destructive" role="alert">
          {error}
        </p>
      ) : null}
    </Disclosure>
  );
}

function assignmentFor(controller: AiProfilesController, task: AiTaskKind): Assignment {
  switch (task) {
    case "flashcard_generation":
      return {
        profileId: controller.defaults.flashcardGenerationProfileId,
        modelId: controller.defaults.flashcardGenerationModelId ?? null,
        reasoningEffort:
          controller.defaults.flashcardGenerationReasoningEffort ?? defaultTaskReasoning(task),
      };
    case "tutor":
      return {
        profileId: controller.defaults.tutorProfileId ?? null,
        modelId: controller.defaults.tutorModelId ?? null,
        reasoningEffort: controller.defaults.tutorReasoningEffort ?? defaultTaskReasoning(task),
      };
    case "embeddings":
      return {
        profileId: controller.defaults.embeddingsProfileId ?? null,
        modelId: controller.defaults.embeddingsModelId ?? null,
        reasoningEffort: defaultTaskReasoning(task),
      };
  }
}
