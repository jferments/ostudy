import { useId, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { RichTextView } from "../../editor/RichTextView";
import { extractRichDocumentText } from "../../editor/richText";
import type { Asset } from "../../ipc/assets";
import type { PublicQuestion, QuestionEvaluation } from "../../ipc/questionAttempts";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Button } from "../../ui/primitives/Button";
import { QuestionTile, tileKeyDown, tileKeyUp } from "./QuestionTile";
import { activePointerTile, answerTile, connectedTile, rematchMatching, matchingRows, tileContent, tileHandle, tileResult, type Placement } from "./questionPresentation";
import { usePracticePointerDrop } from "./usePracticePointerDrop";

export function MatchingBoard({ question, assignments, assets, disabled, evaluation, onChange, onError }: {
  question: Extract<PublicQuestion, { kind: "matching" }>; assignments: Placement[]; assets: Asset[];
  disabled: boolean; evaluation: QuestionEvaluation | null; onChange: (next: Placement[]) => void;
  onError: (message: string) => void;
}) {
  const root = useRef<HTMLDivElement>(null);
  const itemIds = question.items.map((item) => item.id);
  const targetIds = question.targets.map((item) => item.id);
  const seed = JSON.stringify([itemIds, targetIds]);
  const [layout, setLayout] = useState({ seed, order: itemIds });
  const [selection, setSelection] = useState<{ seed: string; itemId: string } | null>(null);
  const selected = selection?.seed === seed ? selection.itemId : null;
  const items = new Map(question.items.map((item) => [item.id, item]));
  const targets = new Map(question.targets.map((item) => [item.id, item]));
  const rows = matchingRows(layout.seed === seed ? layout.order : itemIds, targetIds, assignments);
  const connected = new Set(assignments.map((entry) => entry.itemId));
  const results = new Map(evaluation?.components.flatMap((part) => part.kind === "placement" ? [[part.itemId, part] as const] : []) ?? []);
  function assign(itemId: string, targetId: string): void {
    if (disabled || evaluation || !items.has(itemId) || !targets.has(targetId)) return;
    const next = rematchMatching(assignments, itemId, targetId);
    if (!next) return;
    // Pairing changes only these two visual rows. Rebuilding loose rows from the
    // initial shuffle rotated unrelated tiles after a drop, making targets jump.
    const order = [...rows];
    const from = order.indexOf(itemId);
    const to = targetIds.indexOf(targetId);
    const displaced = order[to];
    if (from >= 0 && to >= 0 && displaced !== undefined) {
      order[from] = displaced;
      order[to] = itemId;
    }
    setLayout({ seed, order });
    setSelection(null);
    onChange(next);
  }
  const pointer = usePracticePointerDrop({ disabled: disabled || evaluation !== null, onDrop: assign, scopeRef: root, resetKey: seed });

  return <div className="tw:grid tw:gap-3">
    {!evaluation ? <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
      Drag a left tile up or down to line up a pair, or onto its match. You can also select two tiles. Drag a connected tile again to change its match; dropping onto another pair swaps them.
    </p> : null}
    <div ref={root} data-matching-drag-active={pointer.draggingItemId !== null || undefined}
      className={cn("tw:relative tw:grid tw:gap-3", pointer.draggingItemId !== null && "tw:select-none tw:[&_*]:select-none")}
      aria-label="Matching pairs">
      {question.targets.map((target, index) => {
        const rowId = rows[index];
        if (!rowId) return null;
        const item = items.get(rowId);
        if (!item) return null;
        const linked = connected.has(item.id);
        const result = results.get(item.id);
        const correct = result ? result.selectedTargetId === result.expectedTargetId : undefined;
        const expected = result ? targets.get(result.expectedTargetId) : null;
        const over = pointer.draggingItemId !== null && pointer.overTargetId === target.id;
        return <div key={target.id} data-practice-drop-target-id={target.id}
          className="tw:grid tw:grid-cols-[minmax(0,1fr)_2.5rem_minmax(0,1fr)] tw:items-stretch tw:gap-1">
          <QuestionTile key={item.id} id={item.id} action="Select"
            fallbackName={extractRichDocumentText(item.content) ? undefined : `item ${index + 1}`}
            content={<RichTextView document={item.content} assets={assets} onLinkError={onError} />}
            dropTargetId={target.id} pointer={pointer} disabled={disabled} locked={evaluation !== null} connected={linked} selected={selected === item.id}
            correct={correct} onSelect={() => setSelection((value) => value?.seed === seed && value.itemId === item.id ? null : { seed, itemId: item.id })}>
            {linked && !evaluation ? <span className="tw:text-caption tw:font-medium tw:text-accent-foreground">Connected</span> : null}
            {result ? <div className={cn("tw:flex tw:items-center tw:gap-1 tw:text-small", correct ? "tw:text-success" : "tw:text-destructive")}>
              <Icon name={correct ? "check" : "close"} size={18} /> {correct ? "Correct" : result.selectedTargetId ? "Incorrect match" : "Not matched"}
            </div> : null}
            {result && !correct && expected ? <div className="tw:grid tw:gap-1 tw:text-small" aria-label="Correct match">
              <span>→ Correct match:</span><RichTextView document={expected.content} assets={assets} onLinkError={onError} />
            </div> : null}
            {result?.rationale ? <div className="tw:text-small"><RichTextView document={result.rationale} assets={assets} onLinkError={onError} /></div> : null}
          </QuestionTile>
          <div className="tw:z-(--ostudy-layer-raised) tw:flex tw:items-center tw:justify-center">
            {linked && !evaluation ? <Button variant="text" size="small" disabled={disabled}
              aria-label={`Unlink ${extractRichDocumentText(item.content) || `pair ${index + 1}`}`} title="Unlink pair"
              onClick={() => { setLayout({ seed, order: rows }); onChange(assignments.filter((entry) => entry.itemId !== item.id)); setSelection(null); }}>
              <Icon name="close" size={16} />
            </Button> : linked ? <span aria-label="Connected pair">↔</span> : null}
          </div>
          <MatchTarget id={target.id} fallbackName={extractRichDocumentText(target.content) ? undefined : `target ${index + 1}`}
            connected={linked} correct={correct} over={over} disabled={disabled} locked={evaluation !== null}
            onActivate={(pointerClick) => {
              if (pointerClick && pointer.shouldSuppressClick()) return;
              if (selected) assign(selected, target.id);
            }}>
            <RichTextView document={target.content} assets={assets} onLinkError={onError} />
          </MatchTarget>
        </div>;
      })}
      {evaluation ? <MatchingCorrections root={root} evaluation={evaluation} /> : null}
    </div>
  </div>;
}

