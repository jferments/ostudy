import { useState, type ReactNode } from "react";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import type {
  AiCapability,
  AiProviderKind,
  AiProviderModel,
  AiProviderProfileDetail,
  AiProviderProbeResult,
  AiProbePhase,
  AiReasoningEffort,
} from "../../ipc/aiProfiles";
import { EmbeddingProfileSettings } from "./EmbeddingProfileSettings";
import { useConfirm } from "../../ui/overlays/overlayContext";
import aiStyles from "./AiLlmSettings.module.css";
import layoutStyles from "./AiModelsLayout.module.css";
import styles from "./Settings.module.css";
import { ownedClassNames, sharedClassMaps } from "../../ui/styles/classOwnership";

const ownedStyleMaps = sharedClassMaps;
const FALLBACK_REASONING_EFFORTS: Exclude<AiReasoningEffort, "automatic">[] = [
  "none",
  "minimal",
  "low",
  "medium",
  "high",
  "xhigh",
  "max",
];

interface AiLlmSettingsProps {
  controller: AiProfilesController;
  defaultBlock?: ReactNode;
}

export function AiLlmSettings({ controller, defaultBlock }: AiLlmSettingsProps) {
  const confirm = useConfirm();
  const availableProviders = controller.catalog.filter(
    (provider) => provider.implementationStatus === "available",
  );

  async function changeProfile(profileId: string): Promise<void> {
    if (profileId === controller.selectedProfileId) return;
    if (!(await confirmDiscardIfNeeded(controller, confirm))) return;
    controller.selectProfile(profileId);
  }

  async function beginCreate(providerKind: AiProviderKind): Promise<void> {
    if (!(await confirmDiscardIfNeeded(controller, confirm))) return;
    controller.beginCreate(providerKind);
  }

  async function deleteProfile(): Promise<void> {
    const profile = controller.selectedProfile;
    if (!profile) return;
    const accepted = await confirm({
      title: `Delete ${profile.displayName}?`,
      description:
        "The AI profile and its saved credential will be removed. Study content is unchanged.",
      confirmLabel: "Delete profile",
      destructive: true,
    });
    if (accepted) await controller.deleteProfile();
  }

  return (
    <div className={layoutStyles.layout}>
      {controller.errorMessage ? (
        <div
          className={ownedClassNames(
            ownedStyleMaps,
            `${styles.fullWidth} inline-alert error-alert`,
          )}
        >
          {controller.errorMessage}
        </div>
      ) : null}

      {defaultBlock}

      <section className={layoutStyles.modelListSection} aria-label="AI models">
        <div className={styles.sectionHeading}>
          <div>
            <h2>AI models</h2>
            <p>Choose a saved model, or add another local server or API connection.</p>
          </div>
          <button
            type="button"
            className={ownedClassNames(ownedStyleMaps, "secondary-button")}
            disabled={controller.mutating || availableProviders.length === 0}
            onClick={() => {
              const preferred =
                availableProviders.find((provider) => provider.providerKind === "llama_cpp") ??
                availableProviders[0];
              if (preferred) void beginCreate(preferred.providerKind);
            }}
          >
            Add model
          </button>
        </div>

        {controller.loading ? (
          <div className={ownedClassNames(ownedStyleMaps, "inline-progress")}>
            Loading AI models…
          </div>
        ) : null}

        <div className={layoutStyles.profileList}>
          {controller.profiles.map((profile) => (
            <button
              key={profile.id}
              type="button"
              className={layoutStyles.profileListItem}
              data-selected={profile.id === controller.selectedProfileId}
              onClick={() => void changeProfile(profile.id)}
            >
              <strong>{profile.displayName}</strong>
              <small>{profile.providerLabel}</small>
              <small>{profile.modelId || "Model not selected"}</small>
              <small>Reasoning: {reasoningLabel(profile.reasoningEffort ?? "low")}</small>
              <span className={layoutStyles.statusText} data-status={profile.configurationStatus}>
                {configurationLabel(profile.configurationStatus)}
              </span>
            </button>
          ))}
          {!controller.loading && controller.profiles.length === 0 ? (
            <p className={styles.emptyCopy}>No AI models configured.</p>
          ) : null}
        </div>
      </section>

      <main className={layoutStyles.detail}>
        {controller.draft ? (
          <>
          <ProfileSetup
            controller={controller}
            availableProviders={availableProviders.map((provider) => ({
              providerKind: provider.providerKind,
              displayName: provider.displayName,
            }))}
            onProviderChange={controller.beginCreate}
            onDelete={() => void deleteProfile()}
          />
          <EmbeddingProfileSettings controller={controller} />
          </>
        ) : (
          <section className={styles.emptyDetail}>
            <h2>Add an AI model</h2>
            <p>Choose a provider to configure a local server or API connection.</p>
          </section>
        )}
      </main>
    </div>
  );
}

