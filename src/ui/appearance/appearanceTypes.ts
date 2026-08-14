import type {
  AppSettings,
  MaterialPreference,
  MotionPreference,
} from "../../ipc/settings";

export type ResolvedMotion = "full" | "reduced" | "none";
export type ResolvedMaterial = "rich" | "standard" | "simple";

export interface AppearanceEnvironment {
  systemReducedMotion: boolean;
  systemReducedTransparency: boolean;
  forcedColors: boolean;
  coarsePointer: boolean;
  hoverAvailable: boolean;
}

export interface AppearanceProfile {
  theme: AppSettings["theme"];
  motion: ResolvedMotion;
  material: ResolvedMaterial;
  artworkEnabled: boolean;
  celebrationsEnabled: boolean;
  tutorialsEnabled: boolean;
  coarsePointer: boolean;
  hoverAvailable: boolean;
  forcedColors: boolean;
}

export function resolveAppearanceProfile(
  settings: AppSettings,
  environment: AppearanceEnvironment,
): AppearanceProfile {
  const motionPreference =
    settings.motionPreference ??
    (settings.animationsEnabled === false
      ? "none"
      : settings.reduceMotion
        ? "reduced"
        : "system");
  const motion = resolveMotion(motionPreference, environment.systemReducedMotion);
  const material = resolveMaterial(
    settings.materialPreference ?? "automatic",
    environment.systemReducedTransparency,
    environment.forcedColors,
  );
  return {
    theme: settings.theme,
    motion,
    material,
    artworkEnabled: (settings.decorativeArtwork ?? true) && !environment.forcedColors,
    celebrationsEnabled: (settings.celebrationsEnabled ?? true) && motion === "full",
    tutorialsEnabled: settings.tutorialsEnabled ?? true,
    coarsePointer: environment.coarsePointer,
    hoverAvailable: environment.hoverAvailable,
    forcedColors: environment.forcedColors,
  };
}

export function resolveMotion(
  preference: MotionPreference,
  systemReducedMotion: boolean,
): ResolvedMotion {
  switch (preference) {
    case "full":
      return "full";
    case "reduced":
      return "reduced";
    case "none":
      return "none";
    case "system":
      return systemReducedMotion ? "reduced" : "full";
  }
}

export function resolveMaterial(
  preference: MaterialPreference,
  systemReducedTransparency: boolean,
  forcedColors: boolean,
): ResolvedMaterial {
  if (forcedColors || systemReducedTransparency || preference === "simple") return "simple";
  return preference === "rich" ? "rich" : "standard";
}

export function resolveFontFamily(value: string, theme: AppSettings["theme"]): string {
  const presets: Record<string, string> = {
    system: "var(--ostudy-font-family-system)",
    atkinson: "var(--ostudy-font-family-study)",
    plex: "var(--ostudy-font-family-technical)",
    humanist: "'Trebuchet MS', 'Segoe UI', ui-sans-serif, sans-serif",
    serif: "var(--ostudy-font-family-serif)",
    mono: "var(--ostudy-font-family-mono)",
  };
  const effective = theme === "console" && value === "system" ? "mono" : value;
  if (effective.startsWith("custom:")) {
    const custom = effective.slice("custom:".length).trim().replaceAll("'", "");
    return custom ? `'${custom}', ${presets.system}` : presets.system;
  }
  return presets[effective] ?? presets.system;
}
