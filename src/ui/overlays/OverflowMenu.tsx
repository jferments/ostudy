import { useEffect, useId, useRef, useState } from "react";
import { flushSync } from "react-dom";
import type { IconName } from "../icons/Icon";
import { IconButton } from "../icons/IconButton";
import { Button } from "../primitives/Button";
import { useLayoutMode } from "../layout/LayoutModeProvider";
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator } from "../vendor/shadcn/dropdown-menu";
import { Sheet } from "./Dialog";
import { useOverlayContext, useOverlayParentId } from "./overlayContext";

export interface OverflowMenuItem { id: string; label: string; onSelect: () => void; disabled?: boolean; destructive?: boolean; }
export interface OverflowMenuProps { label: string; items: ReadonlyArray<OverflowMenuItem>; disabled?: boolean; className?: string; triggerPresentation?: "icon" | "text"; triggerIcon?: IconName; }

export function OverflowMenu({ label, items, disabled = false, className = "", triggerPresentation = "icon", triggerIcon = "overflow" }: OverflowMenuProps) {
  const [open, setOpen] = useState(false);
  const { layoutMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const id = `overflow-${useId()}`;
  const parentId = useOverlayParentId();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const { register } = useOverlayContext();
  const groups = [items.filter((item) => !item.destructive), items.filter((item) => item.destructive)];
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
    ? <IconButton ref={triggerRef} icon={single?.destructive ? "delete" : triggerIcon} label={actionLabel} tooltip={false} className={className} tone={single?.destructive ? "danger" : "neutral"} disabled={disabled || items.length === 0 || single?.disabled} />
    : <Button ref={triggerRef} variant={single?.destructive ? "dangerGhost" : "secondary"} className={className} disabled={disabled || items.length === 0 || single?.disabled}>{actionLabel}</Button>;
  if (single) return triggerPresentation === "icon"
    ? <IconButton ref={triggerRef} icon={single.destructive ? "delete" : triggerIcon} label={single.label} className={className} tone={single.destructive ? "danger" : "neutral"} disabled={disabled || single.disabled} onClick={single.onSelect} />
    : <Button variant={single.destructive ? "dangerGhost" : "secondary"} className={className} disabled={disabled || single.disabled} onClick={single.onSelect}>{single.label}</Button>;

  if (compact) return <>
    {triggerPresentation === "icon"
      ? <IconButton ref={triggerRef} icon={triggerIcon} label={label} tooltip={false} className={className} aria-haspopup="dialog" aria-expanded={open} disabled={disabled || items.length === 0} onClick={() => setOpen(true)} />
      : <Button ref={triggerRef} className={className} aria-haspopup="dialog" aria-expanded={open} disabled={disabled || items.length === 0} onClick={() => setOpen(true)}>{label}</Button>}
    {open ? <Sheet title={label} width="small" onClose={() => setOpen(false)}><div className="tw:grid tw:gap-1 tw:p-3" aria-label={label}>
      {groups.map((group, index) => <div key={index} className={index === 1 && groups[0].length > 0 && group.length > 0 ? "tw:border-0 tw:border-t tw:border-solid tw:border-border tw:pt-2" : undefined}>
        {group.map((item) => <Button key={item.id} variant={item.destructive ? "dangerGhost" : "ghost"} className="tw:w-full tw:justify-start tw:px-4 tw:py-3 tw:text-body" disabled={item.disabled} onClick={() => select(item)}>{item.label}</Button>)}
      </div>)}
    </div></Sheet> : null}
  </>;
  return <DropdownMenu open={open} onOpenChange={setOpen} modal={false}>
    <DropdownMenuTrigger render={trigger} />
    <DropdownMenuContent aria-label={label}>
      {groups[0].map((item) => <DropdownMenuItem key={item.id} disabled={item.disabled} onClick={() => select(item)}>{item.label}</DropdownMenuItem>)}
      {groups[0].length > 0 && groups[1].length > 0 ? <DropdownMenuSeparator /> : null}
      {groups[1].map((item) => <DropdownMenuItem key={item.id} destructive disabled={item.disabled} onClick={() => select(item)}>{item.label}</DropdownMenuItem>)}
    </DropdownMenuContent>
  </DropdownMenu>;
}
