import { memo, useLayoutEffect, useMemo, useRef, type CSSProperties } from "react";

import type { AnnotatedPage } from "../../../ipc/documentAnnotations";
import { Button } from "../../../ui/primitives/Button";
import { Textarea } from "../../../ui/primitives/FormControls";
import {
  arrowHead,
  describeAnnotation,
  handlesFor,
  MAX_TEXT_CHARS,
  shapeBounds,
  TEXT_LINE_HEIGHT,
  TEXT_PADDING,
  type Annotation,
  type AnnotationShape,
  type TextShape,
} from "./annotationModel";
import { usePageImage, type PageImageCache } from "./pageImages";
import { tileDensity, tilesFor, usePageTiles, type PageRegion, type PageTile, type ShownTile } from "./pageTiles";
import { highlightPath, inkPath } from "./strokeOutline";
import type { TextEditing } from "./useDocumentAnnotator";

export interface PageDraft {
  pageNumber: number;
  shape: AnnotationShape;
  complete: boolean;
}

interface AnnotatorPageProps {
  page: AnnotatedPage;
  /** CSS pixels per point. */
  scale: number;
  /** Near the screen: show the image and annotations, not just the page's place. */
  near: boolean;
  images: PageImageCache | null;
  /** Sharp tiles for a zoomed-in page, drawn for where the view last rested. */
  tiles: PageImageCache | null;
  tileScale: number;
  pixelRatio: number;
  /** The part of this page in view, in points; null when none is. */
  region: PageRegion | null;
  annotations: readonly Annotation[];
  /** The rest of these belong to this page, or are null. */
  draft: PageDraft | null;
  preview: Annotation | null;
  erasing: ReadonlySet<string> | null;
  selected: Annotation | null;
  editing: TextEditing | null;
  /** The Select tool is active: annotations are listed for the keyboard. */
  selectable: boolean;
  coarse: boolean;
  textHeight: (annotation: Annotation) => number;
  onMeasureText: (annotation: Annotation, height: number) => void;
  onTypeText: (text: string) => void;
  onFinishText: () => void;
  onFocusAnnotation: (annotation: Annotation) => void;
  onOpenAnnotation: (annotation: Annotation) => void;
}

/** One page: its render, highlights under it all, strokes and shapes, then text notes and the selection. */
const NO_TILES: readonly PageTile[] = [];

export const AnnotatorPage = memo(function AnnotatorPage(props: AnnotatorPageProps) {
  const { page, scale, near, region, tileScale, pixelRatio } = props;
  const image = usePageImage(props.images, page.assetId, near && page.imageAvailable);
  const density = near && page.imageAvailable && region ? tileDensity(page, tileScale, pixelRatio) : null;
  const wanted = useMemo(
    () => (density !== null && region ? tilesFor(page, density, region) : NO_TILES),
    [density, page, region],
  );
  const tiles = usePageTiles(props.tiles, wanted);
  const width = page.width * scale;
  const height = page.height * scale;
  return (
    <div
      data-annotator-page={page.pageNumber}
      role="group"
      aria-label={`Page ${page.pageNumber}`}
      className="tw:relative tw:mx-auto tw:mb-4 tw:overflow-hidden tw:bg-[var(--ostudy-color-neutral-0)] tw:shadow-md tw:ring-1 tw:ring-border tw:select-none tw:forced-color-adjust-none tw:last:mb-0 tw:compact:mb-3"
      style={{ width, height }}
    >
      {near ? <PageContent {...props} imageUrl={image.url} imageFailed={image.failed || !page.imageAvailable} sharpTiles={tiles} /> : null}
    </div>
  );
});

