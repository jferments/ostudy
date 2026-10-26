import { useLayoutEffect, useRef } from "react";

import type { SourcePassage } from "../../components/sourcePassage";
import type { StudyGuideCanvasSource } from "../../ipc/studyGuides";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Sheet } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";

interface GuideSourcesSheetProps {
  /** The section's sources, in number order. */
  sources: readonly StudyGuideCanvasSource[];
  /** The number the learner chose; it is shown first in view and marked. */
  current: number;
  sectionTitle: string;
  /** The guide's document: its title names a passage opened from it. */
  documentId: string;
  documentTitle: string;
  onOpen: (passage: SourcePassage) => void;
  onClose: () => void;
}

/**
 * A section's sources beside its pages: each passage its key points cite, as
 * the document says it, with the page it is on. A passage on a page of a PDF
 * opens there, so the learner can check a key point against its source.
 */
export function GuideSourcesSheet({
  sources, current, sectionTitle, documentId, documentTitle, onOpen, onClose,
}: GuideSourcesSheetProps) {
  const currentRef = useRef<HTMLElement | null>(null);
  useLayoutEffect(() => { currentRef.current?.scrollIntoView?.({ block: "nearest" }); }, [current]);
  return (
    <Sheet title="Sources" description={sectionTitle} width="small" onClose={onClose}>
      <ol className="tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0" aria-label={`Sources for ${sectionTitle}`}>
        {sources.map((source) => {
          const chosen = source.number === current;
          const pageNumber = source.pageNumber;
          return (
            <li key={source.number}>
              <article ref={chosen ? currentRef : undefined} aria-current={chosen || undefined}
                data-guide-source={source.number}
                className={cn("tw:grid tw:min-w-0 tw:gap-2 tw:rounded-lg tw:p-3 tw:ring-1",
                  chosen ? "tw:bg-accent tw:ring-primary" : "tw:bg-background tw:ring-border")}>
                <p className="tw:m-0 tw:flex tw:items-baseline tw:gap-2 tw:text-small">
                  <strong className="tw:text-primary">{source.number}</strong>
                  <span className="tw:text-muted-foreground">{source.label || "Source"}</span>
                </p>
                <blockquote className="tw:m-0 tw:border-0 tw:border-s-2 tw:border-solid tw:border-border-strong tw:ps-3 tw:text-small tw:break-words">
                  {source.excerpt}
                </blockquote>
                {pageNumber !== null ? (
                  <Button size="small" className="tw:justify-self-start" onClick={() => onOpen({
                    documentId: source.documentId,
                    title: source.documentId === documentId ? documentTitle : "Source",
                    pageNumber,
                  })}>
                    <Icon name="document" size={16} />Open page {pageNumber}
                  </Button>
                ) : null}
              </article>
            </li>
          );
        })}
      </ol>
    </Sheet>
  );
}
