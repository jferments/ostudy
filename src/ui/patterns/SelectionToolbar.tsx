import type { ResponsiveAction } from "./ResponsiveActionGroup";
import { ResponsiveActionGroup } from "./ResponsiveActionGroup";
import { IconButton } from "../icons/IconButton";
import { Badge } from "../primitives/Badge";
import { cn } from "../internal/cn";

export interface SelectionToolbarProps {
  selectedCount: number;
  primaryAction: Omit<ResponsiveAction, "placement" | "presentation">;
  secondaryActions?: ReadonlyArray<ResponsiveAction>;
  onClearSelection: () => void;
  label?: string;
  clearLabel?: string;
  className?: string;
  overlay?: boolean;
}

export function SelectionToolbar({
  selectedCount,
  primaryAction,
  secondaryActions = [],
  onClearSelection,
  label = "Selection actions",
  clearLabel = "Clear selection",
  className,
  overlay = false,
}: SelectionToolbarProps) {
  if (selectedCount < 1) return null;
  const actions: ResponsiveAction[] = [
    { ...primaryAction, placement: "always-visible", presentation: "text", variant: primaryAction.variant ?? "primary" },
    ...secondaryActions,
  ];
  const toolbar = (
    <section
      aria-label={label}
      data-selection-toolbar="true"
      className={cn(
        "tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:justify-between tw:gap-2 tw:rounded-lg tw:bg-popover tw:p-2 tw:ring-1 tw:ring-primary/30 tw:forced-colors:border tw:forced-colors:border-solid tw:forced-colors:border-[Highlight]",
        overlay
          ? "tw:relative tw:z-[calc(var(--ostudy-layer-sticky)+1)] tw:shadow-sm"
          : "tw:sticky tw:top-2 tw:z-(--ostudy-layer-sticky) tw:mb-3 tw:compact:top-auto tw:compact:bottom-[calc(var(--safe-bottom)+var(--android-ime-bottom))] tw:compact:items-stretch",
        className,
      )}
    >
      <Badge tone="accent" className="tw:shrink-0 tw:tabular-nums" aria-live="polite" aria-atomic="true">
        {selectedCount} {selectedCount === 1 ? "item" : "items"} selected
      </Badge>
      <div className="tw:flex tw:min-w-0 tw:flex-1 tw:flex-wrap tw:items-center tw:justify-end tw:gap-1.5 tw:compact:w-full tw:compact:justify-between">
        <ResponsiveActionGroup label={label} actions={actions} overflowLabel="More selection actions" />
        <IconButton icon="close" label={clearLabel} onClick={onClearSelection} />
      </div>
    </section>
  );
  if (!overlay) return toolbar;
  return (
    <div
      data-selection-toolbar-layer="true"
      className="tw:pointer-events-none tw:sticky tw:top-2 tw:z-[calc(var(--ostudy-layer-sticky)+1)] tw:h-0 tw:min-w-0 tw:overflow-visible"
    >
      <div className="tw:pointer-events-auto tw:min-w-0">{toolbar}</div>
    </div>
  );
}
