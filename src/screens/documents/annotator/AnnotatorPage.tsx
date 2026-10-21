import { memo, useLayoutEffect, useMemo, useRef, type CSSProperties, type ReactNode, type SVGProps } from "react";

import type { AnnotatedPage, FrameShape } from "../../../ipc/documentAnnotations";
import { managedAssetUrl } from "../../../platform/assets";
import { Icon } from "../../../ui/icons/Icon";
import { cn } from "../../../ui/internal/cn";
import { Dialog } from "../../../ui/overlays/Dialog";
import { Button } from "../../../ui/primitives/Button";
import { Textarea } from "../../../ui/primitives/FormControls";
import {
  arrowHead,
  dashArray,
  segmentPath,
  towardEnd,
  towardStart,
  describeAnnotation,
  handlesFor,
  isStickyNote,
  MAX_STICKY_CHARS,
  MAX_TEXT_CHARS,
  shapeBounds,
  STICKY_HEIGHT,
  STICKY_ICON_SIZE,
  TEXT_LINE_HEIGHT,
  TEXT_PADDING,
  type Annotation,
  type AnnotationShape,
  type TextShape,
} from "./annotationModel";
import { usePageImage, type PageImageCache } from "./pageImages";
import { tileDensity, tilesFor, usePageTiles, type PageRegion, type PageTile, type ShownTile } from "./pageTiles";
import { highlightPath, inkPath, textHighlightPath } from "./strokeOutline";
import type { TextEditing } from "./useDocumentAnnotator";

export interface PageDraft {
  pageNumber: number;
  shape: AnnotationShape;
  complete: boolean;
}

/**
 * Pages of paper instead of page images, as a notebook's are: the paper is
 * drawn under the marks, and a text note may be a link.
 */
export interface PaperPages {
  background: (page: AnnotatedPage) => ReactNode;
  /** Links whose target is gone, shown as removed. */
  unavailableLinks: ReadonlySet<string>;
}

interface AnnotatorPageProps {
  page: AnnotatedPage;
  paperPages?: PaperPages;
  /** CSS pixels per point. */
  scale: number;
  /** Near the screen: show the image and annotations, not just the page's place. */
  near: boolean;
  /** Outlined, as the page a card's source opened at. */
  marked?: boolean;
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
  /** The lassoed annotations on this page, as they are being moved while a drag is under way. */
  group: readonly Annotation[];
  /** The lasso loop being drawn on this page, as `x, y` pairs. */
  loop: readonly number[] | null;
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
      data-marked={props.marked || undefined}
      role="group"
      aria-label={`Page ${page.pageNumber}`}
      className={cn(
        "tw:relative tw:mx-auto tw:mb-4 tw:overflow-hidden tw:bg-[var(--ostudy-color-neutral-0)] tw:shadow-md tw:ring-1 tw:ring-border tw:select-none tw:forced-color-adjust-none tw:last:mb-0 tw:compact:mb-3",
        props.marked && "tw:ring-4 tw:ring-primary tw:forced-colors:outline-solid tw:forced-colors:outline-4 tw:forced-colors:outline-[Highlight]",
      )}
      style={{ width, height }}
    >
      {near ? <PageContent {...props} imageUrl={image.url} imageFailed={image.failed || !page.imageAvailable} sharpTiles={tiles} /> : null}
    </div>
  );
});

