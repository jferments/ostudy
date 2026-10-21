import { useCallback, useEffect, useRef, useState } from "react";

import type { ShowToast } from "../../../components/Toast";
import { getErrorMessage } from "../../../ipc/common";
import { renderDocumentPageTile } from "../../../ipc/documentAnnotations";
import {
  findDocumentFigures, getDocumentFigures, readDocumentFigures, setDocumentFigureHidden, setDocumentFiguresUseImages,
  type DocumentFigure, type DocumentFigures, type FigureBox,
} from "../../../ipc/documentFigures";
import { registerPlatformBackHandler } from "../../../platform/backNavigation";
import { Icon } from "../../../ui/icons/Icon";
import { InlineWorkspace } from "../../../ui/overlays/Dialog";
import { OverflowMenu } from "../../../ui/overlays/OverflowMenu";
import { Button } from "../../../ui/primitives/Button";
import { EmptyState, LoadingState } from "../../../ui/primitives/EmptyState";
import { Checkbox, Disclosure } from "../../../ui/primitives/FormControls";
import { InlineNotice } from "../../../ui/primitives/InlineNotice";
import { useLibraryFullPage } from "../../library/LibraryFrame";
import { cropTile } from "./cropTile";

interface DocumentFiguresWorkspaceProps {
  documentId: string;
  title: string;
  onClose: () => void;
  showToast: ShowToast;
}

/**
 * The figures oStudy found in one PDF: each drawn from the PDF with its
 * caption, the pages that explain it, what the learner's AI model read in it,
 * and a way to hide one that is not worth studying. A document imported before
 * figures existed, or with its images off, can be searched from here.
 */
export function DocumentFiguresWorkspace({ documentId, title, onClose, showToast }: DocumentFiguresWorkspaceProps) {
  useLibraryFullPage(true);
  const figures = useDocumentFigures(documentId, showToast);
  const [withCaptions, setWithCaptions] = useState(true);

  useEffect(() => registerPlatformBackHandler(() => {
    onClose();
    return true;
  }), [onClose]);

  const value = figures.value;
  const canRead = Boolean(value?.useImages && value.state === "found" && value.figures.some((figure) => !figure.hidden));
  const menu = value && value.useImages ? [
    ...(canRead ? [{
      id: "read",
      label: value.reading === "none" ? "Read figures with AI" : "Read figures again",
      disabled: figures.busy,
      onSelect: () => void figures.read(),
    }] : []),
    { id: "find", label: "Find figures again", disabled: figures.busy, onSelect: () => void figures.find() },
    { id: "off", label: "Stop using images", disabled: figures.busy, onSelect: () => void figures.setUseImages(false) },
  ] : [];

  return (
    <InlineWorkspace
      presentation="page"
      title={title}
      description="Figures"
      closeLabel="Back to documents"
      onClose={onClose}
      actions={menu.length ? <OverflowMenu label="Figure actions" items={menu} /> : undefined}
    >
      {figures.error && !value ? (
        <InlineNotice kind="error">{figures.error}</InlineNotice>
      ) : !value ? (
        <LoadingState label="Loading figures…" />
      ) : (
        <FiguresBody
          documentId={documentId}
          value={value}
          busy={figures.busy}
          activity={figures.activity}
          withCaptions={withCaptions}
          onWithCaptionsChange={setWithCaptions}
          onFind={() => void figures.find()}
          onRead={() => void figures.read()}
          onUseImages={() => void figures.setUseImages(true)}
          onHidden={(figureId, hidden) => void figures.setHidden(figureId, hidden)}
        />
      )}
    </InlineWorkspace>
  );
}

function useDocumentFigures(documentId: string, showToast: ShowToast) {
  const [value, setValue] = useState<DocumentFigures | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [activity, setActivity] = useState<"finding" | "reading" | "saving" | null>(null);
  // A newer read or save replaces an older one's result.
  const latest = useRef(0);
  const apply = useCallback(async (work: () => Promise<DocumentFigures>) => {
    const request = ++latest.current;
    try {
      const next = await work();
      if (latest.current === request) {
        setValue(next);
        setError(null);
      }
    } catch (failure: unknown) {
      const message = getErrorMessage(failure);
      if (latest.current === request) setError(message);
      showToast("error", message);
    }
  }, [showToast]);

  useEffect(() => { void apply(() => getDocumentFigures(documentId)); }, [apply, documentId]);

  const run = useCallback(async (kind: "finding" | "reading" | "saving", work: () => Promise<DocumentFigures>) => {
    setActivity(kind);
    try {
      await apply(work);
    } finally {
      setActivity(null);
    }
  }, [apply]);

  return {
    value,
    error,
    busy: activity !== null,
    activity,
    find: () => run("finding", () => findDocumentFigures(documentId)),
    read: () => run("reading", () => readDocumentFigures(documentId)),
    setUseImages: (useImages: boolean) => run(useImages ? "finding" : "saving", async () => {
      const saved = await setDocumentFiguresUseImages(documentId, useImages);
      return useImages ? findDocumentFigures(documentId) : saved;
    }),
    setHidden: (figureId: string, hidden: boolean) => run("saving", () => setDocumentFigureHidden(documentId, figureId, hidden)),
  };
}

