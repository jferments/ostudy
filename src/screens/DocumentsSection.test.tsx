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
    modelId: "unsloth/Qwen3.6-27B-MTP-GGUF",
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

  it("shows only the current flashcard-generation model above Documents", async () => {
    render(<DocumentsSection refreshDecks={vi.fn()} showToast={vi.fn()} />);

    expect(await screen.findByText("Current Flashcard Generation model")).toBeTruthy();
    expect(screen.getByText("unsloth/Qwen3.6-27B-MTP-GGUF")).toBeTruthy();
    expect(screen.queryByText("llama.cpp / Unsloth GGUF")).toBeNull();
    expect(screen.queryByText(/stay on this computer/u)).toBeNull();
    expect(screen.queryByText(/Local computer/u)).toBeNull();
    expect(screen.getByText("Document library content")).toBeTruthy();
  });

  it("shows a compact unconfigured value when no generation model is selected", async () => {
    vi.mocked(aiProfiles.getAiTaskDefaults).mockResolvedValue({
      flashcardGenerationProfileId: null,
    });
    render(<DocumentsSection refreshDecks={vi.fn()} showToast={vi.fn()} />);

    expect(await screen.findByText("Not configured")).toBeTruthy();
    expect(screen.queryByText(/Settings → AI\/LLM/u)).toBeNull();
  });
});
