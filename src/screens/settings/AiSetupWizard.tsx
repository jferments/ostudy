import { useEffect, useState, type ReactNode } from "react";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import { getErrorMessage } from "../../ipc/common";
import { loadVoice } from "../lessons/voiceReady";
import { openExternalUrl } from "../../platform/externalLinks";
import { usePlatformCapabilities } from "../../platform/PlatformProvider";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { Dialog } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { RadioGroup } from "../../ui/primitives/FormControls";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { ListRowMedia } from "../../ui/primitives/ListRow";
import { TextField } from "../../ui/primitives/TextField";

/** The services the wizard walks through; llama.cpp hands over to the Add a service sheet. */
export type AiSetupService = "openrouter" | "claude" | "deepseek" | "llama_cpp";
type HostedService = Exclude<AiSetupService, "llama_cpp">;

/** Where the wizard starts: at the choice of service, or straight at one service's account step. */
export interface AiSetupStart {
  service?: HostedService;
}

type Step = "choose" | "cost" | "account" | "connect" | "ready" | "custom";
const NUMBERED: ReadonlyArray<Step> = ["choose", "cost", "account", "connect", "ready"];

interface ServiceGuide {
  name: string;
  mark: string;
  home: string;
  keys: string;
  account: ReadonlyArray<{ text: ReactNode; detail?: string }>;
  keySteps: ReadonlyArray<ReactNode>;
  keyPlaceholder: string;
}

const GUIDES: Record<HostedService, ServiceGuide> = {
  openrouter: {
    name: "OpenRouter",
    mark: "OR",
    home: "https://openrouter.ai/",
    keys: "https://openrouter.ai/settings/keys",
    account: [
      { text: <>Open <b>openrouter.ai</b> and choose <b>Sign up</b>.</>, detail: "Google, GitHub or an email address all work." },
      { text: <>Open <b>Credits</b> from the account menu and choose <b>Add credits</b>.</>, detail: "$10 is plenty to start. Card payments add a 5.5% fee (at least $0.80)." },
      { text: <>Leave <b>Auto top-up</b> off.</>, detail: "You decide when to add more." },
      { text: "Come back here." },
    ],
    keySteps: [
      <>Open <b>Settings → API Keys</b> on OpenRouter and choose <b>Create API key</b>.</>,
      <>Name it <b>oStudy</b>, set a <b>credit limit</b> of $5 that <b>resets monthly</b>, and leave <b>expiration</b> off. Then choose <b>Create</b>.</>,
      "Copy the key. OpenRouter shows it only once.",
    ],
    keyPlaceholder: "sk-or-v1-…",
  },
  claude: {
    name: "Claude",
    mark: "Cl",
    home: "https://platform.claude.com/",
    keys: "https://platform.claude.com/settings/keys",
    account: [
      { text: <>Open <b>platform.claude.com</b> and sign up.</>, detail: "This is Anthropic's developer console, separate from a Claude.ai plan." },
      { text: <>Open <b>Billing</b> and buy credits.</>, detail: "$5 is enough to start." },
      { text: "Come back here." },
    ],
    keySteps: [
      <>Open <b>Settings → API keys</b> and choose <b>Create key</b>.</>,
      <>Name it <b>oStudy</b>, then copy the key. It is shown only once.</>,
    ],
    keyPlaceholder: "sk-ant-…",
  },
  deepseek: {
    name: "DeepSeek",
    mark: "DS",
    home: "https://platform.deepseek.com/",
    keys: "https://platform.deepseek.com/api_keys",
    account: [
      { text: <>Open <b>platform.deepseek.com</b> and sign up.</> },
      { text: <>Open <b>Top up</b> and add credit.</>, detail: "$2 lasts a long time for cards and guides." },
      { text: "Come back here." },
    ],
    keySteps: [
      <>Open <b>API keys</b> and choose <b>Create new API key</b>.</>,
      <>Name it <b>oStudy</b>, then copy the key. It is shown only once.</>,
    ],
    keyPlaceholder: "sk-…",
  },
};

const CHOICES: ReadonlyArray<{ value: AiSetupService; mark: string; name: string; detail: string }> = [
  { value: "openrouter", mark: "OR", name: "OpenRouter", detail: "Hundreds of models through one account" },
  { value: "deepseek", mark: "DS", name: "DeepSeek", detail: "Text only" },
  { value: "claude", mark: "Cl", name: "Claude", detail: "Text and images" },
  { value: "llama_cpp", mark: "Lc", name: "A server on this computer", detail: "llama.cpp, free, needs a strong computer" },
];