function ProfileSetup({
  controller,
  availableProviders,
  onProviderChange,
  onDelete,
}: {
  controller: AiProfilesController;
  availableProviders: Array<{ providerKind: AiProviderKind; displayName: string }>;
  onProviderChange: (providerKind: AiProviderKind) => void;
  onDelete: () => void;
}) {
  const [credentialVisible, setCredentialVisible] = useState(false);
  const draft = controller.draft;
  if (!draft) return null;

  const profile = controller.selectedProfile;
  const provider = controller.catalog.find((entry) => entry.providerKind === draft.providerKind);
  const selectedModel =
    controller.models.find((candidate) => candidate.id === draft.modelId) ?? null;
  const supportedReasoningOptions = reasoningEffortsForModel(selectedModel);
  const reasoningValid = supportedReasoningOptions.includes(draft.reasoningEffort);
  const reasoningOptions = reasoningValid
    ? supportedReasoningOptions
    : [...supportedReasoningOptions, draft.reasoningEffort];
  const isLlama = draft.providerKind === "llama_cpp";
  const requiresCredential = draft.authMode !== "none";
  const hasCredential = profileHasCredential(profile);
  const connectionDirty =
    profile === null ||
    controller.credentialDraft !== "" ||
    draft.baseUrl !== profile.baseUrl ||
    draft.authMode !== profile.authMode ||
    draft.allowInsecureHttp !== profile.allowInsecureHttp ||
    (draft.requireZeroDataRetention ?? false) !== (profile.requireZeroDataRetention ?? false);
  const metadataDirty =
    profile !== null &&
    (draft.displayName !== profile.displayName ||
      draft.requestTimeoutMs !== profile.requestTimeoutMs);
  const modelDirty =
    profile !== null &&
    (draft.modelId !== profile.modelId ||
      draft.reasoningEffort !== (profile.reasoningEffort ?? "low"));
  const modelSaved = profile !== null && draft.modelId !== "" && !modelDirty;
  const canConnect =
    !controller.mutating &&
    draft.baseUrl.trim() !== "" &&
    (!requiresCredential || hasCredential || controller.credentialDraft.trim() !== "");
  const canSaveProfile =
    profile !== null &&
    !connectionDirty &&
    !controller.mutating &&
    reasoningValid &&
    (metadataDirty || modelDirty);

  return (
    <>
      <section className={styles.settingsCard} aria-busy={controller.mutating}>
        <div className={styles.sectionHeading}>
          <div>
            <h2>{profile ? `Configure ${profile.displayName}` : "Add AI model"}</h2>
            <p>Choose the service, connect it, then select the model you want ostudy to use.</p>
          </div>
        </div>

        <div className={styles.formGrid}>
          <label className={styles.stackedField}>
            <span>Provider</span>
            <select
              aria-label="Provider"
              value={draft.providerKind}
              disabled={profile !== null || controller.mutating}
              onChange={(event) => onProviderChange(event.currentTarget.value as AiProviderKind)}
            >
              {availableProviders.map((entry) => (
                <option key={entry.providerKind} value={entry.providerKind}>
                  {entry.displayName}
                </option>
              ))}
            </select>
          </label>

          {provider?.baseUrlEditable ? (
            <label className={styles.stackedField}>
              <span>{isLlama ? "Server URL" : "API URL"}</span>
              <input
                type="url"
                value={draft.baseUrl}
                maxLength={2048}
                onChange={(event) => controller.updateDraft({ baseUrl: event.currentTarget.value })}
              />
            </label>
          ) : null}

          {requiresCredential ? (
            <label
              className={ownedClassNames(
                ownedStyleMaps,
                `${styles.stackedField} ${provider?.baseUrlEditable ? styles.fullWidth : ""}`,
              )}
            >
              <span>API key</span>
              <span className={aiStyles.credentialField}>
                <input
                  type={credentialVisible ? "text" : "password"}
                  autoComplete="off"
                  value={controller.credentialDraft}
                  placeholder={hasCredential ? "Saved — enter only to replace" : "Enter API key"}
                  onChange={(event) => controller.setCredentialDraft(event.currentTarget.value)}
                />
                <button
                  type="button"
                  className={aiStyles.credentialVisibility}
                  aria-label={credentialVisible ? "Hide API key" : "Show API key"}
                  aria-pressed={credentialVisible}
                  onClick={() => setCredentialVisible((current) => !current)}
                >
                  <CredentialEye crossed={credentialVisible} />
                </button>
              </span>
            </label>
          ) : null}
        </div>

        <div className={styles.actionRow}>
          <button
            type="button"
            className={ownedClassNames(ownedStyleMaps, "secondary-button")}
            disabled={!canConnect || (!connectionDirty && controller.modelsLoaded)}
            onClick={() => void controller.connectSetup()}
          >
            {controller.mutating
              ? "Connecting…"
              : controller.modelsLoaded && !connectionDirty
                ? "Connected"
                : "Connect and load models"}
          </button>
          {profile && !connectionDirty ? (
            <button
              type="button"
              className={ownedClassNames(ownedStyleMaps, "text-button")}
              disabled={controller.refreshingModels || controller.dirty}
              onClick={() => void controller.refreshModels()}
            >
              {controller.refreshingModels ? "Refreshing…" : "Refresh models"}
            </button>
          ) : null}
        </div>
      </section>

      <section className={styles.settingsCard}>
        <div className={styles.sectionHeading}>
          <div>
            <h2>Model and reasoning</h2>
            <p>
              The reasoning preference belongs to this profile, so switching models cannot
              silently change another model's behavior.
            </p>
          </div>
        </div>

        <div className={styles.formGrid}>
          <label className={styles.stackedField}>
            <span>Model</span>
            <select
              aria-label="Model"
              value={draft.modelId}
              disabled={
                controller.refreshingModels ||
                connectionDirty ||
                !controller.modelsLoaded ||
                controller.models.length === 0
              }
              onChange={(event) => {
                const modelId = event.currentTarget.value;
                const model =
                  controller.models.find((candidate) => candidate.id === modelId) ?? null;
                controller.updateDraft({
                  modelId,
                  reasoningEffort: compatibleReasoningEffort(
                    draft.reasoningEffort,
                    model,
                  ),
                });
              }}
            >
              <option value="">
                {controller.refreshingModels
                  ? "Loading models…"
                  : !profile || connectionDirty
                    ? "Connect first"
                    : controller.models.length === 0
                      ? "No models reported"
                      : "Select a model"}
              </option>
              {draft.modelId && !controller.models.some((model) => model.id === draft.modelId) ? (
                <option value={draft.modelId}>{draft.modelId} — saved or embedding model</option>
              ) : null}
              {controller.models.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.displayName || model.id}
                </option>
              ))}
            </select>
          </label>

          <label className={styles.stackedField}>
            <span>Reasoning effort</span>
            <select
              aria-label="Reasoning effort"
              value={draft.reasoningEffort}
              disabled={connectionDirty || draft.modelId === ""}
              onChange={(event) =>
                controller.updateDraft({
                  reasoningEffort: event.currentTarget.value as AiReasoningEffort,
                })
              }
            >
              {reasoningOptions.map((effort) => (
                <option key={effort} value={effort}>
                  {reasoningOptionLabel(effort, selectedModel)}
                </option>
              ))}
            </select>
          </label>
        </div>

        <p className={styles.note}>{reasoningExplanation(selectedModel)}</p>
        {!reasoningValid ? (
          <p className={styles.cloudNotice}>
            The provider no longer advertises the saved effort for this model. Choose Automatic
            or another advertised effort before saving.
          </p>
        ) : null}

        <div className={styles.actionRow}>
          <button
            type="button"
            className={ownedClassNames(ownedStyleMaps, "primary-button")}
            disabled={!canSaveProfile}
            onClick={() => void controller.saveProfile()}
          >
            {controller.mutating ? "Saving…" : "Save profile"}
          </button>
          <button
            type="button"
            className={ownedClassNames(ownedStyleMaps, "secondary-button")}
            disabled={
              controller.testing ||
              controller.mutating ||
              profile === null ||
              !modelSaved ||
              controller.dirty
            }
            onClick={() => void controller.testProfile()}
          >
            {controller.testing ? "Testing…" : "Test connection"}
          </button>
        </div>

        {controller.probeResult ? <ProbeSummary result={controller.probeResult} /> : null}
      </section>

      <details className={aiStyles.advancedSettings}>
        <summary>Advanced settings</summary>
        <div className={aiStyles.advancedBody}>
          <label className={styles.stackedField}>
            <span>Profile name</span>
            <input
              value={draft.displayName}
              maxLength={100}
              onChange={(event) => controller.updateDraft({ displayName: event.currentTarget.value })}
            />
          </label>

          <label className={styles.stackedField}>
            <span>Provider inactivity timeout</span>
            <select
              value={draft.requestTimeoutMs}
              onChange={(event) =>
                controller.updateDraft({ requestTimeoutMs: Number(event.currentTarget.value) })
              }
            >
              <option value={30_000}>30 seconds</option>
              <option value={60_000}>60 seconds</option>
              <option value={120_000}>2 minutes</option>
              <option value={300_000}>5 minutes</option>
            </select>
          </label>

          {isLlama ? (
            <label className={styles.stackedField}>
              <span>Server authentication</span>
              <select
                value={draft.authMode}
                onChange={(event) =>
                  controller.updateDraft({
                    authMode: event.currentTarget.value as "none" | "bearer",
                  })
                }
              >
                <option value="none">None</option>
                <option value="bearer">API key</option>
              </select>
            </label>
          ) : null}

          {requiresCredential ? (
            <fieldset className={styles.persistenceOptions}>
              <legend>API key storage</legend>
              {profile?.credentialStatus.securePersistenceAvailable !== false ? (
                <label>
                  <input
                    type="radio"
                    name="credential-persistence"
                    value="secure"
                    checked={controller.credentialPersistence === "secure"}
                    onChange={() => controller.setCredentialPersistence("secure")}
                  />
                  Save securely on this device
                </label>
              ) : null}
              <label>
                <input
                  type="radio"
                  name="credential-persistence"
                  value="session"
                  checked={controller.credentialPersistence === "session"}
                  onChange={() => controller.setCredentialPersistence("session")}
                />
                Keep only for this app session
              </label>
            </fieldset>
          ) : null}

          {isLlama && draft.baseUrl.startsWith("http://") && !isLoopbackUrl(draft.baseUrl) ? (
            <label className={styles.switchRow}>
              <span>
                <strong>Allow unencrypted LAN connection</strong>
                <small>Required only for an HTTP server elsewhere on your private network.</small>
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
                <small>Restrict this profile to endpoints that declare zero data retention.</small>
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

          <div className={styles.actionRow}>
            {profile && hasCredential ? (
              <button
                type="button"
                className={ownedClassNames(ownedStyleMaps, "secondary-button")}
                disabled={controller.mutating}
                onClick={() => void controller.clearCredential()}
              >
                Remove saved API key
              </button>
            ) : null}
            {profile ? (
              <button
                type="button"
                className={ownedClassNames(ownedStyleMaps, "danger-button")}
                disabled={controller.mutating}
                onClick={onDelete}
              >
                Delete profile
              </button>
            ) : null}
          </div>
        </div>
      </details>
    </>
  );
}

