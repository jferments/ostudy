import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";

import type { Asset } from "../../ipc/assets";
import { createMutationId, getErrorMessage } from "../../ipc/common";
import type {
  CheckPracticeAnswerInput,
  LearnerAnswer,
  QuestionAttemptTarget,
  QuestionAttemptView,
  QuestionResponse,
  SavePracticeAnswerInput,
} from "../../ipc/questionAttempts";
import { useOverlayContext } from "../../ui/overlays/overlayContext";
import { Button } from "../../ui/primitives/Button";
import { practiceStyles } from "./presentation";
import { QuestionBody } from "./QuestionBody";
import { emptyQuestionResponse, responseReady } from "./questionPresentation";

interface InteractiveQuestionActivityProps {
  sessionId: string;
  cardId: string;
  attempt: QuestionAttemptView;
  assets: Asset[];
  disabled: boolean;
  onSave: (input: SavePracticeAnswerInput) => Promise<QuestionAttemptView>;
  onCheck: (input: CheckPracticeAnswerInput) => Promise<QuestionAttemptView>;
  onAttempt: (attempt: QuestionAttemptView) => void;
  onChecked: (attempt: QuestionAttemptView) => void | Promise<void>;
  onError: (message: string) => void;
  /** Where the host keeps its main action. Without one the button follows the question. */
  actionSlot?: HTMLElement | null;
}

export function InteractiveQuestionActivity({
  sessionId, cardId, attempt, assets, disabled, onSave, onCheck, onAttempt, onChecked, onError, actionSlot = null,
}: InteractiveQuestionActivityProps) {
  const target = useMemo<QuestionAttemptTarget>(() => ({
    sessionId, cardId, instanceId: attempt.instanceId,
  }), [attempt.instanceId, cardId, sessionId]);
  const [response, setResponse] = useState<QuestionResponse>(() => initialResponse(attempt));
  const [isSaving, setIsSaving] = useState(false);
  const [isChecking, setIsChecking] = useState(false);
  const revisionRef = useRef(attempt.revision);
  const sequenceRef = useRef(0);
  const latestSequenceRef = useRef(0);
  const chainRef = useRef<Promise<void>>(Promise.resolve());
  const saveErrorRef = useRef<unknown | null>(null);
  const shownAtRef = useRef(Date.now());
  const attemptRef = useRef(attempt);
  const changeResponseRef = useRef<(next: QuestionResponse) => void>(() => undefined);
  attemptRef.current = attempt;
  const checked = attempt.progress.phase === "checked";
  const busy = disabled || isSaving || isChecking;
  const { topId } = useOverlayContext();

  useEffect(() => { revisionRef.current = attempt.revision; }, [attempt.revision]);
  useEffect(() => {
    const currentAttempt = attemptRef.current;
    setResponse(initialResponse(currentAttempt));
    revisionRef.current = currentAttempt.revision;
    sequenceRef.current = 0;
    latestSequenceRef.current = 0;
    chainRef.current = Promise.resolve();
    saveErrorRef.current = null;
    shownAtRef.current = Date.now();
  }, [attempt.instanceId]);

  useEffect(() => {
    if (checked || attempt.question.kind !== "true_false") return;
    function onKeyDown(event: KeyboardEvent): void {
      if (busy || event.repeat || event.defaultPrevented || event.isComposing
        || event.altKey || event.ctrlKey || event.metaKey || topId !== null) return;
      const target = event.target;
      if (target instanceof HTMLElement && (target.isContentEditable
        || ["INPUT", "TEXTAREA", "SELECT", "BUTTON", "A", "AUDIO", "VIDEO"].includes(target.tagName))) return;
      if (event.key.toLowerCase() === "t") {
        event.preventDefault(); changeResponseRef.current({ kind: "true_false", selected: true });
      } else if (event.key.toLowerCase() === "f") {
        event.preventDefault(); changeResponseRef.current({ kind: "true_false", selected: false });
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [attempt.question.kind, busy, checked, topId]);

  function answerFor(next: QuestionResponse): LearnerAnswer {
    return { schemaVersion: 1, instanceId: attempt.instanceId, response: next };
  }
  function changeResponse(next: QuestionResponse): void {
    if (checked || disabled) return;
    setResponse(next);
    saveErrorRef.current = null;
    const sequence = ++sequenceRef.current;
    latestSequenceRef.current = sequence;
    chainRef.current = chainRef.current.then(async () => {
      if (sequence !== latestSequenceRef.current) return;
      setIsSaving(true);
      try {
        const saved = await onSave({
          target,
          request: { mutationId: createMutationId(), expectedRevision: revisionRef.current, answer: answerFor(next) },
        });
        revisionRef.current = saved.revision;
        saveErrorRef.current = null;
        onAttempt(saved);
      } catch (error: unknown) {
        saveErrorRef.current = error; onError(getErrorMessage(error));
      } finally { setIsSaving(false); }
    });
  }
  changeResponseRef.current = changeResponse;
  async function checkAnswer(): Promise<void> {
    if (busy || checked || !responseReady(response)) return;
    await chainRef.current;
    if (saveErrorRef.current) {
      onError("The latest answer could not be saved. Change it or retry before checking."); return;
    }
    setIsChecking(true);
    try {
      const checkedAttempt = await onCheck({
        target,
        request: {
          checkId: createMutationId(), expectedRevision: revisionRef.current, answer: answerFor(response),
          responseTimeMs: Math.max(0, Math.min(86_400_000, Date.now() - shownAtRef.current)),
        },
      });
      revisionRef.current = checkedAttempt.revision;
      onAttempt(checkedAttempt);
      await onChecked(checkedAttempt);
    } catch (error: unknown) { onError(getErrorMessage(error)); }
    finally { setIsChecking(false); }
  }
  const progress = attempt.progress;
  const checkButton = <Button variant="primary" size="large" className={actionSlot ? practiceStyles.primaryAction : undefined}
    disabled={busy || !responseReady(response)} onClick={() => void checkAnswer()}>
    {isChecking ? "Checking…" : isSaving ? "Saving…" : "Check answer"}
  </Button>;
  return <div className="tw:grid tw:gap-5">
    <QuestionBody question={attempt.question} response={progress.phase === "checked" ? progress.firstAnswer.response : response}
      evaluation={progress.phase === "checked" ? progress.evaluation : null} assets={assets} disabled={busy || checked}
      onChange={changeResponse} onError={onError} />
    {checked ? null : actionSlot ? createPortal(checkButton, actionSlot)
      : <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-center tw:gap-3">{checkButton}</div>}
  </div>;
}

function initialResponse(attempt: QuestionAttemptView): QuestionResponse {
  if (attempt.progress.phase === "checked") return attempt.progress.firstAnswer.response;
  return attempt.progress.savedAnswer?.response ?? emptyQuestionResponse(attempt.question);
}
