import { type ReactNode, useState } from "react";
import { IconButton } from "../icons/IconButton";
import { Dialog } from "./Dialog";
import type { OverlayCloseReason } from "./overlayContext";

export interface MetadataDialogProps {
  open: boolean;
  title: string;
  description?: string;
  children: ReactNode;
  onClose: (reason: OverlayCloseReason) => void;
}
/** Read-only details: a sheet on compact screens, a dialog elsewhere. */
export function MetadataDialog({ open, title, description, children, onClose }: MetadataDialogProps) {
  if (!open) return null;
  return <Dialog title={title} description={description} width="medium" presentation="sheet" onClose={onClose}>
    {children}
  </Dialog>;
}

export interface InfoButtonProps {
  label: string;
  title: string;
  description?: string;
  children: ReactNode;
  disabled?: boolean;
  className?: string;
}
export function InfoButton({ label, title, description, children, disabled = false, className = "" }: InfoButtonProps) {
  const [open, setOpen] = useState(false);
  return <>
    <IconButton icon="information" label={label} disabled={disabled} className={className} onClick={() => setOpen(true)} />
    <MetadataDialog open={open} title={title} description={description} onClose={() => setOpen(false)}>{children}</MetadataDialog>
  </>;
}
export function MetadataList({ children }: { children: ReactNode }) {
  return <dl className="tw:m-0 tw:grid tw:min-w-0 tw:gap-3">{children}</dl>;
}
export function MetadataItem({ label, value }: { label: string; value: ReactNode }) {
  return <div className="tw:grid tw:min-w-0 tw:gap-2 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:pb-3 tw:sm:grid-cols-[minmax(8rem,1fr)_2fr] tw:sm:gap-4 tw:forced-colors:border-[CanvasText]">
    <dt className="tw:text-label tw:font-medium tw:text-muted-foreground">{label}</dt>
    <dd className="tw:m-0 tw:min-w-0 tw:[overflow-wrap:anywhere] tw:text-label tw:text-foreground">{value}</dd>
  </div>;
}
