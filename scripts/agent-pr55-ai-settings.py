#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content)


def replace_once(text: str, old: str, new: str, path: str) -> str:
    if old not in text:
        if new in text:
            return text
        raise RuntimeError(f"{path}: missing replacement anchor: {old[:160]!r}")
    if text.count(old) != 1:
        raise RuntimeError(f"{path}: replacement anchor is ambiguous: {old[:160]!r}")
    return text.replace(old, new, 1)


write(
    "src/screens/settings/AiLlmSettings.tsx",
    r'''import { useMemo, useState, type ReactNode } from "react";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import type {
  AiCredentialStatus,
  AiProviderKind,
  AiProviderProbeResult,
  AiProviderProfileDetail,
} from "../../ipc/aiProfiles";
import { useConfirm } from "../../ui/overlays/overlayContext";
import aiStyles from "./AiLlmSettings.module.css";
import styles from "./Settings.module.css";

interface AiLlmSettingsProps {
  controller: AiProfilesController;
  sidebarTop?: ReactNode;
}

export function AiLlmSettings({ controller, sidebarTop }: AiLlmSettingsProps) {
  const confirm = useConfirm();
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [credentialVisible, setCredentialVisible] = useState(false);
  const [modelSearch, setModelSearch] = useState("");
  const draft = controller.draft;
  const profile = controller.selectedProfile;
  const provider = draft
    ? controller.catalog.find((candidate) => candidate.providerKind === draft.providerKind) ?? null
    : null;
  const availableProviders = controller.catalog.filter(
    (candidate) => candidate.implementationStatus === "available",
  );
  const isLocal = draft?.providerKind === "llama_cpp";
  const requiresKey = provider?.authenticationRequired ?? false;
  const credentialReady = profile ? canUseProviderCredential(profile) : false;
  const modelReady = Boolean(profile && draft?.modelId);
  const normalizedSearch = modelSearch.trim().toLocaleLowerCase();
  const matchingModels = useMemo(
    () =>
      controller.models.filter((model) => {
        if (!normalizedSearch) return true;
        return `${model.displayName} ${model.id} ${model.ownedBy ?? ""}`
          .toLocaleLowerCase()
          .includes(normalizedSearch);
      }),
    [controller.models, normalizedSearch],
  );

  async function changeProfile(profileId: string): Promise<void> {
    if (profileId === controller.selectedProfileId) return;
    if (controller.dirty) {
      const accepted = await confirm({
        title: "Discard AI model changes?",
        description: "Unsaved connection, model, and API-key entry will be lost.",
        confirmLabel: "Discard changes",
        destructive: true,
      });
      if (!accepted) return;
      controller.discardDraft();
    }
    controller.selectProfile(profileId);
    setShowAdvanced(false);
    setModelSearch("");
  }

  async function beginCreate(providerKind: AiProviderKind): Promise<void> {
    if (controller.dirty) {
      const accepted = await confirm({
        title: "Discard AI model changes?",
        description: "Unsaved connection, model, and API-key entry will be lost.",
        confirmLabel: "Discard changes",
        destructive: true,
      });
      if (!accepted) return;
      controller.discardDraft();
    }
    controller.beginCreate(providerKind);
    setShowAdvanced(false);
    setModelSearch("");
  }

  async function deleteProfile(): Promise<void> {
    if (!profile) return;
    const accepted = await confirm({
      title: `Delete ${profile.displayName}?`,
      description:
        "The saved connection, task assignment, credential, and verification history will be removed. Study content is unchanged.",
      confirmLabel: "Delete AI model",
      destructive: true,
    });
    if (accepted) await controller.deleteProfile();
  }

  return (
    <div className={aiStyles.layout}>
      {controller.errorMessage ? (
        <div className="inline-alert error-alert">{controller.errorMessage}</div>
      ) : null}

      <aside className={aiStyles.profileSidebar} aria-label="Configured AI models">
        <div className={aiStyles.sidebarHeading}>
          <div>
            <h2>AI models</h2>
            <p>Each saved entry connects ostudy to one model.</p>
          </div>
          <button
            type="button"
            className="primary-button"
            disabled={controller.mutating || availableProviders.length === 0}
            onClick={() => {
              const preferred =
                availableProviders.find((candidate) => candidate.providerKind === "llama_cpp") ??
                availableProviders[0];
              if (preferred) void beginCreate(preferred.providerKind);
            }}
          >
            Add AI model
          </button>
        </div>
        {controller.loading ? <div className="inline-progress">Loading AI models…</div> : null}
        <div className={aiStyles.profileList}>
          {controller.profiles.map((candidate) => (
            <button
              key={candidate.id}
              type="button"
              data-selected={candidate.id === controller.selectedProfileId}
              onClick={() => void changeProfile(candidate.id)}
            >
              <strong>{candidate.displayName}</strong>
              <span>{candidate.modelId || "Model not selected"}</span>
              <small>
                {controller.defaults.flashcardGenerationProfileId === candidate.id
                  ? "Default · "
                  : ""}
                {configurationLabel(candidate.configurationStatus)}
              </small>
            </button>
          ))}
          {!controller.loading && controller.profiles.length === 0 ? (
            <p className={aiStyles.emptyCopy}>No AI model has been configured.</p>
          ) : null}
        </div>
      </aside>

      <main className={aiStyles.editor}>
        {!draft ? (
          <section className={styles.settingsCard}>
            <h2>Add an AI model</h2>
            <p className={styles.note}>
              Choose a service, connect it, and select the model ostudy should use.
            </p>
          </section>
        ) : (
          <section className={styles.settingsCard} aria-busy={controller.mutating}>
            <div className={styles.sectionHeading}>
              <div>
                <h2>{profile ? "AI model settings" : "Add an AI model"}</h2>
                <p>Service, API key or server connection, then model.</p>
              </div>
              {profile ? (
                <span className={aiStyles.statusBadge} data-status={profile.configurationStatus}>
                  {configurationLabel(profile.configurationStatus)}
                </span>
              ) : null}
            </div>

            <div className={aiStyles.setupSteps}>
              <label className={styles.stackedField}>
                <span>1. Service</span>
                <select
                  value={draft.providerKind}
                  disabled={profile !== null || controller.mutating}
                  onChange={(event) => void beginCreate(event.currentTarget.value as AiProviderKind)}
                >
                  {availableProviders.map((candidate) => (
                    <option key={candidate.providerKind} value={candidate.providerKind}>
                      {candidate.displayName}
                    </option>
                  ))}
                </select>
              </label>

              {isLocal ? (
                <label className={styles.stackedField}>
                  <span>2. Server address</span>
                  <input
                    type="url"
                    value={draft.baseUrl}
                    maxLength={2048}
                    placeholder="http://127.0.0.1:8080/"
                    onChange={(event) => controller.updateDraft({ baseUrl: event.currentTarget.value })}
                  />
                </label>
              ) : null}

              <label className={styles.stackedField}>
                <span>{isLocal ? "API key (optional)" : "2. API key"}</span>
                <span className={aiStyles.credentialField}>
                  <input
                    type={credentialVisible ? "text" : "password"}
                    autoComplete="off"
                    value={controller.credentialDraft}
                    placeholder={credentialPlaceholder(profile?.credentialStatus, requiresKey)}
                    onChange={(event) => controller.setCredentialDraft(event.currentTarget.value)}
                  />
                  <button
                    type="button"
                    aria-label={credentialVisible ? "Hide API key" : "Show API key"}
                    aria-pressed={credentialVisible}
                    onClick={() => setCredentialVisible((current) => !current)}
                  >
                    {credentialVisible ? "Hide" : "Show"}
                  </button>
                </span>
              </label>

              {!profile ? (
                <button
                  type="button"
                  className="primary-button"
                  disabled={
                    controller.mutating ||
                    draft.baseUrl.trim() === "" ||
                    (requiresKey && controller.credentialDraft.trim() === "")
                  }
                  onClick={() => void controller.saveSetup()}
                >
                  {controller.mutating ? "Connecting…" : "Connect and retrieve models"}
                </button>
              ) : controller.credentialDraft ? (
                <button
                  type="button"
                  className="secondary-button"
                  disabled={controller.mutating}
                  onClick={() => void controller.saveSetup()}
                >
                  {controller.mutating ? "Saving…" : "Replace API key"}
                </button>
              ) : null}

              {profile ? (
                <div className={aiStyles.modelStep}>
                  <label className={styles.stackedField}>
                    <span>3. Model</span>
                    {controller.models.length > 50 ? (
                      <input
                        type="search"
                        value={modelSearch}
                        placeholder="Search available models"
                        onChange={(event) => setModelSearch(event.currentTarget.value)}
                      />
                    ) : null}
                    <select
                      value={
                        controller.models.some((model) => model.id === draft.modelId)
                          ? draft.modelId
                          : ""
                      }
                      disabled={controller.refreshingModels || controller.mutating}
                      onChange={(event) => controller.updateDraft({ modelId: event.currentTarget.value })}
                    >
                      <option value="">Select a model</option>
                      {matchingModels.map((model) => (
                        <option key={model.id} value={model.id}>
                          {model.displayName || model.id}
                          {model.displayName && model.displayName !== model.id
                            ? ` — ${model.id}`
                            : ""}
                        </option>
                      ))}
                    </select>
                  </label>
                  <div className={styles.actionRow}>
                    <button
                      type="button"
                      className="primary-button"
                      disabled={
                        controller.mutating ||
                        controller.refreshingModels ||
                        !draft.modelId ||
                        draft.modelId === profile.modelId
                      }
                      onClick={() => void controller.saveProfile()}
                    >
                      Save model
                    </button>
                    <button
                      type="button"
                      className="secondary-button"
                      disabled={
                        controller.mutating ||
                        controller.refreshingModels ||
                        controller.dirty
                      }
                      onClick={() => void controller.refreshModels()}
                    >
                      {controller.refreshingModels ? "Retrieving…" : "Refresh models"}
                    </button>
                  </div>
                  {controller.refreshingModels ? (
                    <div className="inline-progress">Retrieving available models…</div>
                  ) : null}
                </div>
              ) : null}
            </div>

            <label className={aiStyles.advancedToggle}>
              <input
                type="checkbox"
                checked={showAdvanced}
                onChange={(event) => setShowAdvanced(event.currentTarget.checked)}
              />
              <span>Show advanced settings</span>
            </label>

            {showAdvanced ? (
              <div className={aiStyles.advancedPanel}>
                <label className={styles.stackedField}>
                  <span>Profile name</span>
                  <input
                    value={draft.displayName}
                    maxLength={100}
                    onChange={(event) =>
                      controller.updateDraft({ displayName: event.currentTarget.value })
                    }
                  />
                </label>
                <label className={styles.stackedField}>
                  <span>Inactivity timeout</span>
                  <select
                    value={draft.requestTimeoutMs}
                    onChange={(event) =>
                      controller.updateDraft({ requestTimeoutMs: Number(event.currentTarget.value) })
                    }
                  >
                    <option value={30_000}>30 seconds</option>
                    <option value={60_000}>1 minute</option>
                    <option value={120_000}>2 minutes</option>
                    <option value={300_000}>5 minutes</option>
                  </select>
                </label>
                <fieldset className={aiStyles.storageOptions}>
                  <legend>API-key storage</legend>
                  <label>
                    <input
                      type="radio"
                      name="credential-persistence"
                      checked={controller.credentialPersistence === "secure"}
                      disabled={profile?.credentialStatus.securePersistenceAvailable === false}
                      onChange={() => controller.setCredentialPersistence("secure")}
                    />
                    Operating-system credential store
                  </label>
                  <label>
                    <input
                      type="radio"
                      name="credential-persistence"
                      checked={controller.credentialPersistence === "session"}
                      onChange={() => controller.setCredentialPersistence("session")}
                    />
                    Current ostudy session only
                  </label>
                </fieldset>
                {isLocal && draft.baseUrl.startsWith("http://") && !isLoopbackUrl(draft.baseUrl) ? (
                  <label className={styles.switchRow}>
                    <span>
                      <strong>Allow unencrypted private-LAN connection</strong>
                      <small>Prompts and an optional API key can be read on the local network.</small>
                    </span>
                    <input
                      type="checkbox"
                      checked={draft.allowInsecureHttp}
                      onChange={(event) =>
                        controller.updateDraft({ allowInsecureHttp: event.currentTarget.checked })
                      }
                    />
                  </label>
                ) : null}
                {draft.providerKind === "openrouter" ? (
                  <label className={styles.switchRow}>
                    <span>
                      <strong>Require zero data retention</strong>
                      <small>Use only OpenRouter endpoints that declare zero-data-retention routing.</small>
                    </span>
                    <input
                      type="checkbox"
                      checked={draft.requireZeroDataRetention ?? false}
                      onChange={(event) =>
                        controller.updateDraft({
                          requireZeroDataRetention: event.currentTarget.checked,
                        })
                      }
                    />
                  </label>
                ) : null}
                {sidebarTop}
                <div className={styles.actionRow}>
                  {profile ? (
                    <>
                      <button
                        type="button"
                        className="secondary-button"
                        disabled={controller.mutating || !controller.dirty}
                        onClick={() => void controller.saveSetup()}
                      >
                        Save advanced settings
                      </button>
                      {canUseProviderCredential(profile) ? (
                        <button
                          type="button"
                          className="secondary-button"
                          disabled={controller.mutating}
                          onClick={() => void controller.clearCredential()}
                        >
                          Remove API key
                        </button>
                      ) : null}
                      <button
                        type="button"
                        className="danger-button"
                        disabled={controller.mutating}
                        onClick={() => void deleteProfile()}
                      >
                        Delete AI model
                      </button>
                    </>
                  ) : null}
                </div>
              </div>
            ) : null}

            {profile ? (
              <ConnectionResult
                controller={controller}
                profile={profile}
                blocked={!credentialReady || !modelReady || controller.dirty}
              />
            ) : null}
          </section>
        )}
      </main>
    </div>
  );
}

function ConnectionResult({
  controller,
  profile,
  blocked,
}: {
  controller: AiProfilesController;
  profile: AiProviderProfileDetail;
  blocked: boolean;
}) {
  const result = controller.probeResult;
  return (
    <div className={aiStyles.connectionPanel}>
      <button
        type="button"
        className="secondary-button"
        disabled={blocked || controller.testing || controller.refreshingModels}
        onClick={() => void controller.testProfile()}
      >
        {controller.testing ? "Testing…" : "Test connection"}
      </button>
      {result ? (
        <div className={aiStyles.connectionResult} data-status={result.overallStatus}>
          <div>
            <strong>{connectionSummary(result)}</strong>
            <span>
              {result.testedModelId || profile.modelId} · {formatLatency(result)} ·{" "}
              {new Date(result.testedAt).toLocaleString()}
              {result.stale ? " · stale" : ""}
            </span>
          </div>
          <details>
            <summary>Details</summary>
            <ul>
              {result.phases.map((phase) => (
                <li key={phase.phase}>
                  <strong>{phase.label}</strong>
                  <span>{phase.error?.message ?? phase.message}</span>
                  {phase.latencyMs !== null ? <small>{phase.latencyMs} ms</small> : null}
                </li>
              ))}
            </ul>
          </details>
        </div>
      ) : (
        <span className={aiStyles.noResult}>Not tested yet.</span>
      )}
    </div>
  );
}

function canUseProviderCredential(profile: AiProviderProfileDetail): boolean {
  if (profile.authMode === "none") return true;
  return (
    profile.credentialStatus.status === "saved_securely" ||
    profile.credentialStatus.status === "session_only"
  );
}

function credentialPlaceholder(
  status: AiCredentialStatus | undefined,
  required: boolean,
): string {
  if (status?.status === "saved_securely" || status?.status === "session_only") {
    return "Saved — enter a replacement only when needed";
  }
  return required ? "Enter API key" : "Optional API key";
}

function configurationLabel(value: string): string {
  switch (value) {
    case "ready":
      return "Ready";
    case "disabled":
      return "Disabled";
    case "planned":
      return "Planned";
    default:
      return "Setup incomplete";
  }
}

function connectionSummary(result: AiProviderProbeResult): string {
  if (result.overallStatus === "verified") return "Connected · structured output verified";
  if (result.overallStatus === "partial") return "Connected · some capabilities unavailable";
  const failed = result.phases.find((phase) => phase.status === "failed");
  return failed?.error?.message ?? failed?.message ?? "Connection test failed";
}

function formatLatency(result: AiProviderProbeResult): string {
  const latency = result.phases.reduce<number | null>((maximum, phase) => {
    if (phase.latencyMs === null) return maximum;
    return maximum === null ? phase.latencyMs : Math.max(maximum, phase.latencyMs);
  }, null);
  return latency === null ? "latency unavailable" : `${latency} ms`;
}

function isLoopbackUrl(value: string): boolean {
  try {
    const hostname = new URL(value).hostname.toLowerCase();
    return hostname === "localhost" || hostname === "127.0.0.1" || hostname === "[::1]";
  } catch {
    return false;
  }
}
''',
)

