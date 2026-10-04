// shadcn/ui Base UI Nova dialog composition; upstream provenance in README.md.
import { type ReactNode, type RefObject, useRef } from "react";
import { Dialog } from "@base-ui/react/dialog";

import { Icon } from "../../icons/Icon";
import { cn } from "../../internal/cn";
import { Button } from "./button";

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
const widths = {
  small: "tw:max-w-md",
  medium: "tw:max-w-xl",
  large: "tw:max-w-4xl",
  workspace: "tw:max-w-none tw:h-full",
};

export function ModalView({
  title,
  description,
  children,
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
  const sheet = compact && presentation !== "fullscreen";
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
          style={{ zIndex: 200 + depth * 10 }}
          className="tw:fixed tw:inset-0 tw:bg-(--ostudy-color-overlay-scrim) tw:forced-colors:bg-[Canvas]"
        />
        <div
          style={{ zIndex: 201 + depth * 10 }}
          className={cn(
            "tw:pointer-events-none tw:fixed tw:inset-0 tw:flex tw:justify-center tw:p-4",
            sheet ? "tw:items-end tw:p-0" : "tw:items-center",
            presentation === "fullscreen" && "tw:p-0",
          )}
        >
          <Dialog.Popup
            ref={popupRef}
            role={role}
            aria-modal="true"
            data-overlay-id={id}
            data-presentation={presentation}
            data-width={width}
            aria-busy={busy || undefined}
            initialFocus={() => initialFocusRef?.current
              ?? popupRef.current?.querySelector<HTMLElement>("[data-autofocus='true'],[autofocus]")
              ?? (compact
                ? popupRef.current?.querySelector<HTMLElement>('[aria-label="Close dialog"]') ?? popupRef.current
                : true)}
            className={cn(
              "tw:pointer-events-auto tw:flex tw:max-h-[calc(100dvh-2rem)] tw:w-full tw:min-w-0 tw:flex-col tw:overflow-hidden tw:rounded-xl tw:bg-popover tw:font-sans tw:text-foreground tw:shadow-lg tw:ring-1 tw:ring-border tw:outline-none",
              widths[width],
              sheet && "tw:max-h-[calc(100dvh-var(--safe-top)-1rem)] tw:max-w-none tw:rounded-b-none tw:pb-[calc(var(--safe-bottom)+var(--android-ime-bottom))]",
              presentation === "fullscreen" && "tw:h-full tw:max-h-none tw:max-w-none tw:rounded-none tw:pt-(--safe-top) tw:pb-[calc(var(--safe-bottom)+var(--android-ime-bottom))] tw:ring-0",
              className,
            )}
          >
            <header className="tw:flex tw:shrink-0 tw:items-start tw:justify-between tw:gap-3 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:px-4 tw:py-3.5">
              <div className="tw:min-w-0 tw:space-y-1">
                <Dialog.Title render={<h2 />} className="tw:m-0 tw:break-words tw:text-body tw:font-medium tw:leading-snug">
                  {title}
                </Dialog.Title>
                {description ? (
                  <Dialog.Description className="tw:m-0 tw:break-words tw:text-small tw:leading-relaxed tw:text-muted-foreground">
                    {description}
                  </Dialog.Description>
                ) : null}
              </div>
              <Dialog.Close render={<Button variant="ghost" size="icon" />} aria-label="Close dialog" disabled={blocked}>
                <Icon name="close" size={18} />
              </Dialog.Close>
            </header>
            <div className="tw:min-h-0 tw:overflow-auto tw:overscroll-contain">{children}</div>
          </Dialog.Popup>
        </div>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
