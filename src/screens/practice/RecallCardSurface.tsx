import { useEffect, useState, type MouseEvent } from "react";

import { RichTextView } from "../../editor/RichTextView";
import type { Asset } from "../../ipc/assets";
import type { RichDocument } from "../../ipc/common";
import { useAppearance } from "../../ui/appearance/AppearanceProvider";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Button } from "../../ui/primitives/Button";
import { practiceStyles as view } from "./presentation";
import styles from "./PracticeCard.module.css";

export interface RecallSurfaceContent {
  frontDoc: RichDocument;
  backDoc: RichDocument;
  frontText: string;
  backText: string;
  assets: Asset[];
}

const FACE_CONTROLS = "a, button, input, textarea, select, audio, video, [contenteditable=true]";

/** The same content surface for Practice and author previews. This component has
 * no session, schedule, persistence, Tutor or provider effects.
 *
 * The card is readable content, never a control: screen readers read the question
 * and answer themselves. The host reveals the answer with its own button, and
 * `sideToggle` adds a real button that turns a revealed animated card over.
 * Clicking the face does the same as a pointer convenience. */
export function RecallCardSurface({ presentation, presentationKey, revealed, busy = false,
  flipAnimationEnabled, displayQuestionOnAnswer = false, sideToggle = false, onReveal, onLinkError,
}: {
  presentation: RecallSurfaceContent;
  presentationKey: string;
  revealed: boolean;
  busy?: boolean;
  flipAnimationEnabled?: boolean;
  displayQuestionOnAnswer?: boolean;
  /** Show a "Show question" / "Show answer" button on a revealed animated card. */
  sideToggle?: boolean;
  onReveal: () => void;
  onLinkError: (message: string) => void;
}) {
  const [showingAnswer, setShowingAnswer] = useState(revealed);
  const appearance = useAppearance();
  const animateFlip = flipAnimationEnabled ?? appearance.motion === "full";
  const toggleShown = animateFlip && sideToggle && revealed;
  const normalizedQuestion = presentation.frontText.trim();
  const combineQuestionAndAnswer = displayQuestionOnAnswer
    && !(normalizedQuestion.length > 0 && presentation.backText.includes(normalizedQuestion));

  useEffect(() => { setShowingAnswer(revealed); }, [presentationKey, revealed]);

  function turnCard(): void {
    if (busy) return;
    if (!revealed) {
      onReveal();
      return;
    }
    setShowingAnswer((side) => !side);
  }

  function clickFace(event: MouseEvent<HTMLElement>): void {
    const target = event.target;
    if (target instanceof Element && target.closest(FACE_CONTROLS)) return;
    // Selecting text on the card is reading it, not asking to turn it over.
    const selection = window.getSelection();
    if (selection && !selection.isCollapsed && selection.anchorNode && event.currentTarget.contains(selection.anchorNode)) return;
    turnCard();
  }

  function sideLabel(side: "question" | "answer") {
    return <span className={cn(view.practiceSideLabel, side === "answer" && view.practiceAnswerLabel,
      toggleShown && view.practiceSideLabelBesideToggle)}>{side === "question" ? "Question" : "Answer"}</span>;
  }

  function questionContent(mediaPlaybackActive: boolean) {
    return <>
      {sideLabel("question")}
      <RichTextView document={presentation.frontDoc} assets={presentation.assets}
        mediaPlaybackActive={mediaPlaybackActive} onLinkError={onLinkError} />
    </>;
  }

  function answerContent(mediaPlaybackActive: boolean) {
    return <>
      {sideLabel("answer")}
      <RichTextView document={presentation.backDoc} assets={presentation.assets}
        mediaPlaybackActive={mediaPlaybackActive} onLinkError={onLinkError} />
    </>;
  }

  return animateFlip ? (
    <section className={cn(styles.cardSurface, view.cardSurface, styles.flipCard)} data-flipped={showingAnswer}
      onClick={clickFace}>
      {toggleShown ? <Button variant="ghost" size="small" className={view.cardSideToggle} disabled={busy}
        onClick={turnCard}>
        <Icon name="restore" size={16} />{showingAnswer ? "Show question" : "Show answer"}
      </Button> : null}
      <div className={styles.flipInner}>
        <div className={cn(styles.flipFace, styles.flipFront, view.cardFrame, view.cardFace)} aria-hidden={showingAnswer} inert={showingAnswer}>
          {questionContent(!showingAnswer)}
        </div>
        <div className={cn(styles.flipFace, styles.flipBack, view.cardFrame, view.cardFace)} aria-hidden={!showingAnswer} inert={!showingAnswer}>
          {combineQuestionAndAnswer ? <>{questionContent(showingAnswer)}<div className={view.practiceAnswerDivider} /></> : null}
          {answerContent(showingAnswer)}
        </div>
      </div>
    </section>
  ) : (
    <section className={cn(styles.cardSurface, view.cardSurface, view.cardFrame, view.cardFace)}>
      {!revealed || combineQuestionAndAnswer ? questionContent(!revealed || combineQuestionAndAnswer) : null}
      {revealed ? <>{combineQuestionAndAnswer ? <div className={view.practiceAnswerDivider} /> : null}{answerContent(true)}</> : null}
    </section>
  );
}