path = "src/controllers/aiProfilesController.ts"
text = read(path)
text = replace_once(text, "  saveProfile: () => Promise<void>;\n", "  saveProfile: () => Promise<void>;\n  saveSetup: () => Promise<void>;\n", path)
anchor = "  async function saveProfile(): Promise<void> {"
setup = r'''  async function saveSetup(): Promise<void> {
    if (!draft || mutating) return;
    const provider = catalog.find((candidate) => candidate.providerKind === draft.providerKind);
    if (!provider) return;
    if (
      selectedProfile === null &&
      provider.authenticationRequired &&
      credentialDraft.trim() === ""
    ) {
      setErrorMessage("Enter an API key before retrieving models from this service.");
      return;
    }

    setMutating(true);
    setErrorMessage(null);
    let createdProfile: AiProviderProfileDetail | null = null;
    try {
      const normalizedDraft: AiProfileDraft = {
        ...draft,
        enabled: true,
        authMode:
          draft.providerKind === "llama_cpp"
            ? credentialDraft.trim() === "" && selectedProfile?.authMode !== "bearer"
              ? "none"
              : "bearer"
            : "bearer",
      };
      let saved = selectedProfile
        ? await updateAiProviderProfile(
            selectedProfile.id,
            selectedProfile.revision,
            normalizedDraft,
          )
        : await createAiProviderProfile(normalizedDraft);
      if (!selectedProfile) createdProfile = saved;
      if (credentialDraft.trim() !== "") {
        saved = await setAiProviderCredential(
          saved.id,
          saved.revision,
          credentialDraft,
          credentialPersistence,
        );
      }
      setSelectedProfile(saved);
      setDraft(draftFromProfile(saved));
      setSelectedProfileId(saved.id);
      setCredentialDraft("");
      setModels([]);
      setModelsLoaded(false);
      setProbeResult(null);
      await loadProfiles();
      showToast("success", selectedProfile ? "AI model settings saved." : "Connected to the AI service.");
      if (canDiscoverModels(saved)) await discoverModelsForProfile(saved, true);
    } catch (error: unknown) {
      if (createdProfile) {
        try {
          await deleteAiProviderProfile(createdProfile.id, createdProfile.revision);
          await loadProfiles();
        } catch {
          // The original safe setup error remains the actionable result. A
          // best-effort rollback failure leaves an ordinary incomplete profile
          // that can be deleted from the same settings page.
        }
      }
      setErrorMessage(getErrorMessage(error));
    } finally {
      setMutating(false);
    }
  }

'''
if "async function saveSetup" not in text:
    text = replace_once(text, anchor, setup + anchor, path)
