import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ShowToast } from "../components/Toast";
import { getErrorCode, getErrorMessage } from "../ipc/common";
import { cancelPdfFlashcardGeneration, deletePdfDocument, generateMorePdfFlashcards,
  getDocumentGenerationProgress, importPdfAndGenerateFlashcards, listDocuments, openDocumentSource,
  retryPdfFlashcardGeneration, type DocumentGenerationProgress, type DocumentSummary } from "../ipc/documents";
import { cancelDocumentStudyGuideGeneration, generateDocumentStudyGuide } from "../ipc/studyGuides";
import { selectExternalImport } from "../platform/files";
import { useConfirm } from "../ui/overlays/overlayContext";
import { useRequestLane } from "./requestLane";

export interface DocumentJobServices {
  refreshDecks: () => Promise<unknown>;
  showToast: ShowToast;
}

/** Application-lifetime job coordination. Only explicit stop cancels native work. */
export function useDocumentJobsController({ refreshDecks, showToast }: DocumentJobServices) {
  const [documents, setDocuments] = useState<DocumentSummary[]>([]);
  const [generationProgress, setGenerationProgress] = useState<
    Record<string, DocumentGenerationProgress | null>
  >({});
  const [clockNow, setClockNow] = useState(() => Date.now());
  const [isLoading, setIsLoading] = useState(true);
  const [isUploading, setIsUploading] = useState(false);
  const [busyDocumentId, setBusyDocumentId] = useState<string | null>(null);
  const [cancelingDocumentId, setCancelingDocumentId] = useState<string | null>(null);
  const [deletingDocumentId, setDeletingDocumentId] = useState<string | null>(null);
  const [guideGeneratingDocumentId, setGuideGeneratingDocumentId] = useState<string | null>(null);
  const [cancelingGuideDocumentId, setCancelingGuideDocumentId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const confirm = useConfirm();

  const alive = useRef(true);
  const writing = useRef(false);
  const stopping = useRef(new Set<string>());
  const latestDocuments = useRef(documents);
  latestDocuments.current = documents;
  const refreshLane = useRequestLane();
  const progressLane = useRequestLane();
  const hasProcessingDocument = useMemo(
    () => documents.some((document) => document.lifecycleState === "processing"), [documents],
  );
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
    const entries = await Promise.all(active.map(async (document) => [
      document.id, await getDocumentGenerationProgress(document.id).catch(() => null),
    ] as const));
    if (alive.current && progressLane.isCurrent(token)) {
      setGenerationProgress(Object.fromEntries(entries));
      setClockNow(Date.now());
    }
  }, [progressLane]);
  useEffect(() => {
    void refreshDocuments().catch((error: unknown) => {
      if (alive.current) setErrorMessage(getErrorMessage(error));
    }).finally(() => { if (alive.current) setIsLoading(false); });
  }, [refreshDocuments]);
  useEffect(() => {
    if (!isUploading && !busyDocumentId && !hasProcessingDocument && !guideGeneratingDocumentId) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const poll = async () => {
      try { await refreshDocuments(); await refreshLiveProgress(); }
      catch { /* The operation owner reports terminal failures. */ }
      finally { if (active) timer = setTimeout(() => { void poll(); }, 1000); }
    };
    void poll();
    return () => { active = false; clearTimeout(timer); };
  }, [isUploading, busyDocumentId, hasProcessingDocument, guideGeneratingDocumentId, refreshDocuments, refreshLiveProgress]);
  useEffect(() => {
    if (cancelingDocumentId && !documents.some((document) => document.id === cancelingDocumentId && document.lifecycleState === "processing")) {
      stopping.current.delete(`document:${cancelingDocumentId}`);
      setCancelingDocumentId(null);
    }
  }, [cancelingDocumentId, documents]);

  function acquire(): boolean {
    if (writing.current || hasProcessingDocument) return false;
    writing.current = true;
    setErrorMessage(null);
    return true;
  }
  async function reconcileSaved(): Promise<void> {
    try { await Promise.all([refreshDocuments(), refreshDecks()]); }
    catch (error: unknown) {
      if (alive.current) setErrorMessage(`Saved successfully; refreshing the library failed: ${getErrorMessage(error)}`);
    }
  }

  async function uploadDocument(): Promise<void> {
    if (!acquire()) return;
    setIsUploading(true);
    try {
      const selected = await selectExternalImport("document");
      if (!selected) return;
      showToast(
        "info",
        "Document processing started. Live stage, source-unit, request, and token progress will appear here.",
      );
      const result = await importPdfAndGenerateFlashcards(selected.id);
      await reconcileSaved();
      showGenerationSuccess(result.createdCardCount, showToast);
    } catch (error: unknown) {
      await handleGenerationFailure(error);
    } finally {
      writing.current = false;
      if (alive.current) setIsUploading(false);
    }
  }

  async function retry(document: DocumentSummary): Promise<void> {
    if (!acquire()) return;
    setBusyDocumentId(document.id);
    setErrorMessage(null);
    try {
      const result = await retryPdfFlashcardGeneration(document.id);
      await reconcileSaved();
      showGenerationSuccess(result.createdCardCount, showToast);
    } catch (error: unknown) {
      await handleGenerationFailure(error);
    } finally {
      writing.current = false;
      if (alive.current) setBusyDocumentId(null);
    }
  }

  async function generateMore(document: DocumentSummary): Promise<void> {
    if (!document.generatedDeckId || !acquire()) return;
    setBusyDocumentId(document.id);
    setErrorMessage(null);
    try {
      const result = await generateMorePdfFlashcards(document.id);
      await reconcileSaved();
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
      writing.current = false;
      if (alive.current) setBusyDocumentId(null);
    }
  }

  async function generateStudyGuide(document: DocumentSummary): Promise<void> {
    if (document.lifecycleState === "processing" || !acquire()) return;
    setGuideGeneratingDocumentId(document.id);
    setCancelingGuideDocumentId(null);
    setErrorMessage(null);
    try {
      const guide = await generateDocumentStudyGuide(document.id);
      await reconcileSaved();
      showToast(
        "success",
        `Study guide revision ${guide.revision} created. Open it from Study Guides.`,
      );
    } catch (error: unknown) {
      const code = getErrorCode(error);
      if (code === "document_study_guide_canceled" || code === "ai_request_canceled") {
        setErrorMessage(null);
        showToast("info", "Study-guide generation stopped. Saved topic progress can be reused on retry.");
      } else {
        const message = getErrorMessage(error);
        setErrorMessage(message);
        showToast("error", message);
      }
      await refreshDocuments().catch(() => undefined);
    } finally {
      writing.current = false;
      stopping.current.delete(`guide:${document.id}`);
      if (alive.current) {
        setGuideGeneratingDocumentId(null);
        setCancelingGuideDocumentId(null);
      }
    }
  }

  async function stopStudyGuide(document: DocumentSummary): Promise<void> {
    const key = `guide:${document.id}`;
    if (guideGeneratingDocumentId !== document.id || stopping.current.has(key)) return;
    stopping.current.add(key);
    setCancelingGuideDocumentId(document.id);
    try {
      await cancelDocumentStudyGuideGeneration(document.id);
      showToast("info", "Stopping study-guide generation…");
    } catch (error: unknown) {
      stopping.current.delete(key);
      setCancelingGuideDocumentId(null);
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
    stopping.current.add(key);
    setCancelingDocumentId(document.id);
    try {
      await cancelPdfFlashcardGeneration(document.id);
      showToast("info", "Stopping the current AI request…");
    } catch (error: unknown) {
      stopping.current.delete(key);
      setCancelingDocumentId(null);
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

  async function deleteDocument(document: DocumentSummary): Promise<void> {
    if (document.lifecycleState === "processing" || !acquire()) return;
    try {
    const accepted = await confirm({
      title: "Delete this document permanently?",
      description:
        "The managed source, extracted structure, local derivatives, learning plan, study guides, annotations, and generation jobs for this document will be removed. Any surviving generated decks and cards remain, with structured source citations and available page, slide, chapter, or section locators preserved and marked as source removed. This cannot be undone.",
      confirmLabel: "Delete document",
      destructive: true,
    });
    if (!accepted) return;

    setDeletingDocumentId(document.id);
      const result = await deletePdfDocument(document.id, document.revision);
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
      writing.current = false;
      if (alive.current) setDeletingDocumentId(null);
    }
  }

  return { documents, generationProgress, clockNow, isLoading, isUploading, busyDocumentId,
    cancelingDocumentId, deletingDocumentId, guideGeneratingDocumentId, cancelingGuideDocumentId,
    errorMessage, hasProcessingDocument, refreshDocuments, uploadDocument, retry, generateMore,
    generateStudyGuide, stopStudyGuide, stopGeneration, openSource, deleteDocument };
}

function showGenerationSuccess(createdCardCount: number, showToast: ShowToast): void {
  showToast("success", `Created ${createdCardCount} flashcard${createdCardCount === 1 ? "" : "s"} in a new deck.`);
}
