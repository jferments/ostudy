import { useEffect, useState, type ReactNode } from "react";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import type { AiProviderKind } from "../../ipc/aiProfiles";
import { HelpTrigger } from "../../ui/help/HelpProvider";
import { Icon } from "../../ui/icons/Icon";
import { Sheet } from "../../ui/overlays/Dialog";
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { Button } from "../../ui/primitives/Button";
import { Disclosure, RadioGroup } from "../../ui/primitives/FormControls";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { ListRow, ListRowMedia, ListRows } from "../../ui/primitives/ListRow";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { Panel } from "../../ui/primitives/Panel";
import { TextField } from "../../ui/primitives/TextField";
import { AiSetupWizard, type AiSetupService, type AiSetupStart } from "./AiSetupWizard";
import { SwitchRow } from "./SettingsControls";

interface AiLlmSettingsProps {
  controller: AiProfilesController;
  defaultBlock?: ReactNode;
  /** Opens the setup wizard; each new request object opens it again. */
  setupRequest?: AiSetupStart | null;
}

type Profile = AiProfilesController["profiles"][number];

const PROVIDER_MARKS: Record<AiProviderKind, string> = {
  claude: "Cl", deepseek: "DS", openrouter: "OR", llama_cpp: "Lc", ollama: "Ol", openai: "AI", gemini: "Ge",
};

const PROVIDER_BLURBS: Partial<Record<AiProviderKind, string>> = {
  claude: "Anthropic's API",
  deepseek: "DeepSeek's API",
  openrouter: "Many models, one key",
  llama_cpp: "A server on this computer or your network",
};

