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
  theme: string;
  uiFont: string;
  cardFont: string;
  uiScale: number;
  cardBaseSize: number;
  motionPreference: string;
  materialPreference: string;
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

export interface NativeLibrarySession {
  generation: number;
  ready: boolean;
  restoring: boolean;
  recoveryRequired: boolean;
  lastRestore: unknown | null;
}

export type NativeWireEncodingFixture =
  | { kind: "ready"; bytes: number[]; optional: string | null; signed: number; unsigned: number }
  | { kind: "empty" };
