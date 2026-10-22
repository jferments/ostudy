import { useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { isActiveArtifactJob, useDocumentArtifactProgress } from "../../controllers/useDocumentArtifactProgress";
import { getErrorCode, getErrorMessage } from "../../ipc/common";
import { cancelDocumentStudyGuideGeneration, generateDocumentStudyGuide } from "../../ipc/studyGuides";
import { guideProgress } from "../documents/LearningArtifactProgress";

function withId(ids: ReadonlySet<string>, id: string, present: boolean): ReadonlySet<string> {
  const next = new Set(ids);
  if (present) next.add(id); else next.delete(id);
  return next;
}

/**
 * Remakes a document's study guide from the Study guides page, and follows
 * every guide run there, wherever it started. The old guide stays readable
 * until the new revision is saved.
 */
export function useGuideRemake({ showToast, onRemade }: { showToast: ShowToast; onRemade: () => Promise<unknown> }) {
  // Runs this page started and is waiting on, by document.
  const [starting, setStarting] = useState<ReadonlySet<string>>(() => new Set());
  const [stopping, setStopping] = useState<ReadonlySet<string>>(() => new Set());
  // The finished job a remake replaces stays the latest until the new run is
  // recorded, so it must not read as the new run's result.
  const [replacedJobIds, setReplacedJobIds] = useState<Record<string, string>>({});
  const progress = useDocumentArtifactProgress(starting.size > 0);

  function jobOf(documentId: string) {
    const latest = progress.jobs[documentId] ?? null;
    return latest !== null && replacedJobIds[documentId] === latest.id ? null : latest;
  }
  function running(documentId: string): boolean {
    return starting.has(documentId) || isActiveArtifactJob(jobOf(documentId));
  }
  /** The bar for a guide being remade, or null when none is. */
  function progressOf(documentId: string): { label: string; value: number | null } | null {
    if (!running(documentId)) return null;
    const guide = guideProgress(jobOf(documentId), starting.has(documentId));
    return { label: stopping.has(documentId) ? "Stopping…" : `Remaking · ${guide.status}`, value: guide.value };
  }
  async function remake(documentId: string): Promise<void> {
    if (running(documentId)) return;
    const replaced = progress.jobs[documentId];
    if (replaced) setReplacedJobIds((ids) => ({ ...ids, [documentId]: replaced.id }));
    setStopping((ids) => withId(ids, documentId, false));
    setStarting((ids) => withId(ids, documentId, true));
    try {
      const guide = await generateDocumentStudyGuide(documentId);
      showToast("success", `Study guide revision ${guide.revision} made.`);
      await onRemade();
    } catch (error: unknown) {
      const code = getErrorCode(error);
      if (code === "document_study_guide_canceled" || code === "ai_request_canceled") {
        showToast("info", "Study guide remake stopped. Saved section progress can be reused.");
      } else {
        showToast("error", `Study guide remake failed: ${getErrorMessage(error)}`);
      }
    } finally {
      setStarting((ids) => withId(ids, documentId, false));
      setStopping((ids) => withId(ids, documentId, false));
    }
  }
  async function stop(documentId: string): Promise<void> {
    if (stopping.has(documentId)) return;
    setStopping((ids) => withId(ids, documentId, true));
    try {
      await cancelDocumentStudyGuideGeneration(documentId);
    } catch (error: unknown) {
      setStopping((ids) => withId(ids, documentId, false));
      showToast("error", getErrorMessage(error));
    }
  }
  return { running, progressOf, stopping: (documentId: string) => stopping.has(documentId), remake, stop };
}
