import { useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";

import { RichTextView } from "../../editor/RichTextView";
import type { Asset } from "../../ipc/assets";
import { createMutationId, getErrorMessage } from "../../ipc/common";
import type {
  CheckPracticeAnswerInput,
  ComponentFeedback,
  LearnerAnswer,
  QuestionAttemptTarget,
  QuestionAttemptView,
  QuestionResponse,
  SavePracticeAnswerInput,
} from "../../ipc/questionAttempts";
import { Button } from "../../ui/primitives/Button";
import { Checkbox, RadioGroup } from "../../ui/primitives/FormControls";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Surface } from "../../ui/primitives/Surface";

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
}

export function InteractiveQuestionActivity({
  sessionId,
  cardId,
  attempt,
  assets,
  disabled,
  onSave,
  onCheck,
  onAttempt,
  onChecked,
  onError,
}: InteractiveQuestionActivityProps) {
  const target = useMemo<QuestionAttemptTarget>(() => ({
    sessionId,
    cardId,
    instanceId: attempt.instanceId,
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
  const checked = attempt.progress.phase === "checked";
  const busy = disabled || isSaving || isChecking;

  useEffect(() => {
    revisionRef.current = attempt.revision;
  }, [attempt.revision]);

  useEffect(() => {
    setResponse(initialResponse(attempt));
    revisionRef.current = attempt.revision;
    sequenceRef.current = 0;
    latestSequenceRef.current = 0;
    chainRef.current = Promise.resolve();
    saveErrorRef.current = null;
    shownAtRef.current = Date.now();
  }, [attempt.instanceId]);

  useEffect(() => {
    if (checked || attempt.question.kind !== "true_false") return;
    function onKeyDown(event: KeyboardEvent): void {
      if (busy || event.repeat || event.altKey || event.ctrlKey || event.metaKey) return;
      const target = event.target;
      if (target instanceof HTMLElement && (target.isContentEditable
        || ["INPUT", "TEXTAREA", "SELECT", "BUTTON", "A", "AUDIO", "VIDEO"].includes(target.tagName))) return;
      if (event.key.toLowerCase() === "t") {
        event.preventDefault();
        changeResponse({ kind: "true_false", selected: true });
      } else if (event.key.toLowerCase() === "f") {
        event.preventDefault();
        changeResponse({ kind: "true_false", selected: false });
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [attempt.question.kind, busy, checked]);

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
          request: {
            mutationId: createMutationId(),
            expectedRevision: revisionRef.current,
            answer: answerFor(next),
          },
        });
        revisionRef.current = saved.revision;
        saveErrorRef.current = null;
        onAttempt(saved);
      } catch (error: unknown) {
        saveErrorRef.current = error;
        onError(getErrorMessage(error));
      } finally {
        setIsSaving(false);
      }
    });
  }

  async function checkAnswer(): Promise<void> {
    if (busy || checked || !responseReady(response)) return;
    await chainRef.current;
    if (saveErrorRef.current) {
      onError("The latest answer could not be saved. Change it or retry before checking.");
      return;
    }
    setIsChecking(true);
    try {
      const checkedAttempt = await onCheck({
        target,
        request: {
          checkId: createMutationId(),
          expectedRevision: revisionRef.current,
          answer: answerFor(response),
          responseTimeMs: Math.max(0, Math.min(86_400_000, Date.now() - shownAtRef.current)),
        },
      });
      revisionRef.current = checkedAttempt.revision;
      onAttempt(checkedAttempt);
      await onChecked(checkedAttempt);
    } catch (error: unknown) {
      onError(getErrorMessage(error));
    } finally {
      setIsChecking(false);
    }
  }

  return (
    <div className="tw:grid tw:gap-5">
      <QuestionBody attempt={attempt} response={response} assets={assets} disabled={busy || checked}
        onChange={changeResponse} onError={onError} />
      {!checked ? (
        <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-center tw:gap-3">
          <Button variant="primary" size="large" disabled={busy || !responseReady(response)} onClick={() => void checkAnswer()}>
            {isChecking ? "Checking…" : isSaving ? "Saving…" : "Check answer"}
          </Button>
        </div>
      ) : <Feedback attempt={attempt} assets={assets} onError={onError} />}
    </div>
  );
}

