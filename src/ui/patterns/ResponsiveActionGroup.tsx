import { IconButton } from "../icons/IconButton";
import type { IconName } from "../icons/Icon";
import { useLayoutMode, type LayoutMode } from "../layout/LayoutModeProvider";
import { OverflowMenu } from "../overlays/OverflowMenu";
import { Button, type ButtonProps } from "../primitives/Button";
import styles from "./ResponsiveActionGroup.module.css";

export type ResponsiveActionPlacement =
  | "always-visible"
  | "wide-and-medium"
  | "wide-only"
  | "overflow-only";

export interface ResponsiveAction {
  id: string;
  label: string;
  onSelect: () => void;
  placement: ResponsiveActionPlacement;
  disabled?: boolean;
  loading?: boolean;
  destructive?: boolean;
  icon?: IconName;
  presentation?: "text" | "icon";
  variant?: ButtonProps["variant"];
}

export interface ResponsiveActionGroupProps {
  label: string;
  actions: ReadonlyArray<ResponsiveAction>;
  className?: string;
  overflowLabel?: string;
}

export function ResponsiveActionGroup({
  label,
  actions,
  className = "",
  overflowLabel = "More actions",
}: ResponsiveActionGroupProps) {
  const { layoutMode } = useLayoutMode();
  const ids = new Set(actions.map((action) => action.id));
  if (ids.size !== actions.length || actions.some((action) => action.id.trim() === "")) {
    throw new Error("ResponsiveActionGroup requires unique, nonblank action IDs.");
  }
  if (actions.length > 0 && !actions.some((action) => action.placement === "always-visible")) {
    throw new Error("ResponsiveActionGroup requires an always-visible primary action.");
  }

  const direct = actions.filter((action) => isDirect(action.placement, layoutMode));
  const overflow = actions.filter((action) => !isDirect(action.placement, layoutMode));

  return (
    <div
      className={`${styles.group} ${className}`.trim()}
      role="group"
      aria-label={label}
      data-layout-mode={layoutMode}
    >
      {direct.map((action) =>
        action.presentation === "icon" && action.icon ? (
          <IconButton
            key={action.id}
            icon={action.icon}
            label={action.label}
            tone={action.destructive ? "danger" : "neutral"}
            disabled={action.disabled}
            loading={action.loading}
            onClick={action.onSelect}
          />
        ) : (
          <Button
            key={action.id}
            variant={
              action.variant ?? (action.destructive ? "dangerSecondary" : "secondary")
            }
            disabled={action.disabled}
            loading={action.loading}
            onClick={action.onSelect}
          >
            {action.label}
          </Button>
        ),
      )}
      {overflow.length > 0 ? (
        <OverflowMenu
          label={overflowLabel}
          items={overflow.map((action) => ({
            id: action.id,
            label: action.label,
            disabled: action.disabled || action.loading,
            destructive: action.destructive,
            onSelect: action.onSelect,
          }))}
        />
      ) : null}
    </div>
  );
}

function isDirect(placement: ResponsiveActionPlacement, layoutMode: LayoutMode): boolean {
  if (placement === "always-visible") return true;
  if (placement === "wide-and-medium") return layoutMode !== "compact";
  if (placement === "wide-only") return layoutMode === "wide";
  return false;
}
