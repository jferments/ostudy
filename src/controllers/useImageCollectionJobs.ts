import { useCallback, useRef, useState } from "react";
import type { ShowToast } from "../components/Toast";
import { createOperationId, getErrorCode, getErrorMessage } from "../ipc/common";
import { generateMoreDocumentFlashcards, listDocuments, retryDocumentFlashcardGeneration } from "../ipc/documents";
import type { StagedExternalFile } from "../ipc/externalFiles";
import { amendImageSet, getImageSet, importImageSet, prepareImageSet,
  type AmendImageSetInput, type ImportImageSetInput } from "../ipc/imageSets";
import { generateDocumentStudyGuide } from "../ipc/studyGuides";
import { stageBrowserImage } from "../platform/imageInput";

export interface CollectionUploadItem {
  id: string;
  name: string;
  status: "queued" | "uploading" | "saved" | "failed";
  error: string | null;
}
export interface ImageCollectionJob {
  documentId: string | null;
  items: CollectionUploadItem[];
  phase: "idle" | "uploading" | "cards" | "guide" | "ready" | "failed" | "stopped";
  error: string | null;
}
interface Selection {
  id: string;
  file: File;
  staged?: StagedExternalFile;
}
interface Batch {
  id: string;
  selections: Selection[];
  request?: { kind: "import"; input: ImportImageSetInput } | { kind: "append"; input: AmendImageSetInput };
}
interface Session {
  key: string;
  value: ImageCollectionJob;
  batches: Batch[];
  running: boolean;
  stopped: boolean;
  needsCards: boolean;
  needsGuide: boolean;
}
interface Services {
  changed: () => Promise<void>;
  activeDocument: (id: string, guide: boolean, active: boolean) => void;
  showToast: ShowToast;
}

/** Application-lifetime owner. Closing the collection page does not cancel work.
 * Each acknowledged source mutation is removed from the queue before AI starts:
 * a failed generation or refresh must never repeat an import or append. */