function PageContent({
  page, scale, annotations, draft, preview, erasing, selected, editing, selectable, coarse, textHeight,
  onMeasureText, onTypeText, onFinishText, onFocusAnnotation, onOpenAnnotation, imageUrl, imageFailed, sharpTiles,
}: AnnotatorPageProps & { imageUrl: string | null; imageFailed: boolean; sharpTiles: readonly ShownTile[] }) {
  const shown = annotations.map((annotation) => preview?.id === annotation.id ? preview : annotation);
  const viewBox = `0 0 ${page.width} ${page.height}`;
  const faded = (annotation: Annotation) => erasing?.has(annotation.id) ? 0.25 : undefined;
  const highlights = shown.filter((annotation) => annotation.shape.kind === "highlight");
  const graphics = shown.filter((annotation) => annotation.shape.kind !== "highlight" && annotation.shape.kind !== "text");
  const notes = shown.filter((annotation) => annotation.shape.kind === "text" && annotation.id !== editing?.annotation.id);
  return (
    <>
      {imageUrl ? (
        <img src={imageUrl} alt="" draggable={false} className="tw:pointer-events-none tw:absolute tw:inset-0 tw:size-full" />
      ) : (
        <p className="tw:pointer-events-none tw:absolute tw:inset-x-0 tw:top-1/2 tw:m-0 tw:-translate-y-1/2 tw:text-center tw:text-small tw:text-[var(--ostudy-color-neutral-950)]/70">
          {imageFailed ? "This page's image could not be read." : null}
        </p>
      )}
      {/* Drawn again from the PDF at the size shown, over the softer page image. */}
      {sharpTiles.map((tile) => (
        <img
          key={tile.key}
          src={tile.url}
          alt=""
          draggable={false}
          data-page-tile=""
          className="tw:pointer-events-none tw:absolute"
          style={{
            left: `${(tile.left / tile.pageWidth) * 100}%`,
            top: `${(tile.top / tile.pageHeight) * 100}%`,
            width: `${(tile.width / tile.pageWidth) * 100}%`,
            height: `${(tile.height / tile.pageHeight) * 100}%`,
          }}
        />
      ))}
      {/* Highlights blend into the page, so text under them stays readable. */}
      <svg viewBox={viewBox} aria-hidden="true" className="tw:pointer-events-none tw:absolute tw:inset-0 tw:size-full tw:mix-blend-multiply">
        {highlights.map((annotation) => (
          <g key={annotation.id} opacity={faded(annotation)}><ShapeGraphic shape={annotation.shape} /></g>
        ))}
        {draft?.shape.kind === "highlight" ? <ShapeGraphic shape={draft.shape} complete={draft.complete} /> : null}
      </svg>
      <svg viewBox={viewBox} aria-hidden="true" className="tw:pointer-events-none tw:absolute tw:inset-0 tw:size-full">
        {graphics.map((annotation) => (
          <g key={annotation.id} opacity={faded(annotation)}><ShapeGraphic shape={annotation.shape} /></g>
        ))}
        {draft && draft.shape.kind !== "highlight" ? <ShapeGraphic shape={draft.shape} complete={draft.complete} /> : null}
      </svg>
      <div
        className="tw:pointer-events-none tw:absolute tw:top-0 tw:left-0 tw:origin-top-left tw:font-sans"
        style={{ width: page.width, height: page.height, transform: `scale(${scale})` }}
      >
        {notes.map((annotation) => (
          <TextNote key={annotation.id} annotation={annotation} opacity={faded(annotation)} onMeasure={onMeasureText} />
        ))}
        {editing ? <TextNoteEditor editing={editing} onType={onTypeText} onFinish={onFinishText} /> : null}
      </div>
      {selected && !editing ? (
        <SelectionOutline page={page} scale={scale} coarse={coarse}
          annotation={preview?.id === selected.id ? preview : selected} textHeight={textHeight} />
      ) : null}
      {selectable && shown.length > 0 ? (
        <ul className="tw:sr-only" aria-label={`Annotations on page ${page.pageNumber}`}>
          {shown.map((annotation) => (
            <li key={annotation.id}>
              <Button
                variant="ghost"
                size="small"
                data-annotation-id={annotation.id}
                aria-pressed={selected?.id === annotation.id}
                onFocus={() => onFocusAnnotation(annotation)}
                onClick={() => onOpenAnnotation(annotation)}
              >
                {describeAnnotation(annotation)}
              </Button>
            </li>
          ))}
        </ul>
      ) : null}
    </>
  );
}

const ShapeGraphic = memo(function ShapeGraphic({ shape, complete = true }: { shape: AnnotationShape; complete?: boolean }) {
  switch (shape.kind) {
    case "ink":
      return <path d={inkPath(shape, complete)} fill={shape.color} />;
    case "highlight":
      return (
        <path d={highlightPath(shape)} fill="none" stroke={shape.color} strokeWidth={shape.strokeWidth}
          strokeLinecap="butt" strokeLinejoin="round" strokeOpacity={0.75} />
      );
    case "line":
      return (
        <line x1={shape.x1} y1={shape.y1} x2={shape.x2} y2={shape.y2} stroke={shape.color}
          strokeWidth={shape.strokeWidth} strokeLinecap="round" />
      );
    case "arrow": {
      const head = arrowHead(shape);
      return (
        <>
          <line x1={shape.x1} y1={shape.y1} x2={head.base.x} y2={head.base.y} stroke={shape.color}
            strokeWidth={shape.strokeWidth} strokeLinecap="round" />
          <polygon fill={shape.color} stroke={shape.color} strokeWidth={Math.min(1, shape.strokeWidth)} strokeLinejoin="round"
            points={`${shape.x2},${shape.y2} ${head.left.x},${head.left.y} ${head.right.x},${head.right.y}`} />
        </>
      );
    }
    case "rectangle":
      return (
        <rect x={shape.x} y={shape.y} width={shape.width} height={shape.height} fill="none" stroke={shape.color}
          strokeWidth={shape.strokeWidth} strokeLinejoin="round" />
      );
    case "ellipse":
      return (
        <ellipse cx={shape.x + shape.width / 2} cy={shape.y + shape.height / 2} rx={shape.width / 2} ry={shape.height / 2}
          fill="none" stroke={shape.color} strokeWidth={shape.strokeWidth} />
      );
    case "text":
      return null;
  }
});

