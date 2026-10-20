import { Checkbox, Disclosure, RadioGroup } from "../../ui/primitives/FormControls";
import { InlineProgress } from "../../ui/primitives/InlineNotice";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Panel } from "../../ui/primitives/Panel";
import { useState, type ReactNode } from "react";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import type { AiProviderKind } from "../../ipc/aiProfiles";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { Icon } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { TextField } from "../../ui/primitives/TextField";



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
    <div className="tw:grid tw:min-w-0 tw:gap-8">
      {controller.errorMessage ? (
        <InlineNotice kind="error">{controller.errorMessage}</InlineNotice>
      ) : null}

      {defaultBlock}

      <Panel className="tw:min-w-0" aria-label="AI providers" title="AI providers"
        description="Manage API keys and local or remote provider connections."
        actions={(
          <Button
            type="button"
            variant="secondary"
            disabled={controller.mutating || availableProviders.length === 0}
            onClick={() => {
              const preferred =
                availableProviders.find((provider) => provider.providerKind === "llama_cpp") ??
                availableProviders[0];
              if (preferred) void beginCreate(preferred.providerKind);
            }}
          >
            Add provider
          </Button>
        )}>

        {controller.loading ? (
          <InlineProgress>Loading AI providers…</InlineProgress>
        ) : null}

        <div className="tw:grid tw:gap-3 tw:sm:grid-cols-2 tw:xl:grid-cols-3">
          {controller.profiles.map((profile) => (
            <Button
              key={profile.id}
              type="button"
              variant="ghost"
              className="tw:h-auto tw:min-h-24 tw:items-start tw:justify-start tw:whitespace-normal tw:rounded-md tw:border tw:border-solid tw:border-border tw:bg-background tw:p-4 tw:text-left tw:font-normal tw:transition-colors tw:hover:border-primary tw:data-[selected=true]:border-primary tw:data-[selected=true]:bg-accent"
              data-selected={profile.id === controller.selectedProfileId}
              onClick={() => void changeProfile(profile.id)}
            >
              <span className="tw:grid tw:min-w-0 tw:gap-1">
                <strong>{profile.displayName}</strong>
                <small>{profile.providerLabel}</small>
                <span className="tw:text-caption tw:font-medium tw:text-muted-foreground tw:data-[status=ready]:text-success tw:data-[status=incomplete]:text-warning" data-status={profile.configurationStatus}>
                  {configurationLabel(profile.configurationStatus)}
                </span>
              </span>
            </Button>
          ))}
          {!controller.loading && controller.profiles.length === 0 ? (
            <p className="tw:m-0 tw:text-label tw:font-normal tw:leading-relaxed tw:text-muted-foreground">No AI providers configured.</p>
          ) : null}
        </div>
      </Panel>

      <div className="tw:grid tw:min-w-0 tw:gap-5">
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
          <Panel className="tw:col-span-full" title="Add an AI provider"
            description="Choose a provider to configure a local server or API connection." />
        )}
      </div>
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
      <Panel className="tw:min-w-0" aria-busy={controller.mutating}
        title={profile ? `Configure ${profile.displayName}` : "Add AI provider"}
        description="Configure the provider connection and credential used by one or more AI models.">

        <div className="tw:grid tw:min-w-0 tw:gap-5 tw:lg:grid-cols-2">
          <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground">
            <span>Provider</span>
            <NativeSelect
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
            </NativeSelect>
          </label>

          {provider?.baseUrlEditable ? (
            <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground">
              <span>{isLlama ? "Server URL" : "API URL"}</span>
              <TextField
                type="url"
                value={draft.baseUrl}
                maxLength={2048}
                onChange={(event) => controller.updateDraft({ baseUrl: event.currentTarget.value })}
              />
            </label>
          ) : null}

          {requiresCredential ? (
            <div className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground">
              <label htmlFor="provider-credential">API key</label>
              <span className="tw:flex tw:min-w-0 tw:items-center tw:gap-2">
                <TextField
                  id="provider-credential"
                  type={credentialVisible ? "text" : "password"}
                  autoComplete="off"
                  value={controller.credentialDraft}
                  placeholder={hasCredential ? "Saved — enter only to replace" : "Enter API key"}
                  onChange={(event) => controller.setCredentialDraft(event.currentTarget.value)}
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="tw:shrink-0"
                  aria-label={credentialVisible ? "Hide API key" : "Show API key"}
                  aria-pressed={credentialVisible}
                  onClick={() => setCredentialVisible((current) => !current)}
                >
                  <Icon name={credentialVisible ? "view-off" : "view"} />
                </Button>
              </span>
            </div>
          ) : null}
        </div>

        <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-3">
          <Button
            type="button"
            variant="secondary"
            disabled={!canConnect || (!connectionDirty && controller.modelsLoaded)}
            onClick={() => void controller.connectSetup()}
          >
            {controller.mutating
              ? "Connecting…"
              : controller.modelsLoaded && !connectionDirty
                ? "Connected"
                : "Connect"}
          </Button>
          {profile && !connectionDirty ? (
            <Button
              type="button"
              variant="text"
              disabled={controller.refreshingModels || controller.dirty}
              onClick={() => void controller.refreshModels()}
            >
              {controller.refreshingModels ? "Refreshing…" : "Refresh model catalog"}
            </Button>
          ) : null}
          {canSaveProfile ? (
            <Button
              type="button"
              variant="primary"
              disabled={controller.mutating}
              onClick={() => void controller.saveProfile()}
            >
              Save provider
            </Button>
          ) : null}
        </div>
      </Panel>

      <Disclosure title="Advanced settings" className="tw:bg-card">
        
        <div className="tw:grid tw:gap-5 tw:pt-3 tw:lg:grid-cols-2">
          <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground">
            <span>Profile name</span>
            <TextField
              value={draft.displayName}
              maxLength={100}
              onChange={(event) => controller.updateDraft({ displayName: event.currentTarget.value })}
            />
          </label>

          <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground">
            <span>Provider inactivity timeout</span>
            <NativeSelect
              value={draft.requestTimeoutMs}
              onChange={(event) =>
                controller.updateDraft({ requestTimeoutMs: Number(event.currentTarget.value) })
              }
            >
              <option value={30_000}>30 seconds</option>
              <option value={60_000}>60 seconds</option>
              <option value={120_000}>2 minutes</option>
              <option value={300_000}>5 minutes</option>
            </NativeSelect>
          </label>

          <label className="tw:flex tw:min-h-11 tw:items-center tw:gap-3 tw:text-label tw:font-normal">
            <Checkbox
              checked={draft.enabled}
              onCheckedChange={(checked) => controller.updateDraft({ enabled: checked })}
            />
            <span>Enable this provider</span>
          </label>

          {isLlama ? (
            <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground">
              <span>Server authentication</span>
              <NativeSelect
                value={draft.authMode}
                onChange={(event) =>
                  controller.updateDraft({ authMode: event.currentTarget.value as "none" | "bearer" })
                }
              >
                <option value="none">None</option>
                <option value="bearer">API key</option>
              </NativeSelect>
            </label>
          ) : null}

          {isLlama && requiresInsecureLanOptIn(draft.baseUrl) ? (
            <label className="tw:flex tw:min-h-11 tw:items-center tw:gap-3 tw:text-label tw:font-normal">
              <Checkbox
                checked={draft.allowInsecureHttp}
                onCheckedChange={(checked) =>
                  controller.updateDraft({ allowInsecureHttp: checked })
                }
              />
              <span>Allow unencrypted LAN connection</span>
            </label>
          ) : null}

          {requiresCredential ? (
            <RadioGroup label="API key storage" name="credential-storage" value={controller.credentialPersistence}
              options={profile?.credentialStatus.securePersistenceAvailable !== false
                ? [{ value: "secure", label: "Save securely" }, { value: "session", label: "This session only" }]
                : [{ value: "session", label: "This session only" }]}
              onValueChange={controller.setCredentialPersistence} disabled={controller.mutating} />
          ) : null}

          {draft.providerKind === "openrouter" ? (
            <label className="tw:flex tw:min-h-11 tw:items-center tw:gap-3 tw:text-label tw:font-normal">
              <Checkbox
                checked={draft.requireZeroDataRetention ?? false}
                onCheckedChange={(checked) =>
                  controller.updateDraft({ requireZeroDataRetention: checked })
                }
              />
              <span>Require OpenRouter zero-data-retention routing</span>
            </label>
          ) : null}

          <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-3">
            {profile && requiresCredential && hasCredential ? (
              <Button
                type="button"
                variant="secondary"
                disabled={controller.mutating}
                onClick={() => void controller.clearCredential()}
              >
                Remove API key
              </Button>
            ) : null}
            {profile ? (
              <Button
                type="button"
                variant="danger"
                disabled={controller.mutating}
                onClick={onDelete}
              >
                Delete provider
              </Button>
            ) : null}
          </div>
        </div>
      </Disclosure>
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

function requiresInsecureLanOptIn(baseUrl: string): boolean {
  try {
    const url = new URL(baseUrl);
    if (url.protocol !== "http:") return false;
    const host = url.hostname.toLowerCase();
    return !["localhost", "127.0.0.1", "::1", "[::1]"].includes(host);
  } catch {
    return false;
  }
}

function configurationLabel(
  status: AiProfilesController["profiles"][number]["configurationStatus"],
): string {
  switch (status) {
    case "ready":
      return "Ready";
    case "incomplete":
      return "Needs API key";
    case "disabled":
      return "Disabled";
    case "planned":
      return "Planned";
  }
}
