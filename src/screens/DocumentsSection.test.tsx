import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as aiProfiles from "../ipc/aiProfiles";
import { DocumentsSection } from "./DocumentsSection";

vi.mock("../ipc/aiProfiles", () => ({
  getAiTaskDefaults: vi.fn(),
  listAiProviderProfiles: vi.fn(),
}));
vi.mock("./DocumentsScreen", () => ({
  DocumentsScreen: () => <div>Document library content</div>,
}));

const profileId = "a".repeat(32);

function profile(
  overrides: Partial<aiProfiles.AiProviderProfileSummary> = {},
): aiProfiles.AiProviderProfileSummary {
  return {
    id: profileId,
    displayName: "Local vision model",
    providerKind: "llama_cpp",
    providerLabel: "llama.cpp / Unsloth GGUF",
    modelId: "medical-vision-model",
    enabled: true,
    networkScope: "local",
    credentialStatus: {
      status: "not_required",
      authenticationRequired: false,
      securePersistenceAvailable: true,
      sessionOnlyAvailable: true,
    },
    configurationStatus: "ready",
    revision: 1,
    updatedAt: 1,
    ...overrides,
  };
}

describe("DocumentsSection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(aiProfiles.getAiTaskDefaults).mockResolvedValue({
      flashcardGenerationProfileId: profileId,
    });
    vi.mocked(aiProfiles.listAiProviderProfiles).mockResolvedValue([profile()]);
  });

  it("shows the selected profile, model, and local destination before the document workflow", async () => {
    render(<DocumentsSection refreshDecks={vi.fn()} showToast={vi.fn()} />);

    expect(await screen.findByText("Local vision model")).toBeTruthy();
    expect(screen.getByText(/medical-vision-model/u)).toBeTruthy();
    expect(screen.getByText(/stay on this computer/u)).toBeTruthy();
    expect(screen.getByText("Document library content")).toBeTruthy();
  });

  it("warns when selected PDF page content will be transmitted to a cloud provider", async () => {
    vi.mocked(aiProfiles.listAiProviderProfiles).mockResolvedValue([
      profile({
        displayName: "Cloud flashcards",
        providerKind: "deepseek",
        providerLabel: "DeepSeek",
        networkScope: "cloud",
      }),
    ]);
    render(<DocumentsSection refreshDecks={vi.fn()} showToast={vi.fn()} />);

    expect(await screen.findByText("Cloud flashcards")).toBeTruthy();
    expect(screen.getByText(/will be transmitted to this cloud provider/u)).toBeTruthy();
  });

  it("explains how to configure the task when no generation default exists", async () => {
    vi.mocked(aiProfiles.getAiTaskDefaults).mockResolvedValue({
      flashcardGenerationProfileId: null,
    });
    render(<DocumentsSection refreshDecks={vi.fn()} showToast={vi.fn()} />);

    expect(await screen.findByText("Not configured")).toBeTruthy();
    expect(screen.getByText(/Settings → AI\/LLM/u)).toBeTruthy();
  });
});
