import { describe, expect, it } from "vitest";

import type { AppSettings } from "../../ipc/settings";
import {
  DEFAULT_FONT_FAMILY,
  DEFAULT_FONT_PRESET,
  FONT_PRESET_OPTIONS,
  resolveAppearanceProfile,
  resolveFontFamily,
  resolveMaterial,
  resolveMotion,
} from "./appearanceTypes";

const settings: AppSettings = {
  theme: "dark",
  uiFont: "plex",
  cardFont: "plex",
  uiScale: 100,
  cardBaseSize: 18,
  motionPreference: "system",
  materialPreference: "automatic",
  decorativeArtwork: true,
  celebrationsEnabled: true,
  tutorialsEnabled: true,
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
  it("uses IBM Plex Sans with the system stack as the shared new-install default", () => {
    expect(DEFAULT_FONT_PRESET).toBe("plex");
    expect(DEFAULT_FONT_FAMILY).toBe(
      "'IBM Plex Sans', var(--ostudy-font-family-system)",
    );
    expect(resolveFontFamily(DEFAULT_FONT_PRESET, "dark")).toBe(DEFAULT_FONT_FAMILY);
  });

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

  it("keeps interactive tutorials off when an older settings shape omits the preference", () => {
    const profile = resolveAppearanceProfile({ ...settings, tutorialsEnabled: undefined }, environment);
    expect(profile.tutorialsEnabled).toBe(false);
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

  it("exposes each bundled font preset and resolves it to a local-first family", () => {
    const bundled = FONT_PRESET_OPTIONS.filter((option) => option.group === "bundled");
    expect(bundled.map((option) => option.value)).toEqual([
      "plex",
      "albert",
      "space-grotesk",
      "spline-sans",
      "sora",
      "roboto",
      "roboto-slab",
      "sofia-sans",
      "lato",
      "ubuntu",
    ]);
    expect(resolveFontFamily("plex", "dark")).toBe(DEFAULT_FONT_FAMILY);
    expect(resolveFontFamily("albert", "dark")).toContain("'Albert Sans'");
    expect(resolveFontFamily("space-grotesk", "dark")).toContain("'Space Grotesk'");
    expect(resolveFontFamily("spline-sans", "dark")).toContain("'Spline Sans'");
    expect(resolveFontFamily("sora", "dark")).toContain("'Sora'");
    expect(resolveFontFamily("roboto", "dark")).toContain("'Roboto'");
    expect(resolveFontFamily("roboto-slab", "dark")).toContain("'Roboto Slab'");
    expect(resolveFontFamily("sofia-sans", "dark")).toContain("'Sofia Sans'");
    expect(resolveFontFamily("lato", "dark")).toContain("'Lato'");
    expect(resolveFontFamily("ubuntu", "dark")).toContain("'Ubuntu'");
  });
});
