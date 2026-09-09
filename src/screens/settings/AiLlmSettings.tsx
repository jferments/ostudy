import { useState, type ReactNode } from "react";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import type { AiProviderKind } from "../../ipc/aiProfiles";
import { useConfirm } from "../../ui/overlays/overlayContext";
import aiStyles from "./AiLlmSettings.module.css";
import layoutStyles from "./AiModelsLayout.module.css";
import styles from "./Settings.module.css";
import { ownedClassNames, sharedClassMaps } from "../../ui/styles/classOwnership";

const ownedStyleMaps = sharedClassMaps;

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
        "The provider connection and its saved credential will be removed. Study content is unchanged.",
      confirmLabel: "Delete provider",
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

      <section className={layoutStyles.modelListSection} aria-label="AI providers">
        <div className={styles.sectionHeading}>
          <div>
            <h2>AI providers</h2>
            <p>Manage API keys and local or remote provider connections.</p>
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
            Add provider
          </button>
        </div>

        {controller.loading ? (
          <div className={ownedClassNames(ownedStyleMaps, "inline-progress")}>
            Loading AI providers…
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
              <span className={layoutStyles.statusText} data-status={profile.configurationStatus}>
                {configurationLabel(profile.configurationStatus)}
              </span>
            </button>
          ))}
          {!controller.loading && controller.profiles.length === 0 ? (
            <p className={styles.emptyCopy}>No AI providers configured.</p>
          ) : null}
        </div>
      </section>

      <main className={layoutStyles.detail}>
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
            <h2>Add an AI provider</h2>
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
    (draft.displayName !== profile.displayName ||
      draft.requestTimeoutMs !== profile.requestTimeoutMs ||
      draft.enabled !== profile.enabled);
  const canConnect =
    !controller.mutating &&
    draft.baseUrl.trim() !== "" &&
    (!requiresCredential || hasCredential || controller.credentialDraft.trim() !== "");
  const canSaveProfile = profile !== null && !connectionDirty && !controller.mutating && metadataDirty;

  return (
    <>
      <section className={styles.settingsCard} aria-busy={controller.mutating}>
        <div className={styles.sectionHeading}>
          <div>
            <h2>{profile ? `Configure ${profile.displayName}` : "Add AI provider"}</h2>
            <p>Configure the provider connection and credential used by one or more AI models.</p>
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
                : "Connect"}
          </button>
          {profile && !connectionDirty ? (
            <button
              type="button"
              className={ownedClassNames(ownedStyleMaps, "text-button")}
              disabled={controller.refreshingModels || controller.dirty}
              onClick={() => void controller.refreshModels()}
            >
              {controller.refreshingModels ? "Refreshing…" : "Refresh model catalog"}
            </button>
          ) : null}
          {canSaveProfile ? (
            <button
              type="button"
              className={ownedClassNames(ownedStyleMaps, "primary-button")}
              disabled={controller.mutating}
              onClick={() => void controller.saveProfile()}
            >
              Save provider
            </button>
          ) : null}
        </div>
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

          <label className={styles.checkboxField}>
            <input
              type="checkbox"
              checked={draft.enabled}
              onChange={(event) => controller.updateDraft({ enabled: event.currentTarget.checked })}
            />
            <span>Enable this provider</span>
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
                    name="credential-storage"
                    value="secure"
                    checked={controller.credentialPersistence === "secure"}
                    onChange={() => controller.setCredentialPersistence("secure")}
                  />
                  Save securely
                </label>
              ) : null}
              <label>
                <input
                  type="radio"
                  name="credential-storage"
                  value="session"
                  checked={controller.credentialPersistence === "session"}
                  onChange={() => controller.setCredentialPersistence("session")}
                />
                This session only
              </label>
            </fieldset>
          ) : null}

          {draft.providerKind === "openrouter" ? (
            <label className={styles.checkboxField}>
              <input
                type="checkbox"
                checked={draft.requireZeroDataRetention ?? false}
                onChange={(event) =>
                  controller.updateDraft({ requireZeroDataRetention: event.currentTarget.checked })
                }
              />
              <span>Require OpenRouter zero-data-retention routing</span>
            </label>
          ) : null}

          <div className={styles.actionRow}>
            {profile && requiresCredential && hasCredential ? (
              <button
                type="button"
                className={ownedClassNames(ownedStyleMaps, "secondary-button")}
                disabled={controller.mutating}
                onClick={() => void controller.clearCredential()}
              >
                Remove API key
              </button>
            ) : null}
            {profile ? (
              <button
                type="button"
                className={ownedClassNames(ownedStyleMaps, "danger-button")}
                disabled={controller.mutating}
                onClick={onDelete}
              >
                Delete provider
              </button>
            ) : null}
          </div>
        </div>
      </details>
    </>
  );
}

async function confirmDiscardIfNeeded(
  controller: AiProfilesController,
  confirm: ReturnType<typeof useConfirm>,
): Promise<boolean> {
  if (!controller.dirty) return true;
  const accepted = await confirm({
    title: "Discard unsaved provider changes?",
    description: "The unsaved provider settings and API-key entry will be discarded.",
    confirmLabel: "Discard changes",
    destructive: true,
  });
  if (accepted) controller.discardDraft();
  return accepted;
}

function profileHasCredential(profile: AiProfilesController["selectedProfile"]): boolean {
  return Boolean(
    profile &&
      (profile.credentialStatus.status === "saved_securely" ||
        profile.credentialStatus.status === "session_only"),
  );
}

function configurationLabel(status: AiProfilesController["profiles"][number]["configurationStatus"]): string {
  switch (status) {
    case "ready": return "Ready";
    case "incomplete": return "Needs API key";
    case "disabled": return "Disabled";
    case "planned": return "Planned";
  }
}

function CredentialEye({ crossed }: { crossed: boolean }) {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M2.4 12s3.5-6 9.6-6 9.6 6 9.6 6-3.5 6-9.6 6-9.6-6-9.6-6Z" />
      <circle cx="12" cy="12" r="2.8" />
      {crossed ? <path d="M4 4 20 20" /> : null}
    </svg>
  );
}
