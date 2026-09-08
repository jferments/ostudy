import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import { getEmbeddingOptions, listEmbeddingModels, saveEmbeddingOptions, testEmbeddingProfile } from "../../ipc/embeddings";
import { EmbeddingProfileSettings } from "./EmbeddingProfileSettings";

vi.mock("../../ipc/embeddings", () => ({ getEmbeddingOptions: vi.fn(), listEmbeddingModels: vi.fn(), saveEmbeddingOptions: vi.fn(), testEmbeddingProfile: vi.fn() }));
const id = "a".repeat(32);
function controller(): AiProfilesController {
  return { selectedProfile: { id, revision: 1, modelId: "embedding-model", providerKind: "openrouter" },
    draft: { modelId: "embedding-model" }, dirty: false, mutating: false, updateDraft: vi.fn() } as unknown as AiProfilesController;
}
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(getEmbeddingOptions).mockResolvedValue({ profileId: id, dimensions: null, inputPolicy: "automatic", revision: 0 });
  vi.mocked(listEmbeddingModels).mockResolvedValue([{ id: "qwen/embedding", name: "Qwen embedding" }]);
  vi.mocked(saveEmbeddingOptions).mockImplementation(async (input) => ({ ...input, revision: input.expectedRevision + 1 }));
  vi.mocked(testEmbeddingProfile).mockResolvedValue({ profileId: id, profileRevision: 1, optionsRevision: 0, dimensions: 768, spaceId: "b".repeat(64), queryAndDocumentTested: true });
});

describe("embedding settings", () => {
  it("does not contact an embedding provider until a requested catalog/probe action", async () => {
    const value = controller();
    render(<EmbeddingProfileSettings controller={value} />);
    await waitFor(() => expect(getEmbeddingOptions).toHaveBeenCalledWith(id));
    expect(testEmbeddingProfile).not.toHaveBeenCalled();
    expect(listEmbeddingModels).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Load embedding models" }));
    await screen.findByRole("combobox", { name: "Embedding model" });
    fireEvent.change(screen.getByRole("combobox", { name: "Embedding model" }), { target: { value: "qwen/embedding" } });
    expect(value.updateDraft).toHaveBeenCalledWith({ modelId: "qwen/embedding", reasoningEffort: "none" });
  });

  it("saves dimension changes with the observed revision and reports a fixed-input probe", async () => {
    render(<EmbeddingProfileSettings controller={controller()} />);
    await waitFor(() => expect((screen.getByRole("combobox", { name: "Embedding dimensions" }) as HTMLSelectElement).disabled).toBe(false));
    fireEvent.change(screen.getByRole("combobox", { name: "Embedding dimensions" }), { target: { value: "768" } });
    await waitFor(() => expect(saveEmbeddingOptions).toHaveBeenCalledWith({ profileId: id, dimensions: 768, inputPolicy: "automatic", expectedRevision: 0 }));
    await waitFor(() => expect((screen.getByRole("button", { name: "Test embeddings" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByRole("button", { name: "Test embeddings" }));
    await screen.findByText(/Query and document embeddings verified: 768/u);
    expect(testEmbeddingProfile).toHaveBeenCalledWith(id);
  });

  it("disables provider tests for unsaved profile changes", () => {
    render(<EmbeddingProfileSettings controller={{ ...controller(), dirty: true }} />);
    expect((screen.getByRole("button", { name: "Test embeddings" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Load embedding models" }) as HTMLButtonElement).disabled).toBe(true);
  });
});
