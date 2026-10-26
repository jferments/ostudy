import type { AppSection, AppWorkflow } from "../controllers/useAppController";

/**
 * The places the main navigation names: Home, Library, Notes, Tutor and
 * Progress. Settings is not one of them: it sits with Search at the foot of
 * the side rail on wider screens and in a phone's top bar.
 */
export type AppDestination = "home" | "library" | "notes" | "tutor" | "progress";

/**
 * A subject's pages, in the order its tabs show them. Flashcards, Documents
 * and Study guides keep their section ids so saved checkpoints and launch
 * requests still name them.
 */
export const SUBJECT_SECTIONS = ["lessons", "study-guides", "decks", "podcasts", "documents"] as const;

export type SubjectSection = (typeof SUBJECT_SECTIONS)[number];

/**
 * Library's sections: its own page of subject tiles (the `overview` id, which
 * older checkpoints name), then a subject's pages.
 */
export const LIBRARY_SECTIONS = ["overview", "decks", "study-guides", "lessons", "podcasts", "documents"] as const;

export type LibrarySection = (typeof LIBRARY_SECTIONS)[number];

export function isSubjectSection(section: AppSection): section is SubjectSection {
  return (SUBJECT_SECTIONS as readonly AppSection[]).includes(section);
}

/**
 * Which subject Library shows: `all` is Library's page of subjects (and the
 * Workbench's All cards), or one subject, or No subject.
 */
export type LibrarySubjectScope =
  | { kind: "all" }
  | { kind: "subject"; subjectId: string }
  | { kind: "none" };

export const ALL_SUBJECTS: LibrarySubjectScope = { kind: "all" };

export function sameSubjectScope(left: LibrarySubjectScope, right: LibrarySubjectScope): boolean {
  if (left.kind === "subject" && right.kind === "subject") return left.subjectId === right.subjectId;
  return left.kind === right.kind;
}

/**
 * Where `openLibrary` opens Library. No scope opens it as the learner last
 * left it; `subjectId` scopes it to one subject (`null` is No subject); and
 * `deckId` opens that deck.
 */
export interface LibraryScope {
  subjectId?: string | null;
  deckId?: string;
}

export function isLibrarySection(section: AppSection): section is LibrarySection {
  return (LIBRARY_SECTIONS as readonly AppSection[]).includes(section);
}

/**
 * The destination the navigation highlights. Review lives on Home, so the
 * legacy Review section and an active Practice session highlight Home;
 * Settings highlights its own button instead.
 */
export function destinationOf(
  section: AppSection,
  activeWorkflow: AppWorkflow | null,
): AppDestination | null {
  if (activeWorkflow === "practice") return "home";
  if (isLibrarySection(section)) return "library";
  switch (section) {
    case "home":
      return "home";
    case "review":
      return "home";
    case "notes":
      return "notes";
    case "tutor":
      return "tutor";
    case "analytics":
      return "progress";
    default:
      return null;
  }
}
