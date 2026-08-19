#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re
import textwrap

ROOT = pathlib.Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content)


def replace_it(path: str, title: str, replacement: str) -> None:
    text = read(path)
    pattern = re.compile(
        r'  it\("' + re.escape(title) + r'",(?: async)? \(\) => \{.*?\n  \}\);',
        re.S,
    )
    updated, count = pattern.subn(textwrap.dedent(replacement).rstrip(), text, count=1)
    if count != 1:
        raise RuntimeError(f"{path}: could not replace test {title!r}")
    write(path, updated)


replace_it(
    "src/screens/flashcards/DeckLibraryWorkspace.test.tsx",
    "persists compact density as a presentation preference",
    '''
      it("uses one canonical compact deck presentation without density controls", async () => {
        renderLibrary();
        await screen.findByRole("heading", { name: "Folders" });
        expect(screen.queryByRole("button", { name: "Comfortable" })).toBeNull();
        expect(screen.queryByRole("button", { name: "Compact" })).toBeNull();
        expect(settingsIpc.saveSettings).not.toHaveBeenCalled();
        const row = screen.getByText("Pharmacology").closest("article");
        expect(row).not.toBeNull();
        expect(row?.getAttribute("data-density")).toBeNull();
      });
    ''',
)

write(
    "src/screens/settings/AiLlmSettings.test.tsx",
    textwrap.dedent(
        r'''import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import type { AiProviderCatalogEntry, AiProviderProfileDetail } from "../../ipc/aiProfiles";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { AiLlmSettings } from "./AiLlmSettings";

const profileId = "a".repeat(32);

function openRouterCatalog(): AiProviderCatalogEntry[] {
  return [{
    providerKind: "openrouter", displayName: "OpenRouter",
    description: "Use OpenRouter's account-visible model catalog.", implementationStatus: "available",
    defaultBaseUrl: "https://openrouter.ai/api/v1/", baseUrlEditable: false,
    authenticationRequired: true, modelIdRequired: true, modelDiscoverySupported: true,
    chatTemplatePolicy: "provider_managed", defaultTimeoutMs: 60_000,
  }];
}

function profile(): AiProviderProfileDetail {
  return {
    id: profileId, displayName: "OpenRouter", providerKind: "openrouter", providerLabel: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1/", baseUrlEditable: false,
    modelId: "provider/model-000", authMode: "bearer", enabled: true,
    allowInsecureHttp: false, requireZeroDataRetention: false, requestTimeoutMs: 60_000,
    credentialEpoch: 1, networkScope: "cloud",
    credentialStatus: {
      status: "saved_securely", authenticationRequired: true,
      securePersistenceAvailable: true, sessionOnlyAvailable: true,
    },
    configurationStatus: "ready", revision: 3, createdAt: 1, updatedAt: 2,
  };
}

function controller(): AiProfilesController {
  const selected = profile();
  const models = Array.from({ length: 411 }, (_, index) => ({
    id: `provider/model-${String(index).padStart(3, "0")}`,
    displayName: `Provider Model ${String(index).padStart(3, "0")}`,
    ownedBy: "provider",
  }));
  models.push({ id: "zeta/final-model", displayName: "Zeta Final Model", ownedBy: "zeta" });
  return {
    catalog: openRouterCatalog(), profiles: [], defaults: { flashcardGenerationProfileId: null },
    selectedProfile: selected, selectedProfileId: selected.id,
    draft: {
      displayName: selected.displayName, providerKind: selected.providerKind,
      baseUrl: selected.baseUrl, modelId: selected.modelId, authMode: selected.authMode,
      enabled: selected.enabled, allowInsecureHttp: selected.allowInsecureHttp,
      requireZeroDataRetention: selected.requireZeroDataRetention,
      requestTimeoutMs: selected.requestTimeoutMs,
    },
    credentialDraft: "", credentialPersistence: "secure", models, modelsLoaded: true,
    probeResult: null, dirty: false, loading: false, mutating: false,
    refreshingModels: false, testing: false, errorMessage: null,
    selectProfile: vi.fn(), beginCreate: vi.fn(), updateDraft: vi.fn(),
    setCredentialDraft: vi.fn(), setCredentialPersistence: vi.fn(),
    saveProfile: vi.fn().mockResolvedValue(undefined), saveSetup: vi.fn().mockResolvedValue(undefined),
    deleteProfile: vi.fn().mockResolvedValue(undefined), saveCredential: vi.fn().mockResolvedValue(undefined),
    clearCredential: vi.fn().mockResolvedValue(undefined), refreshModels: vi.fn().mockResolvedValue(undefined),
    testProfile: vi.fn().mockResolvedValue(undefined), setFlashcardDefault: vi.fn().mockResolvedValue(undefined),
    discardDraft: vi.fn(), reload: vi.fn().mockResolvedValue(undefined),
  };
}

describe("AiLlmSettings", () => {
  it("renders every model returned by a large provider catalog", () => {
    render(<OverlayProvider><AiLlmSettings controller={controller()} /></OverlayProvider>);
    expect(screen.getByPlaceholderText("Search available models")).toBeTruthy();
    const finalOption = screen.getByRole("option", { name: "Zeta Final Model — zeta/final-model" });
    expect(finalOption).toBeTruthy();
    const modelSelect = finalOption.closest("select");
    expect(modelSelect).not.toBeNull();
    expect(modelSelect?.querySelectorAll("option")).toHaveLength(413);
  });
});
'''
    ),
)

