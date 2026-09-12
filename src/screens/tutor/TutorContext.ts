import { createContext, useContext } from "react";

import type { TutorController } from "./useTutorController";

export const TutorContext = createContext<TutorController | null>(null);

export function useOptionalTutor(): TutorController | null {
  return useContext(TutorContext);
}

export function useTutor(): TutorController {
  const tutor = useOptionalTutor();
  if (!tutor) throw new Error("Tutor components require the application TutorProvider.");
  return tutor;
}
