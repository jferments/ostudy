import { createContext, type ReactNode, type RefObject, useContext, useEffect, useId, useLayoutEffect, useRef } from "react";
import { IconButton } from "../icons/IconButton";
import { cn } from "../internal/cn";
import { ModalView } from "../vendor/shadcn/modal";
import { useLayoutMode } from "../layout/LayoutModeProvider";
import { type OverlayCloseReason, OverlayParentContext, useOverlayContext, useOverlayParentId } from "./overlayContext";
import inlineStyles from "./Dialog.module.css";

export interface DialogProps {
  title: string; description?: string; children: ReactNode;
  onClose: (reason: OverlayCloseReason) => void;
  width?: "small" | "medium" | "large" | "workspace";
  presentation?: "dialog" | "sheet" | "fullscreen";
  role?: "dialog" | "alertdialog"; dismissible?: boolean; busy?: boolean;
  initialFocusRef?: RefObject<HTMLElement | null>; className?: string;
}
export interface InlineWorkspaceProps {
  title: string; description?: string; children: ReactNode; onClose: () => void;
  dismissible?: boolean; busy?: boolean; className?: string; closeLabel?: string;
}
const DepthContext = createContext(0);

/** Base UI owns browser modality; oStudy owns platform Back and business dismissal. */
export function Dialog({ title, description, children, onClose, width = "medium", presentation = "dialog", role = "dialog", dismissible = true, busy = false, initialFocusRef, className }: DialogProps) {
  const id = `overlay-${useId().replaceAll(":", "")}`;
  const parentId = useOverlayParentId();
  const depth = useContext(DepthContext);
  const { register, isTop } = useOverlayContext();
  const { layoutMode } = useLayoutMode();
  const current = useRef({ onClose, dismissible, busy });
  current.current = { onClose, dismissible, busy };
  useEffect(() => register({ id, parentId, modal: true, requestClose(reason) {
    const props = current.current;
    if (props.dismissible && !props.busy) props.onClose(reason);
    return true;
  } }), [id, parentId, register]);
  return <ModalView title={title} description={description} width={width} presentation={presentation} role={role}
    blocked={!dismissible || busy} busy={busy} initialFocusRef={initialFocusRef} className={className}
    compact={layoutMode === "compact"} id={id} depth={depth}
    onRequestClose={(reason) => { if (isTop(id) && dismissible && !busy) onClose(reason); }}>
    <DepthContext.Provider value={depth + 1}><OverlayParentContext.Provider value={id}>{children}</OverlayParentContext.Provider></DepthContext.Provider>
  </ModalView>;
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
  return <div className={cn("tw:flex tw:flex-wrap tw:justify-end tw:gap-3 tw:max-[760px]:flex-col-reverse tw:max-[760px]:[&>button]:w-full", className)}>{children}</div>;
}

export function Sheet(props: Omit<DialogProps, "presentation">) {
  return <Dialog {...props} presentation="sheet" />;
}
