import type { PointerEvent } from "react";

import { Formula } from "../../../math/Formula";
import { cn } from "../../../ui/internal/cn";
import { noteLines, noteSpans, type NoteLine } from "./noteText";

/**
 * A text note's words as they are drawn: list items with their bullet, number
 * or checkbox, and formulas. With `onToggle`, a checkbox can be pressed to
 * tick or clear it.
 */
export function NoteBody({ text, onToggle }: { text: string; onToggle?: (line: number) => void }) {
  return noteLines(text).map((line, index) => (
    <div key={index} className="tw:flex tw:min-w-0 tw:items-baseline" style={{ paddingInlineStart: `${line.indent * 1.25}em` }}>
      <Marker line={line} onToggle={onToggle ? () => onToggle(index) : undefined} />
      <span className={cn("tw:min-w-0 tw:flex-1 tw:whitespace-pre-wrap tw:wrap-anywhere", line.kind === "check" && line.checked && "tw:line-through tw:opacity-60")}>
        {line.body ? noteSpans(line.body).map((span, at) => "math" in span
          ? <Formula key={at} source={span.math} block={span.block} />
          : span.text) : "​"}
      </span>
    </div>
  ));
}

function Marker({ line, onToggle }: { line: NoteLine; onToggle?: () => void }) {
  switch (line.kind) {
    case "plain":
      return null;
    case "bullet":
      return <span className="tw:w-[1.1em] tw:shrink-0">{line.indent % 2 === 1 ? "◦" : "•"}</span>;
    case "number":
      return <span className="tw:min-w-[1.6em] tw:shrink-0 tw:pe-[0.3em] tw:tabular-nums">{line.number}</span>;
    case "check": {
      const press = (event: PointerEvent) => {
        // The press ticks the box; it does not select or move the note.
        event.preventDefault();
        event.stopPropagation();
        onToggle?.();
      };
      return (
        <span
          data-note-checkbox={line.checked ? "checked" : "unchecked"}
          className={cn(
            "tw:me-[0.35em] tw:inline-flex tw:size-[0.95em] tw:shrink-0 tw:items-center tw:justify-center tw:self-center tw:rounded-[0.15em] tw:border-[0.1em] tw:border-solid tw:border-current tw:text-[0.8em] tw:leading-none",
            onToggle && "tw:pointer-events-auto tw:cursor-pointer",
          )}
          onPointerDown={onToggle ? press : undefined}
        >
          {line.checked ? "✓" : null}
        </span>
      );
    }
  }
}
