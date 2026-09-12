import type { ResponsiveAction } from "./ResponsiveActionGroup";
import { ResponsiveActionGroup } from "./ResponsiveActionGroup";
import { IconButton } from "../icons/IconButton";
import { Surface } from "../primitives/Surface";
import { Badge } from "../primitives/Badge";
import { cn } from "../internal/cn";
export interface SelectionToolbarProps {
  selectedCount: number;
  primaryAction: Omit<ResponsiveAction, "placement" | "presentation">;
  secondaryActions?: ReadonlyArray<ResponsiveAction>;
  onClearSelection: () => void;
  label?: string; clearLabel?: string; className?: string;
}
export function SelectionToolbar({ selectedCount, primaryAction, secondaryActions = [], onClearSelection, label = "Selection actions", clearLabel = "Clear selection", className }: SelectionToolbarProps) {
  if (selectedCount < 1) return null;
  const actions: ResponsiveAction[] = [{ ...primaryAction, placement: "always-visible", presentation: "text", variant: primaryAction.variant ?? "primary" }, ...secondaryActions];
  return <Surface as="section" aria-label={label} data-selection-toolbar="true" className={cn("tw:sticky tw:top-2 tw:z-20 tw:mb-4 tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:justify-between tw:gap-3 tw:border-primary/40 tw:px-4 tw:py-3 tw:shadow-panel tw:max-[760px]:top-auto tw:max-[760px]:bottom-[calc(var(--safe-bottom)+var(--android-ime-bottom))] tw:max-[760px]:items-stretch tw:forced-colors:border-[Highlight]", className)}>
    <Badge tone="accent" className="tw:shrink-0 tw:tabular-nums" aria-live="polite" aria-atomic="true">{selectedCount} {selectedCount === 1 ? "item" : "items"} selected</Badge>
    <div className="tw:flex tw:min-w-0 tw:flex-1 tw:flex-wrap tw:items-center tw:justify-end tw:gap-2 tw:max-[760px]:w-full tw:max-[760px]:justify-between"><ResponsiveActionGroup label={label} actions={actions} overflowLabel="More selection actions" /><IconButton icon="close" label={clearLabel} onClick={onClearSelection} /></div>
  </Surface>;
}
