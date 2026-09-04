import type { AppSection, AppWorkflow } from "../controllers/useAppController";

/**
 * Selecting the already-active Study Guides destination is a deliberate
 * request to return from a nested guide viewer to the Study Guides library.
 */
export function shouldResetStudyGuidesRoute(
  requestedSection: AppSection,
  currentSection: AppSection,
  activeWorkflow: AppWorkflow | null,
): boolean {
  return (
    requestedSection === "study-guides" &&
    currentSection === "study-guides" &&
    activeWorkflow === null
  );
}
