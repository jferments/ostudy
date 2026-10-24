import type { AppSettings } from "../../ipc/settings";
import type { StudyGuidePdfTheme } from "../../ipc/studyGuides";

/** The PDF look and notes choice from Settings, with the defaults a new library starts with. */
export function guidePdfPreferences(settings: AppSettings): { theme: StudyGuidePdfTheme; includeNotes: boolean } {
  return { theme: settings.guidePdfTheme ?? "sage", includeNotes: settings.guidePdfIncludeNotes ?? true };
}
