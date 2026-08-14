import { describe, expect, it } from "vitest";

import type { AppSettings } from "../../ipc/settings";
import {
  resolveAppearanceProfile,
  resolveMaterial,
  resolveMotion,
} from "./appearanceTypes";

const settings: AppSettings = {
  theme: "dark",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  motionPreference: "system",
  materialPreference: "automatic",
  decorativeArtwork: true,
  celebrationsEnabled: true,
  tutorialsEnabled: true,
  deckViewDensity: "comfortable",
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};

const environment = {
  systemReducedMotion: false,
  systemReducedTransparency: false,
  forcedColors: false,
  coarsePointer: false,
  hoverAvailable: true,
};

describe("appearance resolution", () => {
  it("follows operating-system motion when requested", () => {
    expect(resolveMotion("system", false)).toBe("full");
    expect(resolveMotion("system", true)).toBe("reduced");
  });

  it("forces simple materials for reduced transparency and forced colors", () => {
    expect(resolveMaterial("rich", true, false)).toBe("simple");
    expect(resolveMaterial("rich", false, true)).toBe("simple");
    expect(resolveMaterial("rich", false, false)).toBe("rich");
  });

  it("disables artwork in forced colors and celebrations outside full motion", () => {
    const profile = resolveAppearanceProfile(
      { ...settings, motionPreference: "reduced" },
      { ...environment, forcedColors: true },
    );
    expect(profile.artworkEnabled).toBe(false);
    expect(profile.celebrationsEnabled).toBe(false);
    expect(profile.material).toBe("simple");
  });

  it("maps legacy animation settings during the bundled transition", () => {
    const profile = resolveAppearanceProfile(
      {
        ...settings,
        motionPreference: undefined,
        animationsEnabled: false,
        reduceMotion: false,
      },
      environment,
    );
    expect(profile.motion).toBe("none");
  });
});
