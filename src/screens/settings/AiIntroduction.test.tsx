import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { AiFeaturesProvider, type AiFeatures } from "../../controllers/AiFeaturesProvider";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { AI_INTRODUCTION_STORAGE_KEY, AiIntroduction } from "./AiIntroduction";

const NO_SERVICE: AiFeatures = { enabled: false, known: true, hasServices: false, refresh: () => undefined };

function introduction(ai: AiFeatures, ready: boolean, onSetUpAi: () => void) {
  return (
    <AiFeaturesProvider value={ai}>
      <OverlayProvider>
        <AiIntroduction ready={ready} onSetUpAi={onSetUpAi} />
      </OverlayProvider>
    </AiFeaturesProvider>
  );
}

function renderIntroduction(ai: AiFeatures = NO_SERVICE, ready = true) {
  const onSetUpAi = vi.fn();
  const view = render(introduction(ai, ready, onSetUpAi));
  return { ...view, onSetUpAi };
}

describe("AiIntroduction", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("says what oStudy does on its own and what AI adds, then opens setup", async () => {
    const { onSetUpAi } = renderIntroduction();
    const dialog = await screen.findByRole("dialog", { name: "Welcome to oStudy" });
    expect(within(dialog).getByText(/oStudy is a complete flashcard and note-taking app on its own/u)).toBeTruthy();
    expect(within(dialog).getByText(/Many of oStudy's features also use an AI\s+service/u)).toBeTruthy();
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

  it("shows where AI waits in Settings after Not now, and does not return", async () => {
    const { onSetUpAi, unmount } = renderIntroduction();
    const dialog = await screen.findByRole("dialog", { name: "Welcome to oStudy" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Not now" }));
    const later = await screen.findByRole("dialog", { name: "AI is in Settings" });
    const picture = within(later).getByRole("figure", { name: "Settings, with the AI section highlighted" });
    expect(within(picture).getByText("AI").closest("li")?.getAttribute("aria-current")).toBe("true");
    fireEvent.click(within(later).getByRole("button", { name: "Got it" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(onSetUpAi).not.toHaveBeenCalled();
    unmount();
    renderIntroduction();
    await Promise.resolve();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("stays away when a service is already set up", async () => {
    renderIntroduction({ ...NO_SERVICE, enabled: true, hasServices: true });
    await waitFor(() => expect(window.localStorage.getItem(AI_INTRODUCTION_STORAGE_KEY)).not.toBeNull());
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("waits until the app is ready and the services are known", async () => {
    const onSetUpAi = vi.fn();
    const { rerender } = render(introduction({ ...NO_SERVICE, known: false }, true, onSetUpAi));
    rerender(introduction(NO_SERVICE, false, onSetUpAi));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(window.localStorage.getItem(AI_INTRODUCTION_STORAGE_KEY)).toBeNull();
    rerender(introduction(NO_SERVICE, true, onSetUpAi));
    expect(await screen.findByRole("dialog", { name: "Welcome to oStudy" })).toBeTruthy();
  });
});
