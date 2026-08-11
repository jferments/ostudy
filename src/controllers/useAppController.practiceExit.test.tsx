import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useEffect } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as decksIpc from "../ipc/decks";
import * as settingsIpc from "../ipc/settings";
import { makeDeck, makeSettings } from "../test/factories";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import {
  hasActivePracticeSession,
  registerPracticeExitHandler,
  resetPracticeExitGuardForTests,
} from "./practiceExitGuard";
import { useAppController } from "./useAppController";

vi.mock("../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../ipc/decks", () => ({ listDecks: vi.fn() }));
vi.mock("../ipc/settings", () => ({ getSettings: vi.fn() }));

function Harness({ terminate }: { terminate: () => Promise<boolean> }) {
  const controller = useAppController();

  useEffect(() => {
    if (controller.section !== "practice") return registerPracticeExitHandler(null);
    return registerPracticeExitHandler(terminate);
  }, [controller.section, terminate]);

  if (controller.isLoading) return <span>Loading</span>;
  return (
    <div>
      <output aria-label="section">{controller.section}</output>
      <button type="button" onClick={() => void controller.navigate("practice")}>
        Go practice
      </button>
      <button type="button" onClick={() => void controller.navigate("decks")}>
        Go decks
      </button>
    </div>
  );
}

function renderHarness(terminate: () => Promise<boolean>) {
  return render(
    <OverlayProvider>
      <Harness terminate={terminate} />
    </OverlayProvider>,
  );
}

async function enterPractice(): Promise<void> {
  fireEvent.click(screen.getByRole("button", { name: "Go practice" }));
  await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("practice"));
  // The practice section is committed during render, while the session-exit
  // handler is registered by the following effect. Navigation is only guarded
  // once that handler is active, so the test must wait for the real precondition
  // instead of racing React's effect flush on a loaded test worker.
  await waitFor(() => expect(hasActivePracticeSession()).toBe(true));
}

describe("active practice navigation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetPracticeExitGuardForTests();
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);
  });

  it("asks before leaving and terminates without navigating when the user says no", async () => {
    const terminate = vi.fn().mockResolvedValue(true);
    renderHarness(terminate);
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("decks"));
    await enterPractice();

    fireEvent.click(screen.getByRole("button", { name: "Go decks" }));
    const dialog = await screen.findByRole("alertdialog", {
      name: "Terminate this practice session?",
    });
    expect(dialog.textContent).toContain("review history");
    expect(dialog.textContent).toContain("without opening post-review analytics");
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(terminate).not.toHaveBeenCalled();
    expect(screen.getByLabelText("section").textContent).toBe("practice");
  });

  it("completes the active session before navigating away when confirmed", async () => {
    const terminate = vi.fn().mockResolvedValue(true);
    renderHarness(terminate);
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("decks"));
    await enterPractice();

    fireEvent.click(screen.getByRole("button", { name: "Go decks" }));
    await screen.findByRole("alertdialog", { name: "Terminate this practice session?" });
    fireEvent.click(screen.getByRole("button", { name: "Yes, terminate" }));

    await waitFor(() => expect(terminate).toHaveBeenCalledOnce());
    await waitFor(() => expect(screen.getByLabelText("section").textContent).toBe("decks"));
  });
});
