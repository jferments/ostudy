import { useId, useState, type ReactNode } from "react";

import type { QuizFeedback, TutorTurn } from "../../ipc/tutor";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Button } from "../../ui/primitives/Button";
import { RadioGroup } from "../../ui/primitives/FormControls";
import { TutorMarkdown } from "./TutorMarkdown";

type Outcome = QuizFeedback["outcome"];

const VERDICT: Record<Outcome, string> = {
  correct: "Correct",
  partial: "Partly correct",
  incorrect: "Not yet correct",
  unclear: "Needs clarification",
};

// A run's bar: each answered question in its outcome's color, this one in
// the accent until it is answered, and the rest still to come.
const SEGMENT: Record<Outcome | "current" | "skipped" | "ahead", string> = {
  correct: "tw:bg-success",
  partial: "tw:bg-warning",
  incorrect: "tw:bg-destructive",
  unclear: "tw:bg-muted-foreground/50",
  current: "tw:bg-primary",
  skipped: "tw:bg-muted-foreground/25",
  ahead: "tw:bg-muted",
};

const card = "tw:grid tw:min-w-0 tw:gap-3.5 tw:rounded-2xl tw:border tw:border-solid tw:border-border tw:bg-card tw:p-5 tw:compact:p-4";
const option = "tw:min-h-12 tw:min-w-0 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-background tw:px-3.5 tw:py-2.5 tw:text-left tw:transition-colors tw:duration-(--ostudy-motion-duration-fast) tw:reduce-motion:transition-none tw:coarse:min-h-touch";
const letter = "tw:grid tw:size-6.5 tw:shrink-0 tw:place-items-center tw:rounded-lg tw:bg-muted tw:text-small tw:font-semibold tw:text-muted-foreground";

export interface TutorQuizCardProps {
  /** The question. */
  turn: TutorTurn;
  /** Its graded answer, once there is one. */
  answer: TutorTurn | null;
  /** How the run's earlier questions went, first to last; null for one not answered. */
  earlier: readonly (Outcome | null)[];
  /** The conversation's last question, which alone offers the next one. */
  latest: boolean;
  disabled: boolean;
  citations: ReactNode;
  onChoose: (choice: number) => void;
  onNext: () => void;
  onExplain: () => void;
  onSave: () => void;
}

/**
 * One quiz question as a card: its place in the run, its options when it has
 * them, and once answered, the verdict, what was met and missed, and what to
 * do next.
 */
