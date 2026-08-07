import {
  type KeyboardEvent,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";

import { useMediaQuery } from "../hooks/useMediaQuery";
import { Sheet } from "./Dialog";
import {
  useOverlayContext,
  useOverlayParentId,
} from "./overlayContext";
import styles from "./Menu.module.css";

export interface MenuItem {
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  destructive?: boolean;
}

interface MenuProps {
  label: string;
  items: MenuItem[];
  disabled?: boolean;
  className?: string;
}

export function Menu({ label, items, disabled = false, className = "" }: MenuProps) {
  const [open, setOpen] = useState(false);
  const compact = useMediaQuery("(max-width: 560px)");
  const reactId = useId().replaceAll(":", "");
  const id = `menu-${reactId}`;
  const parentId = useOverlayParentId();
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const { register, isTop } = useOverlayContext();

  useEffect(() => {
    if (!open || compact) return;
    return register({
      id,
      parentId,
      modal: false,
      requestClose() {
        setOpen(false);
        queueMicrotask(() => triggerRef.current?.focus());
        return true;
      },
    });
  }, [compact, id, open, parentId, register]);

  useEffect(() => {
    if (!open || compact) return;
    const first = menuRef.current?.querySelector<HTMLButtonElement>("button:not([disabled])");
    queueMicrotask(() => first?.focus());

    function onPointerDown(event: PointerEvent): void {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (menuRef.current?.contains(target) || triggerRef.current?.contains(target)) return;
      setOpen(false);
    }

    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [compact, open]);

  function closeAndRestore(): void {
    setOpen(false);
    queueMicrotask(() => triggerRef.current?.focus());
  }

  function select(item: MenuItem): void {
    if (item.disabled) return;
    closeAndRestore();
    item.onSelect();
  }

  function onMenuKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    if (!isTop(id)) return;
    const buttons = [...(menuRef.current?.querySelectorAll<HTMLButtonElement>("button:not([disabled])") ?? [])];
    const current = document.activeElement instanceof HTMLButtonElement
      ? buttons.indexOf(document.activeElement)
      : -1;
    if (event.key === "Escape") {
      event.preventDefault();
      closeAndRestore();
      return;
    }
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key) || buttons.length === 0) return;
    event.preventDefault();
    let next = current;
    if (event.key === "Home") next = 0;
    else if (event.key === "End") next = buttons.length - 1;
    else if (event.key === "ArrowDown") next = (current + 1 + buttons.length) % buttons.length;
    else next = (current - 1 + buttons.length) % buttons.length;
    buttons[next]?.focus();
  }

  const trigger = (
    <button
      ref={triggerRef}
      type="button"
      className={`${styles.trigger} ${className}`.trim()}
      aria-haspopup="menu"
      aria-expanded={open}
      disabled={disabled}
      onClick={() => setOpen((current) => !current)}
    >
      {label}
    </button>
  );

  if (compact) {
    return (
      <>
        {trigger}
        {open ? (
          <Sheet title={label} onClose={closeAndRestore}>
            <div className={styles.sheetList}>
              {items.map((item) => (
                <button
                  key={item.label}
                  type="button"
                  className={item.destructive ? styles.destructive : undefined}
                  disabled={item.disabled}
                  onClick={() => select(item)}
                >
                  {item.label}
                </button>
              ))}
            </div>
          </Sheet>
        ) : null}
      </>
    );
  }

  return (
    <div className={styles.root}>
      {trigger}
      {open ? (
        <div
          ref={menuRef}
          className={styles.menu}
          role="menu"
          aria-label={label}
          onKeyDown={onMenuKeyDown}
        >
          {items.map((item) => (
            <button
              key={item.label}
              type="button"
              role="menuitem"
              className={item.destructive ? styles.destructive : undefined}
              disabled={item.disabled}
              onClick={() => select(item)}
            >
              {item.label}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