text = replace_once(
    text,
    '      showToast("success", wasExisting ? "AI provider profile saved." : "AI provider profile created.");\n      if (canDiscoverModels(saved)) {',
    '      showToast("success", wasExisting ? "AI provider profile saved." : "AI provider profile created.");\n      if (!defaults.flashcardGenerationProfileId && saved.modelId) {\n        const selectedDefaults = await setFlashcardGenerationProfile(saved.id);\n        setDefaults(selectedDefaults);\n      }\n      if (canDiscoverModels(saved)) {',
    path,
)
text = replace_once(text, "    saveProfile,\n", "    saveProfile,\n    saveSetup,\n", path)
write(path, text)

write(
    "src/screens/settings/DefaultAiModelSettings.tsx",
    r'''import { useMemo, useState } from "react";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import type { AiReasoningEffort } from "../../ipc/aiProfiles";
import type { AiPromptPreset, AppSettings } from "../../ipc/settings";
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
  const [newPresetName, setNewPresetName] = useState("");
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
    : [selectedEffort as AiReasoningEffort, ...allowedEfforts];
  const presets = settings.aiPromptPresets ?? [];
  const selectedPreset =
    presets.find((preset) => preset.id === settings.aiPromptPresetId) ?? null;
  const selectedPresetIndex = selectedPreset
    ? presets.findIndex((preset) => preset.id === selectedPreset.id)
    : -1;
  const contextMaximum = selectedDefaultModel?.contextWindowTokens ?? null;
  const providerSlugs = (settings.aiOpenRouterProviderSlugs ?? []).join(", ");
  const promptCharacters =
    (selectedPreset?.outlineInstructions.length ?? 0) +
    (selectedPreset?.cardGenerationInstructions.length ?? 0);

  function updatePreset(patch: Partial<AiPromptPreset>): void {
    if (selectedPresetIndex < 0 || !selectedPreset) return;
    const next = presets.map((preset, index) =>
      index === selectedPresetIndex
        ? { ...preset, ...patch, revision: preset.revision + 1, updatedAt: Date.now() }
        : preset,
    );
    update({ aiPromptPresets: next });
  }

  function createPreset(): void {
    const name = newPresetName.trim();
    if (!name) return;
    const now = Date.now();
    const preset: AiPromptPreset = {
      id: crypto.randomUUID().replaceAll("-", ""),
      name,
      outlineInstructions: "",
      cardGenerationInstructions: "",
      revision: 1,
      createdAt: now,
      updatedAt: now,
    };
    update({ aiPromptPresets: [...presets, preset], aiPromptPresetId: preset.id });
    setNewPresetName("");
  }

  function deletePreset(): void {
    if (!selectedPreset) return;
    update({
      aiPromptPresets: presets.filter((preset) => preset.id !== selectedPreset.id),
      aiPromptPresetId: null,
    });
  }

  const effectiveContext = useMemo(() => {
    const manual = settings.aiContextLimitTokens ?? null;
    if (manual === null) return contextMaximum;
    return contextMaximum === null ? manual : Math.min(manual, contextMaximum);
  }, [contextMaximum, settings.aiContextLimitTokens]);

  return (
    <section className={styles.card} aria-labelledby="advanced-ai-policy-title">
      <h2 id="advanced-ai-policy-title">Document-generation policy</h2>

      <label className={styles.field}>
        <span>Default AI model</span>
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
                {profile.modelId ? ` — ${profile.modelId}` : " — incomplete"}
              </option>
            ))}
        </select>
      </label>

      <div className={styles.policyGrid}>
        <label className={styles.field}>
          <span>Reasoning level</span>
          <select
            value={selectedEffort}
            onChange={(event) =>
              update({
                aiReasoningEffort: event.currentTarget.value as AppSettings["aiReasoningEffort"],
              })
            }
          >
            <option value="automatic">
              Automatic{reasoning?.defaultEffort ? ` — ${reasoning.defaultEffort}` : ""}
            </option>
            {visibleEfforts.map((effort) => (
              <option key={effort} value={effort}>
                {reasoningLabel(effort)}
              </option>
            ))}
          </select>
        </label>

        <label className={styles.field}>
          <span>Semantic coverage</span>
          <select
            value={settings.aiCoverageMode ?? "balanced"}
            onChange={(event) =>
              update({
                aiCoverageMode: event.currentTarget.value as AppSettings["aiCoverageMode"],
              })
            }
          >
            <option value="concise">Concise</option>
            <option value="balanced">Balanced</option>
            <option value="thorough">Thorough</option>
          </select>
        </label>

        <label className={styles.field}>
          <span>Rendered page images</span>
          <select
            value={settings.aiImageInputPolicy ?? "automatic"}
            onChange={(event) =>
              update({
                aiImageInputPolicy: event.currentTarget.value as AppSettings["aiImageInputPolicy"],
              })
            }
          >
            <option value="automatic">Automatic when verified</option>
            <option value="never">Never send page images</option>
          </select>
        </label>

        <label className={styles.field}>
          <span>Maximum context ostudy may use</span>
          <input
            type="number"
            min={2048}
            max={contextMaximum ?? 16_777_216}
            step={1024}
            value={settings.aiContextLimitTokens ?? ""}
            placeholder="Automatic"
            onChange={(event) =>
              update({
                aiContextLimitTokens:
                  event.currentTarget.value === "" ? null : Number(event.currentTarget.value),
              })
            }
          />
          <small>
            {contextMaximum
              ? `Detected model context: ${formatTokens(contextMaximum)}. Effective cap: ${formatTokens(effectiveContext ?? contextMaximum)}.`
              : effectiveContext
                ? `Manual request-planning cap: ${formatTokens(effectiveContext)}.`
                : "Automatic uses the verified model or endpoint limit."}
          </small>
        </label>
      </div>

      {defaultProfile?.providerKind === "openrouter" ? (
        <div className={styles.routingPanel}>
          <h3>OpenRouter hosting</h3>
          <label className={styles.field}>
            <span>Routing behavior</span>
            <select
              value={settings.aiOpenRouterRoutingMode ?? "automatic"}
              onChange={(event) =>
                update({
                  aiOpenRouterRoutingMode:
                    event.currentTarget.value as AppSettings["aiOpenRouterRoutingMode"],
                })
              }
            >
              <option value="automatic">Automatic routing</option>
              <option value="prefer">Prefer selected hosts, allow fallback</option>
              <option value="only">Use only selected hosts</option>
            </select>
          </label>
          {(settings.aiOpenRouterRoutingMode ?? "automatic") !== "automatic" ? (
            <label className={styles.field}>
              <span>Hosting-provider slugs</span>
              <input
                value={providerSlugs}
                placeholder="provider-one, provider-two"
                onChange={(event) =>
                  update({
                    aiOpenRouterProviderSlugs: parseCommaList(event.currentTarget.value),
                  })
                }
              />
              <small>Use the endpoint/provider slugs reported by OpenRouter for this model.</small>
            </label>
          ) : null}
        </div>
      ) : null}

      <div className={styles.promptPanel}>
        <h3>Generation instructions</h3>
        <label className={styles.field}>
          <span>Prompt preset</span>
          <select
            value={settings.aiPromptPresetId ?? ""}
            onChange={(event) => update({ aiPromptPresetId: event.currentTarget.value || null })}
          >
            <option value="">Built-in ostudy policy</option>
            {presets.map((preset) => (
              <option key={preset.id} value={preset.id}>
                {preset.name}
              </option>
            ))}
          </select>
        </label>
        <div className={styles.newPresetRow}>
          <input
            value={newPresetName}
            placeholder="New preset name"
            maxLength={100}
            onChange={(event) => setNewPresetName(event.currentTarget.value)}
          />
          <button type="button" className="secondary-button" onClick={createPreset}>
            New preset
          </button>
        </div>
        {selectedPreset ? (
          <>
            <label className={styles.field}>
              <span>Preset name</span>
              <input
                value={selectedPreset.name}
                maxLength={100}
                onChange={(event) => updatePreset({ name: event.currentTarget.value })}
              />
            </label>
            <label className={styles.field}>
              <span>Outline instructions</span>
              <textarea
                rows={5}
                maxLength={12_000}
                value={selectedPreset.outlineInstructions}
                placeholder="Emphasize mechanisms, prerequisite relationships, clinically significant distinctions…"
                onChange={(event) =>
                  updatePreset({ outlineInstructions: event.currentTarget.value })
                }
              />
            </label>
            <label className={styles.field}>
              <span>Card-generation instructions</span>
              <textarea
                rows={7}
                maxLength={20_000}
                value={selectedPreset.cardGenerationInstructions}
                placeholder="Prioritize mechanisms, application, interpretation, and clinically meaningful decisions…"
                onChange={(event) =>
                  updatePreset({ cardGenerationInstructions: event.currentTarget.value })
                }
              />
            </label>
            <div className={styles.promptFooter}>
              <small>
                {promptCharacters.toLocaleString()} characters. These instructions are sent to the
                selected provider. Source-trust rules, schemas, and native validation remain fixed.
              </small>
              <button type="button" className="danger-button" onClick={deletePreset}>
                Delete preset
              </button>
            </div>
          </>
        ) : (
          <p className={styles.note}>
            The built-in policy prioritizes durable concepts, mechanisms, relationships,
            applications, and correct question/answer pairs.
          </p>
        )}
      </div>
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

function formatTokens(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M tokens`;
  if (value >= 1_000) return `${Math.round(value / 1_000)}k tokens`;
  return `${value.toLocaleString()} tokens`;
}

function parseCommaList(value: string): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const raw of value.split(",")) {
    const candidate = raw.trim().toLocaleLowerCase();
    if (candidate && !seen.has(candidate)) {
      seen.add(candidate);
      result.push(candidate);
    }
  }
  return result;
}
''',
)