function ProbeSummary({ result }: { result: AiProviderProbeResult }) {
  return (
    <div className={aiStyles.probeSummary} data-status={result.overallStatus}>
      <strong>{probeStatusLabel(result.overallStatus)}</strong>
      <span>{result.testedModelId || "Configured model"}</span>
      <details className={styles.probeDiagnostics}>
        <summary>Technical details</summary>
        <div className={styles.phaseList}>
          {result.phases.map((phase) => (
            <ProbePhase key={phase.phase} phase={phase} />
          ))}
        </div>
        {result.capabilities.length > 0 ? (
          <div className={styles.capabilityGrid}>
            {result.capabilities.map((capability) => (
              <Capability key={capability.key} capability={capability} />
            ))}
          </div>
        ) : null}
      </details>
    </div>
  );
}

function ProbePhase({ phase }: { phase: AiProbePhase }) {
  return (
    <article data-status={phase.status}>
      <span aria-hidden="true">
        {phase.status === "passed" ? "✓" : phase.status === "failed" ? "!" : "–"}
      </span>
      <div>
        <strong>{phase.label}</strong>
        <p>{phase.message}</p>
      </div>
      <small>{phase.latencyMs === null ? "" : `${phase.latencyMs} ms`}</small>
    </article>
  );
}

function Capability({ capability }: { capability: AiCapability }) {
  return (
    <span className={styles.capabilityBadge} data-status={capability.status}>
      {capabilityLabel(capability.key)}
      <small>{capability.status}</small>
    </span>
  );
}

