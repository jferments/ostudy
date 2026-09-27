/* Generated from src-tauri/src/ipc_wire_contract.rs. Do not edit by hand. */

export interface NativeCommandError {
  code: string;
  message: string;
}

export interface NativeDeckSummary {
  id: string;
  name: string;
  description: string;
  sourceNoteCount: number;
  generatedCardCount: number;
  studyableDueCount: number;
  archivedAt: number | null;
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface NativeAppSettings {
  theme: "light" | "dark" | "console" | "synthwave";
  uiFont: string;
  cardFont: string;
  uiScale: number;
  cardBaseSize: number;
  motionPreference: "system" | "full" | "reduced" | "none";
  materialPreference: "automatic" | "rich" | "simple";
  decorativeArtwork: boolean;
  celebrationsEnabled: boolean;
  tutorialsEnabled: boolean;
  confirmDeletions: boolean;
  practiceShuffle: boolean;
  displayQuestionOnAnswer: boolean;
  desiredRetention: number;
  dailyNewLimit: number;
  dailyReviewLimit: number;
}

export interface NativeCompletedLibraryRestore {
  operationId: string;
  deckCount: number;
  cardCount: number;
  documentCount: number;
  templateCount: number;
  reviewCount: number;
  assetCount: number;
  restartRequired: boolean;
  completed: boolean;
}

export interface NativeLibrarySession {
  generation: number;
  ready: boolean;
  restoring: boolean;
  recoveryRequired: boolean;
  lastRestore: NativeCompletedLibraryRestore | null;
}

export type NativeWireEncodingFixture =
  | { kind: "ready"; bytes: number[]; optional: string | null; signed: number; unsigned: number }
  | { kind: "empty" };