path = "src/ipc/settings.ts"
text = read(path)
if "export type AiCoverageMode" not in text:
    text = text.replace(
        'export type AiReasoningEffort =\n',
        'export type AiImageInputPolicy = "automatic" | "never";\nexport type AiCoverageMode = "concise" | "balanced" | "thorough";\nexport type AiOpenRouterRoutingMode = "automatic" | "prefer" | "only";\n\nexport interface AiPromptPreset {\n  id: string;\n  name: string;\n  outlineInstructions: string;\n  cardGenerationInstructions: string;\n  revision: number;\n  createdAt: number;\n  updatedAt: number;\n}\n\nexport type AiReasoningEffort =\n',
        1,
    )
text = replace_once(
    text,
    "  aiReasoningEffort?: AiReasoningEffort;\n}",
    "  aiReasoningEffort?: AiReasoningEffort;\n  aiContextLimitTokens?: number | null;\n  aiImageInputPolicy?: AiImageInputPolicy;\n  aiCoverageMode?: AiCoverageMode;\n  aiOpenRouterRoutingMode?: AiOpenRouterRoutingMode;\n  aiOpenRouterProviderSlugs?: string[];\n  aiPromptPresetId?: string | null;\n  aiPromptPresets?: AiPromptPreset[];\n}",
    path,
)
write(path, text)