async function confirmDiscardIfNeeded(
  controller: AiProfilesController,
  confirm: ReturnType<typeof useConfirm>,
): Promise<boolean> {
  if (!controller.dirty) return true;
  const accepted = await confirm({
    title: "Discard AI setup changes?",
    description:
      "Unsaved connection, API key entry, model, reasoning, or advanced settings will be lost.",
    confirmLabel: "Discard changes",
    destructive: true,
  });
  if (accepted) controller.discardDraft();
  return accepted;
}

function profileHasCredential(profile: AiProviderProfileDetail | null): boolean {
  return (
    profile?.credentialStatus.status === "saved_securely" ||
    profile?.credentialStatus.status === "session_only"
  );
}

function reasoningEffortsForModel(model: AiProviderModel | null): AiReasoningEffort[] {
  const advertised = model?.reasoning?.supportedEfforts ?? FALLBACK_REASONING_EFFORTS;
  const efforts = advertised.filter(
    (effort) => !(model?.reasoning?.mandatory && effort === "none"),
  );
  return [...new Set<AiReasoningEffort>(["automatic", ...efforts])];
}

function compatibleReasoningEffort(
  current: AiReasoningEffort,
  model: AiProviderModel | null,
): AiReasoningEffort {
  if (current === "automatic") return current;
  const reasoning = model?.reasoning;
  if (!reasoning) return current;
  if (reasoning.mandatory && current === "none") return "automatic";
  if (reasoning.supportedEfforts && !reasoning.supportedEfforts.includes(current)) {
    return "automatic";
  }
  return current;
}

