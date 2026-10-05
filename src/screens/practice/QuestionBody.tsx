import { RichTextView } from "../../editor/RichTextView";
import type { Asset } from "../../ipc/assets";
import type { PublicQuestion, QuestionEvaluation, QuestionResponse } from "../../ipc/questionAttempts";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Surface } from "../../ui/primitives/Surface";
import { AnswerChoices } from "./AnswerChoices";
import { GroupingBoard } from "./GroupingBoard";
import { MatchingBoard } from "./MatchingBoard";
import { OrderingBoard } from "./OrderingBoard";

export interface QuestionBodyProps {
  question: PublicQuestion;
  response: QuestionResponse;
  evaluation: QuestionEvaluation | null;
  assets: Asset[];
  disabled: boolean;
  onChange: (response: QuestionResponse) => void;
  onError: (message: string) => void;
}
const booleans = [true, false].map((value) => ({
  id: String(value), content: { type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text: value ? "True" : "False" }] }] },
}));

/** No session, persistence or scheduler here. Author previews and Practice use
 * the same tiles and native evaluation projection, including component feedback.
 */
export function QuestionBody({ question, response, evaluation, assets, disabled, onChange, onError }: QuestionBodyProps) {
  const checked = evaluation !== null;
  const locked = disabled || checked;
  const choiceFeedback = evaluation?.components.filter((part) => part.kind === "choice") ?? [];
  const booleanFeedback = evaluation?.components.find((part) => part.kind === "boolean");
  const shared = { assets, disabled: locked, evaluation, onError };
  return <Surface data-question-result={evaluation?.verdict ?? "answering"}
    className={cn("tw:grid tw:min-w-0 tw:gap-4 tw:border tw:border-solid tw:p-4 tw:sm:p-5",
      !evaluation ? "tw:border-border" : evaluation.verdict === "correct"
        ? "tw:border-success/50 tw:bg-success-soft" : "tw:border-destructive/50 tw:bg-destructive-soft")}
    onKeyDown={(event) => {
      if (question.kind !== "true_false" || locked || event.defaultPrevented || event.repeat
        || event.altKey || event.ctrlKey || event.metaKey || event.nativeEvent.isComposing) return;
      if (event.target instanceof Element && event.target.closest("input,textarea,select,a,button,audio,video,[contenteditable=true]")) return;
      const key = event.key.toLowerCase();
      if (key === "t" || key === "f") { event.preventDefault(); onChange({ kind: "true_false", selected: key === "t" }); }
    }}>
    {evaluation ? <div role="status" className={cn("tw:flex tw:items-center tw:gap-2 tw:font-semibold",
      evaluation.verdict === "correct" ? "tw:text-success" : "tw:text-destructive")}>
      <Icon name={evaluation.verdict === "correct" ? "check" : "close"} />
      {evaluation.verdict === "correct" ? "Correct" : evaluation.verdict === "incomplete" ? "Incomplete answer" : "Incorrect"}
    </div> : null}
    <RichTextView document={question.kind === "true_false" ? question.statement : question.prompt} assets={assets} onLinkError={onError} />
    {question.kind === "single_choice" ? <AnswerChoices options={question.options} assets={assets} multiple={false}
      selectedIds={response.kind === "single_choice" && response.selectedOptionId ? [response.selectedOptionId] : []}
      feedback={choiceFeedback} disabled={locked} onError={onError}
      onChange={(ids) => onChange({ kind: "single_choice", selectedOptionId: ids[0] ?? null })} /> : null}
    {question.kind === "select_all" ? <>
      {!checked ? <small className="tw:text-muted-foreground">Select all that apply.</small> : null}
      <AnswerChoices options={question.options} assets={assets} multiple selectedIds={response.kind === "select_all" ? response.selectedOptionIds : []}
        feedback={choiceFeedback} disabled={locked} onError={onError}
        onChange={(selectedOptionIds) => onChange({ kind: "select_all", selectedOptionIds })} />
    </> : null}
    {question.kind === "true_false" ? <>
      <AnswerChoices options={booleans} assets={assets} multiple={false} label="True or false"
        selectedIds={response.kind === "true_false" && response.selected !== null ? [String(response.selected)] : []}
        disabled={locked} onError={onError} feedback={booleanFeedback ? booleans.map((option) => ({
          kind: "choice", itemId: option.id, selected: String(booleanFeedback.selected) === option.id,
          expected: String(booleanFeedback.expected) === option.id, rationale: null,
        })) : []} onChange={(ids) => onChange({ kind: "true_false", selected: ids[0] === "true" })} />
      {!checked ? <div className="tw:flex tw:gap-5 tw:text-small tw:text-muted-foreground" aria-label="Keyboard shortcuts">
        <span><Keycap>T</Keycap>: True</span><span><Keycap>F</Keycap>: False</span>
      </div> : null}
      {booleanFeedback?.correctedStatement ? <RichTextView document={booleanFeedback.correctedStatement} assets={assets} onLinkError={onError} /> : null}
    </> : null}
    {question.kind === "matching" ? <MatchingBoard {...shared} question={question}
      assignments={response.kind === "matching" ? response.assignments : []}
      onChange={(assignments) => onChange({ kind: "matching", assignments })} /> : null}
    {question.kind === "grouping" ? <GroupingBoard {...shared} question={question}
      assignments={response.kind === "grouping" ? response.assignments : []}
      onChange={(assignments) => onChange({ kind: "grouping", assignments })} /> : null}
    {question.kind === "ordering" ? <OrderingBoard {...shared} question={question}
      orderedItemIds={response.kind === "ordering" ? response.orderedItemIds : question.items.map((item) => item.id)}
      onChange={(orderedItemIds) => onChange({ kind: "ordering", orderedItemIds })} /> : null}
    {evaluation?.explanation ? <section className="tw:grid tw:gap-2" aria-label="Explanation">
      <RichTextView document={evaluation.explanation} assets={assets} onLinkError={onError} />
    </section> : null}
  </Surface>;
}

function Keycap({ children }: { children: string }) {
  return <kbd className="tw:inline-flex tw:min-h-6 tw:min-w-6 tw:items-center tw:justify-center tw:rounded tw:border tw:border-solid tw:border-border tw:bg-background tw:px-1 tw:font-mono tw:text-caption tw:shadow-sm">{children}</kbd>;
}