export function useImageCollectionJobs(services: Services) {
  const service = useRef(services);
  service.current = services;
  const sessions = useRef(new Map<string, Session>());
  const [jobs, setJobs] = useState<Record<string, ImageCollectionJob>>({});

  const publish = useCallback((session: Session) => {
    setJobs((previous) => ({ ...previous, [session.key]: {
      ...session.value, items: session.value.items.map((item) => ({ ...item })),
    } }));
  }, []);

  const drain = useCallback(async (session: Session): Promise<void> => {
    if (session.running) return;
    session.running = true;
    session.stopped = false;
    session.value.error = null;
    try {
      while (!session.stopped) {
        while (session.batches.length > 0 && !session.stopped) {
          const batch = session.batches[0];
          session.value.phase = "uploading";
          publish(session);
          for (const selection of batch.selections) {
            if (selection.staged) continue;
            const row = session.value.items.find((item) => item.id === selection.id);
            if (!row) throw new Error("The pending image selection is missing.");
            row.status = "uploading";
            row.error = null;
            publish(session);
            try {
              selection.staged = await stageBrowserImage(selection.file, selection.id);
            } catch (error: unknown) {
              row.status = "failed";
              row.error = getErrorMessage(error);
              throw error;
            }
          }
          if (session.stopped) break;
          // Preserve the complete request across an ambiguous IPC acknowledgement.
          // A newly read revision must not accidentally turn retry into another append.
          if (!batch.request) {
            const append = batch.selections.map((selection) => ({
              stagedFileId: selection.id, quarterTurns: 0 as const, inclusion: null,
            }));
            if (session.value.documentId) {
              const current = await getImageSet(session.value.documentId);
              batch.request = { kind: "append", input: {
                operationId: batch.id, documentId: current.documentId,
                expectedRevision: current.revision, expectedFilterRevision: current.filterRevision,
                members: current.members.map((member) => ({
                  memberId: member.id, quarterTurns: member.quarterTurns, inclusion: member.inclusion,
                })), append,
              } };
            } else {
              const firstName = batch.selections[0]?.file.name ?? "Image collection";
              batch.request = { kind: "import", input: {
                operationId: batch.id,
                title: firstName.replace(/\.[^.]+$/u, "").trim() || "Image collection",
                members: append,
              } };
            }
          }
          const result = batch.request.kind === "import"
            ? await importImageSet(batch.request.input)
            : await amendImageSet(batch.request.input);
          session.value.documentId = result.documentId;
          for (const selection of batch.selections) {
            const row = session.value.items.find((item) => item.id === selection.id);
            if (row) { row.status = "saved"; row.error = null; }
          }
          session.batches.shift(); // Release browser File references after durable ownership.
          session.needsCards = true;
          session.needsGuide = true;
          service.current.activeDocument(result.documentId, false, true);
          publish(session);
          await service.current.changed();
        }
        const id = session.value.documentId;
        if (!id || session.stopped) break;
        if (session.needsCards) {
          session.value.phase = "cards";
          service.current.activeDocument(id, false, true);
          publish(session);
          // Native preparation and relevance selection, never user region editing.
          // Additional generation requires the amended canonical source first.
          await prepareImageSet(id);
          if (session.stopped) break;
          const document = (await listDocuments()).find((item) => item.id === id);
          if (!document) throw new Error("The saved image collection could not be found.");
          let count = 0;
          try {
            const result = document.generatedDeckId
              ? await generateMoreDocumentFlashcards(id)
              : await retryDocumentFlashcardGeneration(id);
            count = result.createdCardCount;
          } catch (error: unknown) {
            if (!document.generatedDeckId || getErrorCode(error) !== "document_no_additional_cards") throw error;
          }
          session.needsCards = false;
          service.current.activeDocument(id, false, false);
          await service.current.changed();
          service.current.showToast(count > 0 ? "success" : "info", count > 0
            ? `Created ${count} flashcards from the image collection.`
            : "The existing deck already covers these images; no duplicate cards were added.");
        }
        // Images added during a request belong to this collection, not another deck.
        // Save them before building a guide from an already superseded snapshot.
        if (session.batches.length > 0) continue;
        if (session.needsGuide && !session.stopped) {
          session.value.phase = "guide";
          service.current.activeDocument(id, true, true);
          publish(session);
          await generateDocumentStudyGuide(id);
          session.needsGuide = false;
          service.current.activeDocument(id, true, false);
          await service.current.changed();
          service.current.showToast("success", "Study guide created from the image collection.");
        }
        if (session.batches.length === 0) break;
      }
      session.value.phase = session.stopped ? "stopped" : session.value.documentId ? "ready" : "idle";
    } catch (error: unknown) {
      const code = getErrorCode(error);
      const canceled = session.stopped || code === "ai_request_canceled"
        || code === "document_generation_canceled" || code === "document_study_guide_canceled";
      session.value.phase = canceled ? "stopped" : "failed";
      session.value.error = canceled ? null : getErrorMessage(error);
      if (!canceled) service.current.showToast("error", session.value.error ?? "Image processing failed.");
    } finally {
      session.running = false;
      if (session.value.documentId) {
        service.current.activeDocument(session.value.documentId, false, false);
        service.current.activeDocument(session.value.documentId, true, false);
      }
      publish(session);
      await service.current.changed();
    }
  }, [publish]);

  const addImages = useCallback((key: string, documentId: string | null, files: readonly File[]) => {
    if (files.length === 0) return;
    let session = sessions.current.get(key);
    if (!session) {
      session = { key, value: { documentId, items: [], phase: "uploading", error: null },
        batches: [], running: false, stopped: false, needsCards: false, needsGuide: false };
      sessions.current.set(key, session);
    }
    const pendingBytes = session.batches.reduce((sum, batch) =>
      sum + batch.selections.reduce((size, selection) => size + selection.file.size, 0), 0);
    if (session.value.items.length + files.length > 128
      || pendingBytes + files.reduce((sum, file) => sum + file.size, 0) > 512 * 1024 * 1024) {
      service.current.showToast("error", "A collection supports up to 128 images and 512 MB. Add a smaller batch.");
      return;
    }
    const selections = files.map((file) => ({ id: createOperationId(), file }));
    session.batches.push({ id: createOperationId(), selections });
    session.value.items.push(...selections.map(({ id, file }) => ({
      id, name: file.name || "Clipboard image.png", status: "queued" as const, error: null,
    })));
    publish(session);
    void drain(session);
  }, [drain, publish]);

  const retryImages = useCallback((key: string) => {
    const session = sessions.current.get(key);
    if (session) void drain(session);
  }, [drain]);

  const removeFailedImage = useCallback((key: string, id: string) => {
    const session = sessions.current.get(key);
    if (!session || session.running) return;
    const batch = session.batches.find((candidate) => candidate.selections.some((item) => item.id === id));
    const row = session.value.items.find((item) => item.id === id);
    // Once a source write was attempted, its membership cannot change on retry.
    if (!batch || batch.request || row?.status !== "failed") return;
    batch.selections = batch.selections.filter((item) => item.id !== id);
    session.value.items = session.value.items.filter((item) => item.id !== id);
    session.batches = session.batches.filter((candidate) => candidate.selections.length > 0);
    publish(session);
    void drain(session);
  }, [drain, publish]);

  const stopForDocument = useCallback((id: string) => {
    for (const session of sessions.current.values()) {
      if (session.value.documentId === id) session.stopped = true;
    }
  }, []);

  return { jobs, addImages, retryImages, removeFailedImage, stopForDocument };
}
