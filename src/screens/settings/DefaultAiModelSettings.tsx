import { Panel } from "../../ui/primitives/Panel";
import { useEffect, useMemo, useState, type ReactNode } from "react";

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
import { cn } from "../../ui/internal/cn";
import { useLayoutMode } from "../../ui/layout/LayoutModeProvider";
import { Sheet } from "../../ui/overlays/Dialog";
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
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

const TASKS: ReadonlyArray<{ task: AiTaskKind; label: string }> = [
  { task: "flashcard_generation", label: "Cards and guides" },
  { task: "tutor", label: "Tutor" },
  { task: "embeddings", label: "Search" },
];

/** One row per task: the service, model and, for the Tutor, reasoning it uses. */
export function DefaultAiModelSettings({
  controller,
  extraRows,
  onAddOpenRouter,
}: {
  controller: AiProfilesController;
  extraRows?: ReactNode;
  /** Offered on Search when no connected service can do it. */
  onAddOpenRouter?: () => void;
}) {
  const wide = useLayoutMode().layoutMode === "wide";
  return (
    <Panel title="Default for each task">
      <div role="list" aria-label="Default for each task" className="tw:grid tw:min-w-0">
        {wide ? (
          <div aria-hidden="true" className={cn(TASK_GRID, "tw:pb-1 tw:text-caption tw:text-muted-foreground")}>
            <span /><span>Service</span><span>Model</span><span>Reasoning</span><span />
          </div>
        ) : null}
        {TASKS.map(({ task, label }) => (
          <TaskModelRow key={task} controller={controller} task={task} label={label} wide={wide}
            assignment={assignmentFor(controller, task)} onAddOpenRouter={onAddOpenRouter} />
        ))}
        {extraRows}
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

/** Wide screens line the tasks up as a table: name, service, model, reasoning, menu. */
export const TASK_GRID = "tw:grid tw:grid-cols-[9rem_minmax(0,1fr)_minmax(0,1.2fr)_8.5rem_2.75rem] tw:items-center tw:gap-3";

function TaskModelRow({
  controller,
  task,
  label,
  assignment,
  wide,
  onAddOpenRouter,
}: {
  controller: AiProfilesController;
  task: AiTaskKind;
  label: string;
  assignment: Assignment;
  wide: boolean;
  onAddOpenRouter?: () => void;
}) {
  const [optionsOpen, setOptionsOpen] = useState(false);
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
  // Only the Tutor reasons; document work always asks for no reasoning.
  const reasons = task === "tutor";
  const reasoningValidationError =
    !reasons || !selectedModel
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
      !reasons || !nextSelectedModel
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
      reasons ? nextReasoning : "none",
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
          reasons ? reasoning : "none",
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

  const serviceSelect = (
    <NativeSelect
      aria-label={`${label} service`}
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
      <option value="">None</option>
      {eligibleProfiles.map((profile) => (
        <option key={profile.id} value={profile.id}>
          {profile.displayName}
          {profile.configurationStatus === "ready" ? "" : " (needs setup)"}
        </option>
      ))}
    </NativeSelect>
  );
  const modelSelect = (
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
          ? "Choose a service first"
          : loadingModels
            ? "Loading models…"
            : models.length === 0
              ? "No models reported"
              : "Choose a model"}
      </option>
      {modelId && !models.some((model) => model.id === modelId) ? (
        <option value={modelId}>{modelId} (saved)</option>
      ) : null}
      {models.map((model) => (
        <option key={model.id} value={model.id}>
          {model.label}
        </option>
      ))}
    </NativeSelect>
  );
  const reasoningSelect = reasons ? (
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
          {reasoningLabel(reasoning)} (not supported)
        </option>
      ) : null}
      {reasoningOptions.map((effort) => (
        <option key={effort} value={effort}>
          {reasoningLabel(effort)}
        </option>
      ))}
    </NativeSelect>
  ) : null;
  const menu = (
    <OverflowMenu label={`More for ${label}`} disabled={locked}
      items={[
        { id: "test", label: testing ? "Testing…" : "Test model", disabled: !modelId || testing || Boolean(reasoningValidationError), onSelect: () => void testModel() },
        ...(task === "embeddings" && profileId && modelId ? [{ id: "options", label: "Embedding options", onSelect: () => setOptionsOpen(true) }] : []),
        { id: "clear", label: "Clear", disabled: !profileId, onSelect: () => {
          setProfileId("");
          setModelId("");
          setReasoning(defaultTaskReasoning(task));
          void controller.setTaskDefault(task, null, null, null);
        } },
      ]} />
  );
  const tested = status ? <Icon name="check" size={18} label="Model test succeeded" className="tw:shrink-0 tw:text-success" /> : null;
  const feedback = (
    <>
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
    </>
  );
  const options = optionsOpen && task === "embeddings" && profileId && modelId ? (
    <EmbeddingModelOptions profileId={profileId} modelId={modelId} disabled={locked || testing} onClose={() => setOptionsOpen(false)} />
  ) : null;

  // Search needs embeddings, which text-only services do not offer.
  if (task === "embeddings" && eligibleProfiles.length === 0 && onAddOpenRouter) {
    return <TaskOffRow task={task} label={label} wide={wide} onAddOpenRouter={onAddOpenRouter} />;
  }

  return wide ? (
    <div role="listitem" data-task={task} className="tw:grid tw:min-w-0 tw:gap-1 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:py-2.5 tw:first:border-t-0">
      <div className={TASK_GRID}>
        <span className="tw:flex tw:items-center tw:gap-2 tw:font-medium">{label}{tested}</span>
        {serviceSelect}
        {modelSelect}
        {reasoningSelect ?? <span />}
        {menu}
      </div>
      {feedback}
      {options}
    </div>
  ) : (
    <div role="listitem" data-task={task} className="tw:grid tw:min-w-0 tw:gap-3 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:py-4 tw:first:border-t-0 tw:first:pt-0">
      <div className="tw:flex tw:items-center tw:gap-2">
        <strong className="tw:flex-1 tw:font-medium">{label}</strong>
        {tested}
        {menu}
      </div>
      <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground"><span>Service</span>{serviceSelect}</label>
      <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground"><span>Model</span>{modelSelect}</label>
      {reasoningSelect ? <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground"><span>Reasoning</span>{reasoningSelect}</label> : null}
      {feedback}
      {options}
    </div>
  );
}

