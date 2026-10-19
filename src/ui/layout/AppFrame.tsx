import type { HTMLAttributes, ReactNode } from "react";

import { cn } from "../internal/cn";
import { SidebarInset } from "../vendor/shadcn/sidebar";
import { useLayoutMode } from "./LayoutModeProvider";

/**
 * Layout alone: navigation, workflows and provider lifetimes remain app-owned.
 * Wider screens put the header, as a side rail, beside one scrolling main area.
 * A phone keeps the header and the bottom navigation fixed and pads the main
 * area between them.
 */
export function AppFrame({ children }: { children: ReactNode }) {
  const { layoutMode } = useLayoutMode();
  return (
    <div
      data-ui-root="true"
      data-frame-layout={layoutMode}
      className={cn(
        "tw:relative tw:z-(--ostudy-layer-base) tw:h-dvh tw:w-full tw:min-w-0 tw:overflow-hidden tw:bg-background tw:font-sans tw:text-foreground",
        // The space below a phone's content is the bottom navigation's own height (its 1px top
        // border, 0.375rem top padding, 3.625rem items and the bottom padding that grows into the
        // safe area, as AppNavigation draws it), so it follows the interface scale.
        layoutMode === "compact"
          ? "tw:block tw:pt-[calc(var(--ostudy-navigation-app-bar-height)+var(--safe-top))] tw:pb-[calc(1px+4rem+max(0.5rem,var(--safe-bottom)))] tw:[[data-android-ime-visible=true]_&]:pb-(--android-ime-bottom)"
          : "tw:grid tw:grid-cols-[auto_minmax(0,1fr)]",
      )}
    >
      {children}
    </div>
  );
}

export function AppContent({ children }: { children: ReactNode }) {
  return <SidebarInset className="tw:h-full tw:w-full">{children}</SidebarInset>;
}

interface AppHeaderProps extends Omit<HTMLAttributes<HTMLElement>, "title"> {
  brand: ReactNode;
  /** A phone's header names the current place between the brand and the actions. */
  title?: ReactNode;
  /** Wider screens carry the main navigation in the side rail. */
  navigation?: ReactNode;
  actions: ReactNode;
}

/**
 * The application header. On wider screens it is a side rail: the brand at the
 * top, the main navigation below it and the header actions at the bottom. On a
 * phone it is a fixed bar with the brand, the current place and the actions,
 * and the navigation moves to the bottom of the screen.
 */
export function AppHeader({ brand, title, navigation, actions, className, ...props }: AppHeaderProps) {
  const { layoutMode } = useLayoutMode();
  if (layoutMode === "compact") {
    return (
      <header
        {...props}
        data-slot="app-header"
        className={cn(
          "tw:fixed tw:inset-x-0 tw:top-0 tw:z-(--ostudy-layer-chrome) tw:grid tw:min-h-[calc(var(--ostudy-navigation-app-bar-height)+var(--safe-top))] tw:grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] tw:items-center tw:gap-2 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:bg-background tw:pt-(--safe-top) tw:pr-[max(1rem,var(--safe-right))] tw:pb-1 tw:pl-[max(1rem,var(--safe-left))]",
          className,
        )}
      >
        <div className="tw:flex tw:min-w-0 tw:justify-start">{brand}</div>
        <div className="tw:min-w-0 tw:truncate tw:text-center tw:text-small tw:font-semibold tw:text-foreground">
          {title}
        </div>
        <div className="tw:flex tw:min-w-0 tw:items-center tw:justify-end tw:gap-2">{actions}</div>
      </header>
    );
  }
  return (
    <header
      {...props}
      data-slot="app-header"
      className={cn(
        "tw:relative tw:z-(--ostudy-layer-chrome) tw:flex tw:h-full tw:min-h-0 tw:w-22 tw:flex-col tw:items-center tw:gap-3 tw:overflow-y-auto tw:border-0 tw:border-r tw:border-solid tw:border-navigation-border tw:bg-navigation tw:pt-[max(0.875rem,var(--safe-top))] tw:pb-[max(0.875rem,var(--safe-bottom))] tw:pl-[max(0.5rem,var(--safe-left))] tw:pr-2 tw:text-navigation-foreground",
        className,
      )}
    >
      <div className="tw:flex tw:shrink-0 tw:justify-center tw:pb-1">{brand}</div>
      {navigation}
      <div className="tw:mt-auto tw:flex tw:flex-col tw:items-center tw:gap-1">{actions}</div>
    </header>
  );
}
