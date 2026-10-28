import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { requiredTestValue as required } from "../../test/factories";
import type { AiProfilesController } from "../../controllers/aiProfilesController";
import type { AiProviderProfileDetail } from "../../ipc/aiProfiles";
import { getSpeechSettings, type SpeechSettings } from "../../ipc/speech";
import { openExternalUrl } from "../../platform/externalLinks";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { AiLlmSettings } from "./AiLlmSettings";

vi.mock("../../ipc/speech", () => ({ getSpeechSettings: vi.fn(), getDeviceVoices: vi.fn(async () => ({ supported: false, models: [] })) }));
let browserSignIn: "available" | "unavailable" = "unavailable";
vi.mock("../../platform/PlatformProvider", () => ({
  usePlatformCapabilities: () => ({
    native: { services: { browserSignIn: { state: browserSignIn } } },
  }),
}));
vi.mock("../../platform/externalLinks", () => ({
  openExternalUrl: vi.fn().mockResolvedValue(undefined),
}));

const profileId = "a".repeat(32);

function speech(overrides: Partial<SpeechSettings> = {}): SpeechSettings {
  return {
    profileId,
    modelId: "fish-audio/drama-3",
    voice: "",
    revision: 0,
    models: [
      {
        id: "fish-audio/drama-3",
        label: "Drama 3",
        voices: [],
        defaultVoice: "",
        clonesVoices: true, onDevice: false,
      },
    ],
    connections: [{ id: profileId, name: "OpenRouter" }],
    voices: [],
    ...overrides,
  };
}

function firstRun(): AiProfilesController {
  const value = controller();
  value.profiles = [];
  value.selectedProfile = null;
  value.credentialDraft = "sk-or-v1-test";
  return value;
}

function wizard() {
  return screen.getByRole("dialog", { name: "Set up AI" });
}

function profile(
  overrides: Partial<AiProviderProfileDetail> = {},
): AiProviderProfileDetail {
  return {
    id: profileId,
    displayName: "OpenRouter",
    providerKind: "openrouter",
    providerLabel: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1/",
    baseUrlEditable: false,
    modelId: "",
    reasoningEffort: "automatic",
    authMode: "bearer",
    enabled: true,
    allowInsecureHttp: false,
    requireZeroDataRetention: false,
    requestTimeoutMs: 60_000,
    credentialEpoch: 1,
    networkScope: "cloud",
    credentialStatus: {
      status: "saved_securely",
      authenticationRequired: true,
      securePersistenceAvailable: true,
      sessionOnlyAvailable: true,
    },
    configurationStatus: "ready",
    revision: 3,
    createdAt: 1,
    updatedAt: 2,
    ...overrides,
  };
}

