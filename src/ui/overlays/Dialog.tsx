import { createContext, type FormEvent, type ReactNode, type RefObject, useContext, useEffect, useId, useLayoutEffect, useRef } from "react";
import { IconButton } from "../icons/IconButton";
import { cn } from "../internal/cn";
import { dialogHeaderTitleClassName, ModalView } from "../vendor/shadcn/modal";
import { useLayoutMode } from "../layout/LayoutModeProvider";
import {
  type OverlayCloseReason,
  OverlayParentContext,
  useOverlayActions,
  useOverlayParentId,
} from "./overlayContext";

export interface DialogProps {
  title: string; description?: string;
  /** Body content. The body scrolls between the header and the actions, with standard padding and spacing. */
  children?: ReactNode;
  /** The dialog's actions, pinned below the body so they stay visible while it scrolls. */
  actions?: ReactNode;
  /** Makes body and actions one form: a submit action in `actions` submits it. The page never navigates. */
  onSubmit?: (event: FormEvent<HTMLFormElement>) => void;
  /** "none" lets a workspace draw edge to edge; everything else keeps the standard padding. */
  padding?: "standard" | "none";
  onClose: (reason: OverlayCloseReason) => void;
  width?: "small" | "medium" | "large" | "workspace";
  /** On compact screens a dialog fills the screen and a sheet rises from the bottom edge. */
  presentation?: "dialog" | "sheet" | "fullscreen";
  role?: "dialog" | "alertdialog"; dismissible?: boolean; busy?: boolean;
  initialFocusRef?: RefObject<HTMLElement | null>; className?: string;
}
export interface InlineWorkspaceProps {
  title: string; description?: string; children: ReactNode; onClose: () => void;
  dismissible?: boolean; busy?: boolean; className?: string; closeLabel?: string;
  /**
   * "card" frames the workspace on the page with a close button. "page" makes it the page itself:
   * a top bar that sticks to the top of the scroll area, with a back button, the title over the
   * description and `actions`. A page draws its own gutters, so render it in a PageScaffold
   * without the inset.
   */
  presentation?: "card" | "page";
  /** Page presentation: controls at the end of the top bar. */
  actions?: ReactNode;
}
const DepthContext = createContext(0);