function textStyle(shape: TextShape): CSSProperties {
  return {
    left: shape.x,
    top: shape.y,
    width: shape.width,
    padding: TEXT_PADDING,
    color: shape.color,
    fontSize: shape.fontSize,
    lineHeight: TEXT_LINE_HEIGHT,
  };
}

const TextNote = memo(function TextNote({ annotation, opacity, onMeasure }: {
  annotation: Annotation;
  opacity: number | undefined;
  onMeasure: (annotation: Annotation, height: number) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const height = ref.current?.offsetHeight ?? 0;
    if (height > 0) onMeasure(annotation, height);
  }, [annotation, onMeasure]);
  if (annotation.shape.kind !== "text") return null;
  return (
    <div ref={ref} aria-hidden="true" className="tw:absolute tw:box-border tw:whitespace-pre-wrap tw:wrap-anywhere"
      style={{ ...textStyle(annotation.shape), opacity }}>
      {annotation.shape.text}
    </div>
  );
});

/** The open text note: a field in the note's place, at the note's size, growing with its text. */
function TextNoteEditor({ editing, onType, onFinish }: {
  editing: TextEditing;
  onType: (text: string) => void;
  onFinish: () => void;
}) {
  const ref = useRef<HTMLTextAreaElement>(null);
  const shape = editing.annotation.shape;
  useLayoutEffect(() => {
    const field = ref.current;
    if (!field) return;
    field.focus({ preventScroll: true });
    field.setSelectionRange(field.value.length, field.value.length);
    // On a phone the keyboard may cover the note; keep it in view.
    field.scrollIntoView?.({ block: "nearest", inline: "nearest" });
    // Only when a note opens: typing must not move the caret.
  }, [editing.annotation.id]);
  useLayoutEffect(() => {
    const field = ref.current;
    if (!field) return;
    field.style.height = "auto";
    field.style.height = `${field.scrollHeight}px`;
  }, [editing.text, shape.width, shape.fontSize]);
  return (
    <Textarea
      ref={ref}
      value={editing.text}
      aria-label={editing.isNew ? "New text note" : "Edit text note"}
      maxLength={MAX_TEXT_CHARS}
      rows={1}
      spellCheck
      className="tw:pointer-events-auto tw:absolute tw:m-0 tw:box-border tw:min-h-0 tw:resize-none tw:overflow-hidden tw:rounded-sm tw:border-0 tw:bg-[var(--ostudy-color-neutral-0)]/85 tw:shadow-none tw:outline-2 tw:outline-primary tw:outline-dashed"
      style={textStyle(shape)}
      onChange={(event) => onType(event.target.value)}
      onBlur={onFinish}
      onPointerDown={(event) => event.stopPropagation()}
      onKeyDown={(event) => {
        if (event.key === "Escape" || (event.key === "Enter" && (event.ctrlKey || event.metaKey))) {
          // Escape keeps the note and stays in the annotator.
          event.preventDefault();
          event.stopPropagation();
          onFinish();
        }
      }}
    />
  );
}

function SelectionOutline({ page, scale, coarse, annotation, textHeight }: {
  page: AnnotatedPage;
  scale: number;
  coarse: boolean;
  annotation: Annotation;
  textHeight: (annotation: Annotation) => number;
}) {
  const height = textHeight(annotation);
  const bounds = shapeBounds(annotation.shape, height);
  const margin = 4 / scale;
  const radius = (coarse ? 8 : 5) / scale;
  return (
    <svg viewBox={`0 0 ${page.width} ${page.height}`} aria-hidden="true"
      className="tw:pointer-events-none tw:absolute tw:inset-0 tw:size-full tw:overflow-visible">
      <rect
        x={bounds.left - margin} y={bounds.top - margin}
        width={bounds.right - bounds.left + 2 * margin} height={bounds.bottom - bounds.top + 2 * margin}
        fill="none" className="tw:stroke-primary" strokeWidth={1.5 / scale} strokeDasharray={`${5 / scale} ${4 / scale}`}
      />
      {handlesFor(annotation.shape, height).map(({ handle, point }) => (
        <circle key={handle} cx={point.x} cy={point.y} r={radius}
          className="tw:fill-background tw:stroke-primary" strokeWidth={2 / scale} />
      ))}
    </svg>
  );
}
