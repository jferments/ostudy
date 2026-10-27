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
  /**
   * The local hour, 0 (midnight) to 23, at which a new study day starts.
   * Reviews before it count toward the previous day.
   */
  newDayStartHour: number;
  /** How a study guide looks, read in the app and saved as a PDF; Sage until the learner picks another. */
  guidePdfTheme?: "navy" | "sage" | "slate" | "plum";
  /** Whether a study guide saved as a PDF carries the learner's notes. */
  guidePdfIncludeNotes?: boolean;
  /**
   * Whether a new course's lessons have their narration spoken and kept as
   * soon as they are written, when a lesson voice is set up. On by default.
   */
  lectureAudioWithNewLessons?: boolean;
  /**
   * Seconds of silence after which a spoken Tutor question is sent by itself;
   * 0 sends only when the learner stops recording. 2 until changed.
   */
  speechAutoSendSeconds?: number;
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
      displayQuestionOnAnswer: input.displayQuestionOnAnswer ?? true,
    },
  });
}

export async function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("get_app_info");
}