for path in ["src/controllers/useAppController.ts", "src/screens/SettingsScreen.tsx"]:
    text = read(path)
    marker = '  aiReasoningEffort: "low",\n'
    addition = marker + '  aiContextLimitTokens: null,\n  aiImageInputPolicy: "automatic",\n  aiCoverageMode: "balanced",\n  aiOpenRouterRoutingMode: "automatic",\n  aiOpenRouterProviderSlugs: [],\n  aiPromptPresetId: null,\n  aiPromptPresets: [],\n'
    if "aiContextLimitTokens" not in text:
        text = replace_once(text, marker, addition, path)
    write(path, text)

path = "src/screens/SettingsScreen.tsx"
text = read(path)
text = replace_once(
    text,
    '    if (activeTab === "ai") update({ aiReasoningEffort: DEFAULTS.aiReasoningEffort });',
    '    if (activeTab === "ai") {\n      update({\n        aiReasoningEffort: DEFAULTS.aiReasoningEffort,\n        aiContextLimitTokens: DEFAULTS.aiContextLimitTokens,\n        aiImageInputPolicy: DEFAULTS.aiImageInputPolicy,\n        aiCoverageMode: DEFAULTS.aiCoverageMode,\n        aiOpenRouterRoutingMode: DEFAULTS.aiOpenRouterRoutingMode,\n        aiOpenRouterProviderSlugs: DEFAULTS.aiOpenRouterProviderSlugs,\n        aiPromptPresetId: DEFAULTS.aiPromptPresetId,\n        aiPromptPresets: DEFAULTS.aiPromptPresets,\n      });\n    }',
    path,
)
write(path, text)