/** Base UI owns browser modality; oStudy owns platform Back and business dismissal. */
export function Dialog({ title, description, children, actions, onSubmit, padding = "standard", onClose, width = "medium", presentation = "dialog", role = "dialog", dismissible = true, busy = false, initialFocusRef, className }: DialogProps) {
  const id = `overlay-${useId().replaceAll(":", "")}`;
  const parentId = useOverlayParentId();
  const depth = useContext(DepthContext);
  const { register, isTop } = useOverlayActions();
  const { layoutMode } = useLayoutMode();
  const current = useRef({ onClose, dismissible, busy });
  current.current = { onClose, dismissible, busy };
  useEffect(() => register({ id, parentId, modal: true, requestClose(reason) {
    const props = current.current;
    if (props.dismissible && !props.busy) props.onClose(reason);
    return true;
  } }), [id, parentId, register]);
  const nested = (content: ReactNode) => content === undefined || content === null || content === false ? content
    : <DepthContext.Provider value={depth + 1}><OverlayParentContext.Provider value={id}>{content}</OverlayParentContext.Provider></DepthContext.Provider>;
  return <ModalView title={title} description={description} width={width} presentation={presentation} role={role}
    blocked={!dismissible || busy} busy={busy} initialFocusRef={initialFocusRef} className={className}
    compact={layoutMode === "compact"} id={id} depth={depth} padding={padding} actions={nested(actions)}
    onSubmit={onSubmit ? (event) => { event.preventDefault(); onSubmit(event); } : undefined}
    onRequestClose={(reason) => { if (isTop(id) && dismissible && !busy) onClose(reason); }}>
    {nested(children)}
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
  presentation = "card",
  actions,
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

  if (presentation === "page") return (
    <section
      className={cn("tw:flex tw:min-h-full tw:w-full tw:min-w-0 tw:flex-col", className)}
      role="region"
      aria-labelledby={titleId}
      aria-describedby={description ? descriptionId : undefined}
      aria-busy={busy || undefined}
      data-inline-workspace="true"
      data-presentation="page"
    >
      {/* The title is not a heading: the page's own h1 leads its outline. */}
      <header className="tw:sticky tw:top-0 tw:z-(--ostudy-layer-sticky) tw:flex tw:min-h-14 tw:min-w-0 tw:shrink-0 tw:items-center tw:gap-2 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:bg-background tw:py-1.5 tw:pr-page tw:pl-[calc(var(--ostudy-layout-page-gutter)-0.625rem)] tw:compact:gap-0.5 tw:compact:pr-1 tw:compact:pl-1 tw:forced-colors:border-[CanvasText] tw:forced-colors:bg-[Canvas]">
        <IconButton
          ref={closeRef}
          icon="back"
          label={closeLabel}
          tooltip={false}
          tone="neutral"
          className="tw:size-10 tw:compact:size-11 tw:forced-colors:border tw:forced-colors:border-solid tw:forced-colors:border-[ButtonText] tw:forced-colors:bg-[ButtonFace] tw:forced-colors:text-[ButtonText]"
          disabled={!dismissible || busy}
          onClick={onClose}
        />
        <div className="tw:grid tw:min-w-0 tw:flex-1">
          <p id={titleId} className="tw:m-0 tw:truncate tw:text-body tw:font-semibold tw:text-foreground">{title}</p>
          {description ? <p id={descriptionId} className="tw:m-0 tw:truncate tw:text-label tw:text-muted-foreground">{description}</p> : null}
        </div>
        {actions ? <div className="tw:flex tw:shrink-0 tw:items-center tw:gap-1">{actions}</div> : null}
      </header>
      <div className="tw:flex tw:min-w-0 tw:flex-1 tw:flex-col">{children}</div>
    </section>
  );

  return (
    <section
      className={cn("tw:flex tw:min-h-[min(78dvh,48rem)] tw:w-full tw:min-w-0 tw:flex-1 tw:flex-col tw:overflow-hidden tw:rounded-lg tw:bg-popover tw:text-popover-foreground tw:ring-1 tw:ring-border tw:compact:min-h-[calc(100dvh-var(--safe-top)-var(--safe-bottom))] tw:forced-colors:border tw:forced-colors:border-solid tw:forced-colors:border-[CanvasText] tw:forced-colors:bg-[Canvas] tw:forced-colors:text-[CanvasText]", className)}
      role="region"
      aria-labelledby={titleId}
      aria-describedby={description ? descriptionId : undefined}
      aria-busy={busy || undefined}
      data-inline-workspace="true"
    >
      <header className="tw:sticky tw:top-0 tw:z-(--ostudy-layer-sticky) tw:flex tw:min-w-0 tw:shrink-0 tw:items-start tw:justify-between tw:gap-3 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:bg-popover tw:px-5 tw:py-4 tw:compact:px-4 tw:compact:py-3 tw:forced-colors:border-[CanvasText] tw:forced-colors:bg-[Canvas]">
        <div className="tw:grid tw:min-w-0 tw:gap-1 tw:pt-1 tw:compact:pt-2">
          <h2 id={titleId} className={cn(dialogHeaderTitleClassName, "tw:wrap-anywhere")}>{title}</h2>
          {description ? <p id={descriptionId} className="tw:m-0 tw:text-small tw:text-muted-foreground">{description}</p> : null}
        </div>
        <IconButton
          ref={closeRef}
          icon="close"
          label={closeLabel}
          tooltip={false}
          tone="neutral"
          className="tw:size-9 tw:text-(--ostudy-color-text-secondary) tw:compact:size-11 tw:compact:bg-(--ostudy-color-canvas-inset) tw:forced-colors:border tw:forced-colors:border-solid tw:forced-colors:border-[ButtonText] tw:forced-colors:bg-[ButtonFace] tw:forced-colors:text-[ButtonText]"
          disabled={!dismissible || busy}
          onClick={onClose}
        />
      </header>
      <div className="tw:min-h-0 tw:min-w-0 tw:flex-1 tw:overflow-auto">{children}</div>
    </section>
  );
}

export function Sheet(props: Omit<DialogProps, "presentation">) {
  return <Dialog {...props} presentation="sheet" />;
}
