import { useEffect, useState } from "react";

import { renderDocumentPageTile } from "../../ipc/documentAnnotations";
import { getTutorFigure, type TutorFigure as TutorFigureData } from "../../ipc/tutor";
import { cropTile } from "../documents/figures/cropTile";

interface Drawn {
  figure: TutorFigureData;
  url: string;
  width: number;
  height: number;
}

/**
 * A figure from the learner's document that an answer shows, drawn from its
 * PDF page like the Figures page draws it. It floats beside the answer's text
 * and takes the full width on a phone. A figure the learner has hidden since,
 * or one that cannot be drawn, shows nothing.
 */
export function TutorFigure({ turnId, reference, title }: { turnId: string; reference: string; title: string }) {
  const [drawn, setDrawn] = useState<Drawn | null>(null);

  useEffect(() => {
    const [documentId, figureId] = reference.split("/");
    if (!documentId || !figureId) return undefined;
    let current = true;
    let url: string | null = null;
    void getTutorFigure(turnId, documentId, figureId)
      .then(async (figure) => {
        const tile = cropTile(figure.pageNumber, figure.pageWidth, figure.pageHeight, figure.bounds);
        const bytes = await renderDocumentPageTile(figure.documentId, tile);
        if (!current) return;
        url = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
        setDrawn({ figure, url, width: tile.width, height: tile.height });
      })
      .catch(() => { if (current) setDrawn(null); });
    return () => {
      current = false;
      if (url) URL.revokeObjectURL(url);
    };
  }, [turnId, reference]);

  if (!drawn) return null;
  const name = title || drawn.figure.title || "Figure";
  // A span, so the figure may sit inside the paragraph Markdown put it in.
  return (
    <span
      role="figure"
      aria-label={name}
      className="tw:float-right tw:mb-3 tw:ml-4 tw:block tw:w-[min(45%,18rem)] tw:font-sans tw:compact:float-none tw:compact:ml-0 tw:compact:w-full"
    >
      <img
        src={drawn.url}
        alt={name}
        width={drawn.width}
        height={drawn.height}
        className="tw:block tw:h-auto tw:max-h-[24rem] tw:w-full tw:rounded-sm tw:bg-page tw:object-contain"
      />
      <span className="tw:mt-1 tw:block tw:text-small tw:text-(--ostudy-color-text-secondary)">
        {name} · page {drawn.figure.pageNumber}
      </span>
    </span>
  );
}
