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

export type LayoutMode = "compact" | "medium" | "wide";

const fallbackProfile: AppearanceProfile = {
  theme: "dark",
  motion: "full",
  material: "standard",
  artworkEnabled: true,
  celebrationsEnabled: true,
  tutorialsEnabled: true,
  coarsePointer: false,
  hoverAvailable: true,
  forcedColors: false,
};

const AppearanceContext = createContext<AppearanceProfile>(fallbackProfile);
const LayoutContext = createContext<LayoutMode>("wide");

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
  const compactViewport = useMediaQuery("(max-width: 760px)");
  const mediumViewport = useMediaQuery("(min-width: 761px) and (max-width: 1120px)");
  const layoutMode: LayoutMode = compactViewport ? "compact" : mediumViewport ? "medium" : "wide";
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
    root.dataset.layout = layoutMode;

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
    layoutMode,
    profile,
    settings.cardBaseSize,
    settings.cardFont,
    settings.theme,
    settings.uiFont,
    settings.uiScale,
  ]);

  return (
    <AppearanceContext.Provider value={profile}>
      <LayoutContext.Provider value={layoutMode}>{children}</LayoutContext.Provider>
    </AppearanceContext.Provider>
  );
}

export function useAppearance(): AppearanceProfile {
  return useContext(AppearanceContext);
}

export function useLayoutMode(): LayoutMode {
  return useContext(LayoutContext);
}

export function useVisualCapabilities() {
  const appearance = useAppearance();
  const platform = usePlatformCapabilities();
  const layoutMode = useLayoutMode();
  return {
    appearance,
    platform,
    layoutMode,
    compact: layoutMode === "compact",
  };
}