/** A task no connected service can do: it stays off, with the service that can. */
export function TaskOffRow({
  task,
  label,
  wide,
  onAddOpenRouter,
}: {
  task: string;
  label: string;
  wide: boolean;
  onAddOpenRouter: () => void;
}) {
  const off = <span className="tw:text-muted-foreground">Off · needs OpenRouter</span>;
  const add = (
    <Button variant="secondary" size="small" className="tw:justify-self-start" onClick={onAddOpenRouter}>
      Add OpenRouter
    </Button>
  );
  return wide ? (
    <div role="listitem" data-task={task} className="tw:grid tw:min-w-0 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:py-2.5 tw:first:border-t-0">
      <div className={TASK_GRID}>
        <span className="tw:font-medium">{label}</span>
        {off}
        {add}
        <span />
        <span />
      </div>
    </div>
  ) : (
    <div role="listitem" data-task={task} className="tw:grid tw:min-w-0 tw:gap-2 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:py-4 tw:first:border-t-0 tw:first:pt-0">
      <strong className="tw:font-medium">{label}</strong>
      {off}
      {add}
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
  onClose,
}: {
  profileId: string;
  modelId: string;
  disabled: boolean;
  onClose: () => void;
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
      onClose();
    } catch (failure: unknown) {
      setError(getErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Sheet title="Embedding options" width="small" busy={busy} onClose={onClose}
      actions={<>
        <Button variant="secondary" disabled={busy} onClick={onClose}>Cancel</Button>
        <Button variant="primary" disabled={disabled || busy || !dirty || !validDimensions} onClick={() => void save()}>
          {busy ? "Saving…" : "Save"}
        </Button>
      </>}>
      <div className="tw:grid tw:min-w-0 tw:gap-4">
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
      </div>
      {!validDimensions ? <p className="tw:m-0 tw:text-label tw:leading-relaxed tw:text-destructive">Use 32–4096 dimensions or leave blank.</p> : null}
      {error ? (
        <p className="tw:m-0 tw:text-label tw:leading-relaxed tw:text-destructive" role="alert">
          {error}
        </p>
      ) : null}
    </Sheet>
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
