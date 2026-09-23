import { useCallback, useEffect, useMemo, useRef, useState, type Dispatch, type SetStateAction } from "react";
import type { ShowToast } from "../components/Toast";
import { getErrorCode, getErrorMessage } from "../ipc/common";
import {
  cancelDocumentFlashcardGeneration,
  deleteDocument,
  generateMoreDocumentFlashcards,
  getDocumentGenerationProgress,
  importDocument,
  listDocuments,
  openDocumentSource,
  startDocumentFlashcardGeneration,
  type DocumentGenerationProgress,
  type DocumentSummary,
} from "../ipc/documents";
import {
  cancelDocumentStudyGuideGeneration,
  generateDocumentStudyGuide,
  getDocumentLearningPlan,
  type DocumentLearningPlan,
} from "../ipc/studyGuides";
import { selectExternalImport } from "../platform/files";
import { useConfirm } from "../ui/overlays/overlayContext";
import { useRequestLane } from "./requestLane";
import { useImageCollectionJobs } from "./useImageCollectionJobs";

export interface DocumentJobServices {
  refreshDecks: () => Promise<unknown>;
  showToast: ShowToast;
}

type IdSet = ReadonlySet<string>;

function updateIdSet(
  setter: Dispatch<SetStateAction<Set<string>>>,
  id: string,
  active: boolean,
): void {
  setter((current) => {
    if (current.has(id) === active) return current;
    const next = new Set(current);
    if (active) next.add(id);
    else next.delete(id);
    return next;
  });
}

