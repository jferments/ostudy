import {
  type MouseEvent,
  type ReactNode,
  type RefObject,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
} from "react";
import { createPortal } from "react-dom";

import {
  type OverlayCloseReason,
  useOverlayContext,
} from "./overlayContext";
import styles from "./overlays.module.css";

export interface DialogProps {
  title: string;
  description?: string;
  children: ReactNode;
  onClose: (reason: OverlayCloseReason) => void;
  width?: "small" | "medium" | "large";
  presentation?: "dialog" | "sheet" | "fullscreen";
  role?: "dialog" | "alertdialog";
  dismissible?: boolean;
  busy?: boolean;
  initialFocusRef?: RefObject<HTMLElement | null>;
  className?: string;
}

const FOCUSABLE = [
  "button:not([disabled])",
  "a[href]",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
  "[contenteditable='true']",
].join(",");

export function Dialog({
  title,
  description,
  children,
  onClose,
  width = "medium",
  presentation = "dialog",
  role = "dialog",
  dismissible = true,
  busy = false,
  initialFocusRef,
  className = "",
}: DialogProps) {
  const reactId = useId().replaceAll(":", "");
  const id = `overlay-${reactId}`;
  const titleId = `${id}-title`;
  const descriptionId = `${id}-description`;
  const dialogRef = useRef<HTMLDialogElement | null>(null);
  const openerRef = useRef<HTMLElement | null>(null);
  const propsRef = useRef({ onClose, dismissible, busy });
  propsRef.current = { onClose, dismissible, busy };
  const overlay = useOverlayContext();
  const topmost = overlay.topId === null || overlay.topId === id;

  useEffect(
    () =>
      overlay.register({
        id,
        modal: true,
        requestClose(reason) {
          const current = propsRef.current;
          if (!current.dismissible || current.busy) return true;
          current.onClose(reason);
          return true;
        },
      }),
    [id, overlay],
  );

  useLayoutEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    openerRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (typeof dialog.showModal === "function") {
      if (!dialog.open) dialog.showModal();
    } else {
      dialog.setAttribute("open", "");
    }

    const focusTarget =
      initialFocusRef?.current ??
      dialog.querySelector<HTMLElement>("[data-autofocus='true'], [autofocus]") ??
      dialog.querySelector<HTMLElement>(FOCUSABLE) ??
      dialog;
    queueMicrotask(() => focusTarget.focus());

    return () => {
      if (dialog.open && typeof dialog.close === "function") dialog.close();
      else dialog.removeAttribute("open");
      const opener = openerRef.current;
      if (opener?.isConnected) queueMicrotask(() => opener.focus());
    };
  }, [initialFocusRef]);

  function requestClose(reason: OverlayCloseReason): void {
    if (!topmost || !dismissible || busy) return;
    onClose(reason);
  }

  function onBackdropMouseDown(event: MouseEvent<HTMLDialogElement>): void {
    if (event.target === event.currentTarget) requestClose("backdrop");
  }

  function onKeyDown(event: React.KeyboardEvent<HTMLDialogElement>): void {
    if (!topmost) return;
    if (event.key === "Escape") {
      event.preventDefault();
      requestClose("escape");
      return;
    }
    if (event.key !== "Tab") return;
    const dialog = dialogRef.current;
    if (!dialog) return;
    const focusable = [...dialog.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
      (element) => element.offsetParent !== null || element === document.activeElement,
    );
    if (focusable.length === 0) {
      event.preventDefault();
      dialog.focus();
      return;
    }
    const first = focusable[0] as HTMLElement;
    const last = focusable.at(-1) as HTMLElement;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  if (typeof document === "undefined") return null;

  return createPortal(
    <dialog
      ref={dialogRef}
      className={styles.dialog}
      data-presentation={presentation}
      data-overlay-id={id}
      aria-labelledby={titleId}
      aria-describedby={description ? descriptionId : undefined}
      aria-busy={busy || undefined}
      onCancel={(event) => {
        event.preventDefault();
        requestClose("escape");
      }}
      onKeyDown={onKeyDown}
      onMouseDown={onBackdropMouseDown}
    >
      <section
        className={`${styles.card} ${styles[width]} modal-card modal-${width} ${className}`.trim()}
        role={role}
        aria-modal="true"
      >
        <header className={`${styles.header} modal-header`}>
          <div>
            <h2 id={titleId}>{title}</h2>
            {description ? <p id={descriptionId}>{description}</p> : null}
          </div>
          <button
            type="button"
            className="icon-button"
            aria-label="Close dialog"
            disabled={!dismissible || busy}
            onClick={() => requestClose("close_button")}
          >
            ×
          </button>
        </header>
        <div className={styles.body}>{children}</div>
      </section>
    </dialog>,
    document.body,
  );
}

export function Sheet(props: Omit<DialogProps, "presentation">) {
  return <Dialog {...props} presentation="sheet" />;
}
