import {
  type KeyboardEvent,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from "react";
import { flushSync } from "react-dom";

import type { IconName } from "../icons/Icon";
import { IconButton } from "../icons/IconButton";
import { useLayoutMode } from "../layout/LayoutModeProvider";
import { Sheet } from "./Dialog";
import { useOverlayContext, useOverlayParentId } from "./overlayContext";
import styles from "./OverflowMenu.module.css";

export interface OverflowMenuItem {
  id: string;
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  destructive?: boolean;
}

export interface OverflowMenuProps {
  label: string;
  items: ReadonlyArray<OverflowMenuItem>;
  disabled?: boolean;
  className?: string;
  triggerPresentation?: "icon" | "text";
  triggerIcon?: IconName;
}

export function OverflowMenu({
  label,
  items,
  disabled = false,
  className = "",
  triggerPresentation = "icon",
  triggerIcon = "overflow",
}: OverflowMenuProps) {
  const [open, setOpen] = useState(false);
  const [activeId, setActiveId] = useState<string | null>(null);
  const { layoutMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const reactId = useId().replaceAll(":", "");
  const id = `overflow-menu-${reactId}`;
  const parentId = useOverlayParentId();
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const { register, isTop } = useOverlayContext();
  const ordinaryItems = items.filter((item) => !item.destructive);
  const destructiveItems = items.filter((item) => item.destructive);
  const enabledItems = useMemo(
    () => [
      ...items.filter((item) => !item.destructive),
      ...items.filter((item) => item.destructive),
    ].filter((item) => !item.disabled),
    [items],
  );

  useEffect(() => {
    if (!open || compact || items.length <= 1) return;
    return register({
      id,
      parentId,
      modal: false,
      requestClose() {
        closeAndRestore();
        return true;
      },
    });
  }, [compact, id, items.length, open, parentId, register]);

  useEffect(() => {
    if (!open || items.length <= 1) return;
    const firstEnabled = enabledItems[0];
    setActiveId(firstEnabled?.id ?? null);
    queueMicrotask(() => {
      const target = firstEnabled ? findItemButton(menuRef.current, firstEnabled.id) : null;
      target?.focus();
    });
  }, [enabledItems, items.length, open]);

  useEffect(() => {
    if (!open || compact || items.length <= 1) return;
    function onPointerDown(event: PointerEvent): void {
      if (!isTop(id)) return;
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (menuRef.current?.contains(target) || triggerRef.current?.contains(target)) return;
      closeAndRestore();
    }
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [compact, id, isTop, items.length, open]);

  function closeAndRestore(): void {
    setOpen(false);
    queueMicrotask(() => triggerRef.current?.focus());
  }

  function select(item: OverflowMenuItem): void {
    if (item.disabled) return;
    flushSync(() => setOpen(false));
    triggerRef.current?.focus();
    item.onSelect();
  }

  function moveActive(nextIndex: number): void {
    const item = enabledItems[nextIndex];
    if (!item) return;
    setActiveId(item.id);
    findItemButton(menuRef.current, item.id)?.focus();
  }

  function onMenuKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    if (!compact && !isTop(id)) return;
    if (event.key === "Escape") {
      if (compact) return;
      event.preventDefault();
      closeAndRestore();
      return;
    }
    const current = enabledItems.findIndex((item) => item.id === activeId);
    if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      if (enabledItems.length === 0) return;
      event.preventDefault();
      if (event.key === "Home") moveActive(0);
      else if (event.key === "End") moveActive(enabledItems.length - 1);
      else if (event.key === "ArrowDown") {
        moveActive((current + 1 + enabledItems.length) % enabledItems.length);
      } else {
        moveActive((current - 1 + enabledItems.length) % enabledItems.length);
      }
      return;
    }
    if (event.key !== "Enter" && event.key !== " ") return;
    const item = enabledItems.find((candidate) => candidate.id === activeId);
    if (!item) return;
    event.preventDefault();
    select(item);
  }

  function renderItems(group: ReadonlyArray<OverflowMenuItem>) {
    return group.map((item) => (
      <button
        key={item.id}
        type="button"
        role="menuitem"
        tabIndex={!item.disabled && item.id === activeId ? 0 : -1}
        className={item.destructive ? styles.destructive : undefined}
        data-overflow-item-id={item.id}
        disabled={item.disabled}
        onFocus={() => {
          if (!item.disabled) setActiveId(item.id);
        }}
        onClick={() => select(item)}
      >
        {item.label}
      </button>
    ));
  }

  // A menu containing one command is needless indirection. Render that command
  // directly, preserving its label, disabled state, and destructive treatment.
  const singleItem = items.length === 1 ? items[0] : null;
  if (singleItem) {
    return triggerPresentation === "icon" ? (
      <IconButton
        ref={triggerRef}
        icon={singleItem.destructive ? "delete" : triggerIcon}
        label={singleItem.label}
        className={className}
        tone={singleItem.destructive ? "danger" : "neutral"}
        disabled={disabled || singleItem.disabled}
        onClick={singleItem.onSelect}
      />
    ) : (
      <button
        ref={triggerRef}
        type="button"
        className={`${styles.trigger} ${className}`.trim()}
        disabled={disabled || singleItem.disabled}
        onClick={singleItem.onSelect}
      >
        {singleItem.label}
      </button>
    );
  }

  const trigger = triggerPresentation === "icon" ? (
    <IconButton
      ref={triggerRef}
      icon={triggerIcon}
      label={label}
      className={className}
      aria-haspopup="menu"
      aria-expanded={open}
      disabled={disabled || items.length === 0}
      onClick={() => setOpen((current) => !current)}
    />
  ) : (
    <button
      ref={triggerRef}
      type="button"
      className={`${styles.trigger} ${className}`.trim()}
      aria-haspopup="menu"
      aria-expanded={open}
      disabled={disabled || items.length === 0}
      onClick={() => setOpen((current) => !current)}
    >
      {label}
    </button>
  );

  const menuItems = (
    <div
      ref={menuRef}
      className={compact ? styles.sheetList : styles.menu}
      role="menu"
      aria-label={label}
      onKeyDown={onMenuKeyDown}
    >
      {renderItems(ordinaryItems)}
      {ordinaryItems.length > 0 && destructiveItems.length > 0 ? (
        <div className={styles.separator} role="separator" />
      ) : null}
      {renderItems(destructiveItems)}
    </div>
  );

  if (compact) {
    return (
      <>
        {trigger}
        {open ? (
          <Sheet title={label} width="small" onClose={closeAndRestore}>
            {menuItems}
          </Sheet>
        ) : null}
      </>
    );
  }

  return (
    <div className={styles.root}>
      {trigger}
      {open ? menuItems : null}
    </div>
  );
}

function findItemButton(root: HTMLDivElement | null, id: string): HTMLButtonElement | undefined {
  return [...(root?.querySelectorAll<HTMLButtonElement>("[data-overflow-item-id]") ?? [])].find(
    (button) => button.dataset.overflowItemId === id,
  );
}
