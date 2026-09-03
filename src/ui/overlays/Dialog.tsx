import {
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
  type RefObject,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
} from "react";
import { createPortal } from "react-dom";

import { IconButton } from "../icons/IconButton";
import { Button } from "../primitives/Button";
import {
  type OverlayCloseReason,
  OverlayParentContext,
  useOverlayContext,
  useOverlayParentId,
} from "./overlayContext";
import inlineStyles from "./Dialog.module.css";
import styles from "./overlays.module.css";

export interface DialogProps {
  title: string;
  description?: string;
  children: ReactNode;
  onClose: (reason: OverlayCloseReason) => void;
  width?: "small" | "medium" | "large" | "workspace";
  presentation?: "dialog" | "sheet" | "fullscreen";
  role?: "dialog" | "alertdialog";
  dismissible?: boolean;
  busy?: boolean;
  initialFocusRef?: RefObject<HTMLElement | null>;
  className?: string;
}

export interface InlineWorkspaceProps {
  title: string;
  description?: string;
  children: ReactNode;
  onClose: () => void;
  dismissible?: boolean;
  busy?: boolean;
  className?: string;
  closeLabel?: string;
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
  const parentId = useOverlayParentId();
  const dialogRef = useRef<HTMLDialogElement | null>(null);
  const openerRef = useRef<HTMLElement | null>(null);
  const propsRef = useRef({ onClose, dismissible, busy });
  propsRef.current = { onClose, dismissible, busy };
  const { register, isTop } = useOverlayContext();

  useEffect(
    () =>
      register({
        id,
        parentId,
        modal: true,
        requestClose(reason) {
          const current = propsRef.current;
          if (!current.dismissible || current.busy) return true;
          current.onClose(reason);
          return true;
        },
      }),
    [id, parentId, register],
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
    if (!isTop(id) || !dismissible || busy) return;
    onClose(reason);
  }

  function onBackdropMouseDown(event: MouseEvent<HTMLDialogElement>): void {
    if (event.target === event.currentTarget) requestClose("backdrop");
  }

  function onKeyDown(event: KeyboardEvent<HTMLDialogElement>): void {
    if (!isTop(id)) return;
    if (event.key === "Escape") {
      event.preventDefault();
      requestClose("escape");
      return;
    }
    if (event.key !== "Tab") return;
    const dialog = dialogRef.current;
    if (!dialog) return;
    const focusable = [...dialog.querySelectorAll<HTMLElement>(FOCUSABLE)];
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
      role={role}
      aria-modal="true"
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
      <section className={`${styles.card} ${styles[width]} ${className}`.trim()}>
        <header className={styles.header}>
          <div>
            <h2 id={titleId}>{title}</h2>
            {description ? <p id={descriptionId}>{description}</p> : null}
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label="Close dialog"
            disabled={!dismissible || busy}
            onClick={() => requestClose("close_button")}
          >
            ×
          </Button>
        </header>
        <OverlayParentContext.Provider value={id}>
          <div className={styles.body}>{children}</div>
        </OverlayParentContext.Provider>
      </section>
    </dialog>,
    document.body,
  );
}

/**
 * Route-owned workspace presentation for substantial content that must keep
 * the application navigation visible and usable. Unlike Dialog, this does not
 * use a portal, modal top layer, inert background, or focus trap.
 */
export function InlineWorkspace({
  title,
  description,
  children,
  onClose,
  dismissible = true,
  busy = false,
  className = "",
  closeLabel = "Close workspace",
}: InlineWorkspaceProps) {
  const reactId = useId().replaceAll(":", "");
  const titleId = `inline-workspace-${reactId}-title`;
  const descriptionId = `inline-workspace-${reactId}-description`;
  const closeRef = useRef<HTMLButtonElement | null>(null);
  const openerRef = useRef<HTMLElement | null>(null);

  useLayoutEffect(() => {
    openerRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    queueMicrotask(() => closeRef.current?.focus());
    return () => {
      const opener = openerRef.current;
      if (opener?.isConnected) queueMicrotask(() => opener.focus());
    };
  }, []);

  useEffect(() => {
    if (!dismissible || busy) return;
    function handleEscape(event: globalThis.KeyboardEvent): void {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      event.preventDefault();
      onClose();
    }
    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, [busy, dismissible, onClose]);

  return (
    <section
      className={`${inlineStyles.inlineWorkspace} ${className}`.trim()}
      role="region"
      aria-labelledby={titleId}
      aria-describedby={description ? descriptionId : undefined}
      aria-busy={busy || undefined}
      data-inline-workspace="true"
    >
      <header className={inlineStyles.inlineHeader}>
        <div className={inlineStyles.heading}>
          <h2 id={titleId}>{title}</h2>
          {description ? <p id={descriptionId}>{description}</p> : null}
        </div>
        <IconButton
          ref={closeRef}
          icon="close"
          iconSize={24}
          label={closeLabel}
          tooltip={false}
          className={inlineStyles.inlineClose}
          disabled={!dismissible || busy}
          onClick={onClose}
        />
      </header>
      <div className={inlineStyles.inlineBody}>{children}</div>
    </section>
  );
}

export function DialogActions({
  children,
  className = "",
}: {
  children: ReactNode;
  className?: string;
}) {
  return <div className={`${styles.actions} ${className}`.trim()}>{children}</div>;
}

export function Sheet(props: Omit<DialogProps, "presentation">) {
  return <Dialog {...props} presentation="sheet" />;
}
