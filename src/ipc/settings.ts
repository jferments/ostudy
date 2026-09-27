import { invoke } from "./transport";

export type MotionPreference = "system" | "full" | "reduced" | "none";
export type MaterialPreference = "automatic" | "rich" | "simple";

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
  confirmDeletions: boolean;
  practiceShuffle: boolean;
  /** Show the question again beside the answer after revealing a Basic card. */
  displayQuestionOnAnswer?: boolean;
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
  return invoke<AppSettings>("save_settings", {
    input: {
      ...input,
      displayQuestionOnAnswer: input.displayQuestionOnAnswer ?? false,
    },
  });
}

export async function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("get_app_info");
}
