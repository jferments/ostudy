import type { ThemeName } from "../tokens/generated/tokenNames";

export type MotionPreference = "system" | "full" | "reduced" | "none";
export type MaterialPreference = "automatic" | "rich" | "simple";
export type ResolvedMotion = "full" | "reduced" | "none";
export type ResolvedMaterial = "rich" | "standard" | "simple";

export const FONT_PRESET_OPTIONS = [
  { value: "plex", label: "IBM Plex Sans", group: "bundled" },
  { value: "albert", label: "Albert Sans", group: "bundled" },
  { value: "space-grotesk", label: "Space Grotesk", group: "bundled" },
  { value: "spline-sans", label: "Spline Sans", group: "bundled" },
  { value: "sora", label: "Sora", group: "bundled" },
  { value: "roboto", label: "Roboto", group: "bundled" },
  { value: "roboto-slab", label: "Roboto Slab", group: "bundled" },
  { value: "sofia-sans", label: "Sofia Sans", group: "bundled" },
  { value: "lato", label: "Lato", group: "bundled" },
  { value: "ubuntu", label: "Ubuntu", group: "bundled" },
  { value: "system", label: "System sans-serif", group: "system" },
  { value: "atkinson", label: "Atkinson Hyperlegible", group: "system" },
  { value: "humanist", label: "Humanist sans-serif", group: "system" },
  { value: "serif", label: "System serif", group: "system" },
  { value: "mono", label: "Monospace", group: "system" },
] as const;

/**
 * Presentation-only settings consumed by the shared visual system.
 *
 * Keep this structural contract inside src/ui so appearance primitives never
 * depend on the Tauri IPC layer. The durable AppSettings object is a superset
 * and can be passed here without an adapter.
 */
export interface AppearanceSettings {
  theme: ThemeName;
  uiFont: string;
  cardFont: string;
  uiScale: number;
  cardBaseSize: number;
  motionPreference?: MotionPreference;
  materialPreference?: MaterialPreference;
  decorativeArtwork?: boolean;
  celebrationsEnabled?: boolean;
  tutorialsEnabled?: boolean;
  /** Transitional compatibility with settings written before motionPreference. */
  animationsEnabled?: boolean;
  /** Transitional compatibility with settings written before motionPreference. */
  reduceMotion?: boolean;
}

export interface AppearanceEnvironment {
  systemReducedMotion: boolean;
  systemReducedTransparency: boolean;
  forcedColors: boolean;
  coarsePointer: boolean;
  hoverAvailable: boolean;
}

export interface AppearanceProfile {
  theme: ThemeName;
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
  settings: AppearanceSettings,
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
    tutorialsEnabled: settings.tutorialsEnabled ?? false,
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

export function resolveFontFamily(value: string, theme: ThemeName): string {
  const presets: Record<string, string> = {
    system: "var(--ostudy-font-family-system)",
    atkinson: "var(--ostudy-font-family-study)",
    plex: "'IBM Plex Sans', var(--ostudy-font-family-system)",
    albert: "'Albert Sans', var(--ostudy-font-family-system)",
    "space-grotesk": "'Space Grotesk', var(--ostudy-font-family-system)",
    "spline-sans": "'Spline Sans', var(--ostudy-font-family-system)",
    sora: "'Sora', var(--ostudy-font-family-system)",
    roboto: "'Roboto', var(--ostudy-font-family-system)",
    "roboto-slab": "'Roboto Slab', var(--ostudy-font-family-serif)",
    "sofia-sans": "'Sofia Sans', var(--ostudy-font-family-system)",
    lato: "'Lato', var(--ostudy-font-family-system)",
    ubuntu: "'Ubuntu', var(--ostudy-font-family-system)",
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
