import { lazy, Suspense, type ReactNode } from "react";

import { Sheet } from "../../ui/overlays/Dialog";
import { LoadingState } from "../../ui/primitives/EmptyState";
import { TutorContext } from "./TutorContext";
import { useTutorController } from "./useTutorController";

const TutorWorkspace = lazy(() => import("./TutorWorkspace").then((module) => ({ default: module.TutorWorkspace })));

/** Above the route layer: contextual chat does not unmount a study session. */
export function TutorProvider({ children }: { children: ReactNode }) {
  const tutor = useTutorController();
  return (
    <TutorContext.Provider value={tutor}>
      {children}
      {tutor.panelOpen ? (
        <Sheet title={tutor.target?.mode === "hint" ? "Flashcard hints" : "AI tutor"}
          description={tutor.target?.title ?? "Discuss your study material"}
          width="workspace" onClose={tutor.closePanel}>
          <Suspense fallback={<LoadingState label="Opening tutor…" />}><TutorWorkspace /></Suspense>
        </Sheet>
      ) : null}
    </TutorContext.Provider>
  );
}