path = "src-tauri/src/settings.rs"
text = read(path)
if "AiPromptPresetDto" not in text:
    text = text.replace(
        "use std::sync::{LazyLock, RwLock};",
        "use std::{collections::HashSet, sync::{LazyLock, RwLock}};",
        1,
    )
    text = text.replace(
        'const AI_REASONING_EFFORTS: &[&str] = &[',
        'const AI_IMAGE_INPUT_POLICIES: &[&str] = &["automatic", "never"];\nconst AI_COVERAGE_MODES: &[&str] = &["concise", "balanced", "thorough"];\nconst AI_OPENROUTER_ROUTING_MODES: &[&str] = &["automatic", "prefer", "only"];\nconst MAX_AI_CONTEXT_TOKENS: i64 = 16_777_216;\nconst MAX_AI_PROMPT_PRESETS: usize = 32;\nconst MAX_AI_OUTLINE_INSTRUCTIONS: usize = 12_000;\nconst MAX_AI_CARD_INSTRUCTIONS: usize = 20_000;\nconst AI_REASONING_EFFORTS: &[&str] = &[',
        1,
    )
    dto_anchor = '#[derive(Clone, Debug, Deserialize, Serialize)]\n#[serde(rename_all = "camelCase", deny_unknown_fields)]\npub struct AppSettingsDto {'
    preset = '''#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiPromptPresetDto {
    pub id: String,
    pub name: String,
    pub outline_instructions: String,
    pub card_generation_instructions: String,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

'''
    text = replace_once(text, dto_anchor, preset + dto_anchor, path)
    text = replace_once(
        text,
        "    pub ai_reasoning_effort: String,\n}",
        '''    pub ai_reasoning_effort: String,
    #[serde(default)]
    pub ai_context_limit_tokens: Option<i64>,
    #[serde(default = "default_ai_image_input_policy")]
    pub ai_image_input_policy: String,
    #[serde(default = "default_ai_coverage_mode")]
    pub ai_coverage_mode: String,
    #[serde(default = "default_ai_openrouter_routing_mode")]
    pub ai_openrouter_routing_mode: String,
    #[serde(default)]
    pub ai_openrouter_provider_slugs: Vec<String>,
    #[serde(default)]
    pub ai_prompt_preset_id: Option<String>,
    #[serde(default)]
    pub ai_prompt_presets: Vec<AiPromptPresetDto>,
}''',
        path,
    )
    text = replace_once(
        text,
        "        ai_reasoning_effort,\n    })",
        '''        ai_reasoning_effort,
        ai_context_limit_tokens: get_setting(connection, "aiContextLimitTokens", None::<i64>)?,
        ai_image_input_policy: get_setting(
            connection,
            "aiImageInputPolicy",
            default_ai_image_input_policy(),
        )?,
        ai_coverage_mode: get_setting(
            connection,
            "aiCoverageMode",
            default_ai_coverage_mode(),
        )?,
        ai_openrouter_routing_mode: get_setting(
            connection,
            "aiOpenRouterRoutingMode",
            default_ai_openrouter_routing_mode(),
        )?,
        ai_openrouter_provider_slugs: get_setting(
            connection,
            "aiOpenRouterProviderSlugs",
            Vec::<String>::new(),
        )?,
        ai_prompt_preset_id: get_setting(connection, "aiPromptPresetId", None::<String>)?,
        ai_prompt_presets: get_setting(
            connection,
            "aiPromptPresets",
            Vec::<AiPromptPresetDto>::new(),
        )?,
    })''',
        path,
    )
    save_anchor = '''        (
            "aiReasoningEffort",
            serde_json::to_string(&settings.ai_reasoning_effort),
        ),'''
    save_extra = save_anchor + '''
        (
            "aiContextLimitTokens",
            serde_json::to_string(&settings.ai_context_limit_tokens),
        ),
        (
            "aiImageInputPolicy",
            serde_json::to_string(&settings.ai_image_input_policy),
        ),
        (
            "aiCoverageMode",
            serde_json::to_string(&settings.ai_coverage_mode),
        ),
        (
            "aiOpenRouterRoutingMode",
            serde_json::to_string(&settings.ai_openrouter_routing_mode),
        ),
        (
            "aiOpenRouterProviderSlugs",
            serde_json::to_string(&settings.ai_openrouter_provider_slugs),
        ),
        (
            "aiPromptPresetId",
            serde_json::to_string(&settings.ai_prompt_preset_id),
        ),
        (
            "aiPromptPresets",
            serde_json::to_string(&settings.ai_prompt_presets),
        ),'''
    text = replace_once(text, save_anchor, save_extra, path)
    validate_anchor = '''    validate_choice(
        &settings.ai_reasoning_effort,
        AI_REASONING_EFFORTS,
        "invalid_ai_reasoning_effort",
        "Select an available document AI reasoning level.",
    )?;'''
    validate_extra = validate_anchor + '''
    validate_choice(
        &settings.ai_image_input_policy,
        AI_IMAGE_INPUT_POLICIES,
        "invalid_ai_image_input_policy",
        "Select automatic image grounding or disable page-image input.",
    )?;
    validate_choice(
        &settings.ai_coverage_mode,
        AI_COVERAGE_MODES,
        "invalid_ai_coverage_mode",
        "Select concise, balanced, or thorough semantic coverage.",
    )?;
    validate_choice(
        &settings.ai_openrouter_routing_mode,
        AI_OPENROUTER_ROUTING_MODES,
        "invalid_ai_openrouter_routing_mode",
        "Select automatic, preferred-host, or selected-host-only OpenRouter routing.",
    )?;
    if settings
        .ai_context_limit_tokens
        .is_some_and(|value| !(2_048..=MAX_AI_CONTEXT_TOKENS).contains(&value))
    {
        return Err(CommandError::new(
            "invalid_ai_context_limit",
            "The AI context cap must be blank or between 2,048 and 16,777,216 tokens.",
        ));
    }
    validate_ai_provider_slugs(&settings.ai_openrouter_provider_slugs)?;
    validate_ai_prompt_presets(
        &settings.ai_prompt_presets,
        settings.ai_prompt_preset_id.as_deref(),
    )?;'''
    text = replace_once(text, validate_anchor, validate_extra, path)
    functions_anchor = "fn validate_choice(\n"
    helpers = r'''fn default_ai_image_input_policy() -> String {
    "automatic".to_string()
}

fn default_ai_coverage_mode() -> String {
    "balanced".to_string()
}

fn default_ai_openrouter_routing_mode() -> String {
    "automatic".to_string()
}

fn validate_ai_provider_slugs(values: &[String]) -> Result<(), CommandError> {
    if values.len() > 16 {
        return Err(CommandError::new(
            "invalid_ai_provider_routing",
            "Select at most 16 OpenRouter hosting providers.",
        ));
    }
    let mut seen = HashSet::new();
    for value in values {
        let valid = !value.is_empty()
            && value.len() <= 100
            && value.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '-' | '_' | '.')
            });
        if !valid || !seen.insert(value) {
            return Err(CommandError::new(
                "invalid_ai_provider_routing",
                "OpenRouter hosting-provider slugs must be unique lowercase identifiers.",
            ));
        }
    }
    Ok(())
}

fn validate_ai_prompt_presets(
    presets: &[AiPromptPresetDto],
    selected: Option<&str>,
) -> Result<(), CommandError> {
    if presets.len() > MAX_AI_PROMPT_PRESETS {
        return Err(CommandError::new(
            "too_many_ai_prompt_presets",
            format!("Save at most {MAX_AI_PROMPT_PRESETS} AI prompt presets."),
        ));
    }
    let mut ids = HashSet::new();
    for preset in presets {
        let valid_id = preset.id.len() == 32
            && preset.id.chars().all(|character| character.is_ascii_hexdigit());
        if !valid_id
            || !ids.insert(preset.id.as_str())
            || preset.name.trim().is_empty()
            || preset.name.chars().count() > 100
            || preset.outline_instructions.chars().count() > MAX_AI_OUTLINE_INSTRUCTIONS
            || preset.card_generation_instructions.chars().count() > MAX_AI_CARD_INSTRUCTIONS
            || preset.revision < 1
            || preset.created_at < 0
            || preset.updated_at < preset.created_at
            || preset
                .outline_instructions
                .chars()
                .chain(preset.card_generation_instructions.chars())
                .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
        {
            return Err(CommandError::new(
                "invalid_ai_prompt_preset",
                "An AI prompt preset has invalid identity, name, instructions, or revision metadata.",
            ));
        }
    }
    if selected.is_some_and(|id| !ids.contains(id)) {
        return Err(CommandError::new(
            "ai_prompt_preset_not_found",
            "The selected AI prompt preset is no longer available.",
        ));
    }
    Ok(())
}

'''
    text = replace_once(text, functions_anchor, helpers + functions_anchor, path)
