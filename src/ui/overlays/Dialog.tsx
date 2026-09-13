import { createContext, type ReactNode, type RefObject, useContext, useEffect, useId, useLayoutEffect, useRef } from "react";
import { IconButton } from "../icons/IconButton";
import { cn } from "../internal/cn";
import { ModalView } from "../vendor/shadcn/modal";
import { useLayoutMode } from "../layout/LayoutModeProvider";
import { type OverlayCloseReason, OverlayParentContext, useOverlayContext, useOverlayParentId } from "./overlayContext";

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
      className={cn("tw:flex tw:min-h-[min(78dvh,48rem)] tw:w-full tw:min-w-0 tw:flex-1 tw:flex-col tw:overflow-hidden tw:rounded-xl tw:bg-popover tw:text-popover-foreground tw:ring-1 tw:ring-border tw:max-[760px]:min-h-[calc(100dvh-var(--safe-top)-var(--safe-bottom))] tw:max-[760px]:rounded-lg tw:forced-colors:border tw:forced-colors:border-solid tw:forced-colors:border-[CanvasText] tw:forced-colors:bg-[Canvas] tw:forced-colors:text-[CanvasText]", className)}
      role="region"
      aria-labelledby={titleId}
      aria-describedby={description ? descriptionId : undefined}
      aria-busy={busy || undefined}
      data-inline-workspace="true"
    >
      <header className="tw:sticky tw:top-0 tw:z-(--ostudy-layer-sticky) tw:flex tw:min-w-0 tw:shrink-0 tw:items-start tw:justify-between tw:gap-4 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:bg-popover/95 tw:p-4 tw:backdrop-blur-sm tw:forced-colors:border-[CanvasText] tw:forced-colors:bg-[Canvas]">
        <div className="tw:min-w-0">
          <h2 id={titleId} className="tw:m-0 tw:text-lg tw:font-medium tw:wrap-anywhere">{title}</h2>
          {description ? <p id={descriptionId} className="tw:mt-1 tw:mb-0 tw:text-sm tw:leading-relaxed tw:text-muted-foreground">{description}</p> : null}
        </div>
        <IconButton
          ref={closeRef}
          icon="close"
          iconSize={24}
          label={closeLabel}
          tooltip={false}
          tone="neutral"
          className="tw:size-11.5 tw:min-h-11.5 tw:min-w-11.5 tw:forced-colors:border tw:forced-colors:border-solid tw:forced-colors:border-[ButtonText] tw:forced-colors:bg-[ButtonFace] tw:forced-colors:text-[ButtonText]"
          disabled={!dismissible || busy}
          onClick={onClose}
        />
      </header>
      <div className="tw:min-h-0 tw:min-w-0 tw:flex-1 tw:overflow-auto">{children}</div>
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
  return <div className={cn("tw:flex tw:flex-wrap tw:justify-end tw:gap-2 tw:max-[760px]:flex-col-reverse tw:max-[760px]:[&>button]:w-full", className)}>{children}</div>;
}

export function Sheet(props: Omit<DialogProps, "presentation">) {
  return <Dialog {...props} presentation="sheet" />;
}
