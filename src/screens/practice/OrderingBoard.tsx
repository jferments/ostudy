import { useRef } from "react";
import { RichTextView } from "../../editor/RichTextView";
import { extractRichDocumentText } from "../../editor/richText";
import type { Asset } from "../../ipc/assets";
import type { PublicQuestion, QuestionEvaluation } from "../../ipc/questionAttempts";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Button } from "../../ui/primitives/Button";
import { QuestionTile } from "./QuestionTile";
import { usePracticePointerDrop } from "./usePracticePointerDrop";

export function OrderingBoard({ question, orderedItemIds, assets, disabled, evaluation, onChange, onError }: {
  question: Extract<PublicQuestion, { kind: "ordering" }>; orderedItemIds: string[]; assets: Asset[];
  disabled: boolean; evaluation: QuestionEvaluation | null; onChange: (next: string[]) => void;
  onError: (message: string) => void;
}) {
  const root = useRef<HTMLOListElement>(null);
  const items = new Map(question.items.map((item) => [item.id, item]));
  const results = new Map(evaluation?.components.flatMap((part) => part.kind === "position" ? [[part.itemId, part] as const] : []) ?? []);
  function move(itemId: string, delta: number): void {
    const index = orderedItemIds.indexOf(itemId); const to = index + delta;
    if (disabled || index < 0 || to < 0 || to >= orderedItemIds.length) return;
    const next = [...orderedItemIds]; [next[index], next[to]] = [next[to], next[index]]; onChange(next);
  }
  function drop(itemId: string, targetId: string): void {
    if (disabled || !items.has(itemId) || itemId === targetId) return;
    const next = orderedItemIds.filter((id) => id !== itemId);
    const index = targetId === "order-end" ? next.length : next.indexOf(targetId);
    if (index < 0) return;
    next.splice(index, 0, itemId); onChange(next);
  }
  const pointer = usePracticePointerDrop({ disabled, onDrop: drop, scopeRef: root });
  return <div className="tw:grid tw:gap-3">
    {!evaluation ? <p className="tw:m-0 tw:text-sm tw:text-muted-foreground">Drag tiles into order, or use the arrow buttons.</p> : null}
    <ol ref={root} className="tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0" aria-label="Current item order">
      {orderedItemIds.map((id, index) => {
        const item = items.get(id); if (!item) return null;
        const result = results.get(id);
        const correct = result ? result.selectedPosition === result.expectedPosition : undefined;
        return <li key={id} data-practice-drop-target-id={id} className="tw:grid tw:grid-cols-[minmax(0,1fr)_auto] tw:items-center tw:gap-2">
          <QuestionTile id={id} label={`Move ${extractRichDocumentText(item.content) || `item ${index + 1}`}`} pointer={pointer}
            disabled={disabled} correct={correct}>
            <div className="tw:flex tw:items-start tw:gap-3"><strong>{index + 1}.</strong>
              <div className="tw:min-w-0 tw:flex-1"><RichTextView document={item.content} assets={assets} onLinkError={onError} /></div>
              {result ? <span className={correct ? "tw:text-success" : "tw:text-destructive"}><Icon name={correct ? "check" : "close"} size={20} /></span> : null}
            </div>
            {result && !correct ? <small>→ Correct position: {result.expectedPosition + 1}</small> : null}
          </QuestionTile>
          {!evaluation ? <div className="tw:grid tw:gap-1">
            <Button variant="text" size="small" aria-label="Move up" title="Move up" disabled={disabled || index === 0} onClick={() => move(id, -1)}>
              <Icon name="chevron-right" size={18} className="tw:-rotate-90" />
            </Button>
            <Button variant="text" size="small" aria-label="Move down" title="Move down" disabled={disabled || index === orderedItemIds.length - 1} onClick={() => move(id, 1)}>
              <Icon name="chevron-right" size={18} className="tw:rotate-90" />
            </Button>
          </div> : null}
        </li>;
      })}
      {!disabled ? <li data-practice-drop-target-id="order-end" className={cn("tw:min-h-5 tw:rounded-lg tw:text-center tw:text-sm tw:text-muted-foreground", pointer.draggingItemId && "tw:min-h-11 tw:border tw:border-dashed tw:border-primary tw:p-2")}>
        {pointer.draggingItemId ? "Drop last" : null}
      </li> : null}
    </ol>
  </div>;
}
