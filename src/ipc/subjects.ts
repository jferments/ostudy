import type { SourceRangeInput } from "./collections";
import { invoke } from "./transport";

/** A course: it holds documents, guides and decks. Subjects do not nest. */
export interface Subject {
  id: string;
  name: string;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
  revision: number;
}

/** A live deck's place in the library; `subjectId` null is "No subject". */
export interface SubjectDeckPlacement {
  deckId: string;
  subjectId: string | null;
  sortOrder: number;
}

/** A document's subject; `subjectId` null is "No subject". */
export interface SubjectDocumentPlacement {
  documentId: string;
  subjectId: string | null;
}

/** "Study only these parts" of a document inside its subject. */
export interface DocumentStudyRange {
  id: string;
  documentId: string;
  unitKind: string;
  startUnit: number;
  endUnit: number;
  sourceSha256: string;
  createdAt: number;
}

/**
 * The whole library organization. `libraryRevision` guards the subject list
 * and deck filing and order; `documentFilingRevision` guards document filing
 * and study ranges.
 */
export interface SubjectLibrary {
  libraryRevision: number;
  documentFilingRevision: number;
  subjects: Subject[];
  decks: SubjectDeckPlacement[];
  documents: SubjectDocumentPlacement[];
  studyRanges: DocumentStudyRange[];
  checkTheseCount: number;
}

export type SubjectFilingEntityKind = "subject" | "deck" | "document" | "note";

/** One entry of the "Check these" list. */
export interface SubjectFilingReview {
  entityKind: SubjectFilingEntityKind;
  entityId: string;
  reason: string;
  detail: Record<string, unknown>;
  label: string;
}

export interface CreateSubjectInput {
  name: string;
  expectedLibraryRevision: number;
}

export interface RenameSubjectInput {
  subjectId: string;
  name: string;
  revision: number;
  expectedLibraryRevision: number;
}

export interface MoveSubjectInput {
  subjectId: string;
  targetIndex: number;
  expectedLibraryRevision: number;
}

/** Deleting a subject sends its decks and documents to No subject. */
export interface DeleteSubjectInput {
  subjectId: string;
  revision: number;
  expectedLibraryRevision: number;
}

export interface MoveDeckToSubjectInput {
  deckId: string;
  subjectId: string | null;
  /** Position among the destination's decks; the end when omitted. */
  targetIndex?: number;
  expectedLibraryRevision: number;
}

export interface MoveDocumentToSubjectInput {
  documentId: string;
  subjectId: string | null;
  expectedDocumentFilingRevision: number;
}

export interface SetDocumentStudyRangesInput {
  documentId: string;
  /** An empty list studies the whole document. */
  ranges: SourceRangeInput[];
  expectedDocumentFilingRevision: number;
}

export interface DismissSubjectFilingReviewInput {
  entityKind: SubjectFilingEntityKind;
  entityId: string;
}

export async function getSubjectLibrary(): Promise<SubjectLibrary> {
  return invoke<SubjectLibrary>("get_subject_library");
}

export async function createSubject(input: CreateSubjectInput): Promise<SubjectLibrary> {
  return invoke<SubjectLibrary>("create_subject", { input });
}

export async function renameSubject(input: RenameSubjectInput): Promise<SubjectLibrary> {
  return invoke<SubjectLibrary>("rename_subject", { input });
}

export async function moveSubject(input: MoveSubjectInput): Promise<SubjectLibrary> {
  return invoke<SubjectLibrary>("move_subject", { input });
}

export async function deleteSubject(input: DeleteSubjectInput): Promise<SubjectLibrary> {
  return invoke<SubjectLibrary>("delete_subject", { input });
}

export async function moveDeckToSubject(input: MoveDeckToSubjectInput): Promise<SubjectLibrary> {
  return invoke<SubjectLibrary>("move_deck_to_subject", { input });
}

export async function moveDocumentToSubject(
  input: MoveDocumentToSubjectInput,
): Promise<SubjectLibrary> {
  return invoke<SubjectLibrary>("move_document_to_subject", { input });
}

export async function setDocumentStudyRanges(
  input: SetDocumentStudyRangesInput,
): Promise<SubjectLibrary> {
  return invoke<SubjectLibrary>("set_document_study_ranges", { input });
}

export async function getSubjectFilingReviews(): Promise<SubjectFilingReview[]> {
  return invoke<SubjectFilingReview[]>("get_subject_filing_reviews");
}

export async function dismissSubjectFilingReview(
  input: DismissSubjectFilingReviewInput,
): Promise<SubjectFilingReview[]> {
  return invoke<SubjectFilingReview[]>("dismiss_subject_filing_review", { input });
}
