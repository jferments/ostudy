import { useEffect, useMemo, useState } from "react";

import { StudyMarkdown } from "../components/StudyMarkdown";
import {
  readDocumentSourceAsset,
  type SourceLocator,
  type StudyGuideVisualExplanation,
} from "../ipc/studyGuides";

interface LoadedFigure {
  assetId: string;
  url: string;
}

export function StudyGuideSourceFigures({
  documentId,
  locator,
  explanations,
  sectionNumber = null,
}: {
  documentId: string | null;
  locator: SourceLocator | null;
  explanations: StudyGuideVisualExplanation[];
  /** Numbers the figures "Figure N.1", "Figure N.2"… after their section; plain "Figure 1" otherwise. */
  sectionNumber?: number | null;
}) {
  const assetIds = useMemo(
    () => selectFigureAssetIds(locator?.assetIds ?? [], explanations.length),
    [locator?.assetIds, explanations.length],
  );
  const [figures, setFigures] = useState<LoadedFigure[]>([]);

  useEffect(() => {
    let active = true;
    const created: string[] = [];
    if (!documentId || assetIds.length === 0) {
      setFigures([]);
      return () => undefined;
    }
    void Promise.all(assetIds.map(async (assetId) => {
      try {
        const bytes = await readDocumentSourceAsset(documentId, assetId);
        const mimeType = detectedImageMimeType(bytes);
        if (!mimeType) return null;
        const url = URL.createObjectURL(new Blob([bytes], { type: mimeType }));
        created.push(url);
        return { assetId, url } satisfies LoadedFigure;
      } catch {
        return null;
      }
    })).then((loaded) => {
      if (!active) return;
      setFigures(loaded.filter((value): value is LoadedFigure => value !== null));
    });
    return () => {
      active = false;
      for (const url of created) URL.revokeObjectURL(url);
    };
  }, [assetIds, documentId]);

  if (explanations.length === 0) return null;
  return (
    <div className="tw:grid tw:gap-4 tw:font-sans">
      {explanations.map((visual, index) => {
        const figure = figures[index] ?? null;
        const number = sectionNumber === null ? `Figure ${index + 1}` : `Figure ${sectionNumber}.${index + 1}`;
        const caption = (
          <>
            <span className="tw:block">
              <strong className="tw:font-semibold tw:text-foreground">{number}</strong>
              {" · "}
              <StudyMarkdown text={visual.title} inline />
            </span>
            <StudyMarkdown text={visual.explanation} />
          </>
        );
        return figure ? (
          <figure
            key={`${figure.assetId}-${visual.title}`}
            className="tw:m-0 tw:grid tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-4 tw:compact:p-3"
          >
            <img
              src={figure.url}
              alt={visual.title}
              className="tw:max-h-[32rem] tw:w-full tw:rounded-sm tw:bg-page tw:object-contain"
            />
            <figcaption className="tw:grid tw:gap-1 tw:text-small tw:text-(--ostudy-color-text-secondary)">
              {caption}
            </figcaption>
          </figure>
        ) : (
          <div
            key={`fallback-${visual.title}-${index}`}
            role="note"
            aria-label={number}
            className="tw:grid tw:gap-1 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-4 tw:text-small tw:text-(--ostudy-color-text-secondary) tw:compact:p-3"
          >
            {caption}
          </div>
        );
      })}
    </div>
  );
}

export function selectFigureAssetIds(assetIds: string[], maximum: number): string[] {
  if (maximum <= 0) return [];
  const unique = [...new Set(assetIds)].sort();
  if (unique.length <= maximum) return unique;
  if (maximum === 1) {
    const selected = unique[Math.floor(unique.length / 2)];
    return selected === undefined ? [] : [selected];
  }
  const last = unique.length - 1;
  return Array.from({ length: maximum }, (_, index) => {
    const selected = unique[Math.floor(index * last / (maximum - 1))];
    if (selected === undefined) throw new Error("Figure selection index is outside the available assets.");
    return selected;
  });
}

export function detectedImageMimeType(buffer: ArrayBuffer): string | null {
  const bytes = new Uint8Array(buffer);
  if (
    bytes.length >= 8
    && bytes[0] === 0x89
    && bytes[1] === 0x50
    && bytes[2] === 0x4e
    && bytes[3] === 0x47
  ) return "image/png";
  if (bytes.length >= 3 && bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff) {
    return "image/jpeg";
  }
  if (
    bytes.length >= 12
    && String.fromCharCode(...bytes.slice(0, 4)) === "RIFF"
    && String.fromCharCode(...bytes.slice(8, 12)) === "WEBP"
  ) return "image/webp";
  if (
    bytes.length >= 4
    && ((bytes[0] === 0x49 && bytes[1] === 0x49 && bytes[2] === 0x2a && bytes[3] === 0x00)
      || (bytes[0] === 0x4d && bytes[1] === 0x4d && bytes[2] === 0x00 && bytes[3] === 0x2a))
  ) return "image/tiff";
  return null;
}