function FiguresBody({ documentId, value, busy, activity, withCaptions, onWithCaptionsChange, onFind, onRead, onUseImages, onHidden }: {
  documentId: string;
  value: DocumentFigures;
  busy: boolean;
  activity: "finding" | "reading" | "saving" | null;
  withCaptions: boolean;
  onWithCaptionsChange: (withCaptions: boolean) => void;
  onFind: () => void;
  onRead: () => void;
  onUseImages: () => void;
  onHidden: (figureId: string, hidden: boolean) => void;
}) {
  if (busy && value.state !== "found") return <LoadingState label="Finding figures…" />;
  if (!value.useImages) {
    return (
      <EmptyState title="Images are off for this document" icon={<Icon name="image" size={20} />}
        description="oStudy uses only its text. Turn images on to find its figures."
        actions={<Button variant="primary" onClick={onUseImages}>Use images</Button>} />
    );
  }
  if (value.state === "none" || value.state === "pending") {
    return (
      <EmptyState title="No figures found yet" icon={<Icon name="image" size={20} />}
        description="oStudy has not searched this document for figures yet."
        actions={<Button variant="primary" onClick={onFind}>Find figures</Button>} />
    );
  }
  if (value.state === "unsupported") {
    return (
      <EmptyState title="Figures are found on desktop" icon={<Icon name="image" size={20} />}
        description="This device cannot search PDFs for figures yet. Import the document on a computer to find them." />
    );
  }
  if (value.state === "failed") {
    return (
      <EmptyState title="Figures could not be found" icon={<Icon name="image" size={20} />}
        description="Something went wrong while searching this document. The document is still used as text."
        actions={<Button onClick={onFind}>Try again</Button>} />
    );
  }

  const shown = value.figures.filter((figure) => !figure.hidden);
  const hidden = value.figures.filter((figure) => figure.hidden);
  if (value.figures.length === 0) {
    return (
      <EmptyState title="No figures in this document" icon={<Icon name="image" size={20} />}
        description="oStudy found no pictures or diagrams big enough to study."
        actions={<Button onClick={onFind} disabled={busy}>Search again</Button>} />
    );
  }
  return (
    <div className="tw:grid tw:min-w-0 tw:gap-4">
      <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3">
        <p className="tw:m-0 tw:text-small tw:text-muted-foreground" aria-live="polite">
          {activity === "finding" ? "Finding figures…"
            : activity === "reading" ? "Reading figures…"
              : figureCount(shown.length, hidden.length)}
        </p>
        <label className="tw:flex tw:min-h-11 tw:items-center tw:gap-3">
          <Checkbox checked={withCaptions} onCheckedChange={onWithCaptionsChange} />
          <span className="tw:text-small tw:font-medium tw:text-foreground">Show captions in the picture</span>
        </label>
      </div>
      <ReadingNotice reading={value.reading} busy={busy} onRead={onRead} />
      <FigureGrid documentId={documentId} figures={shown} withCaptions={withCaptions} disabled={busy} onHidden={onHidden} />
      {hidden.length ? (
        <Disclosure title={`Hidden (${hidden.length})`}>
          <div className="tw:p-3">
            <FigureGrid documentId={documentId} figures={hidden} withCaptions={withCaptions} disabled={busy} onHidden={onHidden} />
          </div>
        </Disclosure>
      ) : null}
    </div>
  );
}

/** Why some or all figures have no reading, when that needs saying. */
function ReadingNotice({ reading, busy, onRead }: {
  reading: DocumentFigures["reading"];
  busy: boolean;
  onRead: () => void;
}) {
  if (reading === "unavailable") {
    return (
      <InlineNotice title="Your AI model cannot see images">
        Figures keep their captions and passages. Choose a model that reads images in Settings, then read them again.
      </InlineNotice>
    );
  }
  if (reading === "partial" || reading === "failed") {
    return (
      <InlineNotice kind="warning" title={reading === "partial" ? "Some figures could not be read" : "Figures could not be read"}>
        <span className="tw:flex tw:flex-wrap tw:items-center tw:gap-3">
          They keep their captions and passages.
          <Button size="small" disabled={busy} onClick={onRead}>Try again</Button>
        </span>
      </InlineNotice>
    );
  }
  return null;
}

function figureCount(shown: number, hidden: number): string {
  const figures = `${shown} ${shown === 1 ? "figure" : "figures"}`;
  return hidden ? `${figures}, ${hidden} hidden` : figures;
}