/** The AI section: the learner's services, then which service and model each task uses. */
export function AiLlmSettings({ controller, defaultBlock, setupRequest = null }: AiLlmSettingsProps) {
  const confirm = useConfirm();
  const [sheet, setSheet] = useState<"add" | "edit" | null>(null);
  const [setup, setSetup] = useState<AiSetupStart | null>(setupRequest);
  const availableProviders = controller.catalog.filter(
    (provider) => provider.implementationStatus === "available",
  );

  useEffect(() => {
    if (setupRequest) setSetup(setupRequest);
  }, [setupRequest]);

  function openProfile(profileId: string): void {
    controller.discardDraft();
    controller.selectProfile(profileId);
    setSheet("edit");
  }

  function openAdd(kind?: AiSetupService): void {
    controller.discardDraft();
    const preferred =
      availableProviders.find((provider) => provider.providerKind === (kind ?? "llama_cpp")) ?? availableProviders[0];
    if (!preferred) return;
    controller.beginCreate(preferred.providerKind);
    setSheet("add");
  }

  function closeSetup(): void {
    controller.discardDraft();
    setSetup(null);
  }

  const empty = !controller.loading && controller.profiles.length === 0;

  async function closeSheet(): Promise<void> {
    if (controller.dirty) {
      const accepted = await confirm({
        title: "Discard changes to this service?",
        description: "The changes and any API key you entered will be discarded.",
        confirmLabel: "Discard changes",
        destructive: true,
      });
      if (!accepted) return;
    }
    controller.discardDraft();
    setSheet(null);
  }

  async function removeProfile(profile: Pick<Profile, "id" | "displayName" | "revision">): Promise<void> {
    const accepted = await confirm({
      title: `Remove ${profile.displayName}?`,
      description: "Its connection and saved API key are removed. Your study content is unchanged.",
      confirmLabel: "Remove service",
      destructive: true,
    });
    if (!accepted) return;
    controller.discardDraft();
    await controller.deleteProfile(profile);
    setSheet(null);
  }

  return (
    <div className="tw:grid tw:min-w-0 tw:gap-5">
      {controller.errorMessage && sheet === null ? (
        <InlineNotice kind="error">{controller.errorMessage}</InlineNotice>
      ) : null}

      <Panel className="tw:min-w-0" title="Services" aria-busy={controller.loading}
        actions={<>
          <HelpTrigger topicId="settings.aiServices" label="Help with AI services" />
          {empty ? null : (
            <Button variant="secondary" disabled={controller.mutating || availableProviders.length === 0} onClick={() => openAdd()}>
              <Icon name="plus" size={16} />Add a service
            </Button>
          )}
        </>}>
        {controller.loading ? <InlineProgress>Loading services…</InlineProgress> : null}
        {empty ? (
          <div className="tw:grid tw:justify-items-center tw:gap-4 tw:px-(--card-gutter) tw:py-8 tw:text-center">
            <Icon name="sparkles" size={28} className="tw:text-primary" />
            <span className="tw:grid tw:gap-1">
              <strong className="tw:text-section tw:font-semibold">No AI service yet</strong>
              <span className="tw:text-label tw:text-muted-foreground">
                AI makes cards, study guides and lessons, and answers questions as your Tutor.
              </span>
            </span>
            <span className="tw:flex tw:flex-wrap tw:justify-center tw:gap-3">
              <Button variant="primary" disabled={controller.mutating} onClick={() => setSetup({})}>
                <Icon name="sparkles" size={16} />Set up AI
              </Button>
              <Button variant="secondary" disabled={controller.mutating || availableProviders.length === 0} onClick={() => openAdd()}>
                <Icon name="plus" size={16} />Add a service
              </Button>
            </span>
          </div>
        ) : null}
        {controller.profiles.length > 0 ? (
          <ListRows aria-label="Services">
            {controller.profiles.map((profile) => (
              <ListRow key={profile.id} className="tw:items-center"
                leading={<ListRowMedia label={PROVIDER_MARKS[profile.providerKind]} className="tw:normal-case" />}
                title={profile.displayName}
                meta={profileDetail(profile)}
                onActivate={() => openProfile(profile.id)}
                activateDisabled={controller.mutating}
                trailing={<>
                  <StatusDot status={profile.configurationStatus} />
                  <OverflowMenu label={`More for ${profile.displayName}`} items={[
                    { id: "edit", label: "Edit", onSelect: () => openProfile(profile.id) },
                    { id: "remove", label: "Remove", destructive: true, disabled: controller.mutating, onSelect: () => void removeProfile(profile) },
                  ]} />
                </>} />
            ))}
          </ListRows>
        ) : null}
      </Panel>

      {defaultBlock}

      {setup && !sheet ? (
        <AiSetupWizard
          controller={controller}
          start={setup}
          onClose={closeSetup}
          onAddService={(kind) => {
            setSetup(null);
            openAdd(kind);
          }}
        />
      ) : null}

      {sheet && controller.draft ? (
        <ServiceSheet
          controller={controller}
          adding={sheet === "add"}
          availableProviders={availableProviders.map((provider) => ({
            providerKind: provider.providerKind,
            displayName: provider.displayName,
          }))}
          onClose={() => void closeSheet()}
          onDone={() => setSheet(null)}
          onRemove={() => {
            const profile = controller.selectedProfile;
            if (profile) void removeProfile(profile);
          }}
        />
      ) : null}
    </div>
  );
}

