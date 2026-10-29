import { useEffect, useId, useMemo } from "react";
import { flushSync } from "react-dom";
import { Icon } from "../icons/Icon";
import { useLayoutMode } from "../layout/LayoutModeProvider";
import { Button } from "../primitives/Button";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
} from "../vendor/shadcn/dropdown-menu";
import { Sheet } from "./Dialog";
import type { OverflowMenuItem } from "./OverflowMenu";
import { useOverlayActions, useOverlayParentId } from "./overlayContext";

/** An action, or with `items` a group of choices shown as a submenu (a section in a phone's sheet). */
export interface PointMenuItem extends Omit<OverflowMenuItem, "onSelect"> {
  onSelect?: () => void;
  items?: ReadonlyArray<OverflowMenuItem>;
}

export interface PointMenuProps {
  label: string;
  /** Where the menu opens, in client pixels, such as where a right-click or long press was; null when closed. */
  point: { x: number; y: number } | null;
  items: ReadonlyArray<PointMenuItem>;
  onClose: () => void;
}

/**
 * A menu of what can be done with something, opened where it was pressed: by a
 * right-click or a long press rather than a button. On a phone it opens as a
 * sheet, as the overflow menus do.
 */
export function PointMenu({ label, point, items, onClose }: PointMenuProps) {
  const open = point !== null;
  const { layoutMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const id = `point-menu-${useId()}`;
  const parentId = useOverlayParentId();
  const { register } = useOverlayActions();
  useEffect(() => {
    if (!open || compact) return;
    return register({ id, parentId, modal: false, requestClose() { onClose(); return true; } });
  }, [open, compact, id, parentId, register, onClose]);
  const anchor = useMemo(() => (point ? {
    getBoundingClientRect: () => DOMRect.fromRect({ x: point.x, y: point.y, width: 0, height: 0 }),
  } : undefined), [point]);

  function select(item: OverflowMenuItem | PointMenuItem) {
    if (item.disabled || !item.onSelect) return;
    // A dialog may open in the action: the menu goes first.
    flushSync(onClose);
    item.onSelect();
  }
  const standard = items.filter((item) => !item.destructive);
  const destructive = items.filter((item) => item.destructive);

  if (compact) {
    if (!open) return null;
    const row = (item: OverflowMenuItem | PointMenuItem) => (
      <Button key={item.id} variant={item.destructive ? "dangerGhost" : "ghost"} className="tw:w-full tw:justify-start tw:px-3 tw:py-3 tw:text-body"
        aria-pressed={item.checked} disabled={item.disabled} onClick={() => select(item)}>
        {item.checked !== undefined ? <span aria-hidden="true" className="tw:grid tw:size-5 tw:shrink-0 tw:place-items-center">{item.checked ? <Icon name="check" size={18} /> : null}</span> : null}
        {item.label}
      </Button>
    );
    return (
      <Sheet title={label} width="small" onClose={onClose}>
        <div className="tw:-mx-3 tw:grid tw:gap-1" aria-label={label}>
          {standard.map((item) => item.items ? (
            <section key={item.id} aria-label={item.label} className="tw:border-0 tw:border-t tw:border-solid tw:border-border tw:pt-2">
              <h3 className="tw:m-0 tw:px-3 tw:pb-1 tw:text-label tw:font-medium tw:text-muted-foreground">{item.label}</h3>
              {item.items.map(row)}
            </section>
          ) : row(item))}
          {destructive.length > 0 ? (
            <div className="tw:border-0 tw:border-t tw:border-solid tw:border-border tw:pt-2">{destructive.map(row)}</div>
          ) : null}
        </div>
      </Sheet>
    );
  }
  const entry = (item: OverflowMenuItem) => item.checked !== undefined
    ? <DropdownMenuCheckboxItem key={item.id} checked={item.checked} disabled={item.disabled} onClick={() => select(item)}>{item.label}</DropdownMenuCheckboxItem>
    : <DropdownMenuItem key={item.id} destructive={item.destructive} disabled={item.disabled} onClick={() => select(item)}>{item.label}</DropdownMenuItem>;
  return (
    <DropdownMenu open={open} onOpenChange={(next) => { if (!next) onClose(); }} modal={false}>
      <DropdownMenuContent aria-label={label} anchor={anchor}>
        {standard.map((item) => item.items ? (
          <DropdownMenuSub key={item.id}>
            <DropdownMenuSubTrigger disabled={item.disabled}>{item.label}</DropdownMenuSubTrigger>
            <DropdownMenuSubContent aria-label={item.label}>{item.items.map(entry)}</DropdownMenuSubContent>
          </DropdownMenuSub>
        ) : entry({ ...item, onSelect: item.onSelect ?? (() => {}) }))}
        {standard.length > 0 && destructive.length > 0 ? <DropdownMenuSeparator /> : null}
        {destructive.map((item) => entry({ ...item, onSelect: item.onSelect ?? (() => {}) }))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
