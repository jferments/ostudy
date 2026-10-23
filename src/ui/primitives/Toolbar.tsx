import { forwardRef, type ButtonHTMLAttributes, type HTMLAttributes, type ReactNode } from "react";
import { ToolbarRoot, ToolbarGroup as Group, ToolbarButton } from "../vendor/shadcn/toolbar";

/** Command-only roving toolbar. Keep editable/native picker fields outside it. */
export function Toolbar({ label, disabled, children, ...props }: HTMLAttributes<HTMLDivElement> & {
  label: string; disabled?: boolean; children: ReactNode;
}) {
  return <ToolbarRoot {...props} aria-label={label} disabled={disabled}>{children}</ToolbarRoot>;
}
export function ToolbarGroup({ label, children, ...props }: HTMLAttributes<HTMLDivElement> & {
  label: string; children: ReactNode;
}) {
  return <Group {...props} aria-label={label}>{children}</Group>;
}
export const ToolbarAction = forwardRef<HTMLButtonElement, ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string; pressed?: boolean;
}>(function ToolbarAction({ label, pressed, title, type = "button", ...props }, ref) {
  return <ToolbarButton {...props} ref={ref} type={type} aria-label={label} title={title ?? label} aria-pressed={pressed} />;
});
