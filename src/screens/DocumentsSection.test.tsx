import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";

import * as aiProfiles from "../ipc/aiProfiles";
import { DocumentsSection } from "./DocumentsSection";

vi.mock("../ipc/aiProfiles", () => ({
  getAiTaskDefaults: vi.fn(),
  listAiProviderProfiles: vi.fn(),
}));
vi.mock("./DocumentsScreen", () => ({
  DocumentsScreen: ({ headerSupplement }: { headerSupplement?: ReactNode }) => (
    <div>
      <h1>Documents</h1>
      {headerSupplement}
      <div>Document library content</div>
    </div>
  ),
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

  it("keeps the provider panel focused on the selected generation model", async () => {
    render(
      <DocumentsSection
        refreshDecks={vi.fn()}
        onPracticeCards={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    const heading = screen.getByRole("heading", { name: "Documents" });
    const model = await screen.findByText("Current Flashcard Generation model");
    expect(
      Boolean(heading.compareDocumentPosition(model) & Node.DOCUMENT_POSITION_FOLLOWING),
    ).toBe(true);
    expect(screen.getByText("unsloth/Qwen3.6-27B-MTP-GGUF")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Study guides" })).toBeNull();
    expect(screen.queryByText("llama.cpp / Unsloth GGUF")).toBeNull();
    expect(screen.queryByText(/stay on this computer/u)).toBeNull();
    expect(screen.queryByText(/Local computer/u)).toBeNull();
    expect(screen.getByText("Document library content")).toBeTruthy();
  });

  it("shows a compact unconfigured value when no generation model is selected", async () => {
    vi.mocked(aiProfiles.getAiTaskDefaults).mockResolvedValue({
      flashcardGenerationProfileId: null,
    });
    render(
      <DocumentsSection
        refreshDecks={vi.fn()}
        onPracticeCards={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    expect(await screen.findByText("Not configured")).toBeTruthy();
    expect(screen.queryByText(/Settings → AI\/LLM/u)).toBeNull();
  });
});