function controller(selected = profile()): AiProfilesController {
  return {
    catalog: [
      {
        providerKind: selected.providerKind,
        displayName: selected.providerLabel,
        description: "Provider fixture",
        implementationStatus: "available",
        defaultBaseUrl: selected.baseUrl,
        baseUrlEditable: selected.baseUrlEditable,
        authenticationRequired:
          selected.credentialStatus.authenticationRequired,
        modelIdRequired: true,
        modelDiscoverySupported: true,
        chatTemplatePolicy: "provider_managed",
        defaultTimeoutMs: 60_000,
      },
    ],
    profiles: [{ ...selected }],
    defaults: { flashcardGenerationProfileId: null },
    selectedProfile: selected,
    selectedProfileId: selected.id,
    draft: {
      displayName: selected.displayName,
      providerKind: selected.providerKind,
      baseUrl: selected.baseUrl,
      modelId: "",
      reasoningEffort: "automatic",
      authMode: selected.authMode,
      enabled: selected.enabled,
      allowInsecureHttp: selected.allowInsecureHttp,
      requireZeroDataRetention: selected.requireZeroDataRetention,
      requestTimeoutMs: selected.requestTimeoutMs,
    },
    credentialDraft: "",
    credentialPersistence: "secure",
    models: [
      { id: "one/chat", displayName: "Chat model", ownedBy: "one" },
      { id: "two/embed", displayName: "Embedding model", ownedBy: "two" },
    ],
    modelsLoaded: true,
    probeResult: null,
    dirty: false,
    loading: false,
    mutating: false,
    refreshingModels: false,
    testing: false,
    errorMessage: null,
    selectProfile: vi.fn(),
    beginCreate: vi.fn(),
    updateDraft: vi.fn(),
    setCredentialDraft: vi.fn(),
    setCredentialPersistence: vi.fn(),
    saveProfile: vi.fn().mockResolvedValue(true),
    connectSetup: vi.fn().mockResolvedValue(true),
    signInWithOpenRouter: vi.fn().mockResolvedValue(true),
    signingIn: false,
    cancelSignIn: vi.fn(),
    deleteProfile: vi.fn().mockResolvedValue(undefined),
    saveCredential: vi.fn().mockResolvedValue(undefined),
    clearCredential: vi.fn().mockResolvedValue(undefined),
    refreshModels: vi.fn().mockResolvedValue(undefined),
    testProfile: vi.fn().mockResolvedValue(undefined),
    setFlashcardDefault: vi.fn().mockResolvedValue(undefined),
    setTaskDefault: vi.fn().mockResolvedValue(undefined),
    pendingTaskDefault: null,
    retryTaskDefault: vi.fn().mockResolvedValue(undefined),
    discardDraft: vi.fn(),
    reload: vi.fn().mockResolvedValue(undefined),
  };
}

function renderSettings(value: AiProfilesController) {
  render(
    <OverlayProvider>
      <AiLlmSettings controller={value} />
    </OverlayProvider>,
  );
}

async function openService(name = "OpenRouter") {
  fireEvent.click(
    screen.getByRole("button", { name: new RegExp(`^${name}`, "u") }),
  );
  return screen.findByRole("dialog");
}