function PageContent({
  page, paperPages, scale, annotations, draft, preview, group, loop, erasing, selected, editing, selectable, coarse,
  textHeight, onMeasureText, onTypeText, onFinishText, onFocusAnnotation, onOpenAnnotation, imageUrl, imageFailed, sharpTiles,
}: AnnotatorPageProps & { imageUrl: string | null; imageFailed: boolean; sharpTiles: readonly ShownTile[] }) {
  const moving = new Map(group.map((annotation) => [annotation.id, annotation]));
  const shown = annotations.map((annotation) => preview?.id === annotation.id ? preview : moving.get(annotation.id) ?? annotation);
  const viewBox = `0 0 ${page.width} ${page.height}`;
  const faded = (annotation: Annotation) => erasing?.has(annotation.id) ? 0.25 : undefined;
  const highlights = shown.filter((annotation) => annotation.shape.kind === "highlight");
  const graphics = shown.filter((annotation) => annotation.shape.kind !== "highlight" && annotation.shape.kind !== "text");
  const notes = shown.filter((annotation) => annotation.shape.kind === "text" && !isStickyNote(annotation.shape)
    && annotation.id !== editing?.annotation.id);
  // A sticky note stays on the page while it is open; its text is typed in a dialog.
  const stickies = shown.filter((annotation) => isStickyNote(annotation.shape) && annotation.id !== editing?.annotation.id);
  const openSticky = editing && isStickyNote(editing.annotation.shape)
    ? { ...editing.annotation, shape: { ...editing.annotation.shape, text: editing.text } }
    : null;
  return (
    <>
      {paperPages ? paperPages.background(page) : imageUrl ? (
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
          <TextNote key={annotation.id} annotation={annotation} opacity={faded(annotation)} onMeasure={onMeasureText}
            unavailable={paperPages?.unavailableLinks.has(annotation.id) ?? false} />
        ))}
        {stickies.map((annotation) => (
          <StickyNote key={annotation.id} annotation={annotation} opacity={faded(annotation)} />
        ))}
        {openSticky ? <StickyNote annotation={openSticky} opacity={undefined} open /> : null}
        {editing && !openSticky ? <TextNoteEditor editing={editing} onType={onTypeText} onFinish={onFinishText} /> : null}
      </div>
      {group.length > 0 || loop ? (
        <svg viewBox={viewBox} aria-hidden="true" className="tw:pointer-events-none tw:absolute tw:inset-0 tw:size-full tw:overflow-visible">
          {group.map((annotation) => {
            const bounds = shapeBounds(annotation.shape, textHeight(annotation));
            return (
              <rect key={annotation.id} data-lassoed="" x={bounds.left - 2 / scale} y={bounds.top - 2 / scale}
                width={bounds.right - bounds.left + 4 / scale} height={bounds.bottom - bounds.top + 4 / scale}
                fill="none" className="tw:stroke-primary" strokeWidth={1.5 / scale}
                strokeDasharray={`${4 / scale} ${3 / scale}`} />
            );
          })}
          {loop ? (
            <polygon data-lasso-loop="" points={pairs(loop)} className="tw:fill-primary tw:stroke-primary" fillOpacity={0.06}
              strokeWidth={1.5 / scale} strokeDasharray={`${4 / scale} ${3 / scale}`} />
          ) : null}
        </svg>
      ) : null}
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

/** One mark's graphic in page points; text notes are drawn as HTML instead. */
export const ShapeGraphic = memo(function ShapeGraphic({ shape, complete = true }: { shape: AnnotationShape; complete?: boolean }) {
  switch (shape.kind) {
    case "ink":
      return <path d={inkPath(shape, complete)} fill={shape.color} />;
    case "highlight":
      if (shape.rects) return <path d={textHighlightPath(shape.rects)} fill={shape.color} fillOpacity={0.75} />;
      return (
        <path d={highlightPath(shape)} fill="none" stroke={shape.color} strokeWidth={shape.strokeWidth}
          strokeLinecap="butt" strokeLinejoin="round" strokeOpacity={0.75} />
      );
    case "line":
      return (
        <path d={segmentPath(shape, { x: shape.x1, y: shape.y1 }, { x: shape.x2, y: shape.y2 })} fill="none"
          stroke={shape.color} strokeWidth={shape.strokeWidth} strokeLinecap="round"
          strokeDasharray={dashArray(shape.dash, shape.strokeWidth)} />
      );
    case "arrow": {
      const head = arrowHead(towardEnd(shape));
      const tail = shape.bothEnds ? arrowHead(towardStart(shape)) : null;
      const headStroke = Math.min(1, shape.strokeWidth);
      return (
        <>
          <path d={segmentPath(shape, tail?.base ?? { x: shape.x1, y: shape.y1 }, head.base)} fill="none"
            stroke={shape.color} strokeWidth={shape.strokeWidth} strokeLinecap="round"
            strokeDasharray={dashArray(shape.dash, shape.strokeWidth)} />
          <polygon fill={shape.color} stroke={shape.color} strokeWidth={headStroke} strokeLinejoin="round"
            points={`${shape.x2},${shape.y2} ${head.left.x},${head.left.y} ${head.right.x},${head.right.y}`} />
          {tail ? (
            <polygon fill={shape.color} stroke={shape.color} strokeWidth={headStroke} strokeLinejoin="round"
              points={`${shape.x1},${shape.y1} ${tail.left.x},${tail.left.y} ${tail.right.x},${tail.right.y}`} />
          ) : null}
        </>
      );
    }
    case "rectangle":
      return (
        <rect x={shape.x} y={shape.y} width={shape.width} height={shape.height} {...frameLook(shape)} />
      );
    case "ellipse":
      return (
        <ellipse cx={shape.x + shape.width / 2} cy={shape.y + shape.height / 2} rx={shape.width / 2} ry={shape.height / 2}
          {...frameLook(shape)} />
      );
    case "image":
      return (
        <image href={managedAssetUrl(shape.assetId)} x={shape.x} y={shape.y} width={shape.width} height={shape.height}
          preserveAspectRatio="none" />
      );
    case "text":
      return null;
  }
});

/** A box's or ellipse's fill and outline. */
function frameLook(shape: FrameShape): Pick<SVGProps<SVGRectElement>, "fill" | "stroke" | "strokeWidth" | "strokeLinejoin" | "strokeLinecap" | "strokeDasharray"> {
  const border = shape.border !== false;
  return {
    fill: shape.fill ?? "none",
    stroke: border ? shape.color : "none",
    strokeWidth: shape.strokeWidth,
    strokeLinejoin: "round",
    strokeLinecap: shape.dash ? "round" : undefined,
    strokeDasharray: border ? dashArray(shape.dash, shape.strokeWidth) : undefined,
  };
}

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

const TextNote = memo(function TextNote({ annotation, opacity, onMeasure, unavailable }: {
  annotation: Annotation;
  opacity: number | undefined;
  onMeasure: (annotation: Annotation, height: number) => void;
  /** A link whose target is gone. */
  unavailable: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const height = ref.current?.offsetHeight ?? 0;
    if (height > 0) onMeasure(annotation, height);
  }, [annotation, onMeasure]);
  if (annotation.shape.kind !== "text") return null;
  const { link } = annotation.shape;
  return (
    <div ref={ref} aria-hidden="true" className="tw:absolute tw:box-border tw:whitespace-pre-wrap tw:wrap-anywhere"
      style={{ ...textStyle(annotation.shape), opacity }}>
      {link ? <LinkLabel text={annotation.shape.text} button={link.button === true} unavailable={unavailable} /> : annotation.shape.text}
    </div>
  );
});

