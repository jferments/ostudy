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
import styles from "./Settings.module.css";

interface AiLlmSettingsProps {
  controller: AiProfilesController;
}

export function AiLlmSettings({ controller }: AiLlmSettingsProps) {
  const confirm = useConfirm();
  const availableProviders = controller.catalog.filter(
    (provider) => provider.implementationStatus === "available",
  );
  const plannedProviders = controller.catalog.filter(
    (provider) => provider.implementationStatus === "planned",
  );
  const profileDirty = hasProfileChanges(
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

      <aside className={styles.aiSidebar} aria-label="AI provider profiles">
        <div className={styles.sectionHeading}>
          <div>
            <h2>Provider profiles</h2>
            <p>One profile represents one trusted endpoint, model, and credential boundary.</p>
          </div>
        </div>
        <div className={styles.addProviderGrid}>
          {availableProviders.map((provider) => (
            <button
              key={provider.providerKind}
              type="button"
              className="secondary-button"
              disabled={controller.mutating}
              onClick={() => void beginCreate(provider.providerKind)}
            >
              Add {provider.providerKind === "llama_cpp" ? "llama.cpp" : provider.displayName}
            </button>
          ))}
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

      <main className={styles.aiDetail}>
        {controller.draft ? (
          <>
            <ProfileEditor
              controller={controller}
              profileDirty={profileDirty}
              onDelete={() => void deleteProfile()}
            />
            {controller.selectedProfile ? (
              <>
                <CredentialPanel controller={controller} />
                <ConnectionPanel controller={controller} profileDirty={profileDirty} />
              </>
            ) : (
              <section className={styles.settingsCard}>
                <h2>Credential and connection tests</h2>
                <p className={styles.note}>
                  Save the profile metadata first. Credentials, model discovery, and connection verification are attached only to a saved profile identity.
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

        <section className={styles.settingsCard}>
          <h2>Default task profile</h2>
          <label className={styles.stackedField}>
            <span>Flashcard generation</span>
            <select
              value={controller.defaults.flashcardGenerationProfileId ?? ""}
              disabled={controller.mutating}
              onChange={(event) =>
                void controller.setFlashcardDefault(event.currentTarget.value || null)
              }
            >
              <option value="">Choose each time</option>
              {controller.profiles
                .filter((profile) => profile.configurationStatus !== "planned")
                .map((profile) => (
                  <option key={profile.id} value={profile.id}>
                    {profile.displayName}
                    {profile.configurationStatus === "ready" ? "" : " — incomplete"}
                  </option>
                ))}
            </select>
          </label>
          <p className={styles.securityNote}>
            There is no automatic fallback. A failed local profile will never silently send study material to a cloud provider.
          </p>
        </section>

        <section className={styles.settingsCard}>
          <h2>Planned providers</h2>
          <div className={styles.plannedProviderGrid}>
            {plannedProviders.map((provider) => (
              <article key={provider.providerKind}>
                <strong>{provider.displayName}</strong>
                <p>{provider.description}</p>
              </article>
            ))}
          </div>
        </section>
      </main>
    </div>
  );
}

function ProfileEditor({
  controller,
  profileDirty,
  onDelete,
}: {
  controller: AiProfilesController;
  profileDirty: boolean;
  onDelete: () => void;
}) {
  const draft = controller.draft;
  if (!draft) return null;
  const provider = controller.catalog.find((entry) => entry.providerKind === draft.providerKind);
  const saved = controller.selectedProfile;
  const isLlama = draft.providerKind === "llama_cpp";
  const currentModelInList = controller.models.some((model) => model.id === draft.modelId);
  const modelSelectionDisabled = !saved || controller.refreshingModels || controller.mutating;

  return (
    <section className={styles.settingsCard} aria-busy={controller.mutating}>
      <div className={styles.sectionHeading}>
        <div>
          <h2>{saved ? "Profile configuration" : "New provider profile"}</h2>
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
          <input value={provider?.displayName ?? draft.providerKind} readOnly />
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
        </label>
        <label className={styles.stackedField}>
          <span>Model</span>
          <select
            aria-label="Model"
            value={draft.modelId}
            disabled={modelSelectionDisabled}
            onChange={(event) => controller.updateDraft({ modelId: event.currentTarget.value })}
          >
            <option value="">
              {saved ? "Refresh models to load available choices" : "Create profile, then refresh models"}
            </option>
            {draft.modelId && !currentModelInList ? (
              <option value={draft.modelId}>{draft.modelId} — saved selection</option>
            ) : null}
            {controller.models.map((model) => (
              <option key={model.id} value={model.id}>
                {model.displayName || model.id}
              </option>
            ))}
          </select>
          <small className={styles.fieldHelp}>
            {saved
              ? "Model choices come from this provider's model-list API. Use Refresh models below to update the list."
              : "Create the trusted provider profile first; then ostudy can query that endpoint for its available models."}
          </small>
          {isLlama ? (
            <small className={styles.fieldHelp}>
              llama.cpp may expose a GGUF path as the API model ID unless the server is started with <code>--alias</code>.
            </small>
          ) : null}
        </label>
        <label className={styles.stackedField}>
          <span>Request timeout</span>
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
      </div>

      {isLlama ? (
        <p className={styles.securityNote}>
          llama.cpp applies the chat template embedded with the loaded model. For an Unsloth-exported GGUF, serve the model with its matching template; ostudy does not replace it with a hand-written template.
        </p>
      ) : (
        <p className={styles.cloudNotice}>
          DeepSeek is a cloud provider. Connection tests send a small prompt in non-thinking mode so the bounded probe measures final-answer generation instead of spending its token budget on reasoning.
        </p>
      )}

      <div className={styles.actionRow}>
        <button
          type="button"
          className="primary-button"
          disabled={controller.mutating || !profileDirty}
          onClick={() => void controller.saveProfile()}
        >
          {controller.mutating ? "Saving…" : saved ? "Save profile" : "Create profile"}
        </button>
        {profileDirty ? (
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
  const profile = controller.selectedProfile;
  if (!profile || profile.authMode === "none") {
    return (
      <section className={styles.settingsCard}>
        <h2>Credential</h2>
        <p className={styles.note}>This profile is configured without authentication.</p>
      </section>
    );
  }
  const status = profile.credentialStatus;
  const hasCredential = status.status === "saved_securely" || status.status === "session_only";

  return (
    <section className={styles.settingsCard}>
      <div className={styles.sectionHeading}>
        <div>
          <h2>Credential</h2>
          <p>{credentialLabel(status.status)}</p>
        </div>
        <span className={styles.credentialBadge} data-status={status.status}>
          {credentialShortLabel(status.status)}
        </span>
      </div>
      <label className={styles.stackedField}>
        <span>{hasCredential ? "Replacement credential" : "API credential"}</span>
        <input
          type="password"
          autoComplete="off"
          value={controller.credentialDraft}
          placeholder={hasCredential ? "Enter a replacement" : "Credential is never readable after saving"}
          onChange={(event) => controller.setCredentialDraft(event.currentTarget.value)}
        />
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
      <p className={styles.securityNote}>
        The credential never enters SQLite, React state after submission, diagnostics, `.studydeck` files, or `.ostudybackup` archives.
      </p>
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

function ConnectionPanel({
  controller,
  profileDirty,
}: {
  controller: AiProfilesController;
  profileDirty: boolean;
}) {
  const profile = controller.selectedProfile;
  if (!profile) return null;
  const blocked = profileDirty || controller.credentialDraft !== "";
  return (
    <section className={styles.settingsCard}>
      <div className={styles.sectionHeading}>
        <div>
          <h2>Connection and capabilities</h2>
          <p>Tests run only when requested and use the saved profile configuration.</p>
        </div>
        {controller.probeResult ? (
          <span className={styles.probeBadge} data-status={controller.probeResult.overallStatus}>
            {controller.probeResult.stale ? "Stale" : controller.probeResult.overallStatus}
          </span>
        ) : null}
      </div>
      {blocked ? (
        <div className="inline-alert info-alert">
          Save or discard profile and credential-entry changes before testing.
        </div>
      ) : null}
      <div className={styles.actionRow}>
        <button
          type="button"
          className="secondary-button"
          disabled={blocked || controller.refreshingModels || controller.testing}
          onClick={() => void controller.refreshModels()}
        >
          {controller.refreshingModels ? "Refreshing…" : "Refresh models"}
        </button>
        <button
          type="button"
          className="primary-button"
          disabled={blocked || controller.testing || controller.refreshingModels}
          onClick={() => void controller.testProfile()}
        >
          {controller.testing ? "Testing…" : "Test connection"}
        </button>
      </div>
      {controller.models.length > 0 ? (
        <p className={styles.note}>
          {controller.models.length} available model(s) returned by the provider. Select one in Profile configuration and save before testing a different model.
        </p>
      ) : (
        <p className={styles.note}>Refresh models to query this provider for selectable model IDs.</p>
      )}
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
      {profile.providerKind === "deepseek" ? (
        <p className={styles.capabilityNote}>
          DeepSeek currently documents JSON Object output rather than provider-enforced JSON Schema. ostudy therefore validates DeepSeek structured output against its own schema locally; “Provider JSON Schema: not offered” is expected, not a connection failure.
        </p>
      ) : null}
      <section className={styles.diagnosticLog} aria-label="Connection diagnostic log">
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
  const statusLabel = providerDoesNotOfferSchema
    ? "not offered"
    : capability.status === "unsupported"
      ? "not offered"
      : capability.status;
  const sourceLabel = providerDoesNotOfferSchema ? "provider" : capability.source;
  return (
    <span className={styles.capabilityBadge} data-status={capability.status}>
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
  const completionPath = profile.providerKind === "llama_cpp" ? "v1/chat/completions" : "chat/completions";
  const modelPath = "models";
  const structuredFormat = profile.providerKind === "llama_cpp" ? "json_schema" : "json_object + local_schema_validation";
  const modelPreview = models.slice(0, 20).map((model) => `  - ${model.id}`);
  if (models.length > 20) modelPreview.push(`  - ... ${models.length - 20} more model(s) omitted`);
  if (modelPreview.length === 0) modelPreview.push("  - not refreshed in this UI session");

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
    `  model_discovery: GET ${joinDiagnosticEndpoint(profile.baseUrl, modelPath)}`,
  ];

  if (profile.providerKind === "llama_cpp") {
    lines.push(
      `  server_properties: GET ${joinDiagnosticEndpoint(profile.baseUrl, `props?model=${encodeURIComponent(result.testedModelId || profile.modelId)}`)} (optional metadata)`,
    );
  }
  lines.push(
    `  text_completion: POST ${joinDiagnosticEndpoint(profile.baseUrl, completionPath)} (stream=false; thinking/reasoning disabled for probe)`,
    `  structured_output: POST ${joinDiagnosticEndpoint(profile.baseUrl, completionPath)} (response_format=${structuredFormat}; stream=false)`,
    "",
    "phase_results:",
  );
  for (const phase of result.phases) lines.push(formatDiagnosticPhase(phase));
  lines.push("", `refreshed_models: ${models.length}`, ...modelPreview);
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

function hasProfileChanges(
  draft: AiProfileDraft | null,
  selected: AiProviderProfileDetail | null,
  selectedId: string | null,
): boolean {
  if (!draft) return false;
  if (!selected) return selectedId?.startsWith("new:") === true;
  return JSON.stringify(draft) !== JSON.stringify({
    displayName: selected.displayName,
    providerKind: selected.providerKind,
    baseUrl: selected.baseUrl,
    modelId: selected.modelId,
    authMode: selected.authMode,
    enabled: selected.enabled,
    allowInsecureHttp: selected.allowInsecureHttp,
    requestTimeoutMs: selected.requestTimeoutMs,
  });
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
