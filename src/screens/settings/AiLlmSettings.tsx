import { useState, type ReactNode } from "react";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import type {
  AiCapability,
  AiCredentialStatus,
  AiProfileDraft,
  AiProviderKind,
  AiProviderModel,
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
  const profileDirty = hasProfileChanges(
    controller.draft,
    controller.selectedProfile,
    controller.selectedProfileId,
  );
  const providerSettingsDirty = hasProviderSettingsChanges(
    controller.draft,
    controller.selectedProfile,
    controller.selectedProfileId,
  );

  async function changeProfile(profileId: string): Promise<void> {
    if (profileId === controller.selectedProfileId) return;
    if (controller.dirty) {
      const accepted = await confirm({
        title: "Discard AI profile changes?",
        description: "Unsaved profile metadata and credential entry will be lost.",
        confirmLabel: "Discard changes",
        destructive: true,
      });
      if (!accepted) return;
      controller.discardDraft();
    }
    controller.selectProfile(profileId);
  }

  async function beginCreate(
    providerKind: Parameters<typeof controller.beginCreate>[0],
  ): Promise<void> {
    if (controller.dirty) {
      const accepted = await confirm({
        title: "Discard AI profile changes?",
        description: "Unsaved profile metadata and credential entry will be lost.",
        confirmLabel: "Discard changes",
        destructive: true,
      });
      if (!accepted) return;
      controller.discardDraft();
    }
    controller.beginCreate(providerKind);
  }

  async function deleteProfile(): Promise<void> {
    const profile = controller.selectedProfile;
    if (!profile) return;
    const accepted = await confirm({
      title: `Delete ${profile.displayName}?`,
      description:
        "The profile metadata, saved credential, task assignment, and verification history will be removed. Study content is unchanged.",
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
        <aside className={styles.aiSidebar} aria-label="AI provider profiles">
        <div className={styles.sectionHeading}>
          <div>
            <h2>AI provider profiles</h2>
            <p>One profile represents one trusted endpoint, model, and credential boundary.</p>
          </div>
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
            New profile
          </button>
        </div>
        {controller.loading ? <div className="inline-progress">Loading AI profiles…</div> : null}
        <div className={styles.profileList}>
          {controller.profiles.map((profile) => (
            <button
              key={profile.id}
              type="button"
              className={styles.profileListItem}
              data-selected={profile.id === controller.selectedProfileId}
              onClick={() => void changeProfile(profile.id)}
            >
              <span className={styles.profileTitleRow}>
                <strong>{profile.displayName}</strong>
                <span className={styles.scopeBadge} data-scope={profile.networkScope}>
                  {scopeLabel(profile.networkScope)}
                </span>
              </span>
              <small>{profile.providerLabel}</small>
              <small>{profile.modelId || "Model not selected"}</small>
              <span className={styles.statusText} data-status={profile.configurationStatus}>
                {configurationLabel(profile.configurationStatus)}
              </span>
            </button>
          ))}
          {!controller.loading && controller.profiles.length === 0 ? (
            <p className={styles.emptyCopy}>No AI provider profiles have been configured.</p>
          ) : null}
        </div>
        </aside>
      </div>

      <main className={styles.aiDetail}>
        {controller.draft ? (
          <>
            <ProfileEditor
              controller={controller}
              providerSettingsDirty={providerSettingsDirty}
              onProviderChange={controller.beginCreate}
              onDelete={() => void deleteProfile()}
            />
            {controller.selectedProfile ? (
              <>
                <CredentialPanel controller={controller} />
                <ModelSelectionPanel
                  controller={controller}
                  providerSettingsDirty={providerSettingsDirty}
                />
                <ConnectionPanel controller={controller} profileDirty={profileDirty} />
              </>
            ) : (
              <section className={styles.settingsCard}>
                <h2>Next: credential and model discovery</h2>
                <p className={styles.note}>
                  Create the trusted provider/server profile first. After that, enter any required API credential. ostudy will then query the provider for its available models instead of asking you to type a model ID.
                </p>
              </section>
            )}
          </>
        ) : (
          <section className={styles.emptyDetail}>
            <h2>Configure an AI provider</h2>
            <p>
              ostudy connects to servers you already run or cloud APIs you explicitly configure. It does not install model servers, models, Python, CUDA, or other AI infrastructure.
            </p>
          </section>
        )}
      </main>
    </div>
  );
}

function ProfileEditor({
  controller,
  providerSettingsDirty,
  onProviderChange,
  onDelete,
}: {
  controller: AiProfilesController;
  providerSettingsDirty: boolean;
  onProviderChange: (providerKind: AiProviderKind) => void;
  onDelete: () => void;
}) {
  const draft = controller.draft;
  if (!draft) return null;
  const provider = controller.catalog.find((entry) => entry.providerKind === draft.providerKind);
  const saved = controller.selectedProfile;
  const isLlama = draft.providerKind === "llama_cpp";
  const isOpenRouter = draft.providerKind === "openrouter";

  return (
    <section className={styles.settingsCard} aria-busy={controller.mutating}>
      <div className={styles.sectionHeading}>
        <div>
          <h2>1. Provider connection</h2>
          <p>{provider?.description}</p>
        </div>
        <span
          className={styles.scopeBadge}
          data-scope={saved?.networkScope ?? (isLlama ? "local" : "cloud")}
        >
          {scopeLabel(saved?.networkScope ?? (isLlama ? "local" : "cloud"))}
        </span>
      </div>

      <div className={styles.formGrid}>
        <label className={styles.stackedField}>
          <span>Profile name</span>
          <input
            value={draft.displayName}
            maxLength={100}
            onChange={(event) => controller.updateDraft({ displayName: event.currentTarget.value })}
          />
        </label>
        <label className={styles.stackedField}>
          <span>Provider</span>
          <select
            aria-label="Provider type"
            value={draft.providerKind}
            disabled={saved !== null || controller.mutating}
            onChange={(event) => onProviderChange(event.currentTarget.value as AiProviderKind)}
          >
            {controller.catalog
              .filter((entry) => entry.implementationStatus === "available")
              .map((entry) => (
                <option key={entry.providerKind} value={entry.providerKind}>
                  {entry.displayName}
                </option>
              ))}
          </select>
        </label>
        <label className={`${styles.stackedField} ${styles.fullWidth}`}>
          <span>Base URL</span>
          <input
            type="url"
            value={draft.baseUrl}
            maxLength={2048}
            readOnly={provider?.baseUrlEditable === false}
            onChange={(event) => controller.updateDraft({ baseUrl: event.currentTarget.value })}
          />
          {isLlama ? (
            <small className={styles.fieldHelp}>
              Enter the llama.cpp server root, for example <code>http://localhost:8080/</code>. ostudy uses the server's OpenAI-compatible <code>GET /v1/models</code> and <code>POST /v1/chat/completions</code> routes automatically.
            </small>
          ) : null}
        </label>
        {isLlama ? (
          <label className={styles.stackedField}>
            <span>Authentication</span>
            <select
              value={draft.authMode}
              onChange={(event) =>
                controller.updateDraft({
                  authMode: event.currentTarget.value as AiProfileDraft["authMode"],
                })
              }
            >
              <option value="none">None</option>
              <option value="bearer">Bearer token</option>
            </select>
          </label>
        ) : null}
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
          <small className={styles.fieldHelp}>
            During streamed document work this limits silence between provider events, not total generation time. Heartbeats, reasoning progress, output tokens, and final usage all keep an active request alive.
          </small>
        </label>
        <label className={styles.switchRow}>
          <span>
            <strong>Enable profile</strong>
            <small>Allow future AI tasks to select this profile.</small>
          </span>
          <input
            type="checkbox"
            checked={draft.enabled}
            onChange={(event) => controller.updateDraft({ enabled: event.currentTarget.checked })}
          />
        </label>
        {isLlama && draft.baseUrl.startsWith("http://") && !isLoopbackUrl(draft.baseUrl) ? (
          <label className={`${styles.switchRow} ${styles.fullWidth}`}>
            <span>
              <strong>Allow unencrypted LAN connection</strong>
              <small>HTTP traffic, prompts, and credentials can be read by devices on the network.</small>
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
        {isOpenRouter ? (
          <label className={`${styles.switchRow} ${styles.fullWidth}`}>
            <span>
              <strong>Require zero data retention</strong>
              <small>
                Restrict OpenRouter routing to endpoints that declare zero data retention.
                This can reduce endpoint availability.
              </small>
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
      </div>

      {!isLlama ? (
        <p className={styles.cloudNotice}>{cloudProviderNotice(draft.providerKind)}</p>
      ) : null}

      <div className={styles.actionRow}>
        <button
          type="button"
          className="primary-button"
          disabled={controller.mutating || !providerSettingsDirty}
          onClick={() => void controller.saveProfile()}
        >
          {controller.mutating ? "Saving…" : saved ? "Save provider settings" : "Create profile"}
        </button>
        {providerSettingsDirty && saved ? (
          <button type="button" className="secondary-button" onClick={controller.discardDraft}>
            Discard changes
          </button>
        ) : null}
        {saved ? (
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
    </section>
  );
}

function CredentialPanel({ controller }: { controller: AiProfilesController }) {
  const [credentialVisible, setCredentialVisible] = useState(false);
  const profile = controller.selectedProfile;
  if (!profile) return null;
  if (profile.authMode === "none") {
    return (
      <section className={styles.settingsCard}>
        <h2>2. API credential</h2>
        <p className={styles.note}>
          This server does not require authentication. ostudy can retrieve its available model list directly from the saved endpoint.
        </p>
      </section>
    );
  }
  const status = profile.credentialStatus;
  const hasCredential = status.status === "saved_securely" || status.status === "session_only";

  return (
    <section className={styles.settingsCard}>
      <div className={styles.sectionHeading}>
        <div>
          <h2>2. API credential</h2>
          <p>{credentialLabel(status.status)}</p>
        </div>
        <span className={styles.credentialBadge} data-status={status.status}>
          {credentialShortLabel(status.status)}
        </span>
      </div>
      <label className={styles.stackedField}>
        <span>{hasCredential ? "Replacement credential" : "API credential"}</span>
        <span className={aiStyles.credentialField}>
          <input
            type={credentialVisible ? "text" : "password"}
            autoComplete="off"
            value={controller.credentialDraft}
            placeholder={
              hasCredential ? "Enter a replacement" : "Credential is never readable after saving"
            }
            onChange={(event) => controller.setCredentialDraft(event.currentTarget.value)}
          />
          <button
            type="button"
            className={aiStyles.credentialVisibility}
            aria-label={credentialVisible ? "Hide API credential" : "Show API credential"}
            aria-pressed={credentialVisible}
            onClick={() => setCredentialVisible((current) => !current)}
          >
            <svg aria-hidden="true" viewBox="0 0 24 24">
              {credentialVisible ? (
                <>
                  <path d="M3 3l18 18" />
                  <path d="M10.6 10.6a2 2 0 002.8 2.8" />
                  <path d="M9.9 4.3A10.7 10.7 0 0112 4c5 0 8.7 4.1 9.7 5.4a1 1 0 010 1.2 17.8 17.8 0 01-3.1 3.3" />
                  <path d="M6.2 6.2A18.4 18.4 0 002.3 9.4a1 1 0 000 1.2C3.3 11.9 7 16 12 16c1 0 2-.2 2.9-.5" />
                </>
              ) : (
                <>
                  <path d="M2.3 9.4C3.3 8.1 7 4 12 4s8.7 4.1 9.7 5.4a1 1 0 010 1.2C20.7 11.9 17 16 12 16s-8.7-4.1-9.7-5.4a1 1 0 010-1.2z" />
                  <path d="M12 13a3 3 0 100-6 3 3 0 000 6z" />
                </>
              )}
            </svg>
          </button>
        </span>
      </label>
      <fieldset className={styles.persistenceOptions}>
        <legend>Storage</legend>
        <label>
          <input
            type="radio"
            name="credential-persistence"
            value="secure"
            checked={controller.credentialPersistence === "secure"}
            disabled={!status.securePersistenceAvailable}
            onChange={() => controller.setCredentialPersistence("secure")}
          />
          Save in the operating-system credential store
        </label>
        <label>
          <input
            type="radio"
            name="credential-persistence"
            value="session"
            checked={controller.credentialPersistence === "session"}
            disabled={!status.sessionOnlyAvailable}
            onChange={() => controller.setCredentialPersistence("session")}
          />
          Use only until ostudy closes
        </label>
      </fieldset>
      <div className={styles.actionRow}>
        <button
          type="button"
          className="primary-button"
          disabled={controller.mutating || controller.credentialDraft === ""}
          onClick={() => void controller.saveCredential()}
        >
          {hasCredential ? "Replace credential" : "Save credential"}
        </button>
        {hasCredential ? (
          <button
            type="button"
            className="secondary-button"
            disabled={controller.mutating}
            onClick={() => void controller.clearCredential()}
          >
            Remove credential
          </button>
        ) : null}
      </div>
    </section>
  );
}

function ModelSelectionPanel({
  controller,
  providerSettingsDirty,
}: {
  controller: AiProfilesController;
  providerSettingsDirty: boolean;
}) {
  const [modelSearch, setModelSearch] = useState("");
  const profile = controller.selectedProfile;
  const draft = controller.draft;
  if (!profile || !draft) return null;

  const credentialReady = canUseProviderCredential(profile);
  const unsavedCredential = controller.credentialDraft !== "";
  const canQuery = credentialReady && !providerSettingsDirty && !unsavedCredential;
  const modelAvailable = controller.models.some((model) => model.id === draft.modelId);
  const modelValue = modelAvailable ? draft.modelId : "";
  const modelDirty = draft.modelId !== profile.modelId;
  const savedModelMissing =
    controller.modelsLoaded &&
    profile.modelId !== "" &&
    !controller.models.some((model) => model.id === profile.modelId);
  const normalizedSearch = modelSearch.trim().toLocaleLowerCase();
  const matchingModels = controller.models.filter((model) => {
    if (!normalizedSearch) return true;
    return `${model.displayName} ${model.id} ${model.ownedBy ?? ""}`
      .toLocaleLowerCase()
      .includes(normalizedSearch);
  });

  return (
    <section className={styles.settingsCard} aria-busy={controller.refreshingModels}>
      <div className={styles.sectionHeading}>
        <div>
          <h2>3. Available model</h2>
          <p>
            Model IDs come from the provider API after endpoint and credential setup. You do not need to type a model name manually.
          </p>
        </div>
      </div>

      {!credentialReady ? (
        <div className="inline-alert info-alert">
          Save the API credential above before ostudy requests this provider's model list.
        </div>
      ) : null}
      {providerSettingsDirty ? (
        <div className="inline-alert info-alert">
          Save the provider/server changes above before retrieving models from the new endpoint.
        </div>
      ) : null}
      {unsavedCredential ? (
        <div className="inline-alert info-alert">
          Save or discard the credential entry above before retrieving models.
        </div>
      ) : null}

      {controller.refreshingModels ? (
        <div className="inline-progress">Retrieving available models from the provider…</div>
      ) : null}

      {controller.modelsLoaded && controller.models.length > 0 ? (
        <>
          {savedModelMissing ? (
            <div className="inline-alert info-alert">
              The previously saved model <strong>{profile.modelId}</strong> is not in the provider's current model list. Choose a model returned by the API.
            </div>
          ) : null}
          {controller.models.length > 50 ? (
            <label className={styles.stackedField}>
              <span>Search models</span>
              <input
                type="search"
                value={modelSearch}
                placeholder="Search model name, slug, or provider"
                onChange={(event) => setModelSearch(event.currentTarget.value)}
              />
            </label>
          ) : null}
          <label className={styles.stackedField}>
            <span>Model</span>
            <select
              aria-label="Model"
              value={modelValue}
              disabled={!canQuery || controller.mutating || controller.refreshingModels}
              onChange={(event) => controller.updateDraft({ modelId: event.currentTarget.value })}
            >
              <option value="">Select an available model</option>
              {matchingModels.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.displayName || model.id}
                  {model.displayName && model.displayName !== model.id ? ` — ${model.id}` : ""}
                </option>
              ))}
            </select>
          </label>
          <p className={styles.note}>
            {normalizedSearch
              ? `${matchingModels.length} of ${controller.models.length} models match this search.`
              : `${controller.models.length} model${controller.models.length === 1 ? "" : "s"} returned by the provider.`}
          </p>
          <div className={styles.actionRow}>
            <button
              type="button"
              className="primary-button"
              disabled={!modelDirty || !draft.modelId || controller.mutating || providerSettingsDirty}
              onClick={() => void controller.saveProfile()}
            >
              Save selected model
            </button>
            <button
              type="button"
              className="secondary-button"
              disabled={!canQuery || controller.refreshingModels || controller.mutating || modelDirty}
              onClick={() => void controller.refreshModels()}
            >
              Refresh models
            </button>
          </div>
        </>
      ) : controller.modelsLoaded && !controller.refreshingModels ? (
        <>
          <p className={styles.note}>The provider returned no selectable models.</p>
          <button
            type="button"
            className="secondary-button"
            disabled={!canQuery || controller.refreshingModels || controller.mutating}
            onClick={() => void controller.refreshModels()}
          >
            Retry model discovery
          </button>
        </>
      ) : !controller.refreshingModels ? (
        <>
          <p className={styles.note}>
            {canQuery
              ? "ostudy will request the available model list automatically. You can also request it again manually."
              : "Complete the preceding setup step before model discovery."}
          </p>
          {canQuery ? (
            <button
              type="button"
              className="secondary-button"
              disabled={controller.refreshingModels || controller.mutating}
              onClick={() => void controller.refreshModels()}
            >
              Retrieve models
            </button>
          ) : null}
        </>
      ) : null}
    </section>
  );
}

function ConnectionPanel({
  controller,
  profileDirty,
}: {
  controller: AiProfilesController;
  profileDirty: boolean;
}) {
  const profile = controller.selectedProfile;
  if (!profile) return null;
  const missingCredential = !canUseProviderCredential(profile);
  const missingModel = profile.modelId === "";
  const blocked =
    profileDirty || controller.credentialDraft !== "" || missingCredential || missingModel;
  return (
    <section className={styles.settingsCard}>
      <div className={styles.sectionHeading}>
        <div>
          <h2>4. Connection and capabilities</h2>
          <p>Tests run only when requested and use the saved endpoint, credential, and API-selected model.</p>
        </div>
        {controller.probeResult ? (
          <span className={styles.probeBadge} data-status={controller.probeResult.overallStatus}>
            {controller.probeResult.stale ? "Stale" : controller.probeResult.overallStatus}
          </span>
        ) : null}
      </div>
      {blocked ? (
        <div className="inline-alert info-alert">
          {missingCredential
            ? "Save the required API credential before testing."
            : missingModel
              ? "Retrieve and save an available model above before testing."
              : "Save or discard profile, model, and credential-entry changes before testing."}
        </div>
      ) : null}
      <div className={styles.actionRow}>
        <button
          type="button"
          className="primary-button"
          disabled={blocked || controller.testing || controller.refreshingModels}
          onClick={() => void controller.testProfile()}
        >
          {controller.testing ? "Testing…" : "Test connection"}
        </button>
      </div>
      {controller.probeResult ? (
        <ProbeResultView
          result={controller.probeResult}
          profile={profile}
          models={controller.models}
        />
      ) : (
        <p className={styles.note}>No verification result has been recorded for this profile.</p>
      )}
    </section>
  );
}

function ProbeResultView({
  result,
  profile,
  models,
}: {
  result: NonNullable<AiProfilesController["probeResult"]>;
  profile: AiProviderProfileDetail;
  models: AiProviderModel[];
}) {
  return (
    <div className={styles.probeResult}>
      <p className={styles.note}>
        Last verified {new Date(result.testedAt).toLocaleString()} using {result.testedModelId || "the saved model"}.
        {result.stale ? " The profile or credential changed after this test." : ""}
      </p>
      <div className={styles.phaseList}>
        {result.phases.map((phase) => (
          <article key={phase.phase} data-status={phase.status}>
            <span aria-hidden="true">
              {phase.status === "passed" ? "✓" : phase.status === "failed" ? "!" : "—"}
            </span>
            <div>
              <strong>{phase.label}</strong>
              <p>{phase.error?.message ?? phase.message}</p>
            </div>
            {phase.latencyMs !== null ? <small>{phase.latencyMs} ms</small> : null}
          </article>
        ))}
      </div>
      <div className={styles.capabilityGrid}>
        {result.capabilities.map((capability) => (
          <CapabilityBadge
            key={capability.key}
            capability={capability}
            providerKind={profile.providerKind}
          />
        ))}
      </div>
      {profile.providerKind === "openrouter" ? (
        <p className={aiStyles.capabilityNote}>
          OpenRouter profiles use strict JSON Schema requests with parameter-compatible routing. ostudy also requests endpoints that deny provider data collection and never enables OpenRouter's upstream-body debug echo.
        </p>
      ) : null}
      {profile.providerKind === "deepseek" ? (
        <p className={aiStyles.capabilityNote}>
          DeepSeek currently documents JSON Object output rather than provider-enforced JSON Schema. ostudy therefore validates DeepSeek structured output against its own schema locally; “Provider JSON Schema: not offered” is expected, not a connection failure.
        </p>
      ) : null}
      <section className={aiStyles.diagnosticLog} aria-label="Connection diagnostic log">
        <h3>Connection diagnostic log</h3>
        <p>
          Plaintext and redacted for troubleshooting. It records attempted routes, phase results, normalized error codes, and timing, but never credentials, Authorization headers, raw prompts, provider response text, model reasoning, or full chat templates.
        </p>
        <pre>{buildConnectionDiagnosticLog(result, profile, models)}</pre>
      </section>
    </div>
  );
}

function CapabilityBadge({
  capability,
  providerKind,
}: {
  capability: AiCapability;
  providerKind: AiProviderKind;
}) {
  const providerDoesNotOfferSchema =
    providerKind === "deepseek" && capability.key === "json_schema_output";
  const isNeutralUnsupported = capability.status === "unsupported";
  const statusLabel = providerDoesNotOfferSchema
    ? "not offered"
    : isNeutralUnsupported
      ? "not offered"
      : capability.status;
  const sourceLabel = providerDoesNotOfferSchema ? "provider" : capability.source;
  return (
    <span
      className={`${styles.capabilityBadge} ${isNeutralUnsupported ? aiStyles.neutralCapability : ""}`}
      data-status={isNeutralUnsupported ? "unknown" : capability.status}
    >
      <strong>{capabilityLabel(capability.key, providerKind)}</strong>
      <small>
        {statusLabel} · {sourceLabel}
      </small>
    </span>
  );
}

function buildConnectionDiagnosticLog(
  result: AiProviderProbeResult,
  profile: AiProviderProfileDetail,
  models: AiProviderModel[],
): string {
  const modelEndpoint =
    profile.providerKind === "llama_cpp"
      ? joinLlamaOpenAiDiagnosticEndpoint(profile.baseUrl, "models")
      : joinDiagnosticEndpoint(
          profile.baseUrl,
          profile.providerKind === "openrouter" ? "models/user" : "models",
        );
  const completionEndpoint =
    profile.providerKind === "llama_cpp"
      ? joinLlamaOpenAiDiagnosticEndpoint(profile.baseUrl, "chat/completions")
      : joinDiagnosticEndpoint(profile.baseUrl, "chat/completions");
  const structuredFormat =
    profile.providerKind === "llama_cpp"
      ? "json_schema"
      : profile.providerKind === "openrouter"
        ? "json_schema + require_parameters + data_collection=deny"
        : "json_object + local_schema_validation";
  const modelPreview = models.slice(0, 20).map((model) => `  - ${model.id}`);
  if (models.length > 20) {
    modelPreview.push(`  - ... ${models.length - 20} more model(s) omitted`);
  }
  if (modelPreview.length === 0) modelPreview.push("  - not loaded in this UI session");

  const lines = [
    "ostudy AI provider connection test",
    `tested_at: ${new Date(result.testedAt).toISOString()}`,
    `provider: ${profile.providerLabel} (${profile.providerKind})`,
    `endpoint: ${profile.baseUrl}`,
    `destination: ${scopeLabel(profile.networkScope)}`,
    `configured_model: ${profile.modelId || "<none>"}`,
    `tested_model: ${result.testedModelId || "<none>"}`,
    `profile_revision: ${result.profileRevision}`,
    `credential_epoch: ${result.credentialEpoch}`,
    `adapter_revision: ${result.adapterRevision}`,
    `result_schema_version: ${result.resultSchemaVersion}`,
    `overall_status: ${result.overallStatus}${result.stale ? " (stale)" : ""}`,
    "credential: available status only; secret value redacted",
    "",
    "attempts:",
    `  model_discovery: GET ${modelEndpoint}`,
  ];

  if (profile.providerKind === "llama_cpp") {
    lines.push(
      `  server_properties: GET ${joinLlamaRootDiagnosticEndpoint(
        profile.baseUrl,
        `props?model=${encodeURIComponent(result.testedModelId || profile.modelId)}`,
      )} (optional metadata)`,
    );
  }
  lines.push(
    `  text_completion: POST ${completionEndpoint} (stream=false; thinking/reasoning disabled for probe)`,
    `  structured_output: POST ${completionEndpoint} (response_format=${structuredFormat}; stream=false)`,
    "",
    "phase_results:",
  );
  for (const phase of result.phases) lines.push(formatDiagnosticPhase(phase));
  lines.push("", `loaded_models: ${models.length}`, ...modelPreview);
  return lines.join("\n");
}

function formatDiagnosticPhase(phase: AiProbePhase): string {
  const latency = phase.latencyMs === null ? "" : ` latency_ms=${phase.latencyMs}`;
  const error = phase.error
    ? ` error_code=${phase.error.code} retryable=${phase.error.retryable} message=${phase.error.message}`
    : "";
  return `  ${phase.phase}: ${phase.status}${latency}${error}`;
}

function joinDiagnosticEndpoint(baseUrl: string, path: string): string {
  try {
    return new URL(path, baseUrl).toString();
  } catch {
    return `${baseUrl}${path}`;
  }
}

function joinLlamaOpenAiDiagnosticEndpoint(baseUrl: string, suffix: string): string {
  try {
    const base = new URL(baseUrl);
    const includesV1 = base.pathname.replace(/\/+$/u, "").endsWith("/v1");
    return new URL(includesV1 ? suffix : `v1/${suffix}`, base).toString();
  } catch {
    return `${baseUrl}${baseUrl.endsWith("/v1/") ? suffix : `v1/${suffix}`}`;
  }
}

function joinLlamaRootDiagnosticEndpoint(baseUrl: string, suffix: string): string {
  try {
    const base = new URL(baseUrl);
    const includesV1 = base.pathname.replace(/\/+$/u, "").endsWith("/v1");
    return new URL(includesV1 ? `../${suffix}` : suffix, base).toString();
  } catch {
    return `${baseUrl}${suffix}`;
  }
}

function hasProfileChanges(
  draft: AiProfileDraft | null,
  selected: AiProviderProfileDetail | null,
  selectedId: string | null,
): boolean {
  if (!draft) return false;
  if (!selected) return selectedId?.startsWith("new:") === true;
  return JSON.stringify(draft) !== JSON.stringify(draftFromSelected(selected));
}

function hasProviderSettingsChanges(
  draft: AiProfileDraft | null,
  selected: AiProviderProfileDetail | null,
  selectedId: string | null,
): boolean {
  if (!draft) return false;
  if (!selected) return selectedId?.startsWith("new:") === true;
  return (
    draft.displayName !== selected.displayName ||
    draft.providerKind !== selected.providerKind ||
    draft.baseUrl !== selected.baseUrl ||
    draft.authMode !== selected.authMode ||
    draft.enabled !== selected.enabled ||
    draft.allowInsecureHttp !== selected.allowInsecureHttp ||
    (draft.requireZeroDataRetention ?? false) !==
      (selected.requireZeroDataRetention ?? false) ||
    draft.requestTimeoutMs !== selected.requestTimeoutMs
  );
}

function draftFromSelected(selected: AiProviderProfileDetail): AiProfileDraft {
  return {
    displayName: selected.displayName,
    providerKind: selected.providerKind,
    baseUrl: selected.baseUrl,
    modelId: selected.modelId,
    authMode: selected.authMode,
    enabled: selected.enabled,
    allowInsecureHttp: selected.allowInsecureHttp,
    requireZeroDataRetention: selected.requireZeroDataRetention ?? false,
    requestTimeoutMs: selected.requestTimeoutMs,
  };
}

function canUseProviderCredential(profile: AiProviderProfileDetail): boolean {
  if (profile.authMode === "none") return true;
  return (
    profile.credentialStatus.status === "saved_securely" ||
    profile.credentialStatus.status === "session_only"
  );
}

function cloudProviderNotice(providerKind: AiProviderKind): string {
  if (providerKind === "openrouter") {
    return "OpenRouter is a cloud routing provider. ostudy uses its fixed official API endpoint, requests only parameter-compatible endpoints for structured generation, denies data-collecting endpoints by default, and can require zero-data-retention routing per profile.";
  }
  if (providerKind === "deepseek") {
    return "DeepSeek is a cloud provider. ostudy uses DeepSeek's fixed official API endpoint.";
  }
  return "This cloud provider uses its fixed official API endpoint.";
}

function scopeLabel(scope: "local" | "lan" | "cloud"): string {
  return scope === "local" ? "Local" : scope === "lan" ? "LAN" : "Cloud";
}

function configurationLabel(status: string): string {
  return status === "ready"
    ? "Ready"
    : status === "disabled"
      ? "Disabled"
      : status === "planned"
        ? "Planned"
        : "Needs configuration";
}

function credentialLabel(status: AiCredentialStatus): string {
  switch (status) {
    case "saved_securely":
      return "A credential is stored in the operating-system credential store.";
    case "session_only":
      return "A credential is held only in native memory for this session.";
    case "secure_store_unavailable":
      return "Secure persistence is unavailable; use a session-only credential.";
    case "not_required":
      return "This profile does not require authentication.";
    default:
      return "No credential is available for this profile.";
  }
}

function credentialShortLabel(status: AiCredentialStatus): string {
  switch (status) {
    case "saved_securely":
      return "Saved securely";
    case "session_only":
      return "Session only";
    case "secure_store_unavailable":
      return "Secure store unavailable";
    case "not_required":
      return "Not required";
    default:
      return "Missing";
  }
}

function capabilityLabel(key: string, providerKind: AiProviderKind): string {
  const labels: Record<string, string> = {
    text_input: "Text",
    image_input: "Images",
    json_object_output: "JSON object",
    json_schema_output: providerKind === "deepseek" ? "Provider JSON Schema" : "JSON Schema",
    local_schema_validation: "Local validation",
    model_listing: "Model listing",
    usage_reporting: "Usage reporting",
    server_chat_template: "Server chat template",
    streaming: "Streaming",
    reasoning_control: "Reasoning control",
    embeddings: "Embeddings",
    context_window_tokens: "Context window",
  };
  return labels[key] ?? key;
}

function isLoopbackUrl(value: string): boolean {
  try {
    const hostname = new URL(value).hostname.toLowerCase();
    return (
      hostname === "localhost" ||
      hostname === "127.0.0.1" ||
      hostname === "[::1]" ||
      hostname === "::1"
    );
  } catch {
    return false;
  }
}
