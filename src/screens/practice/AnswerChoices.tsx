import { RichTextView } from "../../editor/RichTextView";
import { extractRichDocumentText } from "../../editor/richText";
import type { Asset } from "../../ipc/assets";
import type { ComponentFeedback } from "../../ipc/questionAttempts";
import type { QuestionItem } from "../../ipc/questions";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Checkbox, RadioGroup } from "../../ui/primitives/FormControls";
import { answerTile, tileResult } from "./questionPresentation";

type ChoiceFeedback = Extract<ComponentFeedback, { kind: "choice" }>;

/** Shared by the answer-key authoring preview and Practice. Colors describe the
 * authored options; selection text independently describes the learner's answer.
 */
export function AnswerChoices({ options, selectedIds, multiple, assets, disabled, onChange, onError, feedback = [], label = "Answer choices" }: {
  options: readonly QuestionItem[]; selectedIds: readonly string[]; multiple: boolean; assets: Asset[];
  disabled: boolean; onChange: (ids: string[]) => void; onError: (message: string) => void;
  feedback?: readonly ChoiceFeedback[]; label?: string;
}) {
  const results = new Map(feedback.map((part) => [part.itemId, part]));
  const selected = new Set(selectedIds);
  function content(option: QuestionItem) {
    const result = results.get(option.id);
    return <div className="tw:grid tw:min-w-0 tw:gap-2" data-choice-id={option.id}
      data-correct={result === undefined ? undefined : String(result.expected)}>
      <RichTextView document={option.content} assets={assets} onLinkError={onError} />
      {result ? <>
        <div className={cn("tw:flex tw:items-center tw:gap-2 tw:text-sm tw:font-medium", result.expected ? "tw:text-success" : "tw:text-destructive")}>
          <Icon name={result.expected ? "check" : "close"} size={18} />
          <span>{result.expected ? "Correct answer" : "Incorrect answer"}</span>
          <span className="tw:text-muted-foreground">{result.selected ? "· Selected" : result.expected ? "· Missed" : "· Not selected"}</span>
        </div>
        {result.rationale ? <div className="tw:text-sm"><RichTextView document={result.rationale} assets={assets} onLinkError={onError} /></div> : null}
      </> : null}
    </div>;
  }
  if (!multiple) return <RadioGroup label={label} value={selectedIds[0] ?? ""} disabled={disabled}
    presentation="cards" className="tw:grid-cols-1 tw:gap-3 tw:sm:grid-cols-2 tw:lg:grid-cols-2"
    options={options.map((option, index) => ({ value: option.id, label: content(option),
      ariaLabel: extractRichDocumentText(option.content).trim() || `Option ${index + 1}`,
      className: results.has(option.id) ? tileResult(results.get(option.id)!.expected) : undefined,
    }))} onValueChange={(id) => onChange([id])} />;
  return <div className="tw:grid tw:gap-3" role="group" aria-label={label}>
    {options.map((option, index) => <label key={option.id}
      className={cn(answerTile, tileResult(results.get(option.id)?.expected), "tw:flex tw:min-h-14 tw:items-start tw:gap-3",
        !results.has(option.id) && selected.has(option.id) && "tw:border-primary tw:bg-accent")}>
      <Checkbox aria-label={extractRichDocumentText(option.content).trim() || `Option ${index + 1}`}
        checked={selected.has(option.id)} disabled={disabled} onCheckedChange={(checked) => {
          const next = new Set(selected);
          if (checked) next.add(option.id); else next.delete(option.id);
          onChange([...next]);
        }} />
      {content(option)}
    </label>)}
  </div>;
}
