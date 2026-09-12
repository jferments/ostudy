// shadcn/ui Base UI dialog composition; upstream provenance in README.md.
import { type ReactNode, type RefObject, useRef } from "react";
import { Dialog } from "@base-ui/react/dialog";
import { cn } from "../../internal/cn";
import { Button } from "./button";
import { Icon } from "../../icons/Icon";

export interface ModalViewProps {
  title: string;
  description?: string;
  children: ReactNode;
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
const widths = { small: "tw:max-w-lg", medium: "tw:max-w-2xl", large: "tw:max-w-5xl", workspace: "tw:max-w-none tw:h-full" };

export function ModalView({ title, description, children, onRequestClose, blocked, initialFocusRef, width, presentation, compact, role, id, depth, busy, className }: ModalViewProps) {
  const popupRef = useRef<HTMLDivElement>(null);
  const sheet = compact && presentation !== "fullscreen";
  return <Dialog.Root open modal disablePointerDismissal={blocked || role === "alertdialog"}
    onOpenChange={(open, details) => {
      if (open || blocked) { details.cancel(); return; }
      onRequestClose(details.reason === "escape-key" ? "escape" : details.reason === "close-press" ? "close_button" : "backdrop");
    }}>
    <Dialog.Portal>
      <Dialog.Backdrop data-slot="dialog-overlay" style={{ zIndex: 200 + depth * 10 }}
        className="tw:fixed tw:inset-0 tw:bg-(--ostudy-color-overlay-scrim) tw:backdrop-blur-sm tw:simple-material:backdrop-blur-none tw:forced-colors:bg-[Canvas]" />
      <div style={{ zIndex: 201 + depth * 10 }} className={cn("tw:pointer-events-none tw:fixed tw:inset-0 tw:flex tw:justify-center tw:p-6", sheet ? "tw:items-end tw:p-0" : "tw:items-center", presentation === "fullscreen" && "tw:p-0")}>
        <Dialog.Popup ref={popupRef} role={role} data-overlay-id={id} data-presentation={presentation} data-width={width}
          aria-busy={busy || undefined}
          initialFocus={() => initialFocusRef?.current ?? popupRef.current?.querySelector<HTMLElement>("[data-autofocus='true'],[autofocus]") ?? (compact ? popupRef.current : true)}
          className={cn("tw:pointer-events-auto tw:flex tw:max-h-[calc(100dvh-3rem)] tw:w-full tw:min-w-0 tw:flex-col tw:overflow-hidden tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-popover tw:font-sans tw:text-foreground tw:shadow-overlay tw:outline-none", widths[width], sheet && "tw:max-h-[calc(100dvh-var(--safe-top)-1rem)] tw:max-w-none tw:rounded-b-none tw:pb-[calc(var(--safe-bottom)+var(--android-ime-bottom))]", presentation === "fullscreen" && "tw:h-full tw:max-h-none tw:max-w-none tw:rounded-none tw:pt-(--safe-top) tw:pb-[calc(var(--safe-bottom)+var(--android-ime-bottom))]", className)}>
          <header className="tw:flex tw:shrink-0 tw:items-start tw:justify-between tw:gap-4 tw:border-b tw:border-solid tw:border-border tw:px-6 tw:py-5">
            <div className="tw:min-w-0 tw:space-y-1.5">
              <Dialog.Title render={<h2 />} className="tw:m-0 tw:text-xl tw:font-semibold tw:tracking-tight">{title}</Dialog.Title>
              {description ? <Dialog.Description className="tw:m-0 tw:text-sm tw:leading-relaxed tw:text-muted-foreground">{description}</Dialog.Description> : null}
            </div>
            <Dialog.Close render={<Button variant="ghost" size="icon" />} aria-label="Close dialog" disabled={blocked}>
              <Icon name="close" size={20} />
            </Dialog.Close>
          </header>
          <div className="tw:min-h-0 tw:overflow-auto tw:overscroll-contain">{children}</div>
        </Dialog.Popup>
      </div>
    </Dialog.Portal>
  </Dialog.Root>;
}
