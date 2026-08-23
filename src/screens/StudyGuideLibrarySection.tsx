import { useCallback, useEffect, useState } from "react";

import type { ShowToast } from "../components/Toast";
import { getErrorMessage } from "../ipc/common";
import {
  listDocumentStudyGuides,
  type StudyGuideLibraryItem,
} from "../ipc/studyGuides";
import { Surface } from "../ui/primitives/Surface";
import { StudyGuideDialog } from "./StudyGuideDialog";
import styles from "./StudyGuideLibrarySection.module.css";

interface StudyGuideLibrarySectionProps {
  showToast: ShowToast;
  onPracticeCards: (cardIds: string[]) => Promise<void>;
}

export function StudyGuideLibrarySection({
  showToast,
  onPracticeCards,
}: StudyGuideLibrarySectionProps) {
  const [guides, setGuides] = useState<StudyGuideLibraryItem[]>([]);
  const [selected, setSelected] = useState<StudyGuideLibraryItem | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const refresh = useCallback(async (): Promise<void> => {
    const loaded = await listDocumentStudyGuides(true);
    setGuides(loaded);
    setErrorMessage(null);
  }, []);

  useEffect(() => {
    let active = true;
    setIsLoading(true);
    void refresh()
      .catch((error: unknown) => {
        if (active) setErrorMessage(getErrorMessage(error));
      })
      .finally(() => {
        if (active) setIsLoading(false);
      });
    return () => {
      active = false;
    };
  }, [refresh]);

  return (
    <>
      <Surface as="section" level={2} className={styles.section} aria-labelledby="study-guides-heading">
        <div className={styles.headingRow}>
          <div>
            <h2 id="study-guides-heading">Study guides</h2>
            <p>Open generated document outlines, add notes, or study the flashcards linked to a topic.</p>
          </div>
          <button
            type="button"
            className="secondary-button"
            disabled={isLoading}
            onClick={() => {
              setIsLoading(true);
              void refresh()
                .catch((error: unknown) => setErrorMessage(getErrorMessage(error)))
                .finally(() => setIsLoading(false));
            }}
          >
            Refresh
          </button>
        </div>

        {errorMessage ? (
          <div className={styles.error} role="alert">
            {errorMessage}
          </div>
        ) : isLoading ? (
          <div className={styles.status} aria-live="polite">
            Loading study guides…
          </div>
        ) : guides.length === 0 ? (
          <div className={styles.empty}>
            <strong>No study guides yet</strong>
            <span>Generate one from the Documents screen, then it will appear here.</span>
          </div>
        ) : (
          <div className={styles.list}>
            {guides.map((guide) => (
              <article key={guide.documentId} className={styles.row}>
                <div>
                  <strong>{guide.documentTitle}</strong>
                  <small>
                    {formatSource(guide)} · revision {guide.guideRevision}
                    {guide.archived ? " · archived source" : ""}
                  </small>
                </div>
                <button
                  type="button"
                  className="secondary-button"
                  onClick={() => setSelected(guide)}
                >
                  Open study guide
                </button>
              </article>
            ))}
          </div>
        )}
      </Surface>

      {selected ? (
        <StudyGuideDialog
          documentId={selected.documentId}
          documentTitle={selected.documentTitle}
          showToast={showToast}
          onPracticeCards={onPracticeCards}
          onClose={() => {
            setSelected(null);
            void refresh().catch(() => undefined);
          }}
        />
      ) : null}
    </>
  );
}

function formatSource(guide: StudyGuideLibraryItem): string {
  const format = guide.sourceFormat.replaceAll("_", " ").toUpperCase();
  const unit = guide.sourceUnitCount === 1 ? singularUnit(guide.sourceUnitKind) : pluralUnit(guide.sourceUnitKind);
  return `${format} · ${guide.sourceUnitCount.toLocaleString()} ${unit} · ${guide.originalFilename}`;
}

function singularUnit(kind: string): string {
  switch (kind) {
    case "slide":
      return "slide";
    case "chapter":
      return "chapter";
    case "section":
      return "section";
    case "sheet":
      return "sheet";
    case "row_group":
      return "row group";
    default:
      return "page";
  }
}

function pluralUnit(kind: string): string {
  const singular = singularUnit(kind);
  return singular === "row group" ? "row groups" : `${singular}s`;
}
