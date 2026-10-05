import { useRef, useState } from "react";
import { RichTextView } from "../../editor/RichTextView";
import { extractRichDocumentText } from "../../editor/richText";
import type { Asset } from "../../ipc/assets";
import type { PublicQuestion, QuestionEvaluation } from "../../ipc/questionAttempts";
import type { QuestionItem } from "../../ipc/questions";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Button } from "../../ui/primitives/Button";
import { QuestionTile } from "./QuestionTile";
import { answerTile, type Placement } from "./questionPresentation";
import { usePracticePointerDrop } from "./usePracticePointerDrop";

export function GroupingBoard({ question, assignments, assets, disabled, evaluation, onChange, onError }: {
  question: Extract<PublicQuestion, { kind: "grouping" }>; assignments: Placement[]; assets: Asset[];
  disabled: boolean; evaluation: QuestionEvaluation | null; onChange: (next: Placement[]) => void;
  onError: (message: string) => void;
}) {
  const root = useRef<HTMLDivElement>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<string[]>([]);
  const assigned = new Map(assignments.map((entry) => [entry.itemId, entry.targetId]));
  const categories = new Map(question.categories.map((item) => [item.id, item]));
  const results = new Map(evaluation?.components.flatMap((part) => part.kind === "placement" ? [[part.itemId, part] as const] : []) ?? []);
  function assign(itemId: string, targetId: string): void {
    if (disabled || evaluation || !categories.has(targetId) || !question.items.some((item) => item.id === itemId)) return;
    onChange([...assignments.filter((entry) => entry.itemId !== itemId), { itemId, targetId }]);
    setSelected(null);
    setExpanded((ids) => ids.filter((id) => id !== targetId));
  }
  const pointer = usePracticePointerDrop({ disabled: disabled || evaluation !== null, onDrop: assign, scopeRef: root });
  function tile(item: QuestionItem) {
    const result = results.get(item.id);
    const correct = result ? result.selectedTargetId === result.expectedTargetId : undefined;
    const expected = result ? categories.get(result.expectedTargetId) : null;
    return <div key={item.id} className="tw:grid tw:gap-1">
      <QuestionTile id={item.id} action="Select" fallbackName={extractRichDocumentText(item.content) ? undefined : "item"}
        content={<RichTextView document={item.content} assets={assets} onLinkError={onError} />}
        pointer={pointer} disabled={disabled} locked={evaluation !== null} selected={selected === item.id} correct={correct}
        onSelect={() => setSelected((value) => value === item.id ? null : item.id)}>
        {result ? <div className={cn("tw:flex tw:items-center tw:gap-1 tw:text-small", correct ? "tw:text-success" : "tw:text-destructive")}>
          <Icon name={correct ? "check" : "close"} size={18} /> {correct ? "Correct" : result.selectedTargetId ? "Wrong group" : "Not placed"}
        </div> : null}
        {result && !correct && expected ? <div className="tw:grid tw:gap-1 tw:text-small" aria-label="Correct group">
          <span>→ Belongs in:</span><RichTextView document={expected.content} assets={assets} onLinkError={onError} />
        </div> : null}
        {result?.rationale ? <div className="tw:text-small"><RichTextView document={result.rationale} assets={assets} onLinkError={onError} /></div> : null}
      </QuestionTile>
      {!evaluation && assigned.has(item.id) ? <Button variant="text" size="small" disabled={disabled}
        onClick={() => { onChange(assignments.filter((entry) => entry.itemId !== item.id)); setSelected(null); }}>
        Return to items
      </Button> : null}
    </div>;
  }
  const loose = question.items.filter((item) => !assigned.has(item.id));
  return <div ref={root} className="tw:grid tw:gap-4">
    {!evaluation ? <p className="tw:m-0 tw:text-small tw:text-muted-foreground">Drag tiles into a group, or select a tile and its group. Open a group to review or change your placements.</p> : null}
    <div className="tw:grid tw:gap-3 tw:sm:grid-cols-2" aria-label="Grouping categories">
      {question.categories.map((category) => {
        const members = question.items.filter((item) => assigned.get(item.id) === category.id);
        const open = evaluation !== null || expanded.includes(category.id);
        return <section key={category.id} data-practice-drop-target-id={category.id}
          data-group-category="true" className={cn(answerTile, "tw:border-2 tw:border-primary/50 tw:bg-accent tw:grid tw:content-start tw:gap-3",
            pointer.overTargetId === category.id && "tw:ring-2 tw:ring-primary")}>
          <Button variant="text" disabled={disabled && !evaluation} className="tw:h-auto tw:min-h-14 tw:w-full tw:justify-start tw:whitespace-normal tw:text-left"
            aria-label={`${extractRichDocumentText(category.content) || "Group"} (${members.length} items)`}
            aria-expanded={open} onClick={() => {
              if (evaluation) return;
              if (selected) assign(selected, category.id);
              else setExpanded((ids) => ids.includes(category.id) ? ids.filter((id) => id !== category.id) : [...ids, category.id]);
            }}>
            <div className="tw:grid tw:w-full tw:gap-1 tw:text-foreground">
              <span className="tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide tw:text-primary">Group</span>
              <div className="tw:text-body-large tw:font-semibold"><RichTextView document={category.content} assets={assets} onLinkError={onError} /></div>
              <small className="tw:text-muted-foreground">{members.length} {members.length === 1 ? "item" : "items"}{!evaluation && members.length > 0 ? " · Review" : ""}</small>
            </div>
          </Button>
          {open ? <div className="tw:grid tw:gap-2">{members.map(tile)}</div> : null}
        </section>;
      })}
    </div>
    {loose.length > 0 ? <div className="tw:grid tw:gap-3 tw:sm:grid-cols-2" aria-label="Items to group">{loose.map(tile)}</div>
      : !evaluation ? <p className="tw:m-0 tw:text-center tw:text-small tw:text-muted-foreground">All items placed. Check your answer when ready.</p> : null}
  </div>;
}
