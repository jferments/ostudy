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

export interface NativeSubject {
  id: string;
  name: string;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface NativeSubjectDeckPlacement {
  deckId: string;
  subjectId: string | null;
  sortOrder: number;
}

export interface NativeSubjectDocumentPlacement {
  documentId: string;
  subjectId: string | null;
}

export interface NativeDocumentStudyRange {
  id: string;
  documentId: string;
  unitKind: string;
  startUnit: number;
  endUnit: number;
  sourceSha256: string;
  createdAt: number;
}

export interface NativeSubjectLibrary {
  libraryRevision: number;
  documentFilingRevision: number;
  subjects: NativeSubject[];
  decks: NativeSubjectDeckPlacement[];
  documents: NativeSubjectDocumentPlacement[];
  studyRanges: NativeDocumentStudyRange[];
  checkTheseCount: number;
}

export interface NativeSubjectFilingReview {
  entityKind: "subject" | "deck" | "document" | "note";
  entityId: string;
  reason: string;
  detail: Record<string, unknown>;
  label: string;
}

export type NativeWireEncodingFixture =
  | { kind: "ready"; bytes: number[]; optional: string | null; signed: number; unsigned: number }
  | { kind: "empty" };
