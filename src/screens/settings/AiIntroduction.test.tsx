import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as aiProfilesIpc from "../../ipc/aiProfiles";
import type { AiProviderProfileSummary } from "../../ipc/aiProfiles";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { AI_INTRODUCTION_STORAGE_KEY, AiIntroduction } from "./AiIntroduction";

vi.mock("../../ipc/aiProfiles", () => ({ listAiProviderProfiles: vi.fn() }));

function renderIntroduction(ready = true) {
  const onSetUpAi = vi.fn();
  const view = render(
    <OverlayProvider>
      <AiIntroduction ready={ready} onSetUpAi={onSetUpAi} />
    </OverlayProvider>,
  );
  return { ...view, onSetUpAi };
}

describe("AiIntroduction", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
    vi.mocked(aiProfilesIpc.listAiProviderProfiles).mockResolvedValue([]);
  });

  it("explains what AI adds on a first start and opens setup", async () => {
    const { onSetUpAi } = renderIntroduction();
    const dialog = await screen.findByRole("dialog", { name: "Welcome to oStudy" });
    expect(within(dialog).getByText(/Without one, oStudy is a flashcard and\s+note-taking app/u)).toBeTruthy();
    const added = within(within(dialog).getByRole("list", { name: "What AI adds" })).getAllByRole("listitem");
    expect(added.map((item) => item.textContent)).toEqual([
      "A Tutor that answers questions from your own material",
      "Study guides, lessons and podcasts made from your documents",
      "Flashcards written from a document or a topic",
      "Figures and images in your documents read and explained",
      "Lessons and Tutor answers read aloud",
    ]);
    fireEvent.click(within(dialog).getByRole("button", { name: "Set up AI" }));
    expect(onSetUpAi).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(window.localStorage.getItem(AI_INTRODUCTION_STORAGE_KEY)).not.toBeNull();
  });

  it("closes with Not now and does not return", async () => {
    const { onSetUpAi, unmount } = renderIntroduction();
    const dialog = await screen.findByRole("dialog", { name: "Welcome to oStudy" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Not now" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(onSetUpAi).not.toHaveBeenCalled();
    unmount();
    renderIntroduction();
    await Promise.resolve();
    expect(aiProfilesIpc.listAiProviderProfiles).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("stays away when a service is already set up", async () => {
    vi.mocked(aiProfilesIpc.listAiProviderProfiles).mockResolvedValue([
      { id: "profile-1" } as AiProviderProfileSummary,
    ]);
    renderIntroduction();
    await waitFor(() => expect(window.localStorage.getItem(AI_INTRODUCTION_STORAGE_KEY)).not.toBeNull());
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("waits until the app is ready, and asks again next start when services cannot be read", async () => {
    const { rerender } = renderIntroduction(false);
    expect(aiProfilesIpc.listAiProviderProfiles).not.toHaveBeenCalled();
    vi.mocked(aiProfilesIpc.listAiProviderProfiles).mockRejectedValue(new Error("offline"));
    rerender(
      <OverlayProvider>
        <AiIntroduction ready onSetUpAi={vi.fn()} />
      </OverlayProvider>,
    );
    await waitFor(() => expect(aiProfilesIpc.listAiProviderProfiles).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(window.localStorage.getItem(AI_INTRODUCTION_STORAGE_KEY)).toBeNull();
  });
});