function reasoningOptionLabel(
  effort: AiReasoningEffort,
  model: AiProviderModel | null,
): string {
  if (effort === "automatic") {
    const providerDefault = model?.reasoning?.defaultEffort;
    return providerDefault
      ? `Automatic — provider default ${reasoningLabel(providerDefault)}`
      : "Automatic";
  }
  return reasoningLabel(effort);
}

function reasoningExplanation(model: AiProviderModel | null): string {
  const reasoning = model?.reasoning;
  if (!reasoning) {
    return "Automatic uses provider-managed behavior. Profiles retain their own preference even when another model becomes the default.";
  }
  if (reasoning.mandatory) {
    return "Reasoning is mandatory for this model. ostudy will resolve Automatic or a stale unsupported value to the lowest advertised valid effort.";
  }
  return "Automatic follows this model's provider default. Explicit efforts are limited to values advertised by the current model catalog.";
}

function reasoningLabel(value: string): string {
  switch (value) {
    case "automatic":
      return "Automatic";
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

function configurationLabel(status: string): string {
  if (status === "ready") return "Ready";
  if (status === "disabled") return "Unavailable";
  if (status === "planned") return "Planned";
  return "Setup incomplete";
}

function probeStatusLabel(status: AiProviderProbeResult["overallStatus"]): string {
  if (status === "verified") return "Connection verified";
  if (status === "partial") return "Connection works with limitations";
  return "Connection test failed";
}

function capabilityLabel(key: string): string {
  return key
    .replaceAll("_", " ")
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function isLoopbackUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return (
      url.hostname === "localhost" ||
      url.hostname === "127.0.0.1" ||
      url.hostname === "[::1]" ||
      url.hostname === "::1"
    );
  } catch {
    return false;
  }
}

function CredentialEye({ crossed }: { crossed: boolean }) {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      {crossed ? (
        <>
          <path d="M3 3l18 18" />
          <path d="M10.6 10.6a2 2 0 002.8 2.8" />
          <path d="M9.9 4.3A10.7 10.7 0 0112 4c5 0 8.7 4.1 9.7 5.4a1 1 0 010 1.2 17.8 17.8 0 01-3.1 3.3" />
          <path d="M6.2 6.2A18.4 18.4 0 002.3 9.4a1 1 0 000 1.2C3.3 11.9 7 16 12 16c1 0 2-.2 2.9-.5" />
        </>
      ) : (
        <>
          <path d="M2.3 9.4C3.3 8.1 7 4 12 4s8.7 4.1 9.7 5.4a1 1 0 010 1.2C20.7 11.9 17 16 12 16s-8.7-4.1-9.7-5.4a1 1 0 010-1.2z" />
          <circle cx="12" cy="10" r="2.5" />
        </>
      )}
    </svg>
  );
}
