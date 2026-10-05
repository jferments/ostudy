// shadcn/ui Base UI Nova dialog composition; upstream provenance in README.md.
import { type FormEvent, type ReactNode, type RefObject, useCallback, useLayoutEffect, useRef, useState } from "react";
import { Dialog } from "@base-ui/react/dialog";

import { Icon } from "../../icons/Icon";
import { cn } from "../../internal/cn";
import { Button } from "./button";

export type ModalLayout = "dialog" | "sheet" | "fullscreen";

export interface ModalViewProps {
  title: string;
  description?: string;
  children?: ReactNode;
  /** Pinned below the scrolling body, so the actions stay visible while content scrolls. */
  actions?: ReactNode;
  /** Makes the body and actions one form; a submit action in the footer submits it. */
  onSubmit?: (event: FormEvent<HTMLFormElement>) => void;
  padding: "standard" | "none";
  onRequestClose: (reason: "escape" | "backdrop" | "close_button") => void;
  blocked: boolean;
  initialFocusRef?: RefObject<HTMLElement | null>;
  width: "small" | "medium" | "large" | "workspace";
  presentation: "dialog" | "sheet" | "fullscreen";
  compact: boolean;
  role: "dialog" | "alertdialog";
  id: string;
  depth: number;
  busy: boolean;
  className?: string;
}

/**
 * On compact screens a dialog takes the whole screen, while sheets and
 * confirmations rise from the bottom edge at their content height. Wider
 * screens center both.
 */
export function resolveModalLayout(
  presentation: ModalViewProps["presentation"],
  role: ModalViewProps["role"],
  compact: boolean,
): ModalLayout {
  if (presentation === "fullscreen") return "fullscreen";
  if (!compact) return "dialog";
  return presentation === "sheet" || role === "alertdialog" ? "sheet" : "fullscreen";
}

/** Nested dialogs stack inside the dialog layer band: each depth adds a backdrop and a popup step. */
function dialogLayer(depth: number, offset = 0): string {
  return `calc(var(--ostudy-layer-dialog) + ${Math.max(0, depth) * 2 + offset})`;
}

const widths = {
  small: "tw:max-w-md",
  medium: "tw:max-w-xl",
  large: "tw:max-w-4xl",
  workspace: "tw:max-w-none tw:h-full",
};

const layouts: Record<ModalLayout, string> = {
  dialog: "tw:max-h-[calc(100dvh-2rem)] tw:rounded-lg tw:shadow-lg",
  sheet: "tw:max-h-[calc(100dvh-var(--safe-top)-1rem)] tw:max-w-none tw:rounded-t-lg tw:rounded-b-none tw:shadow-xl tw:pr-(--safe-right) tw:pb-[calc(var(--safe-bottom)+var(--android-ime-bottom))] tw:pl-(--safe-left)",
  fullscreen: "tw:h-full tw:max-h-none tw:max-w-none tw:rounded-none tw:pt-(--safe-top) tw:pr-(--safe-right) tw:pb-[calc(var(--safe-bottom)+var(--android-ime-bottom))] tw:pl-(--safe-left) tw:ring-0",
};

/** Whether content is hidden above or below the scrolling body, which draws the header and footer rules. */
function useScrollEdges() {
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const [edges, setEdges] = useState({ above: false, below: false });
  const measure = useCallback(() => {
    const body = bodyRef.current;
    if (!body) return;
    const above = body.scrollTop > 0;
    const below = body.scrollTop + body.clientHeight < body.scrollHeight - 1;
    setEdges((current) => (current.above === above && current.below === below ? current : { above, below }));
  }, []);
  useLayoutEffect(() => {
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    if (bodyRef.current) observer.observe(bodyRef.current);
    if (contentRef.current) observer.observe(contentRef.current);
    return () => observer.disconnect();
  }, [measure]);
  return { bodyRef, contentRef, edges, measure };
}

