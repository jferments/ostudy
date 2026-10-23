import { useId, useState } from "react";

import { getErrorMessage } from "../../ipc/common";
import { answerLessonCheck, type LessonCheckResult, type LessonStep } from "../../ipc/lessons";
import { cn } from "../../ui/internal/cn";
import { Button } from "../../ui/primitives/Button";
import { RadioGroup } from "../../ui/primitives/FormControls";
import { InlineNotice } from "../../ui/primitives/InlineNotice";

/**
 * One step's check: choose, check, and see why. `grid` lays the answers out
 * two by two on wider screens, beside a lecture's notes.
 */
export function StepCheck({
  lessonId,
  revisionNumber,
  step,
  answered,
  onAnswered,
  layout = "list",
}: {
  lessonId: string;
  revisionNumber: number;
  step: LessonStep;
  answered: { optionId: string; correct: boolean } | null;
  onAnswered: (result: LessonCheckResult) => void;
  layout?: "list" | "grid";
}) {
  const check = step.check!;
  const [choice, setChoice] = useState(answered?.optionId ?? "");
  const [result, setResult] = useState<LessonCheckResult | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const headingId = useId();
  const outcome = result ? result.correct : answered && answered.optionId === choice ? answered.correct : null;

  const submit = () => {
    if (!choice) return;
    setChecking(true);
    setError(null);
    answerLessonCheck(lessonId, revisionNumber, step.key, choice).then(
      (value) => {
        setResult(value);
        setChecking(false);
        onAnswered(value);
      },
      (failure: unknown) => {
        setError(getErrorMessage(failure));
        setChecking(false);
      },
    );
  };

  return (
    <section
      aria-labelledby={headingId}
      data-testid="lesson-check"
      className="tw:flex tw:flex-col tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-5 tw:compact:p-4"
    >
      <h2 id={headingId} className="tw:m-0 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide tw:text-muted-foreground">
        {check.timing === "before" ? "What do you think?" : "Quick check"}
      </h2>
      <p className="tw:m-0 tw:text-body-large tw:font-medium tw:break-words">{check.prompt}</p>
      <RadioGroup
        label={check.prompt}
        value={choice}
        className={layout === "grid" ? "tw:grid tw:grid-cols-2 tw:gap-2 tw:compact:grid-cols-1" : undefined}
        options={check.options.map((option) => ({
          value: option.id,
          label: <span className="tw:break-words">{option.text}</span>,
          className: cn(
            "tw:rounded-md tw:px-3",
            layout === "grid" && "tw:border tw:border-solid tw:border-border tw:py-2",
            result && option.id === result.correctOptionId && "tw:bg-success-soft",
            result && !result.correct && option.id === choice && "tw:bg-destructive/10",
          ),
        }))}
        onValueChange={(value) => {
          setChoice(value);
          setResult(null);
        }}
      />
      {error ? <InlineNotice kind="error">The answer was not checked: {error}</InlineNotice> : null}
      {outcome === null ? (
        <div>
          <Button variant="primary" className="tw:rounded-full" disabled={!choice || checking} onClick={submit}>
            Check answer
          </Button>
        </div>
      ) : (
        <div
          role="status"
          className={cn(
            "tw:grid tw:gap-1 tw:rounded-md tw:px-4 tw:py-3",
            outcome ? "tw:bg-success-soft" : "tw:bg-warning-soft",
          )}
        >
          <strong className="tw:font-semibold">{outcome ? "Right." : "Not quite."}</strong>
          {result?.explanation ? <span>{result.explanation}</span> : null}
        </div>
      )}
    </section>
  );
}