function FigureGrid({ documentId, figures, withCaptions, disabled, onHidden }: {
  documentId: string;
  figures: DocumentFigure[];
  withCaptions: boolean;
  disabled: boolean;
  onHidden: (figureId: string, hidden: boolean) => void;
}) {
  return (
    <ul className="tw:m-0 tw:grid tw:list-none tw:grid-cols-[repeat(auto-fill,minmax(16rem,1fr))] tw:gap-3 tw:p-0">
      {figures.map((figure) => (
        <li key={figure.figureId} aria-labelledby={`figure-${figure.figureId}`}
          className="tw:grid tw:min-w-0 tw:content-start tw:gap-2 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:p-3">
          <FigureCrop documentId={documentId} figure={figure} withCaption={withCaptions} />
          <div className="tw:flex tw:min-w-0 tw:items-start tw:justify-between tw:gap-2">
            <h3 id={`figure-${figure.figureId}`} className="tw:m-0 tw:text-small tw:font-semibold">
              {figure.label ?? "Figure"}
              <span className="tw:font-normal tw:text-muted-foreground"> · Page {figure.pageNumber}</span>
            </h3>
            <Button size="small" variant="ghost" disabled={disabled}
              aria-label={`${figure.hidden ? "Show" : "Hide"} ${figure.label ?? "figure"} on page ${figure.pageNumber}`}
              onClick={() => onHidden(figure.figureId, !figure.hidden)}>
              {figure.hidden ? "Show" : "Hide"}
            </Button>
          </div>
          {figure.reading ? <FigureReadingSummary reading={figure.reading} /> : null}
          {figure.caption && !withCaptions ? (
            <p className="tw:m-0 tw:text-small tw:text-muted-foreground">{figure.caption}</p>
          ) : null}
          <FigurePassages figure={figure} />
        </li>
      ))}
    </ul>
  );
}

function FigureReadingSummary({ reading }: { reading: NonNullable<DocumentFigure["reading"]> }) {
  return (
    <div className="tw:grid tw:gap-1 tw:text-small">
      <p className="tw:m-0 tw:font-medium tw:text-foreground">{reading.title}</p>
      <p className="tw:m-0 tw:text-muted-foreground">{reading.shows}</p>
      <p className="tw:m-0 tw:text-muted-foreground">
        {importanceLabel(reading.importance)}
        {reading.worthStudying ? null : " · Not worth studying"}
      </p>
      {reading.useInText ? (
        <Disclosure title="How the text uses it">
          <p className="tw:m-0 tw:p-3 tw:text-muted-foreground">{reading.useInText}</p>
        </Disclosure>
      ) : null}
    </div>
  );
}

function importanceLabel(importance: number): string {
  if (importance >= 5) return "Essential";
  if (importance === 4) return "Key figure";
  if (importance === 3) return "Helpful";
  return "Minor";
}

function FigurePassages({ figure }: { figure: DocumentFigure }) {
  const mentions = figure.passages.filter((passage) => passage.kind === "mention");
  const neighbours = figure.passages.filter((passage) => passage.kind === "neighbour");
  if (!mentions.length && !neighbours.length) return null;
  return (
    <div className="tw:grid tw:gap-1 tw:text-small">
      {mentions.map((passage, index) => (
        <p key={`mention-${index}`} className="tw:m-0">
          <span className="tw:font-medium">Page {passage.pageNumber}:</span>{" "}
          <span className="tw:text-muted-foreground">{passage.excerpt}</span>
        </p>
      ))}
      {neighbours.length ? (
        <p className="tw:m-0 tw:text-muted-foreground">
          {neighbours.length === 1 ? "Explained on page " : "Explained on pages "}
          {neighbours.map((passage) => passage.pageNumber).join(" and ")}
        </p>
      ) : null}
    </div>
  );
}

/** The figure drawn from the PDF, sharp enough to read its labels. */
function FigureCrop({ documentId, figure, withCaption }: {
  documentId: string;
  figure: DocumentFigure;
  withCaption: boolean;
}) {
  const area: FigureBox = withCaption && figure.captionBounds ? union(figure.bounds, figure.captionBounds) : figure.bounds;
  const tile = cropTile(figure.pageNumber, figure.pageWidth, figure.pageHeight, area);
  const key = JSON.stringify(tile);
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let current = true;
    let objectUrl: string | null = null;
    setFailed(false);
    renderDocumentPageTile(documentId, tile)
      .then((bytes) => {
        if (!current) return;
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
        setUrl(objectUrl);
      })
      .catch(() => { if (current) setFailed(true); });
    return () => {
      current = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
    // The tile is rebuilt every render; its key says when it changed.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [documentId, key]);
  const alt = figure.caption ?? `${figure.label ?? "Figure"} on page ${figure.pageNumber}`;
  return (
    <div className="tw:grid tw:min-h-32 tw:place-items-center tw:overflow-hidden tw:rounded-lg tw:bg-muted"
      style={{ aspectRatio: `${tile.width} / ${tile.height}` }}>
      {url ? (
        <img src={url} alt={alt} className="tw:block tw:size-full tw:object-contain" />
      ) : failed ? (
        <span className="tw:p-3 tw:text-small tw:text-muted-foreground">This figure could not be drawn.</span>
      ) : (
        <span className="tw:sr-only">Drawing the figure…</span>
      )}
    </div>
  );
}

function union(a: FigureBox, b: FigureBox): FigureBox {
  const left = Math.min(a.left, b.left);
  const top = Math.min(a.top, b.top);
  return {
    left,
    top,
    width: Math.max(a.left + a.width, b.left + b.width) - left,
    height: Math.max(a.top + a.height, b.top + b.height) - top,
  };
}
