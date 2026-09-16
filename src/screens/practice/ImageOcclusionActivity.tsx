import { useMemo, useRef, useState } from "react";

import { RichTextView } from "../../editor/RichTextView";
import { OcclusionImageView } from "../../editor/occlusion/OcclusionImageView";
import { FIT_VIEWPORT, type ImageViewport } from "../../editor/occlusion/geometry";
import type { Asset } from "../../ipc/assets";
import type { ImageOcclusionAttemptView, QuestionAttemptTarget, RevealPracticeQuestionInput } from "../../ipc/questionAttempts";
import { Dialog } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { Surface } from "../../ui/primitives/Surface";
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
}

export function ImageOcclusionActivity({ sessionId, cardId, attempt, assets, disabled, onReveal, onAttempt, onRevealed, onError }: ImageOcclusionActivityProps) {
  const target = useMemo<QuestionAttemptTarget>(() => ({ sessionId, cardId, instanceId: attempt.instanceId }),
    [sessionId, cardId, attempt.instanceId]);
  const [enlarged, setEnlarged] = useState(false);
  const [maximumEdge, setMaximumEdge] = useState(2048);
  const viewport = useRef<ImageViewport>(FIT_VIEWPORT);
  const image = usePracticeOcclusionImage(target, attempt.revision, maximumEdge);
  const reveal = useOcclusionReveal({ target, attempt, disabled, imageReady: image.ready, loadedAt: image.loadedAt, onReveal, onAttempt, onRevealed });
  const progress = attempt.progress;
  const revealed = progress.phase === "revealed";
  const busy = disabled || reveal.busy;

  const imageContent = image.url && image.size ? <OcclusionImageView imageUrl={image.url}
    imageKey={attempt.instanceId} imageSize={image.size} targets={attempt.question.targetRectangles}
    initialViewport={viewport.current} onViewportChange={(next) => { viewport.current = next; }}
    label={revealed ? "Revealed study image" : "Masked study image"} />
    : image.error ? <InlineNotice kind="error">{image.error} <Button disabled={busy} onClick={image.retry}>Retry image</Button></InlineNotice>
      : <InlineProgress>Preparing the study image…</InlineProgress>;

  return <Surface className="tw:grid tw:min-w-0 tw:gap-5 tw:p-4 tw:md:p-5">
    <RichTextView document={attempt.question.prompt} assets={assets} onLinkError={onError} />
    <div className="tw:flex tw:flex-wrap tw:gap-2">
      <Button disabled={!image.ready} onClick={() => setEnlarged(true)}>Enlarge image</Button>
      <Button disabled={busy || !image.ready || maximumEdge === 4096} onClick={() => setMaximumEdge(4096)}>Load sharper image</Button>
    </div>
    {!enlarged ? imageContent : <Dialog title="Study image" width="workspace" presentation="fullscreen"
      onClose={() => setEnlarged(false)}>
      <div className="tw:grid tw:min-w-0 tw:gap-4 tw:p-4">{imageContent}
        <Button onClick={() => setEnlarged(false)}>Return to question</Button>
      </div>
    </Dialog>}
    {reveal.error ? <InlineNotice kind="error">{reveal.error}
      <Button disabled={busy} onClick={() => void reveal.reload()}>Reload saved answer state</Button>
    </InlineNotice> : null}
    {!revealed ? <div className="tw:flex tw:justify-center">
      <Button variant="primary" size="large" disabled={busy || !image.ready} onClick={() => void reveal.reveal()}>
        {reveal.busy ? "Revealing…" : "Reveal answer"}
      </Button>
    </div> : <section className="tw:grid tw:gap-3" aria-label="Self-assessed image answer">
      <h3>Answer</h3>
      <RichTextView document={progress.answer} assets={assets} onLinkError={onError} />
      {progress.explanation ? <RichTextView document={progress.explanation} assets={assets} onLinkError={onError} /> : null}
      <p className="tw:text-sm tw:text-muted-foreground">Compare this answer with your recall, then choose Wrong, Hard, Medium or Easy below. This question is self-assessed.</p>
    </section>}
  </Surface>;
}
