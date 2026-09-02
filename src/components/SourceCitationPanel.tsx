import { useEffect, useState } from "react";

import {
  getNoteSourceCitation,
  listDeckSourceCitations,
  type SourceCitation,
} from "../ipc/documents";
import { getErrorMessage } from "../ipc/common";
import styles from "./SourceCitationPanel.module.css";

interface SourceCitationPanelProps {
  deckId?: string;
  noteId?: string;
  compact?: boolean;
}

/**
 * Domain-facing source provenance for generated decks and notes.
 *
 * The panel deliberately loads through the typed document IPC boundary rather
 * than teaching generic surfaces about documents. Citation snapshots remain
 * renderable after the managed source document has been deleted.
 */
export function SourceCitationPanel({
  deckId,
  noteId,
  compact = false,
}: SourceCitationPanelProps) {
  const [citations, setCitations] = useState<SourceCitation[]>([]);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setCitations([]);
    setErrorMessage(null);

    const request = deckId
      ? listDeckSourceCitations(deckId)
      : noteId
        ? getNoteSourceCitation(noteId).then((citation) =>
            citation ? [citation] : [],
          )
        : Promise.resolve([]);

    void request
      .then((loaded) => {
        if (active) setCitations(loaded);
      })
      .catch((error: unknown) => {
        if (active) setErrorMessage(getErrorMessage(error));
      });

    return () => {
      active = false;
    };
  }, [deckId, noteId]);

  if (errorMessage) {
    return (
      <div className={styles.error} role="status">
        Source details could not be loaded: {errorMessage}
      </div>
    );
  }
  if (citations.length === 0) return null;

  return (
    <section
      className={compact ? `${styles.panel} ${styles.compact}` : styles.panel}
      aria-label={citations.length === 1 ? "Source" : "Sources"}
      data-source-citation-panel
    >
      <div className={styles.heading}>
        <strong>{citations.length === 1 ? "Source" : "Sources"}</strong>
        <span>
          {citations.length} preserved {citations.length === 1 ? "citation" : "citations"}
        </span>
      </div>
      <div className={styles.list}>
        {citations.map((citation) => (
          <article className={styles.citation} key={citation.citationId}>
            <div className={styles.copy}>
              <strong>{citation.sourceTitle}</strong>
              <span>{formatSource(citation)}</span>
              {citation.sourceLocator ? (
                <small>{formatLocator(citation)}</small>
              ) : null}
            </div>
            {citation.sourceRemoved ? (
              <span className={styles.removed}>Source removed · citation preserved</span>
            ) : null}
          </article>
        ))}
      </div>
    </section>
  );
}

function formatSource(citation: SourceCitation): string {
  const format = citation.sourceFormat.replaceAll("_", " ").toUpperCase();
  return `${format} · ${citation.originalFilename}`;
}

function formatLocator(citation: SourceCitation): string {
  const locator = citation.sourceLocator;
  if (!locator) return "";
  const parts: string[] = [];
  const section = locator.sectionLabel?.trim();
  if (section) parts.push(section);
  if (locator.unitNumbers.length > 0) {
    const unit = locator.unitKind.replaceAll("_", " ");
    parts.push(`${unit} ${formatUnitNumbers(locator.unitNumbers)}`);
  }
  return parts.join(" · ");
}

function formatUnitNumbers(values: number[]): string {
  const sorted = [...new Set(values)].sort((left, right) => left - right);
  if (sorted.length <= 4) return sorted.join(", ");
  return `${sorted.slice(0, 3).join(", ")} +${sorted.length - 3}`;
}