/** A right-hand tile. Like a left tile it is readable content under a real button named by
 * "Match with" and what the tile says, and a checked board leaves it plain content. */
function MatchTarget({ id, fallbackName, connected, correct, over, disabled, locked, onActivate, children }: {
  id: string; fallbackName?: string; connected: boolean; correct?: boolean; over: boolean;
  disabled: boolean; locked: boolean; onActivate: (pointerClick: boolean) => void; children: ReactNode;
}) {
  const ids = useId().replaceAll(":", "");
  return <div data-match-target={id} data-practice-drop-target-id={id} data-connected={connected || undefined}
    data-drop-active={over || undefined} data-drop-disabled={disabled || locked || undefined}
    className={cn(answerTile, correct === undefined && connected ? connectedTile : tileResult(correct), "tw:relative tw:grid tw:content-center tw:gap-2",
      over && [activePointerTile, "tw:ring-2 tw:ring-primary"])}>
    {!locked ? <Button variant="ghost" className={tileHandle} disabled={disabled}
      aria-labelledby={`${ids}-action ${ids}-content`}
      onClick={() => onActivate(true)} onKeyDown={(event) => tileKeyDown(event, () => onActivate(false))} onKeyUp={tileKeyUp}>
      <span id={`${ids}-action`} className="tw:sr-only">{fallbackName ? `Match with ${fallbackName}` : "Match with"}</span>
    </Button> : null}
    <div id={`${ids}-content`} className={cn(tileContent, !locked && "tw:pointer-events-none")}>{children}</div>
  </div>;
}

/** Resize-aware correction arrows stay in the gutter; text at each wrong tile
 * provides the same information for screen readers and crowded small displays.
 */
function MatchingCorrections({ root, evaluation }: { root: RefObject<HTMLDivElement | null>; evaluation: QuestionEvaluation }) {
  const marker = useId().replaceAll(":", "");
  const [paths, setPaths] = useState<string[]>([]);
  useLayoutEffect(() => {
    const node = root.current;
    if (!node) return;
    const measure = () => {
      const origin = node.getBoundingClientRect();
      const items = [...node.querySelectorAll<HTMLElement>("[data-question-item-id]")];
      const targets = [...node.querySelectorAll<HTMLElement>("[data-match-target]")];
      const next = evaluation.components.flatMap((part) => {
        if (part.kind !== "placement" || part.selectedTargetId === part.expectedTargetId) return [];
        const left = items.find((item) => item.dataset.questionItemId === part.itemId)?.getBoundingClientRect();
        const right = targets.find((item) => item.dataset.matchTarget === part.expectedTargetId)?.getBoundingClientRect();
        if (!left?.width || !right?.width) return [];
        const x1 = left.right - origin.left; const x2 = right.left - origin.left;
        const y1 = (left.top + left.bottom) / 2 - origin.top; const y2 = (right.top + right.bottom) / 2 - origin.top;
        const middle = (x1 + x2) / 2;
        return [`M ${x1} ${y1} C ${middle} ${y1}, ${middle} ${y2}, ${x2} ${y2}`];
      });
      setPaths((previous) => JSON.stringify(previous) === JSON.stringify(next) ? previous : next);
    };
    measure();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    observer?.observe(node);
    for (const child of node.children) observer?.observe(child);
    window.addEventListener("resize", measure);
    return () => { observer?.disconnect(); window.removeEventListener("resize", measure); };
  }, [root, evaluation]);
  return <svg className="tw:pointer-events-none tw:absolute tw:inset-0 tw:z-(--ostudy-layer-raised) tw:h-full tw:w-full tw:overflow-visible"
    aria-hidden="true" focusable="false">
    <defs><marker id={marker} viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" className="tw:fill-success" />
    </marker></defs>
    {paths.map((path, index) => <path key={index} d={path} fill="none" className="tw:stroke-success" strokeWidth="2" markerEnd={`url(#${marker})`} />)}
  </svg>;
}
