import { invoke } from "@tauri-apps/api/core";

import type { AiReasoningEffort } from "./aiProfiles";

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
  /** Durable read compatibility until all bundled callers use motionPreference. */
  animationsEnabled?: boolean;
  /** Durable read compatibility until all bundled callers use motionPreference. */
  reduceMotion?: boolean;
  confirmDeletions: boolean;
  practiceShuffle: boolean;
  /** Show the question again beside the answer after revealing a Basic card. */
  displayQuestionOnAnswer?: boolean;
  desiredRetention: number;
  dailyNewLimit: number;
  dailyReviewLimit: number;
  /**
   * Transitional source compatibility for pre-Stage-11 fixtures only.
   * Native settings no longer return or persist this value; saveSettings strips
   * it before crossing IPC. Reasoning belongs to each AI profile.
   */
  aiReasoningEffort?: AiReasoningEffort;
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
  const { aiReasoningEffort: _legacyReasoning, ...persisted } = input;
  return invoke<AppSettings>("save_settings", {
    input: {
      ...persisted,
      displayQuestionOnAnswer: input.displayQuestionOnAnswer ?? false,
    },
  });
}

export async function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("get_app_info");
}
