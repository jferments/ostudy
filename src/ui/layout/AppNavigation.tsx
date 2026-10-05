import { useId, type ReactNode } from "react";

import { Icon, type IconName } from "../icons/Icon";
import { cn } from "../internal/cn";
import { ItemTrigger } from "../vendor/shadcn/item";
import { useLayoutMode } from "./LayoutModeProvider";

export interface NavigationBadge {
  /** The short text the badge shows, such as a count. */
  value: string;
  /**
   * What the badge means. It describes the destination without joining its
   * name, so the name stays the visible label.
   */
  description: string;
}

export interface NavigationItem {
  id: string;
  label: string;
  icon: IconName;
  active: boolean;
  onSelect: () => void;
  badge?: NavigationBadge;
}

/**
 * The main navigation: one row of labeled destinations in the header on wider
 * screens, and a bottom bar with icons on a phone. Both keep the landmark name,
 * the item order and `aria-current="page"` on the active destination.
 */
export function AppNavigation({ items }: { items: readonly NavigationItem[] }) {
  const { layoutMode } = useLayoutMode();
  if (layoutMode === "compact") return <BottomNavigation items={items} />;
  return <HeaderNavigation items={items} medium={layoutMode === "medium"} />;
}

function HeaderNavigation({ items, medium }: { items: readonly NavigationItem[]; medium: boolean }) {
  return (
    <nav
      aria-label="Main navigation"
      data-tour-anchor="app.navigation"
      data-slot="header-navigation"
      className="tw:min-w-0 tw:rounded-full tw:border tw:border-solid tw:border-border tw:bg-card tw:p-1 tw:shadow-sm"
    >
      <ul className="tw:m-0 tw:flex tw:list-none tw:items-center tw:gap-1 tw:p-0">
        {items.map((item) => (
          <li key={item.id} className="tw:min-w-0">
            <NavigationButton
              item={item}
              className={cn(
                "tw:h-11 tw:gap-2 tw:rounded-full tw:text-body",
                medium ? "tw:px-3.5" : "tw:px-5",
                item.active
                  ? "tw:bg-accent tw:font-semibold tw:text-accent-foreground"
                  : "tw:font-medium tw:text-muted-foreground tw:hover:bg-muted tw:hover:text-foreground",
              )}
            >
              <span data-slot="nav-label">{item.label}</span>
              {item.badge ? (
                <span
                  aria-hidden="true"
                  data-slot="nav-badge"
                  className="tw:min-w-6 tw:rounded-full tw:bg-primary tw:px-2 tw:text-center tw:text-label tw:leading-[1.375rem] tw:font-semibold tw:tabular-nums tw:text-primary-foreground"
                >
                  {item.badge.value}
                </span>
              ) : null}
            </NavigationButton>
          </li>
        ))}
      </ul>
    </nav>
  );
}

function BottomNavigation({ items }: { items: readonly NavigationItem[] }) {
  return (
    <nav
      aria-label="Main navigation"
      data-tour-anchor="app.navigation"
      data-slot="bottom-navigation"
      className="tw:fixed tw:inset-x-0 tw:bottom-0 tw:z-(--ostudy-layer-chrome) tw:border-0 tw:border-t tw:border-solid tw:border-border tw:bg-card tw:pt-1.5 tw:pr-[max(0.25rem,var(--safe-right))] tw:pb-[max(0.5rem,var(--safe-bottom))] tw:pl-[max(0.25rem,var(--safe-left))]"
    >
      <ul className="tw:m-0 tw:grid tw:list-none tw:grid-cols-5 tw:p-0">
        {items.map((item) => (
          <li key={item.id} className="tw:min-w-0">
            <NavigationButton
              item={item}
              className={cn(
                "tw:min-h-[3.625rem] tw:w-full tw:flex-col tw:gap-1 tw:rounded-md tw:px-0.5 tw:text-caption",
                item.active
                  ? "tw:font-semibold tw:text-foreground"
                  : "tw:font-medium tw:text-muted-foreground",
              )}
            >
              <span
                className={cn(
                  "tw:relative tw:grid tw:h-[1.875rem] tw:w-14 tw:place-items-center tw:rounded-full tw:transition-colors tw:duration-(--ostudy-motion-duration-fast) tw:reduce-motion:transition-none",
                  item.active && "tw:bg-accent tw:text-accent-foreground",
                )}
              >
                <Icon name={item.icon} size={20} />
                {item.badge ? (
                  <span
                    aria-hidden="true"
                    data-slot="nav-badge"
                    className="tw:absolute tw:-top-1 tw:right-0 tw:min-w-4 tw:rounded-full tw:bg-primary tw:px-1.5 tw:text-center tw:text-[0.65625rem] tw:leading-4 tw:font-semibold tw:tabular-nums tw:text-primary-foreground"
                  >
                    {item.badge.value}
                  </span>
                ) : null}
              </span>
              <span data-slot="nav-label" className="tw:max-w-full tw:truncate">
                {item.label}
              </span>
            </NavigationButton>
          </li>
        ))}
      </ul>
    </nav>
  );
}

function NavigationButton({
  item,
  className,
  children,
}: {
  item: NavigationItem;
  className: string;
  children: ReactNode;
}) {
  const descriptionId = useId();
  return (
    <ItemTrigger
      data-testid={`nav-${item.id}`}
      aria-current={item.active ? "page" : undefined}
      aria-describedby={item.badge ? descriptionId : undefined}
      onClick={item.onSelect}
      className={cn(
        "tw:inline-flex tw:justify-center tw:whitespace-nowrap tw:font-sans tw:outline-none tw:transition-colors tw:duration-(--ostudy-motion-duration-fast) tw:ease-standard tw:reduce-motion:transition-none tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50",
        className,
      )}
    >
      {children}
      {item.badge ? (
        <span id={descriptionId} hidden>
          {item.badge.description}
        </span>
      ) : null}
    </ItemTrigger>
  );
}