/** Application-lifetime job coordination. Only explicit stop cancels native work. */
export function useDocumentJobsController({ refreshDecks, showToast }: DocumentJobServices) {
  const [documents, setDocuments] = useState<DocumentSummary[]>([]);
  const [generationProgress, setGenerationProgress] = useState<
    Record<string, DocumentGenerationProgress | null>
  >({});
  const [learningPlans, setLearningPlans] = useState<Record<string, DocumentLearningPlan | null>>({});
  const [clockNow, setClockNow] = useState(() => Date.now());
  const [isLoading, setIsLoading] = useState(true);
  const [isUploading, setIsUploading] = useState(false);
  const [busyDocumentIds, setBusyDocumentIds] = useState<Set<string>>(new Set());
  const [cancelingDocumentIds, setCancelingDocumentIds] = useState<Set<string>>(new Set());
  const [deletingDocumentIds, setDeletingDocumentIds] = useState<Set<string>>(new Set());
  const [guideGeneratingDocumentIds, setGuideGeneratingDocumentIds] = useState<Set<string>>(new Set());
  const [cancelingGuideDocumentIds, setCancelingGuideDocumentIds] = useState<Set<string>>(new Set());
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const confirm = useConfirm();

  const alive = useRef(true);
  const selectingFile = useRef(false);
  const stopping = useRef(new Set<string>());
  const backgroundFlashcardIds = useRef(new Set<string>());
  const automaticGuideIds = useRef(new Set<string>());
  const latestDocuments = useRef(documents);
  latestDocuments.current = documents;
  const refreshLane = useRequestLane();
  const progressLane = useRequestLane();
  const hasProcessingDocument = useMemo(
    () => documents.some((document) => document.lifecycleState === "processing"), [documents],
  );
  const hasControllerJob = busyDocumentIds.size > 0 || guideGeneratingDocumentIds.size > 0;

  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; };
  }, []);

  const refreshDocuments = useCallback(async () => {
    const token = refreshLane.begin();
    const loaded = await listDocuments();
    if (alive.current && refreshLane.isCurrent(token)) setDocuments(loaded);
    return loaded;
  }, [refreshLane]);

  const refreshLiveProgress = useCallback(async () => {
    const token = progressLane.begin();
    const active = latestDocuments.current.filter((document) => document.lifecycleState === "processing");
    const entries = await Promise.all(active.map(async (document) => {
      const [progress, plan] = await Promise.all([
        getDocumentGenerationProgress(document.id).catch(() => null),
        getDocumentLearningPlan(document.id).catch(() => null),
      ]);
      return [document.id, progress, plan] as const;
    }));
    if (alive.current && progressLane.isCurrent(token)) {
      setGenerationProgress(Object.fromEntries(entries.map(([id, progress]) => [id, progress])));
      setLearningPlans((current) => {
        const next = { ...current };
        for (const [id, , plan] of entries) next[id] = plan;
        return next;
      });
      setClockNow(Date.now());
    }
  }, [progressLane]);

  useEffect(() => {
    void refreshDocuments().catch((error: unknown) => {
      if (alive.current) setErrorMessage(getErrorMessage(error));
    }).finally(() => { if (alive.current) setIsLoading(false); });
  }, [refreshDocuments]);

  useEffect(() => {
    if (!isUploading && !hasProcessingDocument && !hasControllerJob) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const poll = async () => {
      try {
        await refreshDocuments();
        await refreshLiveProgress();
      } catch {
        // The operation owner reports terminal failures.
      } finally {
        if (active) timer = setTimeout(() => { void poll(); }, 1000);
      }
    };
    void poll();
    return () => { active = false; clearTimeout(timer); };
  }, [
    isUploading,
    hasProcessingDocument,
    hasControllerJob,
    refreshDocuments,
    refreshLiveProgress,
  ]);

  useEffect(() => {
    const processing = new Set(
      documents
        .filter((document) => document.lifecycleState === "processing")
        .map((document) => document.id),
    );
    setCancelingDocumentIds((current) => {
      const next = new Set([...current].filter((id) => processing.has(id)));
      if (next.size === current.size) return current;
      for (const id of current) {
        if (!next.has(id)) stopping.current.delete(`document:${id}`);
      }
      return next;
    });
  }, [documents]);

  useEffect(() => {
    for (const document of documents) {
      if (!backgroundFlashcardIds.current.has(document.id)
        || document.lifecycleState === "processing") continue;

      backgroundFlashcardIds.current.delete(document.id);
      stopping.current.delete(`document:${document.id}`);
      updateIdSet(setCancelingDocumentIds, document.id, false);
      setGenerationProgress((current) => {
        if (!(document.id in current)) return current;
        const next = { ...current };
        delete next[document.id];
        return next;
      });

      if (document.safeErrorCode === "document_generation_canceled"
        || document.safeErrorCode === "ai_request_canceled") {
        setErrorMessage(null);
        showToast("info", "Generation stopped. The document is still available to retry.");
        continue;
      }
      if (document.lifecycleState === "ready" && document.generatedDeckId) {
        setErrorMessage(null);
        showGenerationSuccess(document.generatedCardCount, showToast);
        void refreshDecks().catch((error: unknown) => {
          if (alive.current) {
            setErrorMessage(
              `Saved successfully; refreshing the deck library failed: ${getErrorMessage(error)}`,
            );
          }
        });
        void ensureAutomaticStudyGuide(document);
        continue;
      }
      const message = document.safeErrorMessage || "Document generation failed.";
      setErrorMessage(message);
      showToast("error", message);
    }
  }, [documents, refreshDecks, showToast]);

  async function startBackgroundFlashcardGeneration(documentId: string): Promise<void> {
    try {
      await startDocumentFlashcardGeneration(documentId);
    } catch (error) {
      throw error;
    }
    // Native Start does not acknowledge until the cancellation identity and
    // durable processing state both exist. Track terminal observation only
    // after that acknowledgement so the pre-start failed/ready row cannot be
    // mistaken for this new job's result.
    backgroundFlashcardIds.current.add(documentId);
    try {
      await refreshDocuments();
    } catch (error: unknown) {
      if (alive.current) {
        setErrorMessage(
          `Generation started; refreshing Documents failed: ${getErrorMessage(error)}`,
        );
      }
    }
  }

  async function ensureAutomaticStudyGuide(document: DocumentSummary): Promise<void> {
    if (automaticGuideIds.current.has(document.id)
      || guideGeneratingDocumentIds.has(document.id)) return;
    automaticGuideIds.current.add(document.id);
    updateIdSet(setGuideGeneratingDocumentIds, document.id, true);
    updateIdSet(setCancelingGuideDocumentIds, document.id, false);
    try {
      const guide = await generateDocumentStudyGuide(document.id);
      await reconcileSaved();
      showToast(
        "success",
        `Study guide revision ${guide.revision} created automatically. Open it from Study Guides.`,
      );
    } catch (error: unknown) {
      const code = getErrorCode(error);
      if (code === "document_study_guide_canceled" || code === "ai_request_canceled") {
        showToast("info", "Automatic study-guide generation stopped. Saved topic progress can be reused.");
      } else {
        const message = getErrorMessage(error);
        setErrorMessage(`Flashcards were saved, but automatic Study Guide generation failed: ${message}`);
        showToast("error", `Flashcards were saved, but the Study Guide failed: ${message}`);
      }
      await refreshDocuments().catch(() => undefined);
    } finally {
      automaticGuideIds.current.delete(document.id);
      stopping.current.delete(`guide:${document.id}`);
      updateIdSet(setGuideGeneratingDocumentIds, document.id, false);
      updateIdSet(setCancelingGuideDocumentIds, document.id, false);
    }
  }

  async function reconcileSaved(): Promise<void> {
    try {
      await Promise.all([refreshDocuments(), refreshDecks()]);
    } catch (error: unknown) {
      if (alive.current) {
        setErrorMessage(`Saved successfully; refreshing the library failed: ${getErrorMessage(error)}`);
      }
    }
  }

  const imageCollections = useImageCollectionJobs({
    changed: reconcileSaved,
    activeDocument(id, guide, active) {
      if (!alive.current) return;
      updateIdSet(guide ? setGuideGeneratingDocumentIds : setBusyDocumentIds, id, active);
      if (!active) {
        updateIdSet(guide ? setCancelingGuideDocumentIds : setCancelingDocumentIds, id, false);
      }
    },
    showToast,
  });

  async function uploadDocument(): Promise<void> {
    if (selectingFile.current) return;
    selectingFile.current = true;
    setIsUploading(true);
    setErrorMessage(null);
    let selected: Awaited<ReturnType<typeof selectExternalImport>>;
    try {
      selected = await selectExternalImport("document");
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
      return;
    } finally {
      selectingFile.current = false;
      if (alive.current) setIsUploading(false);
    }
    if (!selected) return;

    let documentId: string | null = null;
    try {
      const imported = await importDocument(selected.id);
      documentId = imported.documentId;
      updateIdSet(setBusyDocumentIds, documentId, true);
      await startBackgroundFlashcardGeneration(documentId);
      showToast(
        "info",
        "Document processing started. You can add more documents while this job runs.",
      );
    } catch (error: unknown) {
      await handleGenerationFailure(error);
    } finally {
      if (documentId) updateIdSet(setBusyDocumentIds, documentId, false);
    }
  }

  async function retry(document: DocumentSummary): Promise<void> {
    if (document.lifecycleState === "processing"
      || busyDocumentIds.has(document.id)
      || guideGeneratingDocumentIds.has(document.id)) return;
    updateIdSet(setBusyDocumentIds, document.id, true);
    setErrorMessage(null);
    try {
      await startBackgroundFlashcardGeneration(document.id);
    } catch (error: unknown) {
      await handleGenerationFailure(error);
    } finally {
      updateIdSet(setBusyDocumentIds, document.id, false);
    }
  }

  async function generateMore(document: DocumentSummary): Promise<void> {
    if (!document.generatedDeckId
      || busyDocumentIds.has(document.id)
      || guideGeneratingDocumentIds.has(document.id)) return;
    updateIdSet(setBusyDocumentIds, document.id, true);
    setErrorMessage(null);
    try {
      const result = await generateMoreDocumentFlashcards(document.id);
      await reconcileSaved();
      void ensureAutomaticStudyGuide(document);
      if (result.createdCardCount === 0) {
        showToast("info", "The second pass did not find any additional non-duplicate cards to add.");
      } else {
        showToast(
          "success",
          `Added ${result.createdCardCount} new flashcard${result.createdCardCount === 1 ? "" : "s"} to the existing deck.`,
        );
      }
    } catch (error: unknown) {
      await handleGenerationFailure(error);
    } finally {
      updateIdSet(setBusyDocumentIds, document.id, false);
    }
  }

  async function stopStudyGuide(document: DocumentSummary): Promise<void> {
    const key = `guide:${document.id}`;
    if (!guideGeneratingDocumentIds.has(document.id) || stopping.current.has(key)) return;
    imageCollections.stopForDocument(document.id);
    stopping.current.add(key);
    updateIdSet(setCancelingGuideDocumentIds, document.id, true);
    try {
      await cancelDocumentStudyGuideGeneration(document.id);
      showToast("info", "Stopping study-guide generation…");
    } catch (error: unknown) {
      stopping.current.delete(key);
      updateIdSet(setCancelingGuideDocumentIds, document.id, false);
      const code = getErrorCode(error);
      if (code !== "document_generation_not_running") {
        showToast("error", getErrorMessage(error));
      }
    }
  }

  async function handleGenerationFailure(error: unknown): Promise<void> {
    const code = getErrorCode(error);
    if (code === "document_generation_canceled" || code === "ai_request_canceled") {
      showToast("info", "Generation stopped. The document is still available to retry.");
      setErrorMessage(null);
    } else if (code === "document_no_additional_cards") {
      showToast("info", getErrorMessage(error));
      setErrorMessage(null);
    } else {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    }
    await refreshDocuments().catch(() => undefined);
  }

  async function stopGeneration(document: DocumentSummary): Promise<void> {
    const key = `document:${document.id}`;
    if (stopping.current.has(key)) return;
    imageCollections.stopForDocument(document.id);
    stopping.current.add(key);
    updateIdSet(setCancelingDocumentIds, document.id, true);
    try {
      await cancelDocumentFlashcardGeneration(document.id);
      showToast("info", "Stopping the current AI request…");
    } catch (error: unknown) {
      stopping.current.delete(key);
      updateIdSet(setCancelingDocumentIds, document.id, false);
      const code = getErrorCode(error);
      if (code !== "document_generation_not_running") {
        showToast("error", getErrorMessage(error));
      }
      await refreshDocuments().catch(() => undefined);
    }
  }

  async function openSource(document: DocumentSummary): Promise<void> {
    try {
      await openDocumentSource(document.id);
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  async function deleteDocumentRecord(document: DocumentSummary): Promise<void> {
    if (document.lifecycleState === "processing"
      || busyDocumentIds.has(document.id)
      || guideGeneratingDocumentIds.has(document.id)
      || deletingDocumentIds.has(document.id)) return;
    const accepted = await confirm({
      title: "Delete this document permanently?",
      description:
        "The managed source, extracted structure, local derivatives, learning plan, study guides, annotations, and generation jobs for this document will be removed. Any surviving generated decks and cards remain, with structured source citations and available page, slide, chapter, or section locators preserved and marked as source removed. This cannot be undone.",
      confirmLabel: "Delete document",
      destructive: true,
    });
    if (!accepted) return;

    updateIdSet(setDeletingDocumentIds, document.id, true);
    try {
      const result = await deleteDocument(document.id, document.revision);
      await reconcileSaved();
      showToast(
        "success",
        result.preservedDeckCitationCount > 0
          ? `Document deleted. Preserved structured source citations for ${result.preservedDeckCitationCount} deck${result.preservedDeckCitationCount === 1 ? "" : "s"}.`
          : "Document deleted from Documents.",
      );
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    } finally {
      updateIdSet(setDeletingDocumentIds, document.id, false);
    }
  }

  return {
    documents,
    generationProgress,
    learningPlans,
    clockNow,
    isLoading,
    isUploading,
    busyDocumentIds: busyDocumentIds as IdSet,
    cancelingDocumentIds: cancelingDocumentIds as IdSet,
    deletingDocumentIds: deletingDocumentIds as IdSet,
    guideGeneratingDocumentIds: guideGeneratingDocumentIds as IdSet,
    cancelingGuideDocumentIds: cancelingGuideDocumentIds as IdSet,
    errorMessage,
    hasProcessingDocument,
    refreshDocuments,
    uploadDocument,
    retry,
    generateMore,
    stopStudyGuide,
    stopGeneration,
    openSource,
    deleteDocument: deleteDocumentRecord,
    imageCollections,
  };
}

function showGenerationSuccess(createdCardCount: number, showToast: ShowToast): void {
  showToast("success", `Created ${createdCardCount} flashcard${createdCardCount === 1 ? "" : "s"} in a new deck.`);
}