function QuestionBody({ attempt, response, assets, disabled, onChange, onError }: {
  attempt: QuestionAttemptView;
  response: QuestionResponse;
  assets: Asset[];
  disabled: boolean;
  onChange: (response: QuestionResponse) => void;
  onError: (message: string) => void;
}) {
  const question = attempt.question;
  if (question.kind === "single_choice") {
    const selected = response.kind === "single_choice" ? response.selectedOptionId ?? "" : "";
    return <Surface className="tw:grid tw:gap-5 tw:p-5">
      <RichTextView document={question.prompt} assets={assets} onLinkError={onError} />
      <RadioGroup label="Answer choices" value={selected} disabled={disabled} presentation="cards"
        options={question.options.map((option) => ({
          value: option.id,
          label: <RichTextView document={option.content} assets={assets} onLinkError={onError} />,
        }))}
        onValueChange={(selectedOptionId) => onChange({ kind: "single_choice", selectedOptionId })} />
    </Surface>;
  }
  if (question.kind === "select_all") {
    const selected = new Set(response.kind === "select_all" ? response.selectedOptionIds : []);
    return <Surface className="tw:grid tw:gap-5 tw:p-5">
      <div className="tw:grid tw:gap-1">
        <RichTextView document={question.prompt} assets={assets} onLinkError={onError} />
        <small className="tw:text-muted-foreground">Select every correct option. An empty selection is allowed.</small>
      </div>
      <div className="tw:grid tw:gap-3">
        {question.options.map((option) => <label key={option.id}
          className="tw:flex tw:min-h-11 tw:items-start tw:gap-3 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:p-4">
          <Checkbox checked={selected.has(option.id)} disabled={disabled} onCheckedChange={(checked) => {
            const next = new Set(selected);
            if (checked) next.add(option.id); else next.delete(option.id);
            onChange({ kind: "select_all", selectedOptionIds: [...next] });
          }} />
          <span className="tw:min-w-0 tw:flex-1"><RichTextView document={option.content} assets={assets} onLinkError={onError} /></span>
        </label>)}
      </div>
    </Surface>;
  }
  if (question.kind === "true_false") {
    const value = response.kind === "true_false" && response.selected !== null
      ? response.selected ? "true" : "false"
      : "";
    return <Surface className="tw:grid tw:gap-5 tw:p-5">
      <RichTextView document={question.statement} assets={assets} onLinkError={onError} />
      <RadioGroup label="True or false" value={value} disabled={disabled} presentation="cards"
        options={[{ value: "true", label: "True" }, { value: "false", label: "False" }]}
        onValueChange={(selected) => onChange({ kind: "true_false", selected: selected === "true" })} />
      <small className="tw:text-muted-foreground">Keyboard: T = True · F = False</small>
    </Surface>;
  }
  if (question.kind === "matching") {
    const assignments = response.kind === "matching" ? response.assignments : [];
    return <MatchingBoard question={question} assignments={assignments} assets={assets} disabled={disabled}
      onChange={(next) => onChange({ kind: "matching", assignments: next })} onError={onError} />;
  }
  return <InlineNotice kind="error">This interactive question is not available in this practice view yet.</InlineNotice>;
}

interface PointerDrag {
  pointerId: number;
  itemId: string;
  startX: number;
  startY: number;
  active: boolean;
}

