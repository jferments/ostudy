import type { AppSection, AppWorkflow } from "../controllers/useAppController";

/**
 * The five places the main navigation names, on every screen size. Settings
 * and Search live in the header beside them.
 */
export type AppDestination = "home" | "library" | "review" | "tutor" | "progress";

/**
 * Library hosts three of today's sections as its views, in the order its
 * switcher shows them. They keep their section ids so saved checkpoints and
 * launch requests still name them.
 */
export const LIBRARY_SECTIONS = ["decks", "documents", "study-guides"] as const;

export type LibrarySection = (typeof LIBRARY_SECTIONS)[number];

export function isLibrarySection(section: AppSection): section is LibrarySection {
  return (LIBRARY_SECTIONS as readonly AppSection[]).includes(section);
}

/**
 * The destination the navigation highlights. Practice runs inside Review
 * wherever it was started, so an active session highlights Review. Settings
 * belongs to the header gear and highlights no destination.
 */
export function destinationOf(
  section: AppSection,
  activeWorkflow: AppWorkflow | null,
): AppDestination | null {
  if (activeWorkflow === "practice") return "review";
  if (isLibrarySection(section)) return "library";
  switch (section) {
    case "home":
      return "home";
    case "review":
      return "review";
    case "tutor":
      return "tutor";
    case "analytics":
      return "progress";
    default:
      return null;
  }
}
