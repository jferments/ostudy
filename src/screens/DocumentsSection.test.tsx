import { render, screen, waitFor } from "@testing-library/react";
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

  it("leaves the generation model to Settings", async () => {
    render(
      <DocumentsSection
        refreshDecks={vi.fn()}
        onPracticeCards={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    expect(await screen.findByText("Document library content")).toBeTruthy();
    await waitFor(() => expect(aiProfiles.listAiProviderProfiles).toHaveBeenCalled());
    expect(screen.queryByText("unsloth/Qwen3.6-27B-MTP-GGUF")).toBeNull();
    expect(screen.queryByText("Local vision model")).toBeNull();
    expect(screen.queryByText(/model/iu)).toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("says once when no AI provider is set up for documents", async () => {
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

    const heading = screen.getByRole("heading", { name: "Documents" });
    const notice = await screen.findByText(/Set up an AI provider in Settings/u);
    expect(
      Boolean(heading.compareDocumentPosition(notice) & Node.DOCUMENT_POSITION_FOLLOWING),
    ).toBe(true);
  });
});