function ServiceSheet({
  controller,
  adding,
  availableProviders,
  onClose,
  onDone,
  onRemove,
}: {
  controller: AiProfilesController;
  adding: boolean;
  availableProviders: Array<{ providerKind: AiProviderKind; displayName: string }>;
  onClose: () => void;
  onDone: () => void;
  onRemove: () => void;
}) {
  const [credentialVisible, setCredentialVisible] = useState(false);
  const draft = controller.draft;
  if (!draft) return null;

  const profile = controller.selectedProfile;
  const provider = controller.catalog.find((entry) => entry.providerKind === draft.providerKind);
  const isLlama = draft.providerKind === "llama_cpp";
  const requiresCredential = draft.authMode !== "none";
  const hasCredential = profileHasCredential(profile);
  const secureAvailable = profile?.credentialStatus.securePersistenceAvailable !== false;
  const connectionDirty =
    profile === null ||
    controller.credentialDraft !== "" ||
    draft.baseUrl !== profile.baseUrl ||
    draft.authMode !== profile.authMode ||
    draft.allowInsecureHttp !== profile.allowInsecureHttp ||
    (draft.requireZeroDataRetention ?? false) !== (profile.requireZeroDataRetention ?? false);
  const canConnect =
    !controller.mutating &&
    draft.baseUrl.trim() !== "" &&
    (!requiresCredential || hasCredential || controller.credentialDraft.trim() !== "");
  const title = adding ? "Add a service" : (profile?.displayName ?? draft.displayName);

  async function submit(): Promise<void> {
    const ok = connectionDirty ? await controller.connectSetup() : controller.dirty ? await controller.saveProfile() : true;
    if (ok) onDone();
  }

  return (
    <Sheet title={title} width="small" busy={controller.mutating} onClose={onClose}
      actions={<>
        {profile ? <Button variant="dangerGhost" className="tw:me-auto" disabled={controller.mutating} onClick={onRemove}>Remove service</Button> : null}
        <Button variant="secondary" disabled={controller.mutating} onClick={onClose}>Cancel</Button>
        <Button variant="primary" disabled={connectionDirty ? !canConnect : controller.mutating} onClick={() => void submit()}>
          {controller.mutating ? "Connecting…" : connectionDirty ? "Connect" : "Save"}
        </Button>
      </>}>
      <div className="tw:grid tw:min-w-0 tw:gap-5">
        {controller.errorMessage ? <InlineNotice kind="error">{controller.errorMessage}</InlineNotice> : null}

        {adding ? (
          <RadioGroup label="Service" name="ai-service" value={draft.providerKind}
            disabled={controller.mutating}
            onValueChange={(providerKind) => controller.beginCreate(providerKind)}
            options={availableProviders.map((entry) => ({
              value: entry.providerKind,
              ariaLabel: entry.displayName,
              className: "tw:rounded-md tw:border tw:border-solid tw:border-border tw:px-3 tw:has-[[data-checked]]:border-primary tw:has-[[data-checked]]:bg-accent",
              label: <span className="tw:flex tw:items-center tw:gap-3 tw:py-2">
                <ListRowMedia label={PROVIDER_MARKS[entry.providerKind]} className="tw:normal-case" />
                <span className="tw:grid tw:min-w-0">
                  <strong className="tw:font-medium">{entry.displayName}</strong>
                  {PROVIDER_BLURBS[entry.providerKind] ? <span className="tw:text-label tw:text-muted-foreground">{PROVIDER_BLURBS[entry.providerKind]}</span> : null}
                </span>
              </span>,
            }))} />
        ) : profile ? (
          <div className="tw:flex tw:items-center tw:gap-3">
            <ListRowMedia label={PROVIDER_MARKS[profile.providerKind]} className="tw:normal-case" />
            <span className="tw:grid tw:min-w-0 tw:flex-1">
              <strong className="tw:font-medium">{profile.providerLabel}</strong>
              <span className="tw:text-label tw:text-muted-foreground">
                {controller.modelsLoaded ? `Connected · ${controller.models.length} ${controller.models.length === 1 ? "model" : "models"}` : profileDetail(profile)}
              </span>
            </span>
            <StatusDot status={profile.configurationStatus} />
          </div>
        ) : null}

        {!adding ? (
          <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground">
            <span>Name</span>
            <TextField value={draft.displayName} maxLength={100}
              onChange={(event) => controller.updateDraft({ displayName: event.currentTarget.value })} />
          </label>
        ) : null}

        {provider?.baseUrlEditable ? (
          <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground">
            <span>{isLlama ? "Server address" : "API address"}</span>
            <TextField type="url" value={draft.baseUrl} maxLength={2048}
              onChange={(event) => controller.updateDraft({ baseUrl: event.currentTarget.value })} />
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
              <Button type="button" variant="ghost" size="icon" className="tw:shrink-0"
                aria-label={credentialVisible ? "Hide API key" : "Show API key"}
                aria-pressed={credentialVisible}
                onClick={() => setCredentialVisible((current) => !current)}>
                <Icon name={credentialVisible ? "view-off" : "view"} />
              </Button>
            </span>
          </div>
        ) : null}

        {requiresCredential && secureAvailable ? (
          <SwitchRow label="Keep the key on this device" checked={controller.credentialPersistence === "secure"}
            onChange={(checked) => controller.setCredentialPersistence(checked ? "secure" : "session")} />
        ) : null}

        {profile && !connectionDirty ? (
          <Button variant="text" className="tw:justify-self-start" disabled={controller.refreshingModels || controller.dirty}
            onClick={() => void controller.refreshModels()}>
            {controller.refreshingModels ? "Refreshing…" : "Refresh models"}
          </Button>
        ) : null}

        <Disclosure title="More options">
          <div className="tw:grid tw:gap-1 tw:px-4 tw:pb-3">
            {adding ? (
              <label className="tw:grid tw:min-w-0 tw:gap-2 tw:pb-2 tw:text-label tw:font-medium tw:text-foreground">
                <span>Name</span>
                <TextField value={draft.displayName} maxLength={100}
                  onChange={(event) => controller.updateDraft({ displayName: event.currentTarget.value })} />
              </label>
            ) : null}
            <SwitchRow label="Use this service" checked={draft.enabled}
              onChange={(enabled) => controller.updateDraft({ enabled })} />
            <label className="tw:flex tw:min-h-12 tw:items-center tw:justify-between tw:gap-6 tw:text-body tw:font-medium">
              <span>Give up after</span>
              <NativeSelect className="tw:w-40" value={draft.requestTimeoutMs}
                onChange={(event) => controller.updateDraft({ requestTimeoutMs: Number(event.currentTarget.value) })}>
                <option value={30_000}>30 seconds</option>
                <option value={60_000}>60 seconds</option>
                <option value={120_000}>2 minutes</option>
                <option value={300_000}>5 minutes</option>
              </NativeSelect>
            </label>
            {isLlama ? (
              <SwitchRow label="Server needs a key" checked={draft.authMode === "bearer"}
                onChange={(checked) => controller.updateDraft({ authMode: checked ? "bearer" : "none" })} />
            ) : null}
            {isLlama && requiresInsecureLanOptIn(draft.baseUrl) ? (
              <SwitchRow label="Allow an unencrypted network connection" checked={draft.allowInsecureHttp}
                onChange={(allowInsecureHttp) => controller.updateDraft({ allowInsecureHttp })} />
            ) : null}
            {draft.providerKind === "openrouter" ? (
              <SwitchRow label="Only zero-data-retention routes" checked={draft.requireZeroDataRetention ?? false}
                onChange={(requireZeroDataRetention) => controller.updateDraft({ requireZeroDataRetention })} />
            ) : null}
            {profile && requiresCredential && hasCredential ? (
              <Button variant="secondary" className="tw:mt-2 tw:justify-self-start" disabled={controller.mutating}
                onClick={() => void controller.clearCredential()}>
                Remove API key
              </Button>
            ) : null}
          </div>
        </Disclosure>
      </div>
    </Sheet>
  );
}

function StatusDot({ status }: { status: Profile["configurationStatus"] }) {
  return (
    <span data-status={status}
      className="tw:inline-flex tw:items-center tw:gap-1.5 tw:whitespace-nowrap tw:text-label tw:text-muted-foreground tw:data-[status=ready]:text-success tw:data-[status=incomplete]:text-warning">
      <span aria-hidden="true" className="tw:size-2 tw:rounded-full tw:bg-current" />
      {configurationLabel(status)}
    </span>
  );
}

function profileDetail(profile: Profile): string {
  if (profile.credentialStatus.status === "saved_securely") return "Key saved";
  if (profile.credentialStatus.status === "session_only") return "Key kept for this session";
  if (profile.credentialStatus.status === "missing") return "No key yet";
  return profile.providerLabel;
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

function configurationLabel(status: Profile["configurationStatus"]): string {
  switch (status) {
    case "ready":
      return "Ready";
    case "incomplete":
      return "Needs a key";
    case "disabled":
      return "Off";
    case "planned":
      return "Planned";
  }
}
