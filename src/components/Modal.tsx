import type { ReactNode } from "react";

import { Dialog, type DialogProps } from "../ui/overlays/Dialog";

interface ModalProps {
  title: string;
  description?: string;
  children: ReactNode;
  onClose: () => void;
  width?: "small" | "medium" | "large";
  busy?: boolean;
  dismissible?: boolean;
}

/**
 * Backwards-compatible feature wrapper around the Stage 3.13 dialog lifecycle.
 * New UI infrastructure should import Dialog directly; existing feature dialogs
 * inherit focus, stacking, Back, inertness, and responsive presentation here.
 */
export function Modal({
  title,
  description,
  children,
  onClose,
  width = "medium",
  busy = false,
  dismissible = true,
}: ModalProps) {
  return (
    <Dialog
      title={title}
      description={description}
      width={width}
      busy={busy}
      dismissible={dismissible}
      onClose={() => onClose()}
    >
      {children}
    </Dialog>
  );
}

export type { DialogProps };