export function ModalView({
  title,
  description,
  children,
  actions,
  onSubmit,
  padding,
  onRequestClose,
  blocked,
  initialFocusRef,
  width,
  presentation,
  compact,
  role,
  id,
  depth,
  busy,
  className,
}: ModalViewProps) {
  const popupRef = useRef<HTMLDivElement>(null);
  const layout = resolveModalLayout(presentation, role, compact);
  const { bodyRef, contentRef, edges, measure } = useScrollEdges();
  const hasBody = children !== undefined && children !== null && children !== false;
  const body = hasBody ? (
    <div
      ref={bodyRef}
      data-slot="dialog-body"
      onScroll={measure}
      className="tw:min-h-0 tw:flex-1 tw:overflow-auto tw:overscroll-contain"
    >
      <div
        ref={contentRef}
        data-slot="dialog-content"
        className={cn(
          "tw:grid tw:min-w-0 tw:grid-cols-[minmax(0,1fr)] tw:content-start tw:gap-4",
          padding === "standard" && "tw:px-5 tw:pt-1 tw:compact:px-4",
          padding === "standard" && (actions ? "tw:pb-3" : "tw:pb-5 tw:compact:pb-4"),
          width === "workspace" && "tw:min-h-full",
        )}
      >
        {children}
      </div>
    </div>
  ) : null;
  const footer = actions ? (
    <div
      data-slot="dialog-actions"
      data-overflow={edges.below || undefined}
      className="tw:flex tw:shrink-0 tw:flex-wrap tw:items-center tw:justify-end tw:gap-2 tw:border-0 tw:border-t tw:border-solid tw:border-transparent tw:px-5 tw:pt-3 tw:pb-5 tw:data-overflow:border-border tw:compact:flex-col-reverse tw:compact:items-stretch tw:compact:px-4 tw:compact:pb-4 tw:compact:[&>button]:min-h-touch tw:compact:[&>button]:w-full"
    >
      {actions}
    </div>
  ) : null;
  return (
    <Dialog.Root
      open
      modal
      disablePointerDismissal={blocked || role === "alertdialog"}
      onOpenChange={(open, details) => {
        if (open || blocked) {
          details.cancel();
          return;
        }
        onRequestClose(
          details.reason === "escape-key"
            ? "escape"
            : details.reason === "close-press"
              ? "close_button"
              : "backdrop",
        );
      }}
    >
      <Dialog.Portal>
        <Dialog.Backdrop
          data-slot="dialog-overlay"
          style={{ zIndex: dialogLayer(depth) }}
          className="tw:fixed tw:inset-0 tw:bg-(--ostudy-color-overlay-scrim) tw:forced-colors:bg-[Canvas]"
        />
        <div
          style={{ zIndex: dialogLayer(depth, 1) }}
          className={cn(
            "tw:pointer-events-none tw:fixed tw:inset-0 tw:flex tw:justify-center",
            layout === "dialog" && "tw:items-center tw:p-4",
            layout === "sheet" && "tw:items-end",
          )}
        >
          <Dialog.Popup
            ref={popupRef}
            role={role}
            aria-modal="true"
            data-overlay-id={id}
            data-presentation={layout}
            data-width={width}
            aria-busy={busy || undefined}
            initialFocus={() => initialFocusRef?.current
              ?? popupRef.current?.querySelector<HTMLElement>("[data-autofocus='true'],[autofocus]")
              ?? (compact
                ? popupRef.current?.querySelector<HTMLElement>('[aria-label="Close dialog"]') ?? popupRef.current
                : true)}
            className={cn(
              "tw:pointer-events-auto tw:flex tw:w-full tw:min-w-0 tw:flex-col tw:overflow-hidden tw:bg-popover tw:font-sans tw:text-popover-foreground tw:ring-1 tw:ring-border tw:outline-none tw:forced-colors:border tw:forced-colors:border-solid tw:forced-colors:border-[CanvasText]",
              widths[width],
              layouts[layout],
              className,
            )}
          >
            {layout === "sheet" ? (
              <div aria-hidden="true" className="tw:flex tw:shrink-0 tw:justify-center tw:pt-2">
                <span className="tw:h-1.25 tw:w-10 tw:rounded-full tw:bg-border-strong" />
              </div>
            ) : null}
            <header
              data-slot="dialog-header"
              data-overflow={edges.above || undefined}
              className={cn(
                "tw:flex tw:shrink-0 tw:items-start tw:gap-3 tw:border-0 tw:border-b tw:border-solid tw:border-transparent tw:px-5 tw:pt-5 tw:pb-3 tw:data-overflow:border-border tw:compact:px-4 tw:compact:pt-4",
                layout === "sheet" && "tw:pt-3 tw:compact:pt-3",
              )}
            >
              <div className="tw:grid tw:min-w-0 tw:flex-1 tw:gap-1 tw:pt-1 tw:compact:pt-2">
                <Dialog.Title render={<h2 />} className="tw:m-0 tw:break-words tw:text-title-small tw:compact:text-body-large">
                  {title}
                </Dialog.Title>
                {description ? (
                  <Dialog.Description className="tw:m-0 tw:break-words tw:text-small tw:text-muted-foreground">
                    {description}
                  </Dialog.Description>
                ) : null}
              </div>
              <Dialog.Close
                render={(
                  <Button
                    variant="ghost"
                    size="icon-lg"
                    className="tw:text-(--ostudy-color-text-secondary) tw:compact:size-11 tw:compact:bg-(--ostudy-color-canvas-inset)"
                  />
                )}
                aria-label="Close dialog"
                disabled={blocked}
              >
                <Icon name="close" size={18} />
              </Dialog.Close>
            </header>
            {onSubmit ? (
              <form onSubmit={onSubmit} className="tw:flex tw:min-h-0 tw:flex-1 tw:flex-col">
                {body}
                {footer}
              </form>
            ) : (
              <>
                {body}
                {footer}
              </>
            )}
          </Dialog.Popup>
        </div>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

/** Shared header recipe for the nonmodal InlineWorkspace, so both read as one family. */
export const dialogHeaderTitleClassName = "tw:m-0 tw:break-words tw:text-title-small tw:compact:text-body-large";