export function TutorQuizCard({ turn, answer, earlier, latest, disabled, citations, onChoose, onNext, onExplain, onSave }: TutorQuizCardProps) {
  const id = useId();
  const [picked, setPicked] = useState<number | null>(null);
  const question = turn.question;
  if (!question) return null;
  const { number, total, choices } = question;
  const feedback = answer?.feedback ?? null;
  const outcome = feedback?.outcome ?? null;
  const answered = question.answered && answer !== null;
  const chosen = feedback?.chosen ?? null;
  const more = total === null || number < total;
  const segments = total === null ? [] : Array.from({ length: total }, (_, index) => {
    if (index < number - 1) return earlier[index] ?? "skipped";
    if (index === number - 1) return outcome ?? "current";
    return "ahead";
  });
  return (
    <article className={card} aria-label={answered ? `Question ${number}, answered` : `Question ${number}`}>
      <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2.5 tw:text-caption tw:text-muted-foreground">
        <span className="tw:inline-flex tw:h-6 tw:items-center tw:rounded-full tw:bg-primary/12 tw:px-2.5 tw:font-semibold tw:text-primary">
          Question {number}
        </span>
        {choices.length ? "Choose one" : "Short answer"}
        {segments.length ? (
          <span className="tw:ml-auto tw:flex tw:gap-1" role="img" aria-label={`Question ${number} of ${total}`}>
            {segments.map((state, index) => (
              <span key={index} className={cn("tw:h-1 tw:w-5 tw:rounded-full tw:compact:w-4", SEGMENT[state])} />
            ))}
          </span>
        ) : null}
      </div>
      <div className="tw:text-body tw:font-semibold tw:leading-relaxed">
        <TutorMarkdown text={turn.assistantText} />
      </div>
      {choices.length ? (
        <RadioGroup
          label="Answers"
          name={`${id}-choice`}
          className="tw:grid-cols-2 tw:gap-2.5 tw:compact:grid-cols-1"
          disabled={answered || disabled || !turn.pendingQuestion}
          value={String(answered ? chosen ?? "" : picked ?? "")}
          onValueChange={(value) => setPicked(Number(value))}
          options={choices.map((choice, index) => {
            const right = answered && question.correctChoice === index;
            const wrong = answered && chosen === index && !right;
            const selected = !answered && picked === index;
            return {
              value: String(index),
              ariaLabel: `${choice}${right ? ", the right answer" : ""}${answered && chosen === index ? ", your answer" : ""}`,
              className: cn(
                option,
                selected && "tw:border-primary tw:bg-primary/8",
                right && "tw:border-success/30 tw:bg-success-soft",
                wrong && "tw:border-destructive/30 tw:bg-destructive-soft",
                answered && "tw:cursor-default",
              ),
              label: (
                <span className="tw:flex tw:min-w-0 tw:items-center tw:gap-3">
                  <span className={cn(letter, selected && "tw:bg-primary tw:text-primary-foreground", right && "tw:bg-success tw:text-background", wrong && "tw:bg-destructive tw:text-background")} aria-hidden="true">
                    {String.fromCharCode(65 + index)}
                  </span>
                  <span className="tw:min-w-0 tw:break-words">{choice}</span>
                </span>
              ),
            };
          })}
        />
      ) : null}
      {answered && !choices.length ? (
        <div className="tw:rounded-xl tw:bg-muted/50 tw:px-3.5 tw:py-2.5 tw:text-small">
          <small className="tw:block tw:pb-0.5 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide tw:text-muted-foreground">Your answer</small>
          <span className="tw:whitespace-pre-wrap tw:break-words">{answer.userText}</span>
        </div>
      ) : null}
      {answered && outcome ? (
        <>
          <p className={cn(
            "tw:m-0 tw:flex tw:items-center tw:gap-2 tw:text-body tw:font-semibold",
            outcome === "correct" ? "tw:text-success" : outcome === "incorrect" ? "tw:text-destructive" : "tw:text-warning",
          )}>
            <Icon name={outcome === "correct" ? "check" : outcome === "incorrect" ? "close" : "warning"} size={20} />
            {VERDICT[outcome]}
          </p>
          {feedback && feedback.criteriaFeedback.length ? (
            <ul className="tw:m-0 tw:grid tw:list-none tw:gap-2 tw:p-0 tw:text-small tw:leading-relaxed" aria-label="What the answer needed">
              {feedback.criteriaFeedback.map((text, index) => {
                const met = feedback.criteriaMet[index];
                return (
                  <li key={index} className="tw:flex tw:min-w-0 tw:items-start tw:gap-2.5">
                    {met === undefined ? null : (
                      <Icon
                        name={met ? "check" : "close"}
                        size={18}
                        label={met ? "Met" : "Missed"}
                        className={cn("tw:mt-0.5 tw:shrink-0", met ? "tw:text-success" : "tw:text-destructive")}
                      />
                    )}
                    <div className="tw:min-w-0"><TutorMarkdown text={text} /></div>
                  </li>
                );
              })}
            </ul>
          ) : null}
          {feedback?.explanation ? <TutorMarkdown text={feedback.explanation} /> : null}
          {question.expectedAnswer && !choices.length ? (
            <div className="tw:text-small">
              <strong className="tw:font-semibold">Expected answer: </strong>
              <TutorMarkdown text={question.expectedAnswer} />
            </div>
          ) : null}
          {question.explanation ? (
            <div className="tw:text-small tw:text-muted-foreground"><TutorMarkdown text={question.explanation} /></div>
          ) : null}
          {citations}
        </>
      ) : null}
      <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:gap-2">
        {turn.pendingQuestion && choices.length ? (
          <Button variant="primary" disabled={disabled || picked === null} onClick={() => { if (picked !== null) onChoose(picked); }}>
            Check answer
          </Button>
        ) : null}
        {answered && latest && more ? (
          <Button variant="primary" disabled={disabled} onClick={onNext}>Next question</Button>
        ) : null}
        {answered && outcome !== "correct" ? (
          <Button disabled={disabled} onClick={onExplain}>Explain what I missed</Button>
        ) : null}
        {answered && question.expectedAnswer ? (
          <Button variant="ghost" onClick={onSave}>
            <Icon name="cards" size={16} /> Save as flashcard
          </Button>
        ) : null}
      </div>
    </article>
  );
}

/**
 * How the questions before `index` in its run went, first to last: the
 * nearest earlier question at each place, and its graded answer's outcome.
 */
export function earlierOutcomes(turns: readonly TutorTurn[], index: number): (Outcome | null)[] {
  const number = turns[index]?.question?.number ?? 1;
  const outcomes: (Outcome | null)[] = [];
  let at = index;
  for (let place = number - 1; place >= 1; place -= 1) {
    let found = -1;
    for (let scan = at - 1; scan >= 0; scan -= 1) {
      if (turns[scan]?.question?.number === place) { found = scan; break; }
    }
    if (found < 0) { outcomes.unshift(null); continue; }
    at = found;
    const asked = turns[found]!;
    const graded = turns.find((turn) => turn.questionId === asked.id && turn.state === "complete");
    outcomes.unshift(graded?.feedback?.outcome ?? null);
  }
  return outcomes;
}
