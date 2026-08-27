import {
  cloneElement,
  type FocusEvent,
  type KeyboardEvent,
  type PointerEvent,
  type ReactElement,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";

import styles from "./Tooltip.module.css";

export const TOOLTIP_POINTER_DELAY_MS = 360;

interface TooltipChildProps {
  "aria-describedby"?: string;
}

export interface TooltipProps {
  content: string;
  children: ReactElement<TooltipChildProps>;
}

export function Tooltip({ content, children }: TooltipProps) {
  const reactId = useId().replaceAll(":", "");
  const tooltipId = `tooltip-${reactId}`;
  const rootRef = useRef<HTMLSpanElement | null>(null);
  const pointerTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [open, setOpen] = useState(false);

  function clearPointerTimer(): void {
    if (pointerTimerRef.current === null) return;
    clearTimeout(pointerTimerRef.current);
    pointerTimerRef.current = null;
  }

  function dismiss(): void {
    clearPointerTimer();
    setOpen(false);
  }

  useEffect(
    () => () => {
      if (pointerTimerRef.current !== null) {
        clearTimeout(pointerTimerRef.current);
      }
    },
    [],
  );

  function handlePointerEnter(event: PointerEvent<HTMLSpanElement>): void {
    if (event.pointerType !== "mouse" && event.pointerType !== "pen") return;
    clearPointerTimer();
    pointerTimerRef.current = setTimeout(() => {
      pointerTimerRef.current = null;
      setOpen(true);
    }, TOOLTIP_POINTER_DELAY_MS);
  }

  function handleFocus(): void {
    clearPointerTimer();
    setOpen(true);
  }

  function handleBlur(event: FocusEvent<HTMLSpanElement>): void {
    if (
      event.relatedTarget instanceof Node &&
      rootRef.current?.contains(event.relatedTarget)
    ) {
      return;
    }
    dismiss();
  }

  function handleKeyDown(event: KeyboardEvent<HTMLSpanElement>): void {
    if (event.key !== "Escape" || !open) return;
    event.preventDefault();
    event.stopPropagation();
    dismiss();
  }

  const describedBy = [
    children.props["aria-describedby"],
    open ? tooltipId : undefined,
  ]
    .filter(Boolean)
    .join(" ") || undefined;

  return (
    <span
      ref={rootRef}
      className={styles.root}
      onPointerEnter={handlePointerEnter}
      onPointerLeave={dismiss}
      onFocus={handleFocus}
      onBlur={handleBlur}
      onKeyDown={handleKeyDown}
      onClick={dismiss}
    >
      {cloneElement(children, { "aria-describedby": describedBy })}
      {open ? (
        <span id={tooltipId} role="tooltip" className={styles.tooltip}>
          {content}
        </span>
      ) : null}
    </span>
  );
}