write(
    "src/screens/settings/DefaultAiModelSettings.test.tsx",
    textwrap.dedent(
        r'''import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import { makeSettings } from "../../test/factories";
import { DefaultAiModelSettings } from "./DefaultAiModelSettings";

const profileId = "a".repeat(32);
const settings = { ...makeSettings(), aiReasoningEffort: "low" as const };

function controller(): AiProfilesController {
  return {
    catalog: [],
    profiles: [{
      id: profileId, displayName: "Local nursing model", providerKind: "llama_cpp",
      providerLabel: "llama.cpp / Unsloth GGUF", modelId: "Qwen-medical", enabled: true,
      networkScope: "local",
      credentialStatus: {
        status: "not_required", authenticationRequired: false,
        securePersistenceAvailable: true, sessionOnlyAvailable: true,
      },
      configurationStatus: "ready", revision: 1, updatedAt: 1,
    }],
    defaults: { flashcardGenerationProfileId: profileId },
    selectedProfile: null, selectedProfileId: null, draft: null,
    credentialDraft: "", credentialPersistence: "secure", models: [], modelsLoaded: false,
    probeResult: null, dirty: false, loading: false, mutating: false,
    refreshingModels: false, testing: false, errorMessage: null,
    selectProfile: vi.fn(), beginCreate: vi.fn(), updateDraft: vi.fn(),
    setCredentialDraft: vi.fn(), setCredentialPersistence: vi.fn(), saveProfile: vi.fn(),
    saveSetup: vi.fn(), deleteProfile: vi.fn(), saveCredential: vi.fn(), clearCredential: vi.fn(),
    refreshModels: vi.fn(), testProfile: vi.fn(), setFlashcardDefault: vi.fn().mockResolvedValue(undefined),
    discardDraft: vi.fn(), reload: vi.fn().mockResolvedValue(undefined),
  };
}

describe("DefaultAiModelSettings", () => {
  it("shows the document-generation policy and saved model identity", () => {
    render(<DefaultAiModelSettings controller={controller()} settings={settings} update={vi.fn()} />);
    expect(screen.getByRole("heading", { name: "Document-generation policy" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "Local nursing model — Qwen-medical" })).toBeTruthy();
    expect(screen.queryByText(/initial-response timeout/u)).toBeNull();
    expect((screen.getByRole("combobox", { name: "Reasoning level" }) as HTMLSelectElement).value).toBe("low");
  });

  it("updates the default AI model without creating a fallback chain", () => {
    const value = controller();
    render(<DefaultAiModelSettings controller={value} settings={settings} update={vi.fn()} />);
    fireEvent.change(screen.getByRole("combobox", { name: "Default AI model" }), {
      target: { value: "" },
    });
    expect(value.setFlashcardDefault).toHaveBeenCalledWith(null);
  });

  it("updates reasoning through the document-generation policy", () => {
    const update = vi.fn();
    render(<DefaultAiModelSettings controller={controller()} settings={settings} update={update} />);
    fireEvent.change(screen.getByRole("combobox", { name: "Reasoning level" }), {
      target: { value: "minimal" },
    });
    expect(update).toHaveBeenCalledWith({ aiReasoningEffort: "minimal" });
  });
});
'''
    ),
)

# Keep the connection-result regression focused on the approved compact contract.
replace_it(
    "src/screens/SettingsScreen.extended.test.tsx",
    "renders connection verification as a compact summary with expandable phases",
    '''
      it("renders connection verification as a compact summary with expandable phases", async () => {
        const detail = profileDetail("llama_cpp");
        vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(detail)]);
        vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(detail);
        vi.mocked(aiIpc.testAiProviderProfile).mockResolvedValue({
          profileId: llamaId, profileRevision: 1, credentialEpoch: 0,
          testedModelId: "nursing-qwen", adapterRevision: 3, resultSchemaVersion: 1,
          overallStatus: "verified",
          phases: [
            { phase: "model_discovery", label: "Model discovery", status: "passed", message: "Available.", latencyMs: 18, error: null },
            { phase: "structured_output", label: "Structured output", status: "passed", message: "Schema verified.", latencyMs: 42, error: null },
          ],
          capabilities: [], testedAt: 3, persisted: true, stale: false,
        });
        renderSettings();
        await openAiTab();
        await screen.findByRole("combobox", { name: "3. Model" });
        fireEvent.click(screen.getByRole("button", { name: "Test connection" }));
        await waitFor(() => expect(aiIpc.testAiProviderProfile).toHaveBeenCalledWith(llamaId, 1));
        expect(await screen.findByText("Connected · structured output verified")).toBeTruthy();
        expect(screen.getByText("Details")).toBeTruthy();
        expect(screen.queryByLabelText("Connection diagnostic log")).toBeNull();
      });
    ''',
)
