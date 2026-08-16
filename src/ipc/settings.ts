import { invoke } from "@tauri-apps/api/core";

export type MotionPreference = "system" | "full" | "reduced" | "none";
export type MaterialPreference = "automatic" | "rich" | "simple";
export type DeckViewDensity = "comfortable" | "compact";

export interface AppSettings {
  theme: "light" | "dark" | "console" | "synthwave";
  uiFont: string;
  cardFont: string;
  uiScale: number;
  cardBaseSize: number;
  motionPreference?: MotionPreference;
  materialPreference?: MaterialPreference;
  decorativeArtwork?: boolean;
  celebrationsEnabled?: boolean;
  tutorialsEnabled?: boolean;
  deckViewDensity?: DeckViewDensity;
  /** Durable read compatibility until all bundled callers use motionPreference. */
  animationsEnabled?: boolean;
  /** Durable read compatibility until all bundled callers use motionPreference. */
  reduceMotion?: boolean;
  confirmDeletions: boolean;
  practiceShuffle: boolean;
  desiredRetention: number;
  dailyNewLimit: number;
  dailyReviewLimit: number;
}

export interface AppInfo {
  appVersion: string;
  dataDirectory: string;
  databasePath: string;
  imageDirectory: string;
  diagnosticLogPath?: string;
  databaseSchemaVersion: number;
}

export async function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_settings");
}

export async function saveSettings(input: AppSettings): Promise<AppSettings> {
  return invoke<AppSettings>("save_settings", { input });
}

export async function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("get_app_info");
}
