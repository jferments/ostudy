import { useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { RichTextView } from "../../editor/RichTextView";
import { OcclusionImageView } from "../../editor/occlusion/OcclusionImageView";
import { FIT_VIEWPORT, type ImageViewport } from "../../editor/occlusion/geometry";
import type { Asset } from "../../ipc/assets";
import type { ImageOcclusionAttemptView, QuestionAttemptTarget, RevealPracticeQuestionInput } from "../../ipc/questionAttempts";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { Surface } from "../../ui/primitives/Surface";
import { cn } from "../../ui/internal/cn";
import { practiceStyles } from "./presentation";
import { useOcclusionReveal } from "./useOcclusionReveal";
import { usePracticeOcclusionImage } from "./usePracticeOcclusionImage";

interface ImageOcclusionActivityProps {
  sessionId: string;
  cardId: string;
  attempt: ImageOcclusionAttemptView;
  assets: Asset[];
  disabled: boolean;
  onReveal: (input: RevealPracticeQuestionInput) => Promise<ImageOcclusionAttemptView>;
  onAttempt: (attempt: ImageOcclusionAttemptView) => void;
  onRevealed: (attempt: ImageOcclusionAttemptView) => void | Promise<void>;
  onError: (message: string) => void;
  /** Where the host keeps its main action. Without one the button follows the image. */
  actionSlot?: HTMLElement | null;
}

export function ImageOcclusionActivity({ sessionId, cardId, attempt, assets, disabled, onReveal, onAttempt, onRevealed, onError, actionSlot = null }: ImageOcclusionActivityProps) {
  const target = useMemo<QuestionAttemptTarget>(() => ({ sessionId, cardId, instanceId: attempt.instanceId }),
    [sessionId, cardId, attempt.instanceId]);
  const [maximumEdge, setMaximumEdge] = useState(1024);
  const viewport = useRef<ImageViewport>(FIT_VIEWPORT);
  const image = usePracticeOcclusionImage(target, attempt.revision, maximumEdge);
  const reveal = useOcclusionReveal({ target, attempt, disabled, imageReady: image.ready, loadedAt: image.loadedAt, onReveal, onAttempt, onRevealed });
  const progress = attempt.progress;
  const revealed = progress.phase === "revealed";
  const busy = disabled || reveal.busy;

  function updateViewport(next: ImageViewport): void {
    viewport.current = next;
    const requestedEdge = next.zoom >= 3 ? 4096 : next.zoom >= 1.5 ? 2048 : 1024;
    setMaximumEdge((current) => Math.max(current, requestedEdge));
  }

  const imageContent = image.url && image.size ? <OcclusionImageView imageUrl={image.url}
    imageKey={attempt.instanceId} imageSize={image.size} targets={revealed ? [] : attempt.question.targetRectangles}
    initialViewport={viewport.current} onViewportChange={updateViewport}
    label={revealed ? "Revealed study image" : "Masked study image"} />
    : image.error ? <InlineNotice kind="error">{image.error} <Button disabled={busy} onClick={image.retry}>Retry image</Button></InlineNotice>
      : <InlineProgress>Preparing the study image…</InlineProgress>;

  const revealButton = <Button variant="primary" size="large" className={actionSlot ? practiceStyles.primaryAction : undefined}
    disabled={busy || !image.ready} onClick={() => void reveal.reveal()}>
    {reveal.busy ? "Revealing…" : "Reveal answer"}
  </Button>;

  return <Surface className={cn("tw:grid tw:min-w-0 tw:gap-5", practiceStyles.questionCard)}>
    <RichTextView document={attempt.question.prompt} assets={assets} onLinkError={onError} />
    {imageContent}
    {image.error && image.url ? <InlineNotice kind="warning">An updated image could not be prepared. The previously loaded image remains visible. <Button disabled={busy} onClick={image.retry}>Retry</Button></InlineNotice> : null}
    {reveal.error ? <InlineNotice kind="error">{reveal.error}
      <Button disabled={busy} onClick={() => void reveal.reload()}>Reload saved answer state</Button>
    </InlineNotice> : null}
    {!revealed ? actionSlot ? createPortal(revealButton, actionSlot) : <div className="tw:flex tw:justify-center">{revealButton}</div>
      : <section className="tw:grid tw:gap-3" aria-label="Self-assessed image answer">
        <h3 className={cn(practiceStyles.practiceSideLabel, practiceStyles.practiceAnswerLabel, "tw:m-0")}>Answer</h3>
        <RichTextView document={progress.answer} assets={assets} onLinkError={onError} />
        {progress.explanation ? <RichTextView document={progress.explanation} assets={assets} onLinkError={onError} /> : null}
        <p className="tw:m-0 tw:text-small tw:text-muted-foreground">Compare this answer with your recall, then choose Wrong, Hard, Medium or Easy below. This question is self-assessed.</p>
      </section>}
  </Surface>;
}
