import { useState, type ReactNode } from "react";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import type {
  AiCapability,
  AiProviderKind,
  AiProviderProfileDetail,
  AiProviderProbeResult,
  AiProbePhase,
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
    <div className={styles.aiLayout}>
      {controller.errorMessage ? (
        <div className={`${styles.fullWidth} inline-alert error-alert`}>{controller.errorMessage}</div>
      ) : null}

      <div className={styles.aiSidebarStack}>
        {sidebarTop}
        <aside className={styles.aiSidebar} aria-label="AI profiles">
          <div className={styles.sectionHeading}>
            <h2>AI models</h2>
          </div>
          <div className={styles.addProviderGrid}>
            <button
              type="button"
              className="secondary-button"
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
          {controller.loading ? <div className="inline-progress">Loading AI models…</div> : null}
          <div className={styles.profileList}>
            {controller.profiles.map((profile) => (
              <button
                key={profile.id}
                type="button"
                className={styles.profileListItem}
                data-selected={profile.id === controller.selectedProfileId}
                onClick={() => void changeProfile(profile.id)}
              >
                <strong>{profile.displayName}</strong>
                <small>{profile.providerLabel}</small>
                <small>{profile.modelId || "Model not selected"}</small>
                <span className={styles.statusText} data-status={profile.configurationStatus}>
                  {configurationLabel(profile.configurationStatus)}
                </span>
              </button>
            ))}
            {!controller.loading && controller.profiles.length === 0 ? (
              <p className={styles.emptyCopy}>No AI models configured.</p>
            ) : null}
          </div>
        </aside>
      </div>

      <main className={styles.aiDetail}>
        {controller.draft ? (
          <ProfileSetup
            controller={controller}
            availableProviders={availableProviders.map((provider) => ({
              providerKind: provider.providerKind,
              displayName: provider.displayName,
            }))}
            onProviderChange={controller.beginCreate}
            onDelete={() => void deleteProfile()}
          />
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
    (draft.displayName !== profile.displayName || draft.requestTimeoutMs !== profile.requestTimeoutMs);
  const modelDirty = profile !== null && draft.modelId !== profile.modelId;
  const modelSaved = profile !== null && draft.modelId !== "" && !modelDirty;
  const canConnect =
    !controller.mutating &&
    draft.baseUrl.trim() !== "" &&
    (!requiresCredential || hasCredential || controller.credentialDraft.trim() !== "");
  const canSaveProfile =
    profile !== null &&
    !connectionDirty &&
    !controller.mutating &&
    (metadataDirty || modelDirty);

  return (
    <>
      <section className={styles.settingsCard} aria-busy={controller.mutating}>
        <div className={styles.sectionHeading}>
          <div>
            <h2>{profile ? "AI model" : "Add AI model"}</h2>
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
              className={`${styles.stackedField} ${provider?.baseUrlEditable ? styles.fullWidth : ""}`}
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
            className="secondary-button"
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
              className="text-button"
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
            <h2>Model</h2>
            <p>Select a model reported by the configured provider.</p>
          </div>
        </div>

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
            onChange={(event) => controller.updateDraft({ modelId: event.currentTarget.value })}
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
            {controller.models.map((model) => (
              <option key={model.id} value={model.id}>
                {model.displayName || model.id}
              </option>
            ))}
          </select>
        </label>

        <div className={styles.actionRow}>
          <button
            type="button"
            className="primary-button"
            disabled={!canSaveProfile}
            onClick={() => void controller.saveProfile()}
          >
            {controller.mutating ? "Saving…" : "Save profile"}
          </button>
          <button
            type="button"
            className="secondary-button"
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
                  controller.updateDraft({ authMode: event.currentTarget.value as "none" | "bearer" })
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
                className="secondary-button"
                disabled={controller.mutating}
                onClick={() => void controller.clearCredential()}
              >
                Remove saved API key
              </button>
            ) : null}
            {profile ? (
              <button
                type="button"
                className="danger-button"
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
    description: "Unsaved connection, API key entry, model, or advanced settings will be lost.",
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
    return url.hostname === "localhost" || url.hostname === "127.0.0.1" || url.hostname === "::1";
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
