import {
  createContext,
  type ReactNode,
  useContext,
  useLayoutEffect,
  useMemo,
} from "react";

import { usePlatformCapabilities } from "../../platform/PlatformProvider";
import { useMediaQuery } from "../hooks/useMediaQuery";
import {
  type AppearanceProfile,
  type AppearanceSettings,
  resolveAppearanceProfile,
  resolveFontFamily,
} from "./appearanceTypes";

const fallbackProfile: AppearanceProfile = {
  theme: "dark",
  motion: "full",
  material: "standard",
  artworkEnabled: true,
  celebrationsEnabled: true,
  tutorialsEnabled: false,
  coarsePointer: false,
  hoverAvailable: true,
  forcedColors: false,
};

const AppearanceContext = createContext<AppearanceProfile>(fallbackProfile);

export function AppearanceProvider({
  settings,
  children,
}: {
  settings: AppearanceSettings;
  children: ReactNode;
}) {
  const capabilities = usePlatformCapabilities();
  const systemReducedMotion = useMediaQuery("(prefers-reduced-motion: reduce)");
  const systemReducedTransparency = useMediaQuery("(prefers-reduced-transparency: reduce)");
  const forcedColors = useMediaQuery("(forced-colors: active)");
  const profile = useMemo(
    () =>
      resolveAppearanceProfile(settings, {
        systemReducedMotion,
        systemReducedTransparency,
        forcedColors,
        coarsePointer: capabilities.coarsePointer,
        hoverAvailable: capabilities.hoverAvailable,
      }),
    [
      capabilities.coarsePointer,
      capabilities.hoverAvailable,
      forcedColors,
      settings,
      systemReducedMotion,
      systemReducedTransparency,
    ],
  );

  useLayoutEffect(() => {
    const root = document.documentElement;
    root.dataset.theme = profile.theme;
    root.dataset.motion = profile.motion;
    root.dataset.material = profile.material;
    root.dataset.artwork = profile.artworkEnabled ? "enabled" : "disabled";
    root.dataset.celebrations = profile.celebrationsEnabled ? "enabled" : "disabled";
    root.dataset.tutorials = profile.tutorialsEnabled ? "enabled" : "disabled";
    root.dataset.contrast = profile.forcedColors ? "forced" : "normal";
    root.dataset.input = profile.coarsePointer ? "coarse" : "fine";
    root.dataset.hover = profile.hoverAvailable ? "available" : "unavailable";

    // Compatibility attributes remain only while legacy selectors are migrated.
    root.dataset.animationsEnabled = profile.motion === "none" ? "false" : "true";
    root.dataset.reduceMotion = profile.motion === "reduced" ? "true" : "false";

    root.style.setProperty("--ui-scale", String(settings.uiScale / 100));
    root.style.setProperty("--card-base-size", `${settings.cardBaseSize}px`);
    root.style.setProperty("--ui-font-family", resolveFontFamily(settings.uiFont, settings.theme));
    root.style.setProperty(
      "--card-font-family",
      resolveFontFamily(settings.cardFont, settings.theme),
    );
    root.style.colorScheme = settings.theme === "light" ? "light" : "dark";
  }, [
    profile,
    settings.cardBaseSize,
    settings.cardFont,
    settings.theme,
    settings.uiFont,
    settings.uiScale,
  ]);

  return <AppearanceContext.Provider value={profile}>{children}</AppearanceContext.Provider>;
}

export function useAppearance(): AppearanceProfile {
  return useContext(AppearanceContext);
}

export function useVisualCapabilities() {
  const appearance = useAppearance();
  const platform = usePlatformCapabilities();
  return {
    appearance,
    platform,
    compact: appearance.coarsePointer || platform.androidInsets !== null,
  };
}