write(path, text)

path = "src/screens/settings/AiLlmSettings.module.css"
text = read(path)
if ".layout" not in text:
    text += r'''

@layer components {
  .layout { display: grid; min-width: 0; grid-template-columns: minmax(220px, .32fr) minmax(0, 1fr); gap: var(--ostudy-space-5); }
  .layout > .inline-alert { grid-column: 1 / -1; }
  .profileSidebar,
  .editor { min-width: 0; }
  .profileSidebar { display: grid; align-content: start; gap: var(--ostudy-space-3); }
  .sidebarHeading { display: grid; gap: var(--ostudy-space-3); }
  .sidebarHeading h2,
  .sidebarHeading p { margin: 0; }
  .sidebarHeading p { color: var(--ostudy-color-text-muted); font-size: .78rem; }
  .profileList { display: grid; gap: var(--ostudy-space-2); }
  .profileList > button { display: grid; min-width: 0; gap: 2px; border: 1px solid var(--ostudy-color-border-subtle); border-radius: var(--ostudy-radius-medium); padding: var(--ostudy-space-3); background: var(--ostudy-color-surface-base); color: var(--ostudy-color-text-primary); text-align: start; }
  .profileList > button[data-selected="true"] { border-color: var(--ostudy-color-border-accent); background: var(--ostudy-color-surface-selected); box-shadow: var(--ostudy-shadow-level-1); }
  .profileList strong,
  .profileList span,
  .profileList small { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .profileList span,
  .profileList small,
  .emptyCopy { color: var(--ostudy-color-text-muted); font-size: .76rem; }
  .setupSteps { display: grid; gap: var(--ostudy-space-4); }
  .credentialField { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--ostudy-space-2); }
  .credentialField button { border: 1px solid var(--ostudy-color-border-subtle); border-radius: var(--ostudy-radius-small); padding-inline: var(--ostudy-space-3); background: var(--ostudy-color-surface-raised); color: var(--ostudy-color-text-primary); }
  .modelStep { display: grid; gap: var(--ostudy-space-3); }
  .advancedToggle { display: flex; align-items: center; gap: var(--ostudy-space-2); margin-top: var(--ostudy-space-5); border-top: 1px solid var(--ostudy-color-border-subtle); padding-top: var(--ostudy-space-4); font-weight: 760; }
  .advancedPanel { display: grid; gap: var(--ostudy-space-4); margin-top: var(--ostudy-space-3); border-radius: var(--ostudy-radius-medium); padding: var(--ostudy-space-4); background: var(--ostudy-color-surface-soft); }
  .storageOptions { display: grid; gap: var(--ostudy-space-2); border: 1px solid var(--ostudy-color-border-subtle); border-radius: var(--ostudy-radius-small); padding: var(--ostudy-space-3); }
  .storageOptions label { display: flex; align-items: center; gap: var(--ostudy-space-2); }
  .statusBadge { border-radius: var(--ostudy-radius-pill); padding: 4px 9px; background: var(--ostudy-color-surface-soft); color: var(--ostudy-color-text-secondary); font-size: .72rem; font-weight: 800; }
  .statusBadge[data-status="ready"] { background: color-mix(in srgb, var(--ostudy-color-status-success) 14%, transparent); color: var(--ostudy-color-status-success); }
  .connectionPanel { display: flex; min-width: 0; flex-wrap: wrap; align-items: center; gap: var(--ostudy-space-3); margin-top: var(--ostudy-space-5); border-top: 1px solid var(--ostudy-color-border-subtle); padding-top: var(--ostudy-space-4); }
  .connectionResult { display: grid; min-width: min(100%, 440px); flex: 1; gap: var(--ostudy-space-2); border-radius: var(--ostudy-radius-small); padding: var(--ostudy-space-3); background: var(--ostudy-color-surface-soft); }
  .connectionResult > div { display: grid; gap: 2px; }
  .connectionResult span,
  .connectionResult small,
  .noResult { color: var(--ostudy-color-text-muted); font-size: .75rem; }
  .connectionResult summary { cursor: pointer; color: var(--ostudy-color-text-accent); font-weight: 760; }
  .connectionResult ul { display: grid; gap: var(--ostudy-space-2); margin: var(--ostudy-space-2) 0 0; padding: 0; list-style: none; }
  .connectionResult li { display: grid; grid-template-columns: minmax(110px, auto) minmax(0, 1fr) auto; gap: var(--ostudy-space-2); }
  @media (max-width: 960px) { .layout { grid-template-columns: 1fr; } .profileList { grid-template-columns: repeat(auto-fit, minmax(190px, 1fr)); } }
}
'''
write(path, text)

