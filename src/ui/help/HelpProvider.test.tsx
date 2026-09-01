import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { OverlayProvider } from "../overlays/OverlayProvider";
import { HelpProvider, HelpTrigger } from "./HelpProvider";

function renderHelp() {
  return render(
    <OverlayProvider>
      <HelpProvider>
        <HelpTrigger topicId="flashcards.folders" />
      </HelpProvider>
    </OverlayProvider>,
  );
}

describe("HelpProvider", () => {
  it("keeps concise help on the trigger and detailed help in an explicit sheet", async () => {
    renderHelp();
    const trigger = screen.getByRole("button", { name: "Help: Deck folders" });
    expect(trigger.hasAttribute("title")).toBe(false);
    fireEvent.focus(trigger);
    expect(screen.getByRole("tooltip").textContent).toContain("Folders organize decks");

    fireEvent.click(trigger);
    const dialog = await screen.findByRole("dialog", { name: "Deck folders" });
    expect(within(dialog).getByText(/Deleting a folder moves its decks to Unsorted decks/u)).toBeTruthy();
    expect(within(dialog).getByText("Flashcards and deck organization")).toBeTruthy();
  });

  it("opens a topic requested by a tutorial or another semantic caller", async () => {
    renderHelp();
    await act(async () => {
      window.dispatchEvent(
        new CustomEvent("ostudy:open-help", {
          detail: { topicId: "flashcards.search.studyState" },
        }),
      );
    });
    expect(await screen.findByRole("dialog", { name: "Study-state filters" })).toBeTruthy();
  });

  it("hands tutorial restarts through a semantic event instead of routing itself", async () => {
    const listener = vi.fn();
    window.addEventListener("ostudy:start-tutorial", listener);
    renderHelp();
    fireEvent.click(screen.getByRole("button", { name: "Help: Deck folders" }));
    const dialog = await screen.findByRole("dialog", { name: "Deck folders" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Start guided tutorial" }));
    await new Promise((resolve) => window.requestAnimationFrame(resolve));
    expect(listener).toHaveBeenCalled();
    window.removeEventListener("ostudy:start-tutorial", listener);
  });
});