/**
 * A link on a notebook page: underlined words after a link mark, or a button
 * in the note's color. A link whose target is gone is struck through.
 */
function LinkLabel({ text, button, unavailable }: { text: string; button: boolean; unavailable: boolean }) {
  return (
    <span
      data-note-link={button ? "button" : "link"}
      data-unavailable={unavailable || undefined}
      className={cn(
        "tw:inline-flex tw:max-w-full tw:items-baseline tw:gap-[0.3em] tw:font-medium",
        button
          ? "tw:rounded-md tw:bg-[currentColor] tw:px-[0.6em] tw:py-[0.2em] tw:shadow-sm"
          : "tw:underline tw:decoration-1 tw:underline-offset-2",
        unavailable && "tw:line-through tw:opacity-60",
      )}
    >
      <Icon name={button ? "practice" : "link"} className={cn("tw:size-[0.9em] tw:shrink-0 tw:self-center", button && "tw:text-[var(--ostudy-color-neutral-0)]")} />
      <span className={cn(button && "tw:text-[var(--ostudy-color-neutral-0)]")}>{text}</span>
    </span>
  );
}

const STICKY_PADDING = 6;

/**
 * A sticky note on the page: a small square of paper in the note's color, with
 * a folded corner and the first lines of its text. The whole text opens in a
 * dialog. A collapsed note is only a small icon of paper, to fit in a margin.
 */
