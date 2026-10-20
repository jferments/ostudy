import { useEffect, useState } from "react";
import { getNoteSourceCitation, listDeckSourceCitations, type SourceCitation } from "../ipc/documents";
import { getErrorMessage } from "../ipc/common";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { Badge } from "../ui/primitives/Badge";
import { Panel } from "../ui/primitives/Panel";
import { Button } from "../ui/primitives/Button";
import { Icon } from "../ui/icons/Icon";
import { getCardMarkSource, type CardMarkSource } from "../ipc/annotationCards";
import { atMark, sourcePassageFor, type SourcePassage } from "./sourcePassage";

interface SourceCitationPanelProps {
  deckId?: string; noteId?: string; compact?: boolean;
  /** Offered for a cited page of a PDF still in the library; other citations stay text. */
  onOpenPassage?: (passage: SourcePassage) => void;
}

/** Domain-facing provenance. Snapshot citations remain readable after source removal. */
export function SourceCitationPanel({ deckId, noteId, compact = false, onOpenPassage }: SourceCitationPanelProps) {
  const [citations, setCitations] = useState<SourceCitation[]>([]);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [mark, setMark] = useState<CardMarkSource | null>(null);
  const canOpen = onOpenPassage !== undefined;
  useEffect(() => {
    let active = true;
    setCitations([]);
    setErrorMessage(null);
    setMark(null);
    // A card made from a mark opens its source at that mark; without one it
    // opens at the cited page, so a failed read only loses the precision.
    if (noteId && !deckId && canOpen) {
      void getCardMarkSource(noteId).then((loaded) => { if (active) setMark(loaded); }).catch(() => undefined);
    }
    const request = deckId ? listDeckSourceCitations(deckId)
      : noteId ? getNoteSourceCitation(noteId).then((citation) => citation ? [citation] : []) : Promise.resolve([]);
    void request.then((loaded) => { if (active) setCitations(loaded); })
      .catch((error: unknown) => { if (active) setErrorMessage(getErrorMessage(error)); });
    return () => { active = false; };
  }, [deckId, noteId, canOpen]);

  if (errorMessage) return <InlineNotice kind="warning">Source details could not be loaded: {errorMessage}</InlineNotice>;
  if (citations.length === 0) return null;
  return (
    <Panel className={compact ? "tw:my-3 tw:text-small" : "tw:my-5"}
      aria-label={citations.length === 1 ? "Source" : "Sources"} data-source-citation-panel>
      <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:justify-between tw:gap-3 tw:text-small tw:[&>span]:text-caption tw:[&>span]:text-muted-foreground">
        <strong>{citations.length === 1 ? "Source" : "Sources"}</strong>
        <span>{citations.length} preserved {citations.length === 1 ? "citation" : "citations"}</span>
      </div>
      <div className="tw:grid tw:min-w-0 tw:gap-3">
        {citations.map((citation) => {
          const passage = onOpenPassage ? atMark(sourcePassageFor(citation), mark) : null;
          return (
            <article className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-start tw:justify-between tw:gap-3 tw:rounded-lg tw:bg-background tw:p-3" key={citation.citationId}>
              <div className="tw:grid tw:min-w-0 tw:gap-1 tw:break-words tw:[&>span]:text-small tw:[&>span]:text-muted-foreground tw:[&>small]:text-caption tw:[&>small]:text-muted-foreground">
                <strong>{citation.sourceTitle}</strong><span>{formatSource(citation)}</span>
                {citation.sourceLocator ? <small>{formatLocator(citation)}</small> : null}
              </div>
              {citation.sourceRemoved ? <Badge tone="warning">Source removed · citation preserved</Badge> : null}
              {passage && onOpenPassage ? (
                <Button size="small" onClick={() => onOpenPassage(passage)}>
                  <Icon name="document" size={16} />Open page {passage.pageNumber}
                </Button>
              ) : null}
            </article>
          );
        })}
      </div>
    </Panel>
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