const OPENROUTER_PROS = [
  "Simplest setup",
  "Lowest price",
  "Everything you need in one place: text, voice, images and search",
  "Widest variety of models to choose from",
];

const COSTS: ReadonlyArray<{ icon: IconName; text: string; price: string }> = [
  { icon: "study-guide", text: "Study guide and cards from a 300-page textbook", price: "$0.10–0.80" },
  { icon: "listen", text: "A 20-minute lesson read aloud", price: "$0.04" },
  { icon: "chat", text: "An evening of questions to the Tutor", price: "under $0.05" },
  { icon: "search", text: "Making a whole library searchable", price: "under $0.01" },
];

const TASK_SERVICES: ReadonlyArray<{ icon: IconName; task: string; who: string }> = [
  { icon: "study-guide", task: "Cards and guides", who: "Any service" },
  { icon: "chat", task: "Tutor", who: "Any service" },
  { icon: "search", task: "Search", who: "OpenRouter or a server on this computer" },
  { icon: "listen", task: "Voice", who: "OpenRouter" },
  { icon: "image", task: "Images in documents", who: "The Cards and guides model, when it reads images" },
];

/**
 * Walks a learner through their first AI service: which one, what it costs,
 * making the account and key, and the models chosen for them. The native
 * side seeds the first key's task defaults; this only reads them back.
 */
