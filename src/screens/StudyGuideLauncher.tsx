import { useCallback, useEffect, useMemo, useState } from "react";

import type { ShowToast } from "../components/Toast";
import { getErrorMessage } from "../ipc/common";
import { listDocuments, type DocumentSummary } from "../ipc/documents";
import { Dialog } from "../ui/overlays/Dialog";
import { StudyGuideDialog } from "./StudyGuideDialog";
import styles from "./StudyGuideLauncher.module.css";

interface StudyGuideLauncherProps {
  showToast: ShowToast;
}

export function StudyGuideLauncher({ showToast }: StudyGuideLauncherProps) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const [documents, setDocuments] = useState<DocumentSummary[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [selectedDocument, setSelectedDocument] = useState<DocumentSummary | null>(null);

  const loadDocuments = useCallback(async () => {
    setIsLoading(true);
    setErrorMessage(null);
    try {
      const loaded = await listDocuments(false);
      setDocuments(loaded);
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    } finally {
      setIsLoading(false);
    }
  }, [showToast]);

  useEffect(() => {
    if (!pickerOpen) return;
    void loadDocuments();
  }, [loadDocuments, pickerOpen]);

  const eligibleDocuments = useMemo(
    () =>
      documents.filter(
        (document) =>
          document.archivedAt === null &&
          document.lifecycleState !== "processing" &&
          document.sourceUnitCount > 0,
      ),
    [documents],
  );

  function choose(document: DocumentSummary): void {
    setSelectedDocument(document);
    setPickerOpen(false);
  }

  return (
    <>
      <button
        type="button"
        className="secondary-button"
        onClick={() => setPickerOpen(true)}
        aria-haspopup="dialog"
      >
        Study guides
      </button>

      {pickerOpen ? (
        <Dialog
          title="Study guides"
          description="Open an existing guide or create one from a processed document. Flashcards and guides use the same source-grounded learning topics."
          width="medium"
          onClose={() => setPickerOpen(false)}
          busy={isLoading}
        >
          {errorMessage ? (
            <div className={styles.error} role="alert">
              {errorMessage}
            </div>
          ) : null}
          {isLoading ? (
            <p className={styles.status} aria-live="polite">
              Loading documents…
            </p>
          ) : eligibleDocuments.length === 0 ? (
            <div className={styles.empty}>
              <h3>No processed documents are ready yet</h3>
              <p>
                Import a document first. Once its canonical source units are available, you can
                create a study guide independently of flashcard generation.
              </p>
            </div>
          ) : (
            <div className={styles.list}>
              {eligibleDocuments.map((document) => (
                <article key={document.id} className={styles.row}>
                  <div>
                    <strong>{document.title}</strong>
                    <small>
                      {formatDocumentType(document.sourceFormat)} · {document.sourceUnitCount}{" "}
                      {sourceUnitPluralLabel(document.sourceUnitKind)}
                    </small>
                  </div>
                  <button type="button" className="primary-button" onClick={() => choose(document)}>
                    Open study guide
                  </button>
                </article>
              ))}
            </div>
          )}
        </Dialog>
      ) : null}

      {selectedDocument ? (
        <StudyGuideDialog
          documentId={selectedDocument.id}
          documentTitle={selectedDocument.title}
          showToast={showToast}
          onClose={() => setSelectedDocument(null)}
        />
      ) : null}
    </>
  );
}

function formatDocumentType(sourceFormat: string): string {
  switch (sourceFormat) {
    case "pdf":
      return "PDF";
    case "doc":
    case "docx":
      return "Word";
    case "ppt":
    case "pptx":
      return "PowerPoint";
    case "odt":
      return "OpenDocument text";
    case "odp":
      return "OpenDocument presentation";
    case "rtf":
      return "RTF";
    case "epub":
      return "EPUB";
    case "excel":
    case "xls":
    case "xlsx":
      return "Excel";
    case "ods":
      return "OpenDocument spreadsheet";
    case "csv":
      return "CSV";
    default:
      return sourceFormat || "Document";
  }
}

function sourceUnitPluralLabel(sourceUnitKind: string): string {
  switch (sourceUnitKind) {
    case "page":
      return "pages";
    case "slide":
      return "slides";
    case "chapter":
      return "chapters";
    case "section":
      return "sections";
    case "segment":
      return "segments";
    case "image":
      return "images";
    case "time_segment":
      return "time segments";
    default:
      return "source units";
  }
}