function MatchingBoard({ question, assignments, assets, disabled, onChange, onError }: {
  question: Extract<QuestionAttemptView["question"], { kind: "matching" }>;
  assignments: Array<{ itemId: string; targetId: string }>;
  assets: Asset[];
  disabled: boolean;
  onChange: (assignments: Array<{ itemId: string; targetId: string }>) => void;
  onError: (message: string) => void;
}) {
  const [selectedItemId, setSelectedItemId] = useState<string | null>(null);
  const [draggingItemId, setDraggingItemId] = useState<string | null>(null);
  const pointerRef = useRef<PointerDrag | null>(null);
  const suppressClickRef = useRef(false);
  const assignedByItem = new Map(assignments.map((entry) => [entry.itemId, entry.targetId]));
  const itemByTarget = new Map(assignments.map((entry) => [entry.targetId, entry.itemId]));

  function assign(itemId: string, targetId: string): void {
    if (disabled) return;
    const next = assignments.filter((entry) => entry.itemId !== itemId && entry.targetId !== targetId);
    next.push({ itemId, targetId });
    onChange(next);
    setSelectedItemId(null);
  }

  function pointerDown(event: ReactPointerEvent<HTMLElement>, itemId: string): void {
    if (disabled || event.button !== 0) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    pointerRef.current = {
      pointerId: event.pointerId,
      itemId,
      startX: event.clientX,
      startY: event.clientY,
      active: false,
    };
  }

  function pointerMove(event: ReactPointerEvent<HTMLElement>): void {
    const drag = pointerRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    const distance = Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY);
    if (!drag.active && distance >= 8) {
      drag.active = true;
      suppressClickRef.current = true;
      setDraggingItemId(drag.itemId);
    }
    if (drag.active) event.preventDefault();
  }

  function pointerEnd(event: ReactPointerEvent<HTMLElement>): void {
    const drag = pointerRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    pointerRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    if (drag.active) {
      const target = document.elementFromPoint(event.clientX, event.clientY)
        ?.closest<HTMLElement>("[data-match-target-id]");
      const targetId = target?.dataset.matchTargetId;
      if (targetId) assign(drag.itemId, targetId);
    }
    setDraggingItemId(null);
    window.setTimeout(() => { suppressClickRef.current = false; }, 0);
  }

  return <Surface className="tw:grid tw:gap-5 tw:p-5">
    <RichTextView document={question.prompt} assets={assets} onLinkError={onError} />
    <p className="tw:text-sm tw:text-muted-foreground">
      Drag an item by its handle onto a match, or select an item and choose “Match here.” Mouse, touch, pen, and non-drag input use the same answer model.
    </p>
    <div className="tw:grid tw:gap-4 tw:md:grid-cols-2">
      <div className="tw:grid tw:gap-3" aria-label="Items to match">
        {question.items.map((item, index) => <div key={item.id}
          className="tw:grid tw:gap-2 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-background tw:p-4">
          <RichTextView document={item.content} assets={assets} onLinkError={onError} />
          <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-2">
            <Button variant={selectedItemId === item.id ? "primary" : "secondary"} size="small"
              className="tw:touch-none" disabled={disabled}
              onPointerDown={(event) => pointerDown(event, item.id)} onPointerMove={pointerMove}
              onPointerUp={pointerEnd} onPointerCancel={pointerEnd}
              onClick={() => {
                if (suppressClickRef.current) return;
                setSelectedItemId((current) => current === item.id ? null : item.id);
              }}>
              {draggingItemId === item.id ? "Dragging…" : selectedItemId === item.id ? "Selected" : `Drag / select item ${index + 1}`}
            </Button>
            {assignedByItem.has(item.id) ? <small className="tw:text-muted-foreground">Matched</small> : null}
          </div>
        </div>)}
      </div>
      <div className="tw:grid tw:gap-3" aria-label="Matching targets">
        {question.targets.map((target, index) => <div key={target.id} data-match-target-id={target.id}
          className={draggingItemId
            ? "tw:grid tw:gap-2 tw:rounded-xl tw:border tw:border-solid tw:border-primary tw:bg-accent tw:p-4"
            : "tw:grid tw:gap-2 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-background tw:p-4"}>
          <RichTextView document={target.content} assets={assets} onLinkError={onError} />
          <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-2">
            <Button variant="secondary" size="small" disabled={disabled || selectedItemId === null}
              onClick={() => { if (selectedItemId) assign(selectedItemId, target.id); }}>
              Match here
            </Button>
            {itemByTarget.has(target.id) ? <small className="tw:text-muted-foreground">Target {index + 1} assigned</small> : null}
          </div>
        </div>)}
      </div>
    </div>
  </Surface>;
}