export function AiSetupWizard({
  controller,
  start,
  onAddService,
  onClose,
}: {
  controller: AiProfilesController;
  start: AiSetupStart;
  /** Opens the Add a service sheet, for services the wizard does not walk through. */
  onAddService: (service?: AiSetupService) => void;
  onClose: () => void;
}) {
  const [step, setStep] = useState<Step>(start.service ? "account" : "choose");
  const [service, setService] = useState<AiSetupService>(start.service ?? "openrouter");
  const [keyVisible, setKeyVisible] = useState(false);
  const [pasting, setPasting] = useState(false);
  // One-click sign-in needs the browser to return to oStudy; elsewhere the key is pasted.
  const browserSignIn = usePlatformCapabilities().native?.services.browserSignIn?.state === "available";
  const [voice, setVoice] = useState<Awaited<ReturnType<typeof loadVoice>> | null>(null);
  const [voiceError, setVoiceError] = useState<string | null>(null);
  const hosted = service === "llama_cpp" ? null : GUIDES[service];

  useEffect(() => {
    if (step !== "ready") return;
    let current = true;
    loadVoice().then(
      (value) => {
        if (current) setVoice(value);
      },
      (failure: unknown) => {
        if (current) setVoiceError(getErrorMessage(failure));
      },
    );
    return () => {
      current = false;
    };
  }, [step]);

  function goTo(next: Step): void {
    if (next === "connect" && service !== "llama_cpp") {
      controller.discardDraft();
      controller.beginCreate(service);
    }
    setStep(next);
  }

  function leave(next: () => void): void {
    controller.cancelSignIn();
    next();
  }

  function addOpenRouter(): void {
    setService("openrouter");
    setPasting(false);
    setVoice(null);
    setStep("account");
  }

  async function connect(): Promise<void> {
    if (await controller.connectSetup()) setStep("ready");
  }

  async function signIn(): Promise<void> {
    if (await controller.signInWithOpenRouter(openExternalUrl)) setStep("ready");
  }

  const number = NUMBERED.indexOf(step);
  const title = step === "custom" ? "Choose a service for each task" : "Set up AI";
  const busy = controller.mutating;
  const back = (to: Step) => (
    <Button variant="secondary" className="tw:me-auto" disabled={busy} onClick={() => leave(() => setStep(to))}>Back</Button>
  );

  let body: ReactNode;
  let actions: ReactNode;
  switch (step) {
    case "choose":
      body = (
        <>
          <RadioGroup label="AI service" name="ai-setup-service" value={service} onValueChange={setService}
            options={CHOICES.map((choice) => ({
              value: choice.value,
              ariaLabel: choice.name,
              className: "tw:rounded-lg tw:border tw:border-solid tw:border-border tw:px-4 tw:py-2 tw:has-[[data-checked]]:border-primary tw:has-[[data-checked]]:bg-accent",
              label: (
                <span className="tw:grid tw:gap-2 tw:py-1">
                  <span className="tw:flex tw:items-center tw:gap-3">
                    <ListRowMedia label={choice.mark} className="tw:normal-case" />
                    <span className="tw:grid tw:min-w-0 tw:flex-1">
                      <strong className="tw:font-medium">{choice.name}</strong>
                      <span className="tw:text-label tw:text-muted-foreground">{choice.detail}</span>
                    </span>
                    {choice.value === "openrouter" ? (
                      <span className="tw:rounded-full tw:bg-primary tw:px-2.5 tw:py-0.5 tw:text-caption tw:font-medium tw:text-primary-foreground">Recommended</span>
                    ) : null}
                  </span>
                  {choice.value === "openrouter" ? (
                    <ul aria-label="Why OpenRouter" className="tw:m-0 tw:grid tw:list-none tw:gap-1.5 tw:p-0 tw:ps-13">
                      {OPENROUTER_PROS.map((pro) => (
                        <li key={pro} className="tw:flex tw:items-start tw:gap-2 tw:text-label">
                          <Icon name="check" size={16} className="tw:mt-0.5 tw:shrink-0 tw:text-success" />{pro}
                        </li>
                      ))}
                    </ul>
                  ) : null}
                </span>
              ),
            }))} />
          <Button variant="text" className="tw:justify-self-start" onClick={() => setStep("custom")}>
            Choose a service for each task instead
          </Button>
        </>
      );
      actions = (
        <>
          <Button variant="secondary" className="tw:me-auto" onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={() => {
            if (service === "llama_cpp") onAddService("llama_cpp");
            else setStep("cost");
          }}>Next</Button>
        </>
      );
      break;
    case "cost":
      body = (
        <>
          <Heading>You pay only for what you use</Heading>
          <p className="tw:m-0 tw:grid tw:gap-1">
            <strong className="tw:text-section tw:font-semibold">A few dollars a month</strong>
            <span className="tw:text-label tw:text-muted-foreground">at most, for most learners</span>
          </p>
          <ul aria-label="Typical costs" className="tw:m-0 tw:grid tw:list-none tw:gap-0 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:p-0">
            {COSTS.map((cost) => (
              <li key={cost.text} className="tw:flex tw:items-center tw:gap-3 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:px-4 tw:py-3 tw:first:border-t-0">
                <Icon name={cost.icon} size={18} className="tw:shrink-0 tw:text-muted-foreground" />
                <span className="tw:min-w-0 tw:flex-1">{cost.text}</span>
                <span className="tw:whitespace-nowrap tw:font-medium">{cost.price}</span>
              </li>
            ))}
          </ul>
          <div className="tw:grid tw:grid-cols-2 tw:gap-3">
            <p className="tw:m-0 tw:grid tw:gap-1 tw:rounded-lg tw:border tw:border-solid tw:border-primary tw:bg-accent tw:p-3">
              <span className="tw:text-label tw:text-muted-foreground">oStudy with OpenRouter</span>
              <strong className="tw:font-semibold">About $1–3 / month</strong>
            </p>
            <p className="tw:m-0 tw:grid tw:gap-1 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:p-3">
              <span className="tw:text-label tw:text-muted-foreground">Quizlet Plus</span>
              <strong className="tw:font-semibold">$7.99 / month</strong>
            </p>
          </div>
          <p className="tw:m-0 tw:text-label tw:text-muted-foreground">
            $10 of credit lasts most learners several months. Unused credit stays in your account.
          </p>
        </>
      );
      actions = <>{back("choose")}<Button variant="primary" onClick={() => setStep("account")}>Next</Button></>;
      break;
    case "account":
      body = hosted ? (
        <>
          <Heading>Create your {hosted.name} account</Heading>
          <ol className="tw:m-0 tw:grid tw:gap-4 tw:ps-6">
            {hosted.account.map((item, index) => (
              <li key={index} className="tw:grid tw:gap-1">
                <span>{item.text}</span>
                {item.detail ? <span className="tw:text-label tw:text-muted-foreground">{item.detail}</span> : null}
                {index === 0 ? <ExternalButton url={hosted.home}>Open {hosted.home.replace(/^https:\/\/|\/$/gu, "")}</ExternalButton> : null}
              </li>
            ))}
          </ol>
        </>
      ) : null;
      actions = (
        <>
          {start.service ? <Button variant="secondary" className="tw:me-auto" onClick={onClose}>Cancel</Button> : back("cost")}
          <Button variant="primary" onClick={() => goTo("connect")}>Next</Button>
        </>
      );
      break;
    case "connect":
      if (service === "openrouter" && browserSignIn && !pasting) {
        body = (
          <>
            <Heading>Connect oStudy to OpenRouter</Heading>
            <div className="tw:grid tw:gap-2">
              <Button variant="primary" className="tw:min-h-13 tw:text-body" disabled={busy || controller.signingIn} onClick={() => void signIn()}>
                <Icon name="link" size={18} />Connect with OpenRouter
              </Button>
              <span className="tw:text-center tw:text-label tw:text-muted-foreground">
                Opens OpenRouter in your browser to approve a key for oStudy.
              </span>
            </div>
            {controller.signingIn ? (
              <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3">
                <InlineProgress>Waiting for you to approve oStudy in your browser…</InlineProgress>
                <Button variant="secondary" size="small" onClick={controller.cancelSignIn}>Stop waiting</Button>
              </div>
            ) : null}
            {busy && !controller.signingIn ? <InlineProgress>Connecting…</InlineProgress> : null}
            {controller.errorMessage ? <InlineNotice kind="error">{controller.errorMessage}</InlineNotice> : null}
            <p className="tw:m-0 tw:text-label tw:text-muted-foreground">
              You can set a credit limit for the key later under Settings → API Keys on OpenRouter.
            </p>
            <Button variant="text" className="tw:justify-self-start" disabled={busy}
              onClick={() => leave(() => setPasting(true))}>
              Paste a key instead
            </Button>
          </>
        );
        actions = back("account");
        break;
      }
      body = hosted ? (
        <>
          <Heading>Paste {hosted.name === "OpenRouter" ? "an" : "a"} {hosted.name} key</Heading>
          <ol className="tw:m-0 tw:grid tw:gap-4 tw:ps-6">
            {hosted.keySteps.map((item, index) => (
              <li key={index} className="tw:grid tw:gap-2">
                <span>{item}</span>
                {index === 0 ? <ExternalButton url={hosted.keys}>Open API keys</ExternalButton> : null}
              </li>
            ))}
          </ol>
          {controller.errorMessage ? <InlineNotice kind="error">{controller.errorMessage}</InlineNotice> : null}
          <div className="tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground">
            <label htmlFor="ai-setup-key">API key</label>
            <span className="tw:flex tw:min-w-0 tw:items-center tw:gap-2">
              <TextField id="ai-setup-key" type={keyVisible ? "text" : "password"} autoComplete="off"
                value={controller.credentialDraft} placeholder={hosted.keyPlaceholder}
                onChange={(event) => controller.setCredentialDraft(event.currentTarget.value)} />
              <Button type="button" variant="ghost" size="icon" className="tw:shrink-0"
                aria-label={keyVisible ? "Hide API key" : "Show API key"} aria-pressed={keyVisible}
                onClick={() => setKeyVisible((visible) => !visible)}>
                <Icon name={keyVisible ? "view-off" : "view"} />
              </Button>
            </span>
          </div>
        </>
      ) : null;
      actions = (
        <>
          {back("account")}
          <Button variant="primary" disabled={busy || controller.credentialDraft.trim() === ""} onClick={() => void connect()}>
            {busy ? "Connecting…" : "Connect"}
          </Button>
        </>
      );
      break;
    case "ready": {
      const defaults = controller.defaults;
      const modelName = (id: string | null | undefined) =>
        id ? (controller.models.find((model) => model.id === id)?.displayName || id) : null;
      const voiceModel = voice?.ready ? voice.speech.models.find((model) => model.id === voice.speech.modelId) : null;
      const rows: ReadonlyArray<{ icon: IconName; task: string; model: string | null; id?: string | null }> = [
        { icon: "study-guide", task: "Cards and guides", model: modelName(defaults.flashcardGenerationModelId), id: defaults.flashcardGenerationModelId },
        { icon: "chat", task: "Tutor", model: modelName(defaults.tutorModelId), id: defaults.tutorModelId },
        { icon: "search", task: "Search", model: modelName(defaults.embeddingsModelId), id: defaults.embeddingsModelId },
        { icon: "listen", task: "Voice", model: voice === null && !voiceError ? "…" : (voiceModel?.label ?? null), id: voiceModel?.id },
      ];
      const missing = rows.some((row) => row.model === null);
      body = (
        <>
          <Heading>{missing ? "You're set up for text" : "You're set up"}</Heading>
          <p className="tw:m-0 tw:flex tw:items-center tw:gap-2 tw:text-label">
            <Icon name="check" size={16} className="tw:text-success" />
            Connected to {hosted?.name ?? "your service"}
          </p>
          <ul aria-label="Models chosen for you" className="tw:m-0 tw:grid tw:list-none tw:gap-0 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:p-0">
            {rows.map((row) => (
              <li key={row.task} className="tw:flex tw:items-center tw:gap-3 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:px-4 tw:py-3 tw:first:border-t-0">
                <Icon name={row.icon} size={18} className="tw:shrink-0 tw:text-muted-foreground" />
                <span className="tw:w-32 tw:shrink-0 tw:font-medium">{row.task}</span>
                <span className="tw:grid tw:min-w-0 tw:flex-1">
                  {row.model === null ? (
                    <span className="tw:text-muted-foreground">Off</span>
                  ) : (
                    <>
                      <span className="tw:truncate">{row.model}</span>
                      {row.id && row.id !== row.model ? <span className="tw:truncate tw:font-mono tw:text-caption tw:text-muted-foreground">{row.id}</span> : null}
                    </>
                  )}
                </span>
              </li>
            ))}
          </ul>
          {missing && service !== "openrouter" ? (
            <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-3 tw:rounded-lg tw:bg-muted tw:p-4">
              <span className="tw:grid tw:min-w-0 tw:flex-1 tw:basis-60">
                <strong className="tw:font-semibold">Add voice and search</strong>
                <span className="tw:text-label tw:text-muted-foreground">
                  {hosted?.name ?? "This service"} handles text. An OpenRouter key adds lessons read aloud and search across your library.
                </span>
              </span>
              <Button variant="secondary" onClick={addOpenRouter}>Add OpenRouter</Button>
            </div>
          ) : null}
          <p className="tw:m-0 tw:text-label tw:text-muted-foreground">
            Change any of these under Default for each task.
          </p>
        </>
      );
      actions = <Button variant="primary" onClick={onClose}>Done</Button>;
      break;
    }
    case "custom":
      body = (
        <>
          <p className="tw:m-0 tw:text-muted-foreground">
            Add each service you want with Add a service, then choose its model for each task under Default for each task.
          </p>
          <ul aria-label="Who can do each task" className="tw:m-0 tw:grid tw:list-none tw:gap-0 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:p-0">
            {TASK_SERVICES.map((row) => (
              <li key={row.task} className="tw:flex tw:items-center tw:gap-3 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:px-4 tw:py-3 tw:first:border-t-0">
                <Icon name={row.icon} size={18} className="tw:shrink-0 tw:text-muted-foreground" />
                <span className="tw:w-40 tw:shrink-0 tw:font-medium">{row.task}</span>
                <span className="tw:min-w-0 tw:flex-1 tw:text-label tw:text-muted-foreground">{row.who}</span>
              </li>
            ))}
          </ul>
        </>
      );
      actions = <>{back("choose")}<Button variant="primary" onClick={() => onAddService()}>Add a service</Button></>;
      break;
  }

  return (
    <Dialog title={title} width="medium" busy={busy} onClose={() => leave(onClose)}
      description={number >= 0 ? `Step ${number + 1} of ${NUMBERED.length}` : undefined}
      actions={actions}>
      <div className="tw:grid tw:min-w-0 tw:gap-5">{body}</div>
    </Dialog>
  );
}

function Heading({ children }: { children: ReactNode }) {
  return <h3 className="tw:m-0 tw:text-section tw:font-semibold">{children}</h3>;
}

function ExternalButton({ url, children }: { url: string; children: ReactNode }) {
  const [failed, setFailed] = useState(false);
  return (
    <span className="tw:grid tw:justify-items-start tw:gap-1">
      <Button variant="secondary" size="small" onClick={() => {
        setFailed(false);
        openExternalUrl(url).catch(() => setFailed(true));
      }}>
        <Icon name="link" size={16} />{children}
      </Button>
      {failed ? <span className="tw:text-label tw:text-muted-foreground">Open {url} in your browser.</span> : null}
    </span>
  );
}