path = "src/screens/settings/DefaultAiModelSettings.module.css"
text = read(path)
if ".policyGrid" not in text:
    text += r'''

.policyGrid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--ostudy-space-3); }
.routingPanel,
.promptPanel { display: grid; gap: var(--ostudy-space-3); margin-top: var(--ostudy-space-4); border-top: 1px solid var(--ostudy-color-border-subtle); padding-top: var(--ostudy-space-4); }
.routingPanel h3,
.promptPanel h3 { margin: 0; font-size: 1rem; }
.newPresetRow { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--ostudy-space-2); }
.promptFooter { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: var(--ostudy-space-3); }
.promptFooter small { max-width: 680px; color: var(--ostudy-color-text-muted); }
@media (max-width: 720px) { .policyGrid { grid-template-columns: 1fr; } .newPresetRow { grid-template-columns: 1fr; } }
'''
write(path, text)

for doc, addition in {
    "docs/architecture/ai-and-document-processing.md": '''\n\n## Configurable document-generation policy\n\nThe ordinary AI setup path is service, credential or local server connection, and model selection. Provider authentication syntax, structured-output adapters, native validation, and retry contracts remain internal. Advanced settings may cap the context ostudy uses, select a supported reasoning level, disable rendered-page input, choose semantic coverage, constrain OpenRouter hosting, and select a custom educational prompt preset. These non-secret policies are included in settings backups; credentials remain excluded.\n\nCustom instructions are inserted between immutable source-trust and output-contract layers. They cannot remove the rule that source material is untrusted, change the structured schema, bypass native validation, or enable cross-provider fallback.\n''',
    "docs/user-guide/documents-and-ai-generation.md": '''\n\n## Advanced generation settings\n\nNormal setup requires only a service, an API key or local server address, and a model. **Show advanced settings** exposes optional reasoning, request-context, page-image, coverage, OpenRouter-hosting, privacy, and prompt-preset controls. A context value limits what ostudy sends; it does not reconfigure the context size used to load a local model server.\n\nPrompt presets contain separate outline and card-generation instructions. They are sent to the selected provider with document requests. ostudy always preserves its source-trust boundary, structured output contract, native card validation, and bounded retries.\n''',
}.items():
    text = read(doc)
    if addition.strip().splitlines()[0] not in text:
        text += addition
    write(doc, text)
