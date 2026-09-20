import { useEffect, useState } from "react";

import { RichTextView } from "../../editor/RichTextView";
import type { Asset } from "../../ipc/assets";
import type { RichDocument } from "../../ipc/common";
import { useAppearance } from "../../ui/appearance/AppearanceProvider";
import { cn } from "../../ui/internal/cn";
import { practiceStyles as view } from "./presentation";
import styles from "./PracticeCard.module.css";

export interface RecallSurfaceContent {
  frontDoc: RichDocument;
  backDoc: RichDocument;
  frontText: string;
  backText: string;
  assets: Asset[];
}

/** The same content surface for Practice and author previews. This component has
 * no session, schedule, persistence, Tutor or provider effects. */
export function RecallCardSurface({ presentation, presentationKey, revealed, busy = false,
  flipAnimationEnabled, displayQuestionOnAnswer = false, onReveal, onLinkError,
}: {
  presentation: RecallSurfaceContent;
  presentationKey: string;
  revealed: boolean;
  busy?: boolean;
  flipAnimationEnabled?: boolean;
  displayQuestionOnAnswer?: boolean;
  onReveal: () => void;
  onLinkError: (message: string) => void;
}) {
  const [showingAnswer, setShowingAnswer] = useState(revealed);
  const appearance = useAppearance();
  const animateFlip = flipAnimationEnabled ?? appearance.motion === "full";
  const normalizedQuestion = presentation.frontText.trim();
  const combineQuestionAndAnswer = displayQuestionOnAnswer
    && !(normalizedQuestion.length > 0 && presentation.backText.includes(normalizedQuestion));

  useEffect(() => { setShowingAnswer(revealed); }, [presentationKey, revealed]);

  function activateAnimatedCard(): void {
    if (busy) return;
    if (!revealed) {
      onReveal();
      return;
    }
    setShowingAnswer((side) => !side);
  }

  function questionContent(mediaPlaybackActive: boolean) {
    return <>
      <span className={view.practiceSideLabel}>Question</span>
      <RichTextView document={presentation.frontDoc} assets={presentation.assets}
        mediaPlaybackActive={mediaPlaybackActive} onLinkError={onLinkError} />
    </>;
  }

  function answerContent(mediaPlaybackActive: boolean) {
    return <>
      <span className={view.practiceSideLabel}>Answer</span>
      <RichTextView document={presentation.backDoc} assets={presentation.assets}
        mediaPlaybackActive={mediaPlaybackActive} onLinkError={onLinkError} />
    </>;
  }

  return animateFlip ? (
    <section className={cn(styles.cardSurface, view.cardSurface, styles.flipCard)} data-flipped={showingAnswer}
      role="button" tabIndex={0} aria-label={!revealed ? "Reveal answer" : showingAnswer ? "Show question" : "Show answer"}
      aria-disabled={busy ? "true" : undefined}
      onClick={(event) => {
        const target = event.target;
        if (target instanceof HTMLElement && target.closest("a, button, input, textarea, select, audio, video")) return;
        activateAnimatedCard();
      }}
      onKeyDown={(event) => {
        if (event.key !== " " && event.key !== "Enter") return;
        if (event.target !== event.currentTarget) return;
        event.preventDefault();
        event.stopPropagation();
        activateAnimatedCard();
      }}>
      <div className={styles.flipInner}>
        <div className={cn(styles.flipFace, styles.flipFront)} aria-hidden={showingAnswer} inert={showingAnswer}>
          {questionContent(!showingAnswer)}
          {!showingAnswer ? <small className={view.practiceFlipHint}>Click the card to flip it</small> : null}
        </div>
        <div className={cn(styles.flipFace, styles.flipBack)} aria-hidden={!showingAnswer} inert={!showingAnswer}>
          {combineQuestionAndAnswer ? <>{questionContent(showingAnswer)}<div className={view.practiceAnswerDivider} /></> : null}
          {answerContent(showingAnswer)}
        </div>
      </div>
    </section>
  ) : (
    <section className={cn(styles.cardSurface, view.cardSurface)}>
      {!revealed || combineQuestionAndAnswer ? questionContent(!revealed || combineQuestionAndAnswer) : null}
      {revealed ? <>{combineQuestionAndAnswer ? <div className={view.practiceAnswerDivider} /> : null}{answerContent(true)}</> : null}
    </section>
  );
}
