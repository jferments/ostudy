import { useMemo } from "react";

import { useDocumentJobs } from "../../controllers/DocumentJobsProvider";
import { processingPhaseLabel } from "../documents/DocumentProgress";
import type { DocumentPreparations, SubjectPreparation } from "./homeModel";

/** The documents being read or written up now, by id, with the step each is on. */
export function useDocumentPreparations(): DocumentPreparations {
  const jobs = useDocumentJobs();
  return useMemo<DocumentPreparations>(() => {
    const running = new Map<string, SubjectPreparation>();
    if (!jobs) return running;
    for (const document of jobs.documents) {
      if (document.lifecycleState === "processing") {
        running.set(document.id, {
          step: processingPhaseLabel(
            document,
            jobs.cancelingDocumentIds.has(document.id),
            jobs.generationProgress[document.id] ?? null,
          ),
          documentTitle: document.title,
        });
      } else if (jobs.guideGeneratingDocumentIds.has(document.id)) {
        running.set(document.id, { step: "Writing a study guide", documentTitle: document.title });
      }
    }
    return running;
  }, [jobs]);
}