function Feedback({ attempt, assets, onError }: {
  attempt: QuestionAttemptView;
  assets: Asset[];
  onError: (message: string) => void;
}) {
  if (attempt.progress.phase !== "checked") return null;
  const evaluation = attempt.progress.evaluation;
  const title = evaluation.verdict === "correct"
    ? "Correct"
    : evaluation.verdict === "incomplete" ? "Incomplete" : "Not correct";
  const kind = evaluation.verdict === "correct" ? "success" : "warning";
  return <InlineNotice kind={kind} title={title}>
    <p>{evaluation.matchedCount} of {evaluation.requiredCount} required parts matched.</p>
    {evaluation.explanation ? <RichTextView document={evaluation.explanation} assets={assets} onLinkError={onError} /> : null}
    {evaluation.components.length > 0 ? <div className="tw:grid tw:gap-3">
      {evaluation.components.map((component, index) => <Surface key={`${component.kind}-${index}`} className="tw:grid tw:gap-2 tw:p-3">
        <strong>{componentSummary(component)}</strong>
        {component.kind === "choice" && component.rationale ? <RichTextView document={component.rationale} assets={assets} onLinkError={onError} /> : null}
        {component.kind === "placement" && component.rationale ? <RichTextView document={component.rationale} assets={assets} onLinkError={onError} /> : null}
        {component.kind === "boolean" && component.correctedStatement ? <RichTextView document={component.correctedStatement} assets={assets} onLinkError={onError} /> : null}
      </Surface>)}
    </div> : null}
  </InlineNotice>;
}

function componentSummary(component: ComponentFeedback): string {
  if (component.kind === "choice") {
    if (component.selected === component.expected) return component.expected ? "Correct option selected" : "Correctly left unselected";
    return component.expected ? "A correct option was missed" : "An incorrect option was selected";
  }
  if (component.kind === "boolean") {
    return component.selected === component.expected ? "Correct true/false choice" : "True/false choice was incorrect";
  }
  if (component.kind === "placement") {
    return component.selectedTargetId === component.expectedTargetId ? "Correct match" : "Incorrect match";
  }
  return component.selectedPosition === component.expectedPosition ? "Correct position" : "Incorrect position";
}

function initialResponse(attempt: QuestionAttemptView): QuestionResponse {
  if (attempt.progress.phase === "checked") return attempt.progress.firstAnswer.response;
  if (attempt.progress.savedAnswer) return attempt.progress.savedAnswer.response;
  const question = attempt.question;
  if (question.kind === "single_choice") return { kind: "single_choice", selectedOptionId: null };
  if (question.kind === "select_all") return { kind: "select_all", selectedOptionIds: [] };
  if (question.kind === "true_false") return { kind: "true_false", selected: null };
  if (question.kind === "matching") return { kind: "matching", assignments: [] };
  if (question.kind === "grouping") return { kind: "grouping", assignments: [] };
  return { kind: "ordering", orderedItemIds: question.items.map((item) => item.id) };
}

function responseReady(response: QuestionResponse): boolean {
  if (response.kind === "single_choice") return response.selectedOptionId !== null;
  if (response.kind === "true_false") return response.selected !== null;
  if (response.kind === "matching" || response.kind === "grouping") return response.assignments.length > 0;
  if (response.kind === "ordering") return response.orderedItemIds.length > 0;
  return true;
}
