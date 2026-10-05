import type { FlashcardsRoute } from "../../../navigation/flashcardsRoutes";
import type { WorkbenchLaunch } from "./LibraryWorkbench";

/**
 * Reads the Library request the app made (Search, a subject's Manage button,
 * a deck to open, or a tag from Progress) as where the Workbench opens. The
 * route types stay as they are, so saved checkpoints and launch requests keep
 * opening Library.
 */
export function workbenchLaunchFor(route: FlashcardsRoute): WorkbenchLaunch {
  if (route.view === "deck") {
    return { requestId: route.requestId, scope: { kind: "deck", deckId: route.deckId } };
  }
  if (route.subjectFocus) {
    return { requestId: route.requestId, scope: { kind: "subject", subjectId: route.subjectFocus.subjectId } };
  }
  const tagId = route.analyticsSearchLaunch?.tagIds[0];
  if (tagId) return { requestId: route.requestId, scope: { kind: "tag", tagId } };
  if (route.workspace === "search") {
    return { requestId: route.requestId, scope: { kind: "view", view: "all" }, focusSearch: true };
  }
  return { requestId: route.requestId };
}
