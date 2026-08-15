import { describe, expect, it, vi } from "vitest";

import { makeSettings } from "../../test/factories";
import { announceSettingsSaved, listenForSavedSettings } from "./settingsEvents";

describe("settingsEvents", () => {
  it("delivers the complete persisted settings object and can unsubscribe", () => {
    const listener = vi.fn();
    const unsubscribe = listenForSavedSettings(listener);
    const settings = makeSettings({ deckViewDensity: "compact" });

    announceSettingsSaved(settings);
    expect(listener).toHaveBeenCalledWith(settings);

    unsubscribe();
    announceSettingsSaved(makeSettings({ deckViewDensity: "comfortable" }));
    expect(listener).toHaveBeenCalledTimes(1);
  });
});
