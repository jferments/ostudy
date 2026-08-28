import { render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import type { AppearanceSettings } from "./appearanceTypes";
import { DEFAULT_FONT_FAMILY } from "./appearanceTypes";
import { AppearanceProvider } from "./AppearanceProvider";

const settings: AppearanceSettings = {
  theme: "dark",
  uiFont: "plex",
  cardFont: "serif",
  uiScale: 100,
  cardBaseSize: 18,
};

afterEach(() => {
  const root = document.documentElement;
  root.style.removeProperty("--ui-font-family");
  root.style.removeProperty("--card-font-family");
});

describe("AppearanceProvider typography", () => {
  it("publishes independent semantic interface and card font families", () => {
    const { rerender } = render(
      <AppearanceProvider settings={settings}>
        <span>Typography consumer</span>
      </AppearanceProvider>,
    );
    const root = document.documentElement;

    expect(root.style.getPropertyValue("--ui-font-family")).toBe(DEFAULT_FONT_FAMILY);
    expect(root.style.getPropertyValue("--card-font-family")).toBe(
      "var(--ostudy-font-family-serif)",
    );

    rerender(
      <AppearanceProvider settings={{ ...settings, uiFont: "system", cardFont: "plex" }}>
        <span>Typography consumer</span>
      </AppearanceProvider>,
    );

    expect(root.style.getPropertyValue("--ui-font-family")).toBe(
      "var(--ostudy-font-family-system)",
    );
    expect(root.style.getPropertyValue("--card-font-family")).toBe(DEFAULT_FONT_FAMILY);
  });

  it("keeps the deterministic system-family fallback when the Plex face is unavailable", () => {
    render(
      <AppearanceProvider settings={{ ...settings, cardFont: "plex" }}>
        <span>Typography consumer</span>
      </AppearanceProvider>,
    );

    expect(document.documentElement.style.getPropertyValue("--card-font-family")).toBe(
      "'IBM Plex Sans', var(--ostudy-font-family-system)",
    );
  });
});
