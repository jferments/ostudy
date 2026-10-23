import { invoke } from "./transport";

/**
 * How far the learner has read a document's active guide. A section counts as
 * read while its text is the text that was marked; remaking the guide keeps
 * marks on unchanged sections only. Reading never changes a card's schedule.
 */
export interface StudyGuideReading {
  documentId: string;
  guideId: string;
  sectionCount: number;
  /** In guide order. */
  readTopicIds: string[];
  /** The section to reopen, while it is still part of this guide. */
  lastTopicId: string | null;
  lastOpenedAt: number | null;
}

/** Null when the document has no guide. */
export async function getStudyGuideReading(
  documentId: string,
): Promise<StudyGuideReading | null> {
  return invoke<StudyGuideReading | null>("get_document_study_guide_reading", {
    input: { documentId },
  });
}

/**
 * Records the section the learner opened, so the guide reopens there. Fails
 * with `study_guide_changed` when `guideId` is no longer the document's guide.
 */
export async function openStudyGuideSection(
  documentId: string,
  guideId: string,
  topicId: string,
): Promise<StudyGuideReading> {
  return invoke<StudyGuideReading>("open_document_study_guide_section", {
    input: { documentId, guideId, topicId },
  });
}

/** Marks a section read or unread; saving the same state again is safe. */
export async function setStudyGuideSectionRead(
  documentId: string,
  guideId: string,
  topicId: string,
  read: boolean,
): Promise<StudyGuideReading> {
  return invoke<StudyGuideReading>("set_document_study_guide_section_read", {
    input: { documentId, guideId, topicId, read },
  });
}
