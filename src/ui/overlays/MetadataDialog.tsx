import { type ReactNode, useState } from "react";

import { IconButton } from "../icons/IconButton";
import { useLayoutMode } from "../layout/LayoutModeProvider";
import { Dialog } from "./Dialog";
import type { OverlayCloseReason } from "./overlayContext";
import styles from "./MetadataDialog.module.css";

export interface MetadataDialogProps {
  open: boolean;
  title: string;
  description?: string;
  children: ReactNode;
  onClose: (reason: OverlayCloseReason) => void;
}

export function MetadataDialog({
  open,
  title,
  description,
  children,
  onClose,
}: MetadataDialogProps) {
  const { layoutMode } = useLayoutMode();
  if (!open) return null;
  return (
    <Dialog
      title={title}
      description={description}
      width="medium"
      presentation={layoutMode === "compact" ? "sheet" : "dialog"}
      className={styles.dialogCard}
      onClose={onClose}
    >
      <div className={styles.body}>{children}</div>
    </Dialog>
  );
}

export interface InfoButtonProps {
  label: string;
  title: string;
  description?: string;
  children: ReactNode;
  disabled?: boolean;
  className?: string;
}

export function InfoButton({
  label,
  title,
  description,
  children,
  disabled = false,
  className = "",
}: InfoButtonProps) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <IconButton
        icon="information"
        label={label}
        disabled={disabled}
        className={className}
        onClick={() => setOpen(true)}
      />
      <MetadataDialog
        open={open}
        title={title}
        description={description}
        onClose={() => setOpen(false)}
      >
        {children}
      </MetadataDialog>
    </>
  );
}

export function MetadataList({ children }: { children: ReactNode }) {
  return <dl className={styles.list}>{children}</dl>;
}

export function MetadataItem({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className={styles.item}>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}