const StickyNote = memo(function StickyNote({ annotation, opacity, open = false }: {
  annotation: Annotation;
  opacity: number | undefined;
  open?: boolean;
}) {
  if (!isStickyNote(annotation.shape)) return null;
  const { x, y, width, color, fontSize, text } = annotation.shape;
  if (annotation.shape.collapsed) {
    return (
      <div
        aria-hidden="true"
        data-sticky-note=""
        data-collapsed=""
        className={cn(
          "tw:absolute tw:box-border tw:flex tw:flex-col tw:justify-center tw:gap-[2px] tw:rounded-[2px] tw:px-[4px] tw:shadow-sm",
          open && "tw:outline-2 tw:outline-primary tw:outline-dashed",
        )}
        style={{ left: x, top: y, width: STICKY_ICON_SIZE, height: STICKY_ICON_SIZE, opacity, backgroundColor: color }}
      >
        {/* Lines of writing, so the icon reads as a note. */}
        {[1, 0.75, 1].map((share, index) => (
          <span key={index} className="tw:block tw:h-[1.5px] tw:bg-[var(--ostudy-color-neutral-950)]/50" style={{ width: `${share * 100}%` }} />
        ))}
      </div>
    );
  }
  const lineHeight = fontSize * TEXT_LINE_HEIGHT;
  const lines = Math.max(1, Math.floor((STICKY_HEIGHT - STICKY_PADDING * 2) / lineHeight));
  return (
    <div
      aria-hidden="true"
      data-sticky-note=""
      className={cn(
        "tw:absolute tw:box-border tw:overflow-hidden tw:rounded-sm tw:text-[var(--ostudy-color-neutral-950)] tw:shadow-md",
        open && "tw:outline-2 tw:outline-primary tw:outline-dashed",
      )}
      style={{
        left: x, top: y, width, height: STICKY_HEIGHT, padding: STICKY_PADDING, opacity,
        backgroundColor: color, fontSize, lineHeight: TEXT_LINE_HEIGHT,
      }}
    >
      {/* The folded corner. */}
      <span
        className="tw:absolute tw:right-0 tw:bottom-0 tw:size-3 tw:bg-[var(--ostudy-color-neutral-950)]/20"
        style={{ clipPath: "polygon(100% 0, 100% 100%, 0 100%)" }}
      />
      <div className="tw:whitespace-pre-wrap tw:wrap-anywhere"
        style={{ display: "-webkit-box", WebkitBoxOrient: "vertical", WebkitLineClamp: lines, overflow: "hidden" }}>
        {text}
      </div>
    </div>
  );
});

/**
 * The open sticky note's whole text, in a dialog: a sticky note holds more than
 * fits on the page. Closing keeps what was typed; an emptied note is removed.
 */
export function StickyNoteEditor({ editing, onType, onFinish }: {
  editing: TextEditing;
  onType: (text: string) => void;
  onFinish: () => void;
}) {
  const ref = useRef<HTMLTextAreaElement>(null);
  useLayoutEffect(() => {
    const field = ref.current;
    if (field) field.setSelectionRange(field.value.length, field.value.length);
  }, [editing.annotation.id]);
  return (
    <Dialog
      title={editing.isNew ? "New sticky note" : "Sticky note"}
      description={`Page ${editing.annotation.pageNumber}`}
      width="small"
      initialFocusRef={ref}
      onClose={onFinish}
      actions={<>
        <Button variant="secondary" className="tw:text-destructive" onClick={() => { onType(""); onFinish(); }}>
          {editing.isNew ? "Discard" : "Delete note"}
        </Button>
        <Button variant="primary" onClick={onFinish}>Done</Button>
      </>}
    >
      <Textarea
        ref={ref}
        value={editing.text}
        aria-label="Note"
        maxLength={MAX_STICKY_CHARS}
        rows={10}
        spellCheck
        onChange={(event) => onType(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
            event.preventDefault();
            onFinish();
          }
        }}
      />
    </Dialog>
  );
}

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
      {(annotation.shape.kind === "line" || annotation.shape.kind === "arrow") && annotation.shape.curve ? (
        // A curve's control points hang off its ends on thin lines, as in drawing apps.
        <path className="tw:stroke-primary" fill="none" strokeWidth={1 / scale}
          d={`M${annotation.shape.x1} ${annotation.shape.y1}L${annotation.shape.curve[0]} ${annotation.shape.curve[1]}`
            + `M${annotation.shape.x2} ${annotation.shape.y2}L${annotation.shape.curve[2]} ${annotation.shape.curve[3]}`} />
      ) : null}
      {handlesFor(annotation.shape, height).map(({ handle, point }) => (
        handle === "c1" || handle === "c2" ? (
          <rect key={handle} x={point.x - radius * 0.8} y={point.y - radius * 0.8} width={radius * 1.6} height={radius * 1.6}
            className="tw:fill-primary tw:stroke-background" strokeWidth={1.5 / scale} />
        ) : (
          <circle key={handle} cx={point.x} cy={point.y} r={radius}
            className="tw:fill-background tw:stroke-primary" strokeWidth={2 / scale} />
        )
      ))}
    </svg>
  );
}

function pairs(points: readonly number[]): string {
  const out: string[] = [];
  for (let index = 0; index + 1 < points.length; index += 2) out.push(`${points[index]},${points[index + 1]}`);
  return out.join(" ");
}
