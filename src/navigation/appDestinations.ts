import type { AppSection, AppWorkflow } from "../controllers/useAppController";

/**
 * The five places the main navigation names: Home, Library, Tutor, Progress and
 * Settings. Settings sits at the foot of the side rail on wider screens and in
 * the bottom bar on a phone; Search lives in the header.
 */
export type AppDestination = "home" | "library" | "tutor" | "progress" | "settings";

/**
 * Library's pages, in the order its tabs show them. Each one shows what the
 * chosen subject holds (see LibrarySubjectScope). Flashcards, Documents and
 * Study guides keep their section ids so saved checkpoints and launch
 * requests still name them.
 */
export const LIBRARY_SECTIONS = ["overview", "lessons", "study-guides", "documents", "notes", "decks"] as const;

export type LibrarySection = (typeof LIBRARY_SECTIONS)[number];

/** Which subject's materials Library shows: all of them, one subject, or No subject. */
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
 * legacy Review section and an active Practice session highlight Home.
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
    case "tutor":
      return "tutor";
    case "analytics":
      return "progress";
    case "settings":
      return "settings";
    default:
      return null;
  }
}
