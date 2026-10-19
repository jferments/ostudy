import { useEffect, useId, useRef, useState } from "react";
import { flushSync } from "react-dom";
import type { IconName } from "../icons/Icon";
import { IconButton } from "../icons/IconButton";
import { cn } from "../internal/cn";
import { Button, type ButtonProps } from "../primitives/Button";
import { useLayoutMode } from "../layout/LayoutModeProvider";
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator } from "../vendor/shadcn/dropdown-menu";
import { Sheet } from "./Dialog";
import { useOverlayActions, useOverlayParentId } from "./overlayContext";

export interface OverflowMenuItem { id: string; label: string; onSelect: () => void; disabled?: boolean; destructive?: boolean; }
export interface OverflowMenuProps { label: string; items: ReadonlyArray<OverflowMenuItem>; disabled?: boolean; className?: string; triggerPresentation?: "icon" | "text"; triggerIcon?: IconName; triggerVariant?: ButtonProps["variant"]; }

export function OverflowMenu({ label, items, disabled = false, className = "", triggerPresentation = "icon", triggerIcon = "overflow", triggerVariant = "secondary" }: OverflowMenuProps) {
  const [open, setOpen] = useState(false);
  const { layoutMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const id = `overflow-${useId()}`;
  const parentId = useOverlayParentId();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const { register } = useOverlayActions();
  const standardItems = items.filter((item) => !item.destructive);
  const destructiveItems = items.filter((item) => item.destructive);
  const groups = [standardItems, destructiveItems];
  const iconTriggerClassName = cn(
    className,
    triggerIcon === "timer" && "tw:[&_svg]:translate-y-0.5",
  );
  useEffect(() => {
    if (!open || compact || items.length <= 1) return;
    return register({ id, parentId, modal: false, requestClose() { setOpen(false); return true; } });
  }, [open, compact, items.length, id, parentId, register]);

  function select(item: OverflowMenuItem) {
    if (item.disabled || disabled) return;
    // A new dialog may open in the action. Remove this popup before it mounts.
    flushSync(() => setOpen(false));
    triggerRef.current?.focus();
    item.onSelect();
  }
  const single = items.length === 1 ? items[0] : undefined;
  const actionLabel = single?.label ?? label;
  const trigger = triggerPresentation === "icon"
    ? <IconButton ref={triggerRef} icon={single?.destructive ? "delete" : triggerIcon} label={actionLabel} tooltip={false} className={iconTriggerClassName} tone={single?.destructive ? "danger" : "neutral"} disabled={disabled || items.length === 0 || single?.disabled} />
    : <Button ref={triggerRef} variant={single?.destructive ? "dangerGhost" : triggerVariant} className={className} disabled={disabled || items.length === 0 || single?.disabled}>{actionLabel}</Button>;
  if (single) return triggerPresentation === "icon"
    ? <IconButton ref={triggerRef} icon={single.destructive ? "delete" : triggerIcon} label={single.label} className={iconTriggerClassName} tone={single.destructive ? "danger" : "neutral"} disabled={disabled || single.disabled} onClick={single.onSelect} />
    : <Button variant={single.destructive ? "dangerGhost" : "secondary"} className={className} disabled={disabled || single.disabled} onClick={single.onSelect}>{single.label}</Button>;

  if (compact) return <>
    {triggerPresentation === "icon"
      ? <IconButton ref={triggerRef} icon={triggerIcon} label={label} tooltip={false} className={iconTriggerClassName} aria-haspopup="dialog" aria-expanded={open} disabled={disabled || items.length === 0} onClick={() => setOpen(true)} />
      : <Button ref={triggerRef} variant={triggerVariant} className={className} aria-haspopup="dialog" aria-expanded={open} disabled={disabled || items.length === 0} onClick={() => setOpen(true)}>{label}</Button>}
    {open ? <Sheet title={label} width="small" onClose={() => setOpen(false)}><div className="tw:-mx-3 tw:grid tw:gap-1" aria-label={label}>
      {groups.map((group, index) => <div key={index} className={index === 1 && standardItems.length > 0 && group.length > 0 ? "tw:border-0 tw:border-t tw:border-solid tw:border-border tw:pt-2" : undefined}>
        {group.map((item) => <Button key={item.id} variant={item.destructive ? "dangerGhost" : "ghost"} className="tw:w-full tw:justify-start tw:px-3 tw:py-3 tw:text-body" disabled={item.disabled} onClick={() => select(item)}>{item.label}</Button>)}
      </div>)}
    </div></Sheet> : null}
  </>;
  return <DropdownMenu open={open} onOpenChange={setOpen} modal={false}>
    <DropdownMenuTrigger render={trigger} />
    <DropdownMenuContent aria-label={label}>
      {standardItems.map((item) => <DropdownMenuItem key={item.id} disabled={item.disabled} onClick={() => select(item)}>{item.label}</DropdownMenuItem>)}
      {standardItems.length > 0 && destructiveItems.length > 0 ? <DropdownMenuSeparator /> : null}
      {destructiveItems.map((item) => <DropdownMenuItem key={item.id} destructive disabled={item.disabled} onClick={() => select(item)}>{item.label}</DropdownMenuItem>)}
    </DropdownMenuContent>
  </DropdownMenu>;
}
