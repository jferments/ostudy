import { invoke } from "@tauri-apps/api/core";

export interface AppSettings {
  theme: "light" | "dark" | "console" | "synthwave";
  uiFont: string;
  cardFont: string;
  uiScale: number;
  cardBaseSize: number;
  reduceMotion: boolean;
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
