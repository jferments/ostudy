import { useEffect, useRef, useState } from "react";
import { listLatestDocumentStudyGuideJobs } from "../ipc/documentArtifactProgress";
import type { DocumentArtifactJob } from "../ipc/studyGuides";

export function isActiveArtifactJob(job: DocumentArtifactJob | null | undefined): boolean {
  return job !== null && job !== undefined
    && ["queued", "preparing", "generating", "committing"].includes(job.state);
}

/** Observe durable work. Unmounting stops reads only, never native generation. */
export function useDocumentArtifactProgress(watchingDocuments: boolean) {
  const [jobs, setJobs] = useState<Record<string, DocumentArtifactJob>>({});
  const [readFailed, setReadFailed] = useState(false);
  const latest = useRef(jobs);
  latest.current = jobs;
  useEffect(() => {
    let disposed = false;
    let inFlight = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const poll = async () => {
      if (disposed || inFlight) return;
      clearTimeout(timer);
      inFlight = true;
      try {
        const rows = await listLatestDocumentStudyGuideJobs();
        if (disposed) return;
        const snapshot = Object.fromEntries(rows.map((job) => [job.documentId, job]));
        latest.current = snapshot;
        setJobs(snapshot);
        setReadFailed(false);
      } catch {
        // A failed read is not an empty library or a failed generation. Keep the
        // last acknowledged job snapshot and retry while work remains active.
        if (!disposed) setReadFailed(true);
      } finally {
        inFlight = false;
        if (!disposed && (watchingDocuments || Object.values(latest.current).some(isActiveArtifactJob))) {
          timer = setTimeout(() => { void poll(); }, 1_000);
        }
      }
    };
    const refresh = () => { void poll(); };
    refresh();
    window.addEventListener("focus", refresh);
    return () => {
      disposed = true;
      clearTimeout(timer);
      window.removeEventListener("focus", refresh);
    };
  }, [watchingDocuments]);
  return { jobs, readFailed };
}
