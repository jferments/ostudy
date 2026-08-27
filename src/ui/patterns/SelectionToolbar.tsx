import type { ResponsiveAction } from "./ResponsiveActionGroup";
import { ResponsiveActionGroup } from "./ResponsiveActionGroup";
import { IconButton } from "../icons/IconButton";
import styles from "./SelectionToolbar.module.css";

export interface SelectionToolbarProps {
  selectedCount: number;
  primaryAction: Omit<ResponsiveAction, "placement" | "presentation">;
  secondaryActions?: ReadonlyArray<ResponsiveAction>;
  onClearSelection: () => void;
  label?: string;
  clearLabel?: string;
  className?: string;
}

export function SelectionToolbar({
  selectedCount,
  primaryAction,
  secondaryActions = [],
  onClearSelection,
  label = "Selection actions",
  clearLabel = "Clear selection",
  className = "",
}: SelectionToolbarProps) {
  if (selectedCount < 1) return null;
  const actions: ResponsiveAction[] = [
    {
      ...primaryAction,
      placement: "always-visible",
      presentation: "text",
      variant: primaryAction.variant ?? "primary",
    },
    ...secondaryActions,
  ];

  return (
    <section
      className={`${styles.toolbar} ${className}`.trim()}
      aria-label={label}
      data-selection-toolbar="true"
    >
      <strong className={styles.count} aria-live="polite" aria-atomic="true">
        {selectedCount} {selectedCount === 1 ? "item" : "items"} selected
      </strong>
      <div className={styles.actions}>
        <ResponsiveActionGroup label={label} actions={actions} overflowLabel="More selection actions" />
        <IconButton icon="close" label={clearLabel} onClick={onClearSelection} />
      </div>
    </section>
  );
}
