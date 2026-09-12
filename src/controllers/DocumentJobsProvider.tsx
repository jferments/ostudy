import { createContext, useContext, type ReactNode } from "react";
import { useDocumentJobsController, type DocumentJobServices } from "./useDocumentJobsController";

const Context = createContext<ReturnType<typeof useDocumentJobsController> | null>(null);

/** Mounted above route transitions: navigation is not a cancellation intent. */
export function DocumentJobsProvider({ children, ...services }: DocumentJobServices & { children: ReactNode }) {
  const jobs = useDocumentJobsController(services);
  return <Context.Provider value={jobs}>{children}</Context.Provider>;
}

export function useDocumentJobs() {
  return useContext(Context);
}
