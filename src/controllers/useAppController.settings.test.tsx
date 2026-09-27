import { act, renderHook, waitFor } from "@testing-library/react";
import type { PropsWithChildren } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as decksIpc from "../ipc/decks";
import * as settingsIpc from "../ipc/settings";
import { makeSettings } from "../test/factories";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { useAppController } from "./useAppController";

vi.mock("../ipc/decks", () => ({ listDecks: vi.fn() }));
vi.mock("../ipc/settings", () => ({ getSettings: vi.fn() }));
vi.mock("../ipc/uiCheckpoint", () => ({
  getUiCheckpoint: vi.fn(async () => null),
  saveUiCheckpoint: vi.fn(async () => null),
}));
vi.mock("../ipc/platform", async (importOriginal) => ({
  ...await importOriginal<typeof import("../ipc/platform")>(),
  listenToLifecycle: vi.fn(async () => () => undefined),
}));

function wrapper({ children }: PropsWithChildren) {
  return <OverlayProvider>{children}</OverlayProvider>;
}

describe("current application settings ownership", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(decksIpc.listDecks).mockResolvedValue([]);
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
  });

  it.each([true, false])("preserves display-question preference %s through load and local save", async (displayQuestionOnAnswer) => {
    const saved = makeSettings({ displayQuestionOnAnswer });
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(saved);
    const { result } = renderHook(() => useAppController(), { wrapper });
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.fatalError).toBeNull();
    expect(result.current.savedSettings.displayQuestionOnAnswer).toBe(displayQuestionOnAnswer);
    expect(result.current.activeSettings.displayQuestionOnAnswer).toBe(displayQuestionOnAnswer);

    const updated = makeSettings({ displayQuestionOnAnswer: !displayQuestionOnAnswer, uiScale: 110 });
    act(() => result.current.saveSettingsLocally(updated));
    expect(result.current.savedSettings.displayQuestionOnAnswer).toBe(!displayQuestionOnAnswer);
    expect(result.current.activeSettings).toEqual(result.current.savedSettings);
    expect(result.current.activeSettings.uiScale).toBe(110);
  });

  it("does not reintroduce fixture-only global reasoning into active settings", async () => {
    // A historical harness can still supply extra runtime properties. The
    // current settings projection owns only current preferences, not AI profiles.
    const oldHarness = { ...makeSettings(), aiReasoningEffort: "high" };
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(oldHarness);
    const { result } = renderHook(() => useAppController(), { wrapper });
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.activeSettings).not.toHaveProperty("aiReasoningEffort");
    act(() => result.current.saveSettingsLocally(oldHarness));
    expect(result.current.savedSettings).not.toHaveProperty("aiReasoningEffort");
    expect(result.current.activeSettings.theme).toBe(oldHarness.theme);
  });
});