describe("AiLlmSettings", () => {
  it("lists services with their status and keeps model assignment out of the service sheet", async () => {
    renderSettings(controller());

    expect(screen.getByRole("heading", { name: "Services" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Add a service" })).toBeTruthy();
    const list = screen.getByRole("list", { name: "Services" });
    expect(within(list).getByText("Key saved")).toBeTruthy();
    expect(within(list).getByText("Ready")).toBeTruthy();

    const sheet = await openService();
    expect(within(sheet).queryByRole("combobox", { name: "Model" })).toBeNull();
    expect(
      within(sheet).queryByRole("option", { name: "Chat model" }),
    ).toBeNull();
    expect(
      (within(sheet).getByLabelText("API key") as HTMLInputElement).type,
    ).toBe("password");
  });

  it("keeps connection policy and credentials service-scoped", async () => {
    const value = controller();
    renderSettings(value);
    const sheet = await openService();
    expect(value.selectProfile).toHaveBeenCalledWith(profileId);

    expect(
      (within(sheet).getByRole("textbox", { name: "Name" }) as HTMLInputElement)
        .value,
    ).toBe("OpenRouter");
    fireEvent.click(within(sheet).getByText("More options"));
    expect(
      within(sheet)
        .getByRole("switch", { name: "Use this service" })
        .getAttribute("aria-checked"),
    ).toBe("true");
    const zdr = within(sheet).getByRole("switch", {
      name: "Only zero-data-retention routes",
    });
    expect(zdr.getAttribute("aria-checked")).toBe("false");
    fireEvent.click(zdr);
    expect(value.updateDraft).toHaveBeenCalledWith({
      requireZeroDataRetention: true,
    });
    fireEvent.click(
      within(sheet).getByRole("switch", {
        name: "Keep the key on this device",
      }),
    );
    expect(value.setCredentialPersistence).toHaveBeenCalledWith("session");
    expect(
      within(sheet).getByRole("button", { name: "Refresh models" }),
    ).toBeTruthy();

    fireEvent.change(within(sheet).getByRole("textbox", { name: "Name" }), {
      target: { value: "OpenRouter 2" },
    });
    expect(value.updateDraft).toHaveBeenCalledWith({
      displayName: "OpenRouter 2",
    });
  });

  it("adds a service from the sheet and closes it once connected", async () => {
    const value = controller();
    value.selectedProfile = null;
    value.credentialDraft = "sk-test";
    renderSettings(value);
    fireEvent.click(screen.getByRole("button", { name: "Add a service" }));
    expect(value.beginCreate).toHaveBeenCalledWith("openrouter");
    const sheet = await screen.findByRole("dialog", { name: "Add a service" });
    expect(
      within(sheet).getByRole("radio", { name: /^OpenRouter/u }),
    ).toBeTruthy();
    fireEvent.click(within(sheet).getByRole("button", { name: "Connect" }));
    await waitFor(() => expect(value.connectSetup).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Add a service" }),
      ).toBeNull(),
    );
  });

  it("removes a service from its menu after confirmation", async () => {
    const value = controller();
    renderSettings(value);
    fireEvent.click(
      screen.getByRole("button", { name: "More for OpenRouter" }),
    );
    fireEvent.click(await screen.findByRole("menuitem", { name: "Remove" }));
    fireEvent.click(
      await screen.findByRole("button", { name: "Remove service" }),
    );
    await waitFor(() =>
      expect(value.deleteProfile).toHaveBeenCalledWith(
        expect.objectContaining({ id: profileId }),
      ),
    );
  });

  it("shows the explicit LAN HTTP opt-in only for remote llama.cpp HTTP endpoints", async () => {
    const selected = profile({
      displayName: "LAN llama.cpp",
      providerKind: "llama_cpp",
      providerLabel: "llama.cpp / Unsloth GGUF",
      baseUrl: "http://192.168.1.50:8080/",
      baseUrlEditable: true,
      authMode: "none",
      networkScope: "lan",
      credentialStatus: {
        status: "not_required",
        authenticationRequired: false,
        securePersistenceAvailable: true,
        sessionOnlyAvailable: true,
      },
    });
    const value = controller(selected);
    required(value.catalog[0], "catalog profile").chatTemplatePolicy =
      "server_model";
    renderSettings(value);
    const sheet = await openService("LAN llama.cpp");
    const trigger = within(sheet).getByRole("button", { name: "More options" });
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(trigger);
    expect(trigger.getAttribute("aria-expanded")).toBe("true");
    const optIn = within(sheet).getByRole("switch", {
      name: "Allow an unencrypted network connection",
    });
    expect(optIn.getAttribute("aria-checked")).toBe("false");
    fireEvent.click(optIn);
    expect(value.updateDraft).toHaveBeenCalledWith({ allowInsecureHttp: true });
  });

  it("walks a first learner through OpenRouter and shows the models chosen for them", async () => {
    vi.mocked(getSpeechSettings).mockResolvedValue(speech());
    const value = firstRun();
    value.defaults = {
      flashcardGenerationProfileId: profileId,
      flashcardGenerationModelId: "deepseek/deepseek-v4.1-flash-1017",
      flashcardGenerationReasoningEffort: "none",
      tutorProfileId: profileId,
      tutorModelId: "deepseek/deepseek-v4.1-flash-1017",
      tutorReasoningEffort: "low",
      embeddingsProfileId: profileId,
      embeddingsModelId: "qwen/qwen3-embedding-8b",
      embeddingsReasoningEffort: "none",
    };
    renderSettings(value);

    expect(screen.getByText("No AI service yet")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Set up AI" }));
    expect(within(wizard()).getByText("Step 1 of 5")).toBeTruthy();
    expect(
      within(wizard())
        .getByRole("radio", { name: /^OpenRouter/u })
        .getAttribute("aria-checked"),
    ).toBe("true");
    expect(within(wizard()).getByText("Simplest setup")).toBeTruthy();
    expect(
      within(wizard()).getByText(
        "Everything you need in one place: text, voice, images and search",
      ),
    ).toBeTruthy();

    fireEvent.click(within(wizard()).getByRole("button", { name: "Next" }));
    expect(
      within(wizard()).getByRole("heading", {
        name: "You pay only for what you use",
      }),
    ).toBeTruthy();
    expect(within(wizard()).getByText("$7.99 / month")).toBeTruthy();
    expect(
      within(wizard()).getByText("What you make yourself is always free"),
    ).toBeTruthy();
    expect(
      within(wizard()).getByText(/turned into text on your device, so it is free/u),
    ).toBeTruthy();
    expect(
      within(wizard()).getByText(/Reading aloud can be free too/u).textContent,
    ).toContain("This device (free)");

    fireEvent.click(within(wizard()).getByRole("button", { name: "Next" }));
    expect(
      within(wizard()).getByRole("heading", {
        name: "Create your OpenRouter account",
      }),
    ).toBeTruthy();
    expect(
      within(wizard()).getByText(/Already have an account with credit at OpenRouter\?/u),
    ).toBeTruthy();
    fireEvent.click(
      within(wizard()).getByRole("button", { name: "Open openrouter.ai" }),
    );
    expect(openExternalUrl).toHaveBeenCalledWith("https://openrouter.ai/");

    fireEvent.click(within(wizard()).getByRole("button", { name: "Next" }));
    expect(value.beginCreate).toHaveBeenCalledWith("openrouter");
    expect(
      within(wizard()).getByRole("heading", {
        name: "Paste an OpenRouter key",
      }),
    ).toBeTruthy();
    expect(
      (within(wizard()).getByLabelText("API key") as HTMLInputElement).type,
    ).toBe("password");
    fireEvent.click(
      within(wizard()).getByRole("button", { name: "Open API keys" }),
    );
    expect(openExternalUrl).toHaveBeenCalledWith(
      "https://openrouter.ai/settings/keys",
    );

    fireEvent.click(within(wizard()).getByRole("button", { name: "Connect" }));
    await waitFor(() => expect(value.connectSetup).toHaveBeenCalledTimes(1));
    expect(
      await within(wizard()).findByRole("heading", { name: "You're set up" }),
    ).toBeTruthy();
    const models = within(wizard()).getByRole("list", {
      name: "Models chosen for you",
    });
    expect(
      within(models).getAllByText("deepseek/deepseek-v4.1-flash-1017"),
    ).toHaveLength(2);
    expect(within(models).getByText("qwen/qwen3-embedding-8b")).toBeTruthy();
    expect(await within(models).findByText("Drama 3")).toBeTruthy();
    expect(
      within(wizard()).queryByRole("button", { name: "Add OpenRouter" }),
    ).toBeNull();

    fireEvent.click(within(wizard()).getByRole("button", { name: "Done" }));
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Set up AI" })).toBeNull(),
    );
  });

  it("leaves search and voice off after a text-only service and offers OpenRouter for them", async () => {
    vi.mocked(getSpeechSettings).mockResolvedValue(
      speech({ profileId: null, connections: [] }),
    );
    const value = firstRun();
    value.defaults = {
      flashcardGenerationProfileId: profileId,
      flashcardGenerationModelId: "claude-sonnet-5-5",
      flashcardGenerationReasoningEffort: "none",
      tutorProfileId: profileId,
      tutorModelId: "claude-sonnet-5-5",
      tutorReasoningEffort: "low",
      embeddingsProfileId: null,
      embeddingsModelId: null,
      embeddingsReasoningEffort: null,
    };
    renderSettings(value);
    fireEvent.click(screen.getByRole("button", { name: "Set up AI" }));
    fireEvent.click(within(wizard()).getByRole("radio", { name: /^Claude/u }));
    fireEvent.click(within(wizard()).getByRole("button", { name: "Next" }));
    fireEvent.click(within(wizard()).getByRole("button", { name: "Next" }));
    expect(
      within(wizard()).getByRole("heading", {
        name: "Create your Claude account",
      }),
    ).toBeTruthy();
    fireEvent.click(within(wizard()).getByRole("button", { name: "Next" }));
    expect(value.beginCreate).toHaveBeenCalledWith("claude");
    fireEvent.click(within(wizard()).getByRole("button", { name: "Connect" }));

    expect(
      await within(wizard()).findByRole("heading", {
        name: "You're set up for text",
      }),
    ).toBeTruthy();
    const models = within(wizard()).getByRole("list", {
      name: "Models chosen for you",
    });
    await waitFor(() =>
      expect(within(models).getAllByText("Off")).toHaveLength(2),
    );
    fireEvent.click(
      within(wizard()).getByRole("button", { name: "Add OpenRouter" }),
    );
    expect(
      within(wizard()).getByRole("heading", {
        name: "Create your OpenRouter account",
      }),
    ).toBeTruthy();
  });

  it("hands a server on this computer over to the Add a service sheet", async () => {
    const value = firstRun();
    const [entry] = value.catalog;
    value.catalog = [
      ...value.catalog,
      {
        ...required(entry, "catalog entry"),
        providerKind: "llama_cpp",
        displayName: "llama.cpp",
      },
    ];
    renderSettings(value);
    fireEvent.click(screen.getByRole("button", { name: "Set up AI" }));
    fireEvent.click(
      within(wizard()).getByRole("radio", {
        name: /^A server on this computer/u,
      }),
    );
    fireEvent.click(within(wizard()).getByRole("button", { name: "Next" }));
    expect(
      await screen.findByRole("dialog", { name: "Add a service" }),
    ).toBeTruthy();
    expect(value.beginCreate).toHaveBeenLastCalledWith("llama_cpp");
  });

  it("connects OpenRouter in one click where the browser can return to oStudy", async () => {
    browserSignIn = "available";
    vi.mocked(getSpeechSettings).mockResolvedValue(speech());
    const value = firstRun();
    value.credentialDraft = "";
    renderSettings(value);
    fireEvent.click(screen.getByRole("button", { name: "Set up AI" }));
    fireEvent.click(within(wizard()).getByRole("button", { name: "Next" }));
    fireEvent.click(within(wizard()).getByRole("button", { name: "Next" }));
    fireEvent.click(within(wizard()).getByRole("button", { name: "Next" }));
    expect(
      within(wizard()).getByRole("heading", {
        name: "Connect oStudy to OpenRouter",
      }),
    ).toBeTruthy();
    expect(within(wizard()).queryByLabelText("API key")).toBeNull();

    fireEvent.click(
      within(wizard()).getByRole("button", { name: "Connect with OpenRouter" }),
    );
    await waitFor(() =>
      expect(value.signInWithOpenRouter).toHaveBeenCalledWith(openExternalUrl),
    );
    expect(
      await within(wizard()).findByRole("heading", { name: /^You're set up/u }),
    ).toBeTruthy();
    browserSignIn = "unavailable";
  });

  it("keeps paste-a-key as the way in beside one-click sign-in", () => {
    browserSignIn = "available";
    const value = firstRun();
    renderSettings(value);
    fireEvent.click(screen.getByRole("button", { name: "Set up AI" }));
    for (let step = 0; step < 3; step += 1)
      fireEvent.click(within(wizard()).getByRole("button", { name: "Next" }));
    fireEvent.click(
      within(wizard()).getByRole("button", { name: "Paste a key instead" }),
    );
    expect(value.cancelSignIn).toHaveBeenCalled();
    expect(
      within(wizard()).getByRole("heading", {
        name: "Paste an OpenRouter key",
      }),
    ).toBeTruthy();
    expect(within(wizard()).getByLabelText("API key")).toBeTruthy();
    browserSignIn = "unavailable";
  });
});
